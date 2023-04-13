use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use log::{debug, error, info, LevelFilter};
use model::{
    config::Config,
    creds,
    messages::{CentralMessage, DiscordMessage, ServerMessage, TwitchMessage},
};
use tokio::{
    sync::{
        broadcast::{self, Receiver, Sender},
        RwLock,
    },
    task::JoinHandle,
};

const WORKSPACE_CRATES: [&str; 6] = [
    "commands",
    "discord",
    "model",
    "scripting",
    "server",
    "twitch",
];

pub static IS_RUNNING: AtomicBool = AtomicBool::new(true);

fn start_twitch_bot(
    sender: Sender<TwitchMessage>,
    receiver: Receiver<CentralMessage>,
    config: Arc<RwLock<Config>>,
) -> JoinHandle<()> {
    let sender = sender;
    let receiver = receiver;
    let creds = creds::TwitchCreds::new(
        env!("TWITCH_REFRESH_TOKEN"),
        env!("TWITCH_CLIENT_ID"),
        env!("TWITCH_CLIENT_SECRET"),
        env!("TWITCH_BOT_NAME"),
        env!("TWITCH_CHANNEL_NAME"),
    );

    tokio::spawn({
        let mut wait_interval = tokio::time::interval(Duration::from_secs_f32(5.0));
        async move {
            while twitch::run_bot(
                config.clone(),
                creds.clone(),
                receiver.resubscribe(),
                sender.clone(),
            )
            .await
            .is_err()
            {
                wait_interval.tick().await;
            }
        }
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!(
        "Starting build {} with rev {}",
        env!("BUILD_NAME"),
        env!("GIT_REV")
    );

    let logging_builder = logging::LoggingBuilder::new()
        .app_name("sometimes-youwin-bot")
        .organization("sometimesyouwin")
        .qualifier("win")
        .global_level(LevelFilter::Warn);

    if cfg!(debug_assertions) {
        let mut logging_builder = logging_builder;
        for c in WORKSPACE_CRATES {
            logging_builder = logging_builder.level_for(c, LevelFilter::Debug);
        }
        logging_builder.finish()?;
    } else {
        logging_builder.finish()?;
    }

    info!("Logging initted!");

    let config = Arc::new(RwLock::new(Config::new()));

    let (host_sender, _) = broadcast::channel(10);
    let (discord_sender, mut discord_receiver) = broadcast::channel(10);
    let (twitch_sender, mut twitch_receiver) = broadcast::channel(10);
    #[cfg(feature = "server")]
    let (server_sender, mut server_receiver) = broadcast::channel(10);

    {
        let interrupt_sender = host_sender.clone();

        ctrlc::set_handler(move || {
            IS_RUNNING.store(false, Ordering::Relaxed);
            if let Err(e) = interrupt_sender.send(CentralMessage::Shutdown) {
                error!("{e}");
            }
        })?;

        debug!("Set ctrl+c handler!");
    }

    let discord_join_handle = {
        let config = config.clone();
        let creds = creds::DiscordCreds::new(
            env!("DISCORD_TOKEN"),
            env!("DISCORD_BOT_ID"),
            env!("DISCORD_ADMIN_ID"),
            env!("DISCORD_GUILD_ID"),
            env!("DISCORD_BOT_DATA_CHANNEL_ID"),
        )
        .expect("Unable to parse discord creds");
        let receiver = host_sender.subscribe();
        tokio::spawn({
            async move {
                if let Err(e) = discord::run_bot(config, creds, receiver, discord_sender).await {
                    error!("{e}");
                }
            }
        })
    };

    let mut twitch_join_handle: Option<JoinHandle<()>> = None;
    #[cfg(feature = "server")]
    let mut server_join_handle: Option<JoinHandle<()>> = None;

    let mut interval =
        tokio::time::interval(Duration::from_secs_f32(config.read().await.tick_duration));

    loop {
        let _ = interval.tick().await;

        if !IS_RUNNING.load(Ordering::Relaxed) {
            break;
        }

        match discord_receiver.try_recv() {
            Ok(v) => match v {
                DiscordMessage::Ready => {
                    info!("Discord ready!");

                    {
                        twitch_join_handle = Some(start_twitch_bot(
                            twitch_sender.clone(),
                            host_sender.subscribe(),
                            config.clone(),
                        ));

                        debug!("Spawned task for Twitch bot!");
                    }

                    #[cfg(feature = "server")]
                    {
                        server_join_handle = {
                            let config = config.clone();
                            let receiver = host_sender.subscribe();
                            let sender = server_sender.clone();

                            Some(tokio::spawn(async move {
                                if let Err(e) = server::run(config.clone(), receiver, sender).await
                                {
                                    error!("{e}");
                                }
                            }))
                        };

                        debug!("Spawned task for Server!");
                    }
                }
                DiscordMessage::ConfigUpdated(c) => {
                    *config.write().await = c;
                    debug!("Config updated!");

                    if let Err(e) = host_sender.send(CentralMessage::ConfigUpdated) {
                        error!("{e}");
                    }
                }
                DiscordMessage::Debug(m) => {
                    debug!("Discord: {m}");
                }
                DiscordMessage::Error(m) => {
                    error!("Discord: {m}");
                }
            },
            Err(e) => match e {
                broadcast::error::TryRecvError::Empty => {}
                broadcast::error::TryRecvError::Closed => {
                    error!("Discord receiver closed");
                    host_sender.send(CentralMessage::Shutdown).unwrap();
                    break;
                }
                broadcast::error::TryRecvError::Lagged(n) => {
                    error!("Discord receiver lagged by {} messages", n)
                }
            },
        }

        match twitch_receiver.try_recv() {
            Ok(v) => match v {
                TwitchMessage::Ready => {}
                TwitchMessage::ChannelLive { .. } => {
                    debug!("Channel is live: {:?}", &v);
                    if let Err(e) = host_sender.send(CentralMessage::Twitch(v)) {
                        error!("{e}");
                    }
                }
                TwitchMessage::Debug(m) => {
                    debug!("{m}");
                }
                TwitchMessage::Error(m) => {
                    error!("{m}");
                }
                TwitchMessage::TokenExpired => {
                    // TODO check memory usage to see if this is actually killing the task
                    if let Some(handle) = twitch_join_handle {
                        handle.abort();
                    }

                    twitch_join_handle = Some(start_twitch_bot(
                        twitch_sender.clone(),
                        host_sender.subscribe(),
                        config.clone(),
                    ));
                }
            },
            Err(e) => match e {
                broadcast::error::TryRecvError::Empty => {}
                broadcast::error::TryRecvError::Closed => {
                    error!("Twitch receiver closed");
                    host_sender.send(CentralMessage::Shutdown).unwrap();
                    break;
                }
                broadcast::error::TryRecvError::Lagged(n) => {
                    error!("Twitch receiver lagged by {} messages", n)
                }
            },
        }

        #[cfg(feature = "server")]
        match server_receiver.try_recv() {
            Ok(v) => match v {
                ServerMessage::Ready => {}
                ServerMessage::Debug(m) => {
                    debug!("{m}");
                }
                ServerMessage::Error(m) => {
                    error!("{m}");
                }
            },
            Err(e) => match e {
                broadcast::error::TryRecvError::Empty => {}
                broadcast::error::TryRecvError::Closed => {
                    error!("Server receiver closed");
                    host_sender.send(CentralMessage::Shutdown).unwrap();
                    break;
                }
                broadcast::error::TryRecvError::Lagged(n) => {
                    error!("Server receiver lagged by {} messages", n)
                }
            },
        }
    }

    discord_join_handle.abort();
    assert!(discord_join_handle.await.unwrap_err().is_cancelled());

    if let Some(handle) = twitch_join_handle {
        handle.abort();
        assert!(handle.await.is_ok());
    }

    #[cfg(feature = "server")]
    if let Some(handle) = server_join_handle {
        handle.abort();
    }

    info!("Finished!");

    Ok(())
}
