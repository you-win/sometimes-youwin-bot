use std::sync::Arc;

use log::{debug, error, info};
use tokio::sync::broadcast::Sender;
use tokio::sync::{broadcast::Receiver, RwLock};
use twitch_api::helix::streams::GetStreamsRequest;
use twitch_api::types::UserNameRef;
use twitch_api::{
    twitch_oauth2::{
        tokens::errors::ValidationError, AccessToken, ClientId, ClientSecret, RefreshToken,
        UserToken,
    },
    TwitchClient,
};
use twitchchat::messages::Privmsg;

use crate::commands;
use crate::CentralMessage;

#[derive(Debug, Clone)]
pub enum BotMessage {
    Debug(String),

    Ready,
    ChannelLive { channel: String, title: String },
    Shutdown,
}

struct Bot<'a> {
    config: Arc<RwLock<crate::Config>>,

    user_token: UserToken,

    api_client: TwitchClient<'a, reqwest::Client>,
    irc_client: twitchchat::AsyncRunner,

    receiver: Receiver<CentralMessage>,
    sender: Sender<BotMessage>,
}

impl<'a> Bot<'a> {
    async fn new(
        config: Arc<RwLock<crate::Config>>,
        central_receiver: Receiver<CentralMessage>,
        twitch_sender: Sender<BotMessage>,
    ) -> anyhow::Result<Bot<'a>> {
        let (api_client, user_token) = create_api_resources().await?;
        let irc_client = create_irc_resources(
            &user_token.access_token.secret().to_string(),
            &crate::TWITCH_BOT_NAME.to_string(),
            &crate::TWITCH_CHANNEL_NAME.to_string(),
        )
        .await?;

