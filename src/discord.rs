mod antispam;
mod commands;

use std::{
    collections::HashMap,
    str::FromStr,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use crate::{config::Config, utils};
use lazy_static::lazy_static;
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
        prelude::{ChannelId, GuildId, MessageId, Reaction, ReactionType, Ready, UserId},
    },
    prelude::*,
    Client,
};
use tokio::sync::{
    broadcast::{Receiver, Sender},
    RwLock,
};

lazy_static! {
    static ref DISCORD_ROLES_CHANNEL_ID_U64: u64 = crate::DISCORD_ROLES_CHANNEL_ID.parse().unwrap();
}

struct Handler {
    is_initted: Arc<AtomicBool>,
    central_receiver: Receiver<crate::CentralMessage>,
    sender: Sender<BotMessage>,

    bot_id: u64,
    admin_id: u64,

    guild_id: GuildId,

    antispam: Arc<RwLock<antispam::Antispam>>,
    /// Emoji to role id
    reaction_roles: Arc<RwLock<HashMap<String, u64>>>,
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

            bot_id: crate::DISCORD_BOT_ID.parse().unwrap(),
            admin_id: crate::DISCORD_ADMIN_ID.parse().unwrap(),

            guild_id: GuildId(crate::DISCORD_GUILD_ID.parse().unwrap()),

            antispam: Arc::new(RwLock::new(antispam::Antispam::new())),
            reaction_roles: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Discord bot connected!");

        // TODO can directly access the channel instead of iterating through all channels
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
                            crate::CONFIG.write().await.from(&c);
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

