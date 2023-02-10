mod twitch_bot;

use log::{debug, error, info};
use std::sync::Arc;
use tokio::sync::{
    broadcast::{Receiver, Sender},
    RwLock,
};

use model::{
    config::Config,
    creds::TwitchCreds,
    messages::{CentralMessage, TwitchMessage},
};

pub async fn run_bot(
    config: Arc<RwLock<Config>>,
    creds: TwitchCreds,
    receiver: Receiver<CentralMessage>,
    sender: Sender<TwitchMessage>,
) -> anyhow::Result<()> {
    info!("Starting Twitch bot");

    let mut bot = twitch_bot::Bot::new(config, creds, receiver, sender).await?;

    tokio::spawn({
        let mut ticks: u64 = 0;

        async move {
            loop {
                bot.tick().await;

                ticks += 1;
                if ticks >= bot.check_live_ticks {
                    ticks = 0;

                    debug!("Checking if channel is live");

                    if let Err(e) = bot.check_channel_live().await {
                        error!("{e}");
                    }
                }

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
        }
    });

    Ok(())
}
