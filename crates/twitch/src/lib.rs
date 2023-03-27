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

use crate::twitch_bot::create_bots;

pub async fn run_bot(
    config: Arc<RwLock<Config>>,
    creds: TwitchCreds,
    receiver: Receiver<CentralMessage>,
    sender: Sender<TwitchMessage>,
) -> anyhow::Result<()> {
    info!("Starting Twitch bot");

    let (mut api_bot, mut chat_bot) = create_bots(config, creds, receiver, sender).await?;

    let handle = tokio::spawn(async move {
        loop {
            if let Err(e) = chat_bot.handle_chat().await {
                error!("{e}");
                break;
            }
        }
    });

    tokio::spawn({
        let mut ticks: u64 = 0;

        async move {
            loop {
                api_bot.tick().await;

                ticks += 1;
                if ticks >= api_bot.check_live_ticks {
                    ticks = 0;

                    if let Err(e) = api_bot.check_channel_live().await {
                        error!("{e}");
                    }
                }

                if !api_bot.handle_central_message().await {
                    handle.abort();
                    break;
                }
            }
        }
    });

    Ok(())
}
