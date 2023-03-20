use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use fern::{
    colors::{Color, ColoredLevelConfig},
    Dispatch,
};
use log::{debug, error, info, LevelFilter};
use model::{
    config::Config,
    creds,
    messages::{CentralMessage, DiscordMessage, TwitchMessage},
};
use tokio::{
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

pub static IS_RUNNING: AtomicBool = AtomicBool::new(true);

fn logging() -> anyhow::Result<()> {
    let colors = ColoredLevelConfig::new()
        .info(Color::Blue)
        .warn(Color::Yellow)
        .error(Color::Red)
        .debug(Color::Magenta)
        .trace(Color::BrightGreen);

    let term_config = Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}] {} - {}",
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::Warn)
        .level_for("sometimes_youwin_bot", LevelFilter::Debug)
        .chain(std::io::stdout());

    let dirs = directories::ProjectDirs::from("win", "sometimesyou", "bot")
        .expect("Unable to get project directory.");
    let project_path = dirs.data_dir();

    {
        let log_dir = project_path.join("logs");
        let log_dir = log_dir.as_path();

        std::fs::create_dir_all(log_dir)?;
    }

    let file_config = Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] {} {} - {}",
                record.level(),
                chrono::Local::now().format("%Y-%m-%d_%H:%M:%S"),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::Warn)
        .level_for("sometimes_youwin_bot", LevelFilter::Debug)
        .chain(fern::log_file(format!(
            "{}/logs/syb.log",
            project_path.to_str().expect("Unable to get project path")
        ))?);

    Dispatch::new()
        .chain(term_config)
        .chain(file_config)
        .apply()?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!(
        "Starting build {} with rev {}",
        env!("BUILD_NAME"),
        env!("GIT_REV")
    );

    env_logger::Builder::new()
        .parse_filters("warn,twitchchat=warn,sometimes_youwin_bot=debug,discord=debug,twitch=debug")
        .init();

    info!("Logging initted!");

    let config = Arc::new(RwLock::new(Config::new()));

    let (host_sender, _) = broadcast::channel(10);
    let (discord_sender, mut discord_receiver) = broadcast::channel(10);
    let (twitch_sender, mut twitch_receiver) = broadcast::channel(10);

    {
        let interrupt_sender = host_sender.clone();

        ctrlc::set_handler(move || {
            IS_RUNNING.store(false, Ordering::Relaxed);
            if let Err(e) = interrupt_sender.send(CentralMessage::Shutdown) {
                error!("{e}");
            }
        })?;
    }

    debug!("Set ctrl+c handler!");

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

                    twitch_join_handle = {
                        let sender = twitch_sender.clone();
                        let receiver = host_sender.subscribe();
                        let config = config.clone();
                        let creds = creds::TwitchCreds::new(
                            env!("TWITCH_REFRESH_TOKEN"),
                            env!("TWITCH_CLIENT_ID"),
                            env!("TWITCH_CLIENT_SECRET"),
                            env!("TWITCH_BOT_NAME"),
                            env!("TWITCH_CHANNEL_NAME"),
                        );

                        Some(tokio::spawn({
                            let mut wait_interval =
                                tokio::time::interval(Duration::from_secs_f32(5.0));
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
                        }))
                    };

                    debug!("Spawned task for Twitch bot!");
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
    }

    discord_join_handle.abort();
    assert!(discord_join_handle.await.unwrap_err().is_cancelled());

    if twitch_join_handle.is_some() {
        let twitch_join_handle = twitch_join_handle.unwrap();

        twitch_join_handle.abort();
        assert!(twitch_join_handle.await.is_ok());
    }

    info!("Finished!");

    Ok(())
}
