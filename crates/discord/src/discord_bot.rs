use super::Antispam;
use commands::CommandOutput;
use model::{
    config::{self, Config},
    creds::{BotCreds, DiscordCreds},
    messages::{CentralMessage, DiscordMessage},
};

use log::{debug, error, info};
use serenity::{async_trait, http::CacheHttp, model::prelude::*, prelude::*};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::{
        broadcast::{error::TryRecvError, Receiver, Sender},
        Mutex, RwLock,
    },
    time::Interval,
};

pub struct Bot {
    config: Arc<RwLock<Config>>,
    creds: DiscordCreds,

    is_initted: AtomicBool,

    antispam: Arc<RwLock<Antispam>>,
    reaction_roles: Arc<RwLock<HashMap<String, u64>>>,

    receiver: Receiver<CentralMessage>,
    sender: Sender<DiscordMessage>,

    interval: Arc<Mutex<Interval>>,
}

impl Bot {
    pub fn new(
        config: Arc<RwLock<Config>>,
        creds: DiscordCreds,
        receiver: Receiver<CentralMessage>,
        sender: Sender<DiscordMessage>,
    ) -> Self {
        Self {
            config,
            creds,

            is_initted: AtomicBool::new(false),

            antispam: Arc::new(RwLock::new(Antispam::new())),
            reaction_roles: Arc::new(RwLock::new(HashMap::new())),

            receiver,
            sender,

            interval: Arc::new(Mutex::new(tokio::time::interval(Duration::from_secs_f32(
                config::default_tick_duration(),
            )))),
        }
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        if !self.is_initted.load(Ordering::Relaxed) {
            self.is_initted.store(true, Ordering::Relaxed);
            start_job_thread(self, &ctx).await;
        }

        {
            debug!("Reading Discord config");

            let data_channel = ChannelId(self.creds.data_channel);

            let _ = data_channel
                .messages(&ctx.http, |x| x)
                .await
                .expect("Unable to read bot data.")
                .into_iter()
                .for_each(|m| {
                    let content = m
                        .content
                        .trim()
                        .trim_start_matches("`")
                        .trim_start_matches("TOML")
                        .trim_end_matches("`")
                        .trim();

                    match toml::from_str::<Config>(content) {
                        Ok(c) => {
                            if let Err(e) = self.sender.send(DiscordMessage::ConfigUpdated(c)) {
                                error!("{e}");
                            }
                            debug!("Sent ConfigUpdated message!");
                        }
                        Err(e) => error!("{e}"),
                    }
                });

            debug!("Finished reading Discord config");
        }

        if let Err(e) = self.sender.send(DiscordMessage::Ready) {
            error!("{e}");
        } else {
            info!("Discord bot ready!");
        }
    }

    async fn message(&self, ctx: Context, message: Message) {
        let author_id = message.author.id.as_u64();

        if author_id == &self.creds.bot_id {
            return;
        }
        if !&message.content.starts_with(self.creds.bot_prefix()) {
            return;
        }

        let config = &*self.config.read().await;

        let output = commands::parse(
            &message.content,
            commands::AdditionalInfo::Discord {
                name: message.author.name.clone(),
                user_id: *message.id.as_u64(),
                channel_id: *message.channel_id.as_u64(),
            },
            &config,
        );

        match output {
            CommandOutput::Command { value, command } => {
                if let Some(v) = value {
                    reply_mention(&ctx, &message, &v).await;
                }
            }
            CommandOutput::AdminCommand { value, command } => {}
            CommandOutput::Error(e) => {
                reply_mention(
                    &ctx,
                    &message,
                    &format!(
                        "{e}\nAd-hoc commands: {}",
                        config.ad_hoc_commands().join(", ")
                    ),
                )
                .await
            }
        }
    }
}

async fn reply_mention(cache_http: impl CacheHttp, message: &Message, text: &String) {
    if let Err(e) = message.reply_mention(cache_http, text).await {
        error!("{e}");
    }
}

