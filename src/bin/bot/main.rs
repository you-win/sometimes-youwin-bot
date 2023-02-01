use std::{sync::atomic::Ordering, time::Duration};

use log::{debug, error, info};
use tokio::sync::broadcast;

use sometimes_youwin as yw;
use yw::{config::Config, discord, twitch, IS_RUNNING, TICK_DURATION};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting build {} with rev {}", yw::BUILD_NAME, yw::GIT_REV);

    env_logger::Builder::new()
        .parse_filters(
            format!(
                "warn,bot={},sometimes_youwin={}",
                yw::LOG_LEVEL,
                yw::LOG_LEVEL
            )
            .as_str(),
        )
        .init();

    IS_RUNNING.store(true, Ordering::Relaxed);

    let (host_sender, _) = broadcast::channel(10);
    let (discord_sender, mut discord_receiver) = broadcast::channel(10);
    let (twitch_sender, mut twitch_receiver) = broadcast::channel(10);

    {
        let interrupt_sender = host_sender.clone();

        ctrlc::set_handler(move || {
            IS_RUNNING.store(false, Ordering::Relaxed);
            match interrupt_sender.send(yw::CentralMessage::Shutdown) {
                Ok(_) => info!("Interrupt sent!"),
                Err(e) => error!("{}", e),
            }
        })?;
    }

    let host_sender_discord = host_sender.subscribe();
    let discord_join_handle = tokio::spawn(async move {
        discord::run_bot(host_sender_discord, discord_sender)
            .await
            .unwrap();
    });

    let host_sender_twitch = host_sender.subscribe();
    let twitch_join_handle = tokio::spawn(async move {
        let mut wait_interval = tokio::time::interval(Duration::from_secs_f32(5.0));
        while twitch::run_bot(host_sender_twitch.resubscribe(), twitch_sender.clone())
            .await
            .is_err()
        {
            wait_interval.tick().await;
        }
    });

    let mut interval = tokio::time::interval(*TICK_DURATION);
    loop {
        interval.tick().await;

        if !IS_RUNNING.load(Ordering::Relaxed) {
            break;
        }

        match discord_receiver.try_recv() {
            Ok(v) => match v {
                discord::BotMessage::Debug(t) => debug!("Discord receiver: {}", t),
                discord::BotMessage::Error(t) => error!("Discord receiver: {}", t),
                discord::BotMessage::Ready => info!("Discord receiver ready!"),
                discord::BotMessage::ConfigUpdated(c) => {
                    host_sender
                        .send(yw::CentralMessage::ConfigUpdated(c))
                        .unwrap();
                }
                discord::BotMessage::Shutdown => debug!("Discord shutdown received"),
                _ => {}
            },
            Err(e) => match e {
                broadcast::error::TryRecvError::Empty => {}
                broadcast::error::TryRecvError::Closed => {
                    error!("Discord receiver closed");
                    host_sender.send(yw::CentralMessage::Shutdown).unwrap();
                    break;
                }
                broadcast::error::TryRecvError::Lagged(n) => {
                    error!("Discord receiver lagged by {} messages", n)
                }
            },
        }

        match twitch_receiver.try_recv() {
            Ok(v) => match v {
                twitch::BotMessage::ChannelLive { .. } => {
                    info!("Channel live!");
                    if let Err(e) = host_sender.send(yw::CentralMessage::Twitch(v)) {
                        error!("{:?}", e);
                    }
                }
                _ => {}
            },
            Err(e) => match e {
                broadcast::error::TryRecvError::Empty => {}
                broadcast::error::TryRecvError::Closed => {
                    error!("Discord receiver closed");
                    host_sender.send(yw::CentralMessage::Shutdown).unwrap();
                    break;
                }
                broadcast::error::TryRecvError::Lagged(n) => {
                    error!("Discord receiver lagged by {} messages", n)
                }
            },
        }
    }

    {
        discord_join_handle.abort();
        twitch_join_handle.abort();
    }

    // Discord doesn't have a way to break out of its main loop, so it will return an error when
    // forcibly aborted.
    {
        assert!(discord_join_handle.await.unwrap_err().is_cancelled());
        assert!(twitch_join_handle.await.is_ok());
    }

    info!("Finished!");

    Ok(())
}
