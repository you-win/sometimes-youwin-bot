use commands::CommandOutput;
use log::{debug, error, info};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{
        broadcast::{error::TryRecvError, Receiver, Sender},
        RwLock,
    },
    time::Interval,
};

use model::{
    config::{self, Config},
    creds::{BotCreds, TwitchCreds},
    messages::{CentralMessage, TwitchMessage},
};
use twitch_api::{
    helix::streams::GetStreamsRequest,
    twitch_oauth2::{ClientId, ClientSecret, RefreshToken, UserToken},
    types::UserNameRef,
    TwitchClient,
};
use twitchchat::messages::Privmsg;

pub struct Bot<'a> {
    config: Arc<RwLock<Config>>,
    creds: TwitchCreds,

    user_token: UserToken,

    api_client: TwitchClient<'a, reqwest::Client>,
    irc_client: twitchchat::AsyncRunner,

    receiver: Receiver<CentralMessage>,
    sender: Sender<TwitchMessage>,

    interval: Interval,
    pub check_live_ticks: u64,
}

impl<'a> Bot<'a> {
    pub async fn new(
        config: Arc<RwLock<Config>>,
        creds: TwitchCreds,
        receiver: Receiver<CentralMessage>,
        sender: Sender<TwitchMessage>,
    ) -> anyhow::Result<Bot<'a>> {
        let (api_client, user_token) = create_api_resources(&creds).await?;
        let irc_client = create_irc_resources(
            user_token.access_token.secret(),
            creds.bot_name.as_str(),
            creds.channel_name.as_str(),
        )
        .await?;

        Ok(Bot {
            config,
            creds,
            user_token,
            api_client,
            irc_client,
            receiver,
            sender,
            interval: tokio::time::interval(Duration::from_secs_f32(
                config::default_tick_duration(),
            )),
            check_live_ticks: config::default_check_live_ticks(),
        })
    }

    pub async fn tick(&mut self) {
        let _ = self.interval.tick();
    }

    pub async fn check_channel_live(&self) -> anyhow::Result<()> {
        debug!("Checking if channel is live");

        match self
            .api_client
            .helix
            .req_get(
                GetStreamsRequest::user_logins(
                    [UserNameRef::from_str(self.creds.channel_name.as_str())].as_slice(),
                ),
                &self.user_token,
            )
            .await
        {
            Ok(r) => {
                if r.data.is_empty() {
                    return Ok(());
                }
                let stream = r.data.first().unwrap();

                self.sender
                    .send(TwitchMessage::ChannelLive {
                        channel: stream.user_name.to_string(),
                        title: stream.title.to_string(),
                    })
                    .map(|_| ())
                    .map_err(anyhow::Error::from)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn handle_chat(&mut self) -> anyhow::Result<bool> {
        use twitchchat::{
            commands,
            messages::Commands,
            runner::{Status, StepResult},
        };

        match self.irc_client.step().await? {
            StepResult::Nothing => Ok(true),
            StepResult::Status(Status::Quit) => {
                self.irc_client
                    .writer()
                    .encode(commands::raw("QUIT\r\n"))
                    .await?;

                Ok(false)
            }
            StepResult::Status(Status::Message(c)) => {
                match c {
                    Commands::IrcReady(v) => {
                        debug!("{:?}", v);
                        if let Err(e) = self.sender.send(TwitchMessage::Ready) {
                            error!("{e}");
                        }
                    }
                    Commands::Ready(v) => {
                        debug!("{:?}", v);
                        if let Err(e) = self.send_chat_message("Bot ready!").await {
                            error!("{e}");
                        }
                    }
                    Commands::Notice(v) => {
                        debug!(
                            "{:?} - {:?}",
                            v.msg_id()
                                .unwrap_or(twitchchat::messages::MessageId::NoHelp),
                            v.message()
                        );
                    }
                    Commands::Privmsg(v) => {
                        self.handle_privmsg(&v).await?;
                    }
                    _ => {}
                }

                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub async fn send_chat_message(&self, message: &str) -> anyhow::Result<()> {
        self.irc_client
            .writer()
            .encode(twitchchat::commands::privmsg(
                self.creds.channel_name.as_str(),
                message,
            ))
            .await
            .map_err(anyhow::Error::from)
    }

    pub async fn handle_privmsg(&self, msg: &Privmsg<'_>) -> anyhow::Result<()> {
        let id = msg.tags().get("id").unwrap_or_default();
        if id.is_empty() {
            debug!("No id found for message {:?}", msg.data());
            return Ok(());
        }

        let text = msg.data();
        if !text.starts_with(self.creds.bot_prefix()) {
            return Ok(());
        }

        let config = &*self.config.read().await;

        let output = commands::parse(
            text,
            commands::AdditionalInfo::Twitch {
                name: msg.name().to_string(),
                is_vip: msg.is_vip(),
            },
            &config,
        );

        let chat_message = match output {
            CommandOutput::Command { value, .. } | CommandOutput::AdminCommand { value, .. } => {
                value.unwrap_or("No output!".into())
            }
            CommandOutput::Error { .. } => {
                let mut cli_commands = commands::Commands::commands();
                cli_commands.append(&mut config.ad_hoc_commands());

                cli_commands.join(", ")
            }
        };

        self.send_chat_message(chat_message.as_str()).await?;

        Ok(())
    }

    /// Handles a message from the main controller. Returns false if the loop for
    /// the twitch bot should stop running.
    pub async fn handle_central_message(&mut self) -> bool {
        match self.receiver.try_recv() {
            Ok(m) => match m {
                CentralMessage::ConfigUpdated => {
                    let config = self.config.read().await;

                    self.interval =
                        tokio::time::interval(Duration::from_secs_f32(config.tick_duration));
                    self.check_live_ticks = config.check_live_ticks;

                    true
                }
                CentralMessage::Discord(_) => {
                    // TODO stub
                    true
                }
                CentralMessage::Shutdown => {
                    info!("Shutdown received!");

                    false
                }
                _ => true,
            },
            Err(e) => match e {
                TryRecvError::Closed => {
                    error!("Channel closed");

                    false
                }
                TryRecvError::Lagged(n) => {
                    debug!("Channel lagged by {} messages", n);

                    true
                }
                _ => true,
            },
        }
    }

    pub async fn shutdown(&self) -> bool {
        self.irc_client.quit_handle().notify().await
    }
}

async fn create_api_resources(
    creds: &TwitchCreds,
) -> anyhow::Result<(TwitchClient<'static, reqwest::Client>, UserToken)> {
    let refresh_token = RefreshToken::new(creds.refresh_token.to_string());
    let client_id = ClientId::new(creds.client_id.to_string());
    let client_secret = ClientSecret::new(creds.client_secret.to_string());

    let client = TwitchClient::new();

    let (access_token, _, _) = refresh_token
        .refresh_token(&client, &client_id, &client_secret)
        .await?;

    let user_token =
        UserToken::from_existing(&client, access_token, refresh_token, client_secret).await?;

    Ok((client, user_token))
}

async fn create_irc_resources(
    token: &str,
    bot_name: &str,
    channel_name: &str,
) -> anyhow::Result<twitchchat::AsyncRunner> {
    use twitchchat::{connector::tokio::Connector, AsyncRunner, UserConfig};

    let config = UserConfig::builder()
        .token(format!("oauth:{}", token))
        .name(bot_name)
        .enable_all_capabilities()
        .build()?;

    let connector = Connector::twitch()?;
    let mut client = AsyncRunner::connect(connector, &config).await?;

    client.join(channel_name).await?;

    Ok(client)
}
