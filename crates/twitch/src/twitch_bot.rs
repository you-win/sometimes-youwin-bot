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
    config::Config,
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

pub struct BotCommon {
    config: Arc<RwLock<Config>>,
    creds: TwitchCreds,
    user_token: UserToken,

    receiver: Receiver<CentralMessage>,
    sender: Sender<TwitchMessage>,
}

impl Clone for BotCommon {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            creds: self.creds.clone(),
            user_token: self.user_token.clone(),
            receiver: self.receiver.resubscribe(),
            sender: self.sender.clone(),
        }
    }
}

pub struct ApiBot<'a> {
    common: BotCommon,
    client: TwitchClient<'a, reqwest::Client>,
    interval: Interval,
    pub check_live_ticks: u64,
}

impl<'a> std::ops::Deref for ApiBot<'a> {
    type Target = BotCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

impl<'a> std::ops::DerefMut for ApiBot<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.common
    }
}

impl<'a> ApiBot<'a> {
    pub async fn tick(&mut self) {
        let _ = self.interval.tick().await;
    }

    pub async fn check_channel_live(&self) -> anyhow::Result<()> {
        debug!("Checking if channel is live");

        match self
            .client
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

    /// Handles a message from the main controller. Returns false if the loop for
    /// the twitch bot should stop running.
    pub async fn handle_central_message(&mut self) -> bool {
        match &self.receiver.try_recv() {
            Ok(m) => match m {
                CentralMessage::ConfigUpdated => {
                    debug!("Updating from config");

                    let config = &self.config.clone();
                    let config = config.read().await;

                    self.interval =
                        tokio::time::interval(Duration::from_secs_f32(config.tick_duration));

                    debug!("Finished updating from config");

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
}

pub struct ChatBot {
    common: BotCommon,
    client: twitchchat::AsyncRunner,
}

impl std::ops::Deref for ChatBot {
    type Target = BotCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

impl ChatBot {
    pub async fn handle_chat(&mut self) -> anyhow::Result<()> {
        match self.client.next_message().await? {
            twitchchat::Status::Message(m) => {
                self.handle_message(m).await?;
            }
            twitchchat::Status::Quit => anyhow::bail!("Quit detected from Twitch chat"),
            _ => {}
        }

        Ok(())
    }

    async fn handle_message(
        &mut self,
        command: twitchchat::messages::Commands<'_>,
    ) -> anyhow::Result<()> {
        match command {
            twitchchat::messages::Commands::IrcReady(v) => {
                debug!("{:?}", v);
                if let Err(e) = self.sender.send(TwitchMessage::Ready) {
                    error!("{e}");
                }
            }
            twitchchat::messages::Commands::Ready(v) => {
                debug!("{:?}", v);
                if let Err(e) = self.send_chat_message("Bot ready!").await {
                    error!("{e}");
                }
            }
            twitchchat::messages::Commands::Notice(v) => {
                debug!(
                    "{:?} - {:?}",
                    v.msg_id()
                        .unwrap_or(twitchchat::messages::MessageId::NoHelp),
                    v.message()
                );
            }
            twitchchat::messages::Commands::Privmsg(m) => {
                self.handle_privmsg(&m).await?;
            }
            twitchchat::messages::Commands::Reconnect(_) => {
                let handle = self.client.quit_handle();
                let _ = handle.notify();

                self.client = create_irc_resources(
                    self.common.user_token.access_token.secret(),
                    &self.creds.bot_name,
                    &self.creds.channel_name,
                )
                .await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_privmsg(&self, msg: &Privmsg<'_>) -> anyhow::Result<()> {
        let id = msg.tags().get("id").unwrap_or_default();
        if id.is_empty() {
            debug!("No id found for message {:?}", msg.data());
            return Ok(());
        }

        let text = msg.data();
        if !text.starts_with(self.common.creds.bot_prefix()) {
            return Ok(());
        }

        let config = &*self.common.config.read().await;

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

    async fn send_chat_message(&self, message: &str) -> anyhow::Result<()> {
        self.client
            .writer()
            .encode(twitchchat::commands::privmsg(
                self.common.creds.channel_name.as_str(),
                message,
            ))
            .await
            .map_err(anyhow::Error::from)
    }
}

pub async fn create_bots<'a>(
    config: Arc<RwLock<Config>>,
    creds: TwitchCreds,
    receiver: Receiver<CentralMessage>,
    sender: Sender<TwitchMessage>,
) -> anyhow::Result<(ApiBot<'a>, ChatBot)> {
    let (api_client, user_token) = create_api_resources(&creds).await?;
    let irc_client = create_irc_resources(
        user_token.access_token.secret(),
        creds.bot_name.as_str(),
        creds.channel_name.as_str(),
    )
    .await?;

    let common = BotCommon {
        config: config.clone(),
        creds,
        user_token,
        receiver,
        sender,
    };

    let config = &config.read().await;

    let api_bot = ApiBot {
        common: common.clone(),
        client: api_client,
        interval: tokio::time::interval(Duration::from_secs_f32(config.tick_duration)),
        check_live_ticks: config.check_live_ticks,
    };

    let chat_bot = ChatBot {
        common: common.clone(),
        client: irc_client,
    };

    Ok((api_bot, chat_bot))
}

// pub struct Bot<'a> {
//     config: Arc<RwLock<Config>>,
//     creds: TwitchCreds,

//     user_token: UserToken,

//     api_client: TwitchClient<'a, reqwest::Client>,
//     irc_client: twitchchat::AsyncRunner,

//     receiver: Receiver<CentralMessage>,
//     sender: Sender<TwitchMessage>,

//     interval: Interval,
//     pub check_live_ticks: u64,
// }

// impl<'a> Bot<'a> {
//     pub async fn new(
//         config: Arc<RwLock<Config>>,
//         creds: TwitchCreds,
//         receiver: Receiver<CentralMessage>,
//         sender: Sender<TwitchMessage>,
//     ) -> anyhow::Result<Bot<'a>> {
//         let (api_client, user_token) = create_api_resources(&creds).await?;
//         let irc_client = create_irc_resources(
//             user_token.access_token.secret(),
//             creds.bot_name.as_str(),
//             creds.channel_name.as_str(),
//         )
//         .await?;

//         let interval: Interval;
//         let check_live_ticks: u64;
//         {
//             let config = config.read().await;
//             interval = tokio::time::interval(Duration::from_secs_f32(config.tick_duration));
//             check_live_ticks = config.check_live_ticks;
//         }

//         Ok(Bot {
//             config,
//             creds,
//             user_token,
//             api_client,
//             irc_client,
//             receiver,
//             sender,
//             interval,
//             check_live_ticks,
//         })
//     }

//     pub async fn tick(&mut self) {
//         let _ = self.interval.tick();
//     }

//     pub async fn check_channel_live(&self) -> anyhow::Result<()> {
//         debug!("Checking if channel is live");

//         match self
//             .api_client
//             .helix
//             .req_get(
//                 GetStreamsRequest::user_logins(
//                     [UserNameRef::from_str(self.creds.channel_name.as_str())].as_slice(),
//                 ),
//                 &self.user_token,
//             )
//             .await
//         {
//             Ok(r) => {
//                 if r.data.is_empty() {
//                     return Ok(());
//                 }
//                 let stream = r.data.first().unwrap();

//                 self.sender
//                     .send(TwitchMessage::ChannelLive {
//                         channel: stream.user_name.to_string(),
//                         title: stream.title.to_string(),
//                     })
//                     .map(|_| ())
//                     .map_err(anyhow::Error::from)
//             }
//             Err(e) => Err(e.into()),
//         }
//     }

//     pub async fn handle_chat(&mut self) -> anyhow::Result<bool> {
//         use twitchchat::{
//             commands,
//             messages::Commands,
//             runner::{Status, StepResult},
//         };

//         debug!("handle_chat");

//         match self.irc_client.step().await? {
//             StepResult::Nothing => {
//                 debug!("nothing");
//                 Ok(true)
//             }
//             StepResult::Status(Status::Quit) => {
//                 debug!("status quit");

//                 self.irc_client
//                     .writer()
//                     .encode(commands::raw("QUIT\r\n"))
//                     .await?;

//                 Ok(false)
//             }
//             StepResult::Status(Status::Message(c)) => {
//                 debug!("status message");
//                 match c {
//                     Commands::IrcReady(v) => {
//                         debug!("{:?}", v);
//                         if let Err(e) = self.sender.send(TwitchMessage::Ready) {
//                             error!("{e}");
//                         }
//                     }
//                     Commands::Ready(v) => {
//                         debug!("{:?}", v);
//                         if let Err(e) = self.send_chat_message("Bot ready!").await {
//                             error!("{e}");
//                         }
//                     }
//                     Commands::Notice(v) => {
//                         debug!(
//                             "{:?} - {:?}",
//                             v.msg_id()
//                                 .unwrap_or(twitchchat::messages::MessageId::NoHelp),
//                             v.message()
//                         );
//                     }
//                     Commands::Privmsg(v) => {
//                         self.handle_privmsg(&v).await?;
//                     }
//                     _ => {}
//                 }

//                 Ok(true)
//             }
//             _ => Ok(false),
//         }
//     }

//     pub async fn send_chat_message(&self, message: &str) -> anyhow::Result<()> {
//         self.irc_client
//             .writer()
//             .encode(twitchchat::commands::privmsg(
//                 self.creds.channel_name.as_str(),
//                 message,
//             ))
//             .await
//             .map_err(anyhow::Error::from)
//     }

//     pub async fn handle_privmsg(&self, msg: &Privmsg<'_>) -> anyhow::Result<()> {
//         let id = msg.tags().get("id").unwrap_or_default();
//         if id.is_empty() {
//             debug!("No id found for message {:?}", msg.data());
//             return Ok(());
//         }

//         let text = msg.data();
//         if !text.starts_with(self.creds.bot_prefix()) {
//             return Ok(());
//         }

//         let config = &*self.config.read().await;

//         let output = commands::parse(
//             text,
//             commands::AdditionalInfo::Twitch {
//                 name: msg.name().to_string(),
//                 is_vip: msg.is_vip(),
//             },
//             &config,
//         );

//         let chat_message = match output {
//             CommandOutput::Command { value, .. } | CommandOutput::AdminCommand { value, .. } => {
//                 value.unwrap_or("No output!".into())
//             }
//             CommandOutput::Error { .. } => {
//                 let mut cli_commands = commands::Commands::commands();
//                 cli_commands.append(&mut config.ad_hoc_commands());

//                 cli_commands.join(", ")
//             }
//         };

//         self.send_chat_message(chat_message.as_str()).await?;

//         Ok(())
//     }

//     /// Handles a message from the main controller. Returns false if the loop for
//     /// the twitch bot should stop running.
//     pub async fn handle_central_message(&mut self) -> bool {
//         match self.receiver.try_recv() {
//             Ok(m) => match m {
//                 CentralMessage::ConfigUpdated => {
//                     debug!("Updating from config");

//                     let config = self.config.read().await;

//                     self.interval =
//                         tokio::time::interval(Duration::from_secs_f32(config.tick_duration));
//                     self.check_live_ticks = config.check_live_ticks;

//                     debug!("Finished updating from config");

//                     true
//                 }
//                 CentralMessage::Discord(_) => {
//                     // TODO stub
//                     true
//                 }
//                 CentralMessage::Shutdown => {
//                     info!("Shutdown received!");

//                     false
//                 }
//                 _ => true,
//             },
//             Err(e) => match e {
//                 TryRecvError::Closed => {
//                     error!("Channel closed");

//                     false
//                 }
//                 TryRecvError::Lagged(n) => {
//                     debug!("Channel lagged by {} messages", n);

//                     true
//                 }
//                 _ => true,
//             },
//         }
//     }

//     pub async fn shutdown(&self) -> bool {
//         self.irc_client.quit_handle().notify().await
//     }
// }

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