async fn start_job_thread(bot: &Bot, ctx: &Context) {
    debug!("Starting Discord job thread.");

    tokio::spawn({
        let client = ctx.clone();

        let config = bot.config.clone();
        let creds = bot.creds.clone();

        let antispam = bot.antispam.clone();
        let reaction_roles = bot.reaction_roles.clone();

        let mut receiver = bot.receiver.resubscribe();
        let sender = bot.sender.clone();

        let interval = bot.interval.clone();

        async move {
            loop {
                let _ = interval.lock().await.tick().await;

                match receiver.try_recv() {
                    Ok(m) => match m {
                        CentralMessage::ConfigUpdated => {
                            debug!("Updating config");

                            let config = config.read().await;

                            *interval.lock().await = tokio::time::interval(
                                Duration::from_secs_f32(config.tick_duration),
                            );

                            {
                                let mut rr = reaction_roles.write().await;

                                // TODO panic or try again
                                let guild = GuildId(creds.guild_id);
                                for (id, role) in
                                    guild.roles(&client.http).await.unwrap_or_else(|e| {
                                        error!("Unable to get guild roles: {e}");
                                        HashMap::new()
                                    })
                                {
                                    let id = id.as_u64();
                                    let emoji = match config.reaction_roles.get(&role.name) {
                                        Some(v) => v,
                                        None => continue,
                                    };
                                    rr.insert(emoji.clone(), id.clone());
                                }
                            }

                            {
                                let rr = reaction_roles.read().await;

                                let roles_channel = ChannelId(config.roles_channel);
                                match roles_channel.messages(&client.http, |m| m).await {
                                    Ok(v) => {}
                                    Err(e) => error!("Unable to process old role: {e}"),
                                }

                                if let Ok(messages) =
                                    roles_channel.messages(&client.http, |m| m).await
                                {
                                    process_old_reaction_roles(
                                        &client,
                                        &creds,
                                        &rr.clone(),
                                        &messages,
                                    )
                                    .await;
                                } else {
                                    error!("Unable to process old roles.");
                                }
                            }

                            debug!("Finished updating config!");
                        }
                        CentralMessage::Twitch(m) => {
                            //
                        }
                        CentralMessage::Shutdown => {
                            break;
                        }
                        _ => {}
                    },
                    Err(e) => match e {
                        TryRecvError::Closed => {
                            error!("Channel closed");

                            break;
                        }
                        TryRecvError::Lagged(n) => {
                            debug!("Channel lagged by {n} messages");
                        }
                        _ => {}
                    },
                }
            }
        }
    });

    debug!("Started Discord job thread!");
}

async fn process_old_reaction_roles(
    ctx: &Context,
    creds: &DiscordCreds,
    cached_rr: &HashMap<String, u64>,
    messages: &Vec<Message>,
) {
    debug!("Processing old reaction roles");

    let guild = GuildId(creds.guild_id);

    for message in messages {
        for (emoji, id) in cached_rr {
            const PAGE_MAX: u8 = 100;
            let mut starting_user_id: u64 = 0;
            loop {
                let users = match message
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
                        error!("Unable to get users that reacted to {emoji}: {e}");
                        continue;
                    }
                };

                let last_user_id = match users.last() {
                    Some(v) => v.id.as_u64(),
                    None => break,
                };

                for user in users.iter() {
                    let has_role = match user.has_role(&ctx.http, guild, *id).await {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Error occurred for user: {}: {e}", user.name);
                            continue;
                        }
                    };

                    if !has_role {
                        let mut member = match guild.member(&ctx.http, user.id).await {
                            Ok(v) => v,
                            Err(e) => {
                                error!("Unable to get member data for {}: {e}", user.name);
                                continue;
                            }
                        };

                        if let Err(e) = member.add_role(&ctx.http, *id).await {
                            error!("Unable to handle role {emoji} for user {}", user.name);
                            continue;
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

    debug!("Finished processing old reaction roles");
}