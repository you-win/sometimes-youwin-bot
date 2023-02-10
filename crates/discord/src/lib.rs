mod antispam;
mod commands;
mod discord_bot;

use antispam::Antispam;
use model::{
    config::Config,
    creds::{BotCreds, DiscordCreds},
    messages::{CentralMessage, DiscordMessage},
};
use serenity::{framework::StandardFramework, model::prelude::*, prelude::*};

use std::sync::Arc;
use tokio::sync::{
    broadcast::{Receiver, Sender},
    RwLock,
};

pub async fn run_bot(
    config: Arc<RwLock<Config>>,
    creds: DiscordCreds,
    receiver: Receiver<CentralMessage>,
    sender: Sender<DiscordMessage>,
) -> anyhow::Result<()> {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix(creds.bot_prefix()).allow_dm(false))
        // .group(&commands::GENERAL_GROUP)
        ;

    let token = creds.token.to_owned();
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_INVITES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT;
    let bot = discord_bot::Bot::new(config, creds, receiver, sender);

    let mut client = Client::builder(token, intents)
        .event_handler(bot)
        .framework(framework)
        .await?;

    client.start().await.map_err(anyhow::Error::from)
}