        Ok(Bot {
            config,
            user_token,
            api_client,
            irc_client,
            receiver: central_receiver,
            sender: twitch_sender,
        })
    }

    async fn check_channel_live(&self) -> anyhow::Result<()> {
        match self
            .api_client
            .helix
            .req_get(
                GetStreamsRequest::user_logins(
                    [UserNameRef::from_str(crate::TWITCH_CHANNEL_NAME)].as_slice(),
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
                    .send(BotMessage::ChannelLive {
                        channel: stream.user_name.to_string(),
                        title: stream.title.to_string(),
                    })
                    .map(|_| ())
                    .map_err(anyhow::Error::from)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Poll irc once and potentially handle a chat command. Returns false
    /// if the bot should shutdown.
    async fn handle_chat(&mut self) -> anyhow::Result<bool> {
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
                        if let Err(e) = self.sender.send(BotMessage::Ready) {
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

    async fn send_chat_message(&self, message: &str) -> anyhow::Result<()> {
        self.irc_client
            .writer()
            .encode(twitchchat::commands::privmsg(
                crate::TWITCH_CHANNEL_NAME,
                message,
            ))
            .await
            .map_err(anyhow::Error::from)
    }

    async fn handle_privmsg(&self, msg: &Privmsg<'_>) -> anyhow::Result<()> {
        let id = msg.tags().get("id").unwrap_or_default();
        if id.is_empty() {
            debug!("No id found for message {:?}", msg.data());
            return Ok(());
        }

        let text = msg.data();
        if !text.starts_with(crate::BOT_PREFIX) {
            return Ok(());
        }

        let cmd = text.strip_prefix(crate::BOT_PREFIX).unwrap_or_default();
        let (cmd, args) = cmd.split_once(" ").unwrap_or((cmd, ""));

        // TODO refactor to use procedural macro?
        let output: String = match cmd {
            "ping" => commands::ping(),
            "whoami" => commands::whoami(&msg.name().into()),
            "high-five" => commands::high_five(),
            "ferris-say" | "ferrissay" | "cowsay" => {
                format!("{} is not supported in Twitch chat", cmd).into()
            }
            "roll" => {
                let num: u64 = match args.parse() {
                    Ok(v) => v,
                    Err(_) => 6,
                };

                commands::roll(&num.into()).to_string()
            }
            "help" => {
                format!("?ping, ?whoami, ?high-five, ?roll <number>")
            }
            _ => return Ok(()),
        };

        self.send_chat_message(output.as_str()).await
    }

    async fn handle_central_message(&mut self) -> bool {
        match self.receiver.try_recv() {
            Ok(m) => match m {
                // crate::CentralMessage::ConfigUpdated(c) => {
                //     bot_config = c.clone();
                // }
                crate::CentralMessage::Discord(_) => {
                    // TODO stub
                    return true;
                }
                crate::CentralMessage::Shutdown => {
                    info!("Shutdown received!");
                    return false;
                }
                _ => return true,
            },
            Err(e) => match e {
                tokio::sync::broadcast::error::TryRecvError::Closed => {
                    error!("Channel closed");
                    return false;
                }
                tokio::sync::broadcast::error::TryRecvError::Lagged(n) => {
                    debug!("Channel lagged by {} messages", n);
                    return true;
                }
                _ => return true,
            },
        }
    }

    async fn shutdown(&self) -> bool {
        self.irc_client.quit_handle().notify().await
    }
}

pub async fn run_bot(
    config: Arc<RwLock<crate::Config>>,
    central_receiver: Receiver<crate::CentralMessage>,
    twitch_sender: Sender<BotMessage>,
) -> anyhow::Result<()> {
    let mut bot = Bot::new(
        config,
        central_receiver.resubscribe(),
        twitch_sender.clone(),
    )
    .await?;

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(*crate::TICK_DURATION);

        // TODO this sucks
        let mut ticks: u16 = 0;
        const CHECK_LIVE_TICKS: u16 = 240;

        loop {
            interval.tick().await;

            ticks += 1;
            if ticks >= CHECK_LIVE_TICKS {
                ticks = 0;

                debug!("Checking if channel is live");

                if let Err(e) = bot.check_channel_live().await {
                    error!("{e}");
                }
            }

            // TODO try to reconnect
            match bot.handle_chat().await {
                Ok(was_successful) => {
                    if !was_successful {
                        if !bot.shutdown().await {
                            error!("Twitch bot failed to cleanly shutdown!");
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to handle twitch chat message: {e}");
                    if !bot.shutdown().await {
                        error!("Twitch bot failed to cleanly shutdown!");
                        break;
                    }
                }
            }

            if !bot.handle_central_message().await {
                break;
            }
        }
    });

    Ok(())
}

async fn create_api_resources(
) -> anyhow::Result<(TwitchClient<'static, reqwest::Client>, UserToken)> {
    let refresh_token = RefreshToken::new(crate::TWITCH_REFRESH_TOKEN.into());
    let client_id = ClientId::new(crate::TWITCH_CLIENT_ID.into());
    let client_secret = ClientSecret::new(crate::TWITCH_CLIENT_SECRET.into());

    let api_client = TwitchClient::<reqwest::Client>::new();

    let (access_token, _, _) = refresh_token
        .refresh_token(&api_client, &client_id, &client_secret)
        .await?;

    let user_token =
        UserToken::from_existing(&api_client, access_token, refresh_token, client_secret).await?;

    Ok((api_client, user_token))
}

async fn create_irc_resources(
    token: &String,
    bot_name: &String,
    channel_name: &String,
) -> anyhow::Result<twitchchat::AsyncRunner> {
    use twitchchat::{connector::tokio::Connector, AsyncRunner, UserConfig};

    let irc_user_config = UserConfig::builder()
        .token(format!("oauth:{}", token))
        .name(bot_name)
        .enable_all_capabilities()
        .build()?;

    let connector = Connector::twitch()?;
    let mut irc_client = AsyncRunner::connect(connector, &irc_user_config).await?;

    irc_client.join(channel_name.as_str()).await?;

    Ok(irc_client)
}
