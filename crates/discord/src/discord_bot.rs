use super::Antispam;
use log::{debug, error, info};
use model::{
    config::{self, Config},
    creds::DiscordCreds,
    messages::{CentralMessage, DiscordMessage},
};
use serenity::{async_trait, model::prelude::*, prelude::*};

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
        info!("Discord bot ready!");

        if !self.is_initted.load(Ordering::Relaxed) {
            self.is_initted.store(true, Ordering::Relaxed);
            start_job_thread(self, &ctx);
        }

        {
            let data_channel = ChannelId(self.creds.data_channel);
            let _ = data_channel
                .messages(&ctx.http, |x| x)
                .await
                .expect("Unable to read bot data.")
                .into_iter()
                .map(|m| {
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
                        }
                        Err(e) => error!("{e}"),
                    }
                });
        }
    }

    async fn message(&self, ctx: Context, new_message: Message) {
        // TODO stub
    }
}

fn start_job_thread(bot: &Bot, ctx: &Context) {
    tokio::spawn({
        let client = ctx.http.clone();
        let config = bot.config.clone();

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
                            let config = config.read().await;

                            *interval.lock().await = tokio::time::interval(
                                Duration::from_secs_f32(config.tick_duration),
                            );
                            // TODO need to convert string to u64
                            // reaction_roles
                            //     .write()
                            //     .await
                            //     .clone_from(&config.reaction_roles);
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
}