        // Job thread
        if !self.is_initted.load(std::sync::atomic::Ordering::Relaxed) {
            self.is_initted
                .store(true, std::sync::atomic::Ordering::Relaxed);

            let tick_duration = crate::CONFIG.read().await.job_tick_duration;

            let mut receiver = self.central_receiver.resubscribe();
            let sender = self.sender.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs_f32(tick_duration));
                loop {
                    interval.tick().await;
                    match receiver.try_recv() {
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
                            tokio::sync::broadcast::error::TryRecvError::Closed => {
                                error!("Channel closed");
                                break;
                            }
                            tokio::sync::broadcast::error::TryRecvError::Lagged(n) => {
                                debug!("Channel lagged by {} messages", n);
                            }
                            _ => {}
                        },
                    }
                }
            });
        }

        // Reaction roles
        {
            let configured_roles = &crate::CONFIG.read().await.reaction_roles;
            let mut cached_rr = self.reaction_roles.write().await;
            let roles = self.guild_id.roles(&ctx.http).await.unwrap();
            for (_, (id, role)) in roles.iter().enumerate() {
                let id = id.as_u64();
                let emoji = match configured_roles.get(&role.name) {
                    Some(v) => v,
                    None => continue,
                };
                cached_rr.insert(emoji.clone(), id.clone());
            }
        }

        {
            let cached_rr = self.reaction_roles.read().await;

            let roles_channel = ChannelId(*DISCORD_ROLES_CHANNEL_ID_U64);
            match roles_channel.messages(&ctx.http, |f| f).await {
                Ok(v) => {
                    for m in v.iter() {
                        for (_, (emoji, id)) in cached_rr.iter().enumerate() {
                            const PAGE_MAX: u8 = 100;
                            let mut starting_user_id: u64 = 0;
                            loop {
                                let users = match m
                                    .reaction_users(
                                        &ctx.http,
                                        ReactionType::Unicode(emoji.clone()),
                                        Some(PAGE_MAX),
                                        if starting_user_id == 0 {
                                            None
                                        } else {
                                            Some(UserId(starting_user_id))
                                        },
                                    )
                                    .await
                                {
                                    Ok(v) => v,
                                    Err(e) => {
                                        error!("Unable to get users that reacted to {}", &emoji);
                                        break;
                                    }
                                };

                                let mut last_user_id = match users.last() {
                                    Some(v) => v.id.as_u64(),
                                    None => break,
                                };
                                for user in users.iter() {
                                    let has_role = match user
                                        .has_role(&ctx.http, self.guild_id, *id)
                                        .await
                                    {
                                        Ok(v) => v,
                                        Err(e) => {
                                            error!("Error occurred for user {}: {}", &user.name, e);
                                            continue;
                                        }
                                    };

                                    if !has_role {
                                        let mut member =
                                            match self.guild_id.member(&ctx.http, user.id).await {
                                                Ok(v) => v,
                                                Err(e) => {
                                                    error!(
                                                        "Unable to get member data for {}",
                                                        &user.name
                                                    );
                                                    continue;
                                                }
                                            };

                                        match member.add_role(&ctx.http, *id).await {
                                            Ok(_) => {}
                                            Err(e) => {
                                                error!(
                                                    "Unable to handle role {} for user {}",
                                                    &emoji, &user.name
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                }

                                if users.len() >= PAGE_MAX.into() {
                                    starting_user_id = last_user_id.clone();
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => error!("Unable to process old roles"),
            }
        }

        // Set application commands
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

        let mut antispam = self.antispam.write().await;

        if antispam.is_spam(author_id) {
            debug!("Spammer detected {}", &message.author.name);
            match message.reply_mention(&ctx, "Please do not spam").await {
                Ok(_) => {}
                Err(e) => error!("Unable to send spammer detected message: {}", e.to_string()),
            }
            match message.delete(&ctx).await {
                Ok(_) => {}
                Err(e) => error!("Unable to delete spam message: {}", e.to_string()),
            }

            if antispam.should_timeout(author_id) {
                timeout_member(&ctx, &self.guild_id, author_id).await;
            }
        }
    }

    async fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        if *DISCORD_ROLES_CHANNEL_ID_U64 != *add_reaction.channel_id.as_u64() {
            return;
        }

        let user_id = match add_reaction.user_id {
            Some(id) => id,
            None => {
                error!("Unknown user added reaction");
                return;
            }
        };

        match add_reaction.emoji {
            ReactionType::Unicode(u) => {
                debug!("Reaction added: {}", u);

                let rr = self.reaction_roles.write().await;

                match rr.get(&u) {
                    Some(v) => match self.guild_id.member(&ctx.http, user_id).await {
                        Ok(mut m) => match m.add_role(&ctx.http, *v).await {
                            Ok(_) => debug!("Added role {} to {}", &u, &m.user.name),
                            Err(e) => error!("{}", e),
                        },
                        Err(e) => error!("{}", e),
                    },
                    None => error!("Tried to add unknown role from emoji: {}", &u),
                }
            }
            ReactionType::Custom { id, name, .. } => {
                ChannelId(crate::DISCORD_BOT_CONTROLLER_CHANNEL_ID.parse().unwrap())
                    .send_message(&ctx.http, |f| {
                        f.content(format!(
                            "Non-standard reaction emoji used: id - {}, name - {}",
                            id.as_u64(),
                            name.unwrap_or_default()
                        ))
                    })
                    .await
                    .unwrap();
            }
            _ => {
                ChannelId(crate::DISCORD_BOT_CONTROLLER_CHANNEL_ID.parse().unwrap())
                    .send_message(&ctx.http, |f| f.content("Unknown reaction added"))
                    .await
                    .unwrap();
            }
        }
    }

    async fn reaction_remove(&self, ctx: Context, remove_reaction: Reaction) {
        if *DISCORD_ROLES_CHANNEL_ID_U64 != *remove_reaction.channel_id.as_u64() {
            return;
        }

        let user_id = match remove_reaction.user_id {
            Some(id) => id,
            None => {
                error!("Unknown user added reaction");
                return;
            }
        };

        match remove_reaction.emoji {
            ReactionType::Unicode(u) => {
                debug!("Reaction added: {}", u);

                let rr = self.reaction_roles.write().await;

                match rr.get(&u) {
                    Some(v) => match self.guild_id.member(&ctx.http, user_id).await {
                        Ok(mut m) => match m.remove_role(&ctx.http, *v).await {
                            Ok(_) => debug!("Removed role {} from {}", &u, &m.user.name),
                            Err(e) => error!("{}", e),
                        },
                        Err(e) => error!("{}", e),
                    },
                    None => error!("Tried to add unknown role from emoji: {}", &u),
                }
            }
            ReactionType::Custom { id, name, .. } => {
                ChannelId(crate::DISCORD_BOT_CONTROLLER_CHANNEL_ID.parse().unwrap())
                    .send_message(&ctx.http, |f| {
                        f.content(format!(
                            "Non-standard reaction emoji used: id - {}, name - {}",
                            id.as_u64(),
                            name.unwrap_or_default()
                        ))
                    })
                    .await
                    .unwrap();
            }
            _ => {
                ChannelId(crate::DISCORD_BOT_CONTROLLER_CHANNEL_ID.parse().unwrap())
                    .send_message(&ctx.http, |f| f.content("Unknown reaction added"))
                    .await
                    .unwrap();
            }
        }
    }
}

/// Try and timeout a guild member if there is a valid timeout role configured.
async fn timeout_member(ctx: &Context, guild_id: &GuildId, author_id: &u64) {
    let timeout_role_id = crate::CONFIG.read().await.timeout_role_id;
    if timeout_role_id == 0 {
        debug!("No timeout role specified");
        return;
    }

    match guild_id.member(ctx, *author_id).await {
        Ok(mut m) => match m.add_role(ctx, timeout_role_id).await {
            Ok(_) => {}
            Err(e) => error!("Unable to timeout member {:?}: {:?}", author_id, e),
        },
        Err(e) => error!(
            "Tried to timeout non-existent member {:?}: {:?}",
            author_id, e
        ),
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
) -> anyhow::Result<()> {
    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix(crate::BOT_PREFIX).allow_dm(false);
            c
        })
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
        .await?;

    client.start().await.map_err(|e| anyhow::Error::from(e))
}
