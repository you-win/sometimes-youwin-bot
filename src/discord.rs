mod antispam;
mod commands;

use std::{
    str::FromStr,
    sync::{atomic::AtomicBool, Arc},
};

use crate::{config::Config, utils};
use log::{debug, error, info};
use serenity::{
    async_trait,
    builder::GetMessages,
    framework::{
        standard::{
            macros::{command, group},
            CommandResult, Configuration,
        },
        StandardFramework,
    },
    model::{
        channel::Message,
        prelude::{ChannelId, GuildId, Reaction, Ready},
    },
    prelude::*,
    Client,
};
use tokio::sync::{
    broadcast::{Receiver, Sender},
    RwLock,
};

struct Handler {
    is_initted: Arc<AtomicBool>,
    central_receiver: Receiver<crate::CentralMessage>,
    sender: Sender<BotMessage>,

    config: Arc<RwLock<Config>>,

    bot_id: u64,
    admin_id: u64,

    guild_id: GuildId,

    antispam: Arc<RwLock<antispam::Antispam>>,
}

impl Handler {
    pub fn new(
        central_receiver: Receiver<crate::CentralMessage>,
        discord_sender: Sender<BotMessage>,
    ) -> Self {
        Self {
            is_initted: Arc::new(AtomicBool::new(false)),
            central_receiver: central_receiver,
            sender: discord_sender,

            config: Arc::new(RwLock::new(Config::new())),

            bot_id: crate::DISCORD_BOT_ID.parse().unwrap(),
            admin_id: crate::DISCORD_ADMIN_ID.parse().unwrap(),

            guild_id: GuildId(crate::DISCORD_GUILD_ID.parse().unwrap()),

            antispam: Arc::new(RwLock::new(antispam::Antispam::new())),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Discord bot connected!");

        let mut channels = self.guild_id.channels(&ctx.http).await.unwrap();
        match channels.get_mut(&ChannelId::from_str(crate::DISCORD_BOT_DATA_CHANNEL_ID).unwrap()) {
            Some(c) => {
                let messages = c.messages(&ctx.http, |m| m).await.unwrap();

                for message in messages.iter() {
                    let content = message
                        .content
                        .trim()
                        .trim_start_matches("`")
                        .trim_start_matches("TOML")
                        .trim_end_matches("`")
                        .trim();

                    match toml::from_str(content) {
                        Ok(c) => {
                            // self.sender.send(BotMessage::ConfigUpdated(c)).unwrap();
                            crate::CONFIG.lock().unwrap().from(&c);
                        }
                        Err(e) => {
                            self.sender.send(BotMessage::Error(e.to_string())).unwrap();
                        }
                    };
                }
            }
            None => panic!(
                "Unable to read config from {}",
                crate::DISCORD_BOT_DATA_CHANNEL_ID
            ),
        }

        if !self.is_initted.load(std::sync::atomic::Ordering::Relaxed) {
            self.is_initted
                .store(true, std::sync::atomic::Ordering::Relaxed);

            let mut receiver = self.central_receiver.resubscribe();
            let sender = self.sender.clone();
            let config = self.config.clone();
            tokio::spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok(m) => match m {
                            // crate::CentralMessage::ConfigUpdated(c) => {
                            //     config.write().await.from(&c);
                            // }
                            crate::CentralMessage::Twitch(_) => {
                                // TODO stub
                            }
                            crate::CentralMessage::Shutdown => {
                                info!("Shutdown received");
                                sender.send(BotMessage::Shutdown).unwrap();
                                break;
                            }
                            _ => {}
                        },
                        Err(e) => match e {
                            tokio::sync::broadcast::error::RecvError::Closed => {
                                error!("Channel closed");
                                break;
                            }
                            tokio::sync::broadcast::error::RecvError::Lagged(n) => {
                                debug!("Channel lagged by {} messages", n);
                            }
                        },
                    }
                }
            });
        }

        self.guild_id
            .set_application_commands(&ctx.http, |c| c)
            .await
            .unwrap();

        match self.sender.send(BotMessage::Ready) {
            Ok(_) => debug!("Sent ready message"),
            Err(e) => error!("{}", e),
        }

        info!("Discord bot ready!");
    }

    async fn message(&self, ctx: Context, message: Message) {
        let author_id = message.author.id.as_u64();

        if author_id == &self.bot_id {
            return;
        }

        if self.antispam.write().await.is_spam(author_id) {
            debug!("Spammer detected {}", &message.author.name);
            match message.reply_mention(&ctx, "Please do not spam").await {
                Ok(_) => {}
                Err(e) => error!("Unable to send spammer detected message: {}", e.to_string()),
            }
            match message.delete(ctx).await {
                Ok(_) => {}
                Err(e) => error!("Unable to delete spam message: {}", e.to_string()),
            }

            if self.antispam.write().await.should_timeout(author_id) {
                // TODO set user role here
            }
        }
    }

    async fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        //
    }

    async fn reaction_remove(&self, ctx: Context, remove_reaction: Reaction) {
        //
    }
}

#[derive(Debug, Clone)]
pub enum BotMessage {
    Debug(String),
    Error(String),

    Ready,

    ConfigUpdated(Config),

    Shutdown,
}

pub async fn run_bot(
    central_receiver: Receiver<crate::CentralMessage>,
    discord_sender: Sender<BotMessage>,
) {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix(crate::BOT_PREFIX).allow_dm(false))
        .group(&commands::GENERAL_GROUP);

    let token = crate::DISCORD_TOKEN;
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_INVITES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT;
    let handler = Handler::new(central_receiver, discord_sender);

    let mut client = Client::builder(token, intents)
        .event_handler(handler)
        .framework(framework)
        .await
        .unwrap();

    client.start().await.unwrap();
}
