use std::{sync::atomic::Ordering, time::Duration};

use log::{debug, error, info};
use tokio::sync::broadcast;

use sometimes_youwin as yw;
use yw::{config::Config, discord, twitch, IS_RUNNING};

#[tokio::main]
async fn main() {
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

    let (host_sender, _host_receiver) = broadcast::channel(10);
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
        })
        .unwrap();
    }

    let host_sender_discord = host_sender.subscribe();
    let discord_join_handle = tokio::spawn(async move {
        discord::run_bot(host_sender_discord, discord_sender).await;
    });

    let user_token = twitch::create_user_token().await.unwrap();

    let host_sender_twitch = host_sender.subscribe();
    let twitch_join_handle = tokio::spawn(async move {
        twitch::run_bot(host_sender_twitch, twitch_sender, user_token).await;
    });

    loop {
        if !IS_RUNNING.load(Ordering::Relaxed) {
            break;
        }

        match discord_receiver.try_recv() {
            Ok(v) => match v {
                discord::BotMessage::Debug(t) => debug!("Discord receiver: {}", t),
                discord::BotMessage::Error(t) => error!("Discord receiver: {}", t),
                discord::BotMessage::Ready => info!("Discord receiver ready!"),
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
    }

    {
        discord_join_handle.abort();
        twitch_join_handle.abort();
    }

    {
        assert!(discord_join_handle.await.unwrap_err().is_cancelled());
        assert!(twitch_join_handle.await.unwrap_err().is_cancelled());
    }

    info!("Finished!");
}
