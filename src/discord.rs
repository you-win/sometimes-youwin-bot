mod antispam;
mod commands;

use std::{str::FromStr, sync::Arc};

use crate::utils;
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
        prelude::{ChannelId, GuildId, Reaction, Ready},
    },
    prelude::*,
    Client,
};

struct Handler {
    bot_id: u64,
    admin_id: u64,

    guild_id: GuildId,

    antispam: Arc<RwLock<antispam::Antispam>>,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            bot_id: crate::DISCORD_BOT_ID.parse().unwrap(),
            admin_id: crate::DISCORD_ADMIN_ID.parse().unwrap(),

            guild_id: GuildId(crate::DISCORD_GUILD_ID.parse().unwrap()),

            antispam: Arc::new(RwLock::new(antispam::Antispam::new())),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Discord bot connected!");

        let mut channels = self.guild_id.channels(&ctx.http).await.unwrap();
        match channels.get_mut(&ChannelId::from_str(crate::DISCORD_BOT_DATA_CHANNEL_ID).unwrap()) {
            Some(c) => {
                let messages = c.messages(&ctx.http, |m| m).await.unwrap();

                for message in messages.iter() {
                    debug!("{}", &message.content);
                    //
                }
            }
            None => panic!(
                "Unable to read config from {}",
                crate::DISCORD_BOT_DATA_CHANNEL_ID
            ),
        }

        // TODO read configuration from bot data channel

        info!("Discord bot ready!");
    }

    async fn message(&self, ctx: Context, message: Message) {
        let author_id = message.author.id.as_u64();

        if author_id == &self.bot_id || author_id == &self.admin_id {
            return;
        }

        if self.antispam.write().await.is_spam(author_id) {
            debug!("Spammer detected {}", &message.author.name);
            match message.reply_mention(&ctx, "Please do not spam").await {
                Ok(_) => {}
                Err(e) => error!("Unable to send spammer detected message: {}", e.to_string()),
            }
            match message.delete(ctx).await {
                Ok(_) => {}
                Err(e) => error!("Unable to delete spam message: {}", e.to_string()),
            }

            if self.antispam.write().await.should_timeout(author_id) {
                // TODO set user role here
            }
        }
    }

    async fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        //
    }

    async fn reaction_remove(&self, ctx: Context, remove_reaction: Reaction) {
        //
    }
}

#[tokio::main]
pub async fn run_bot() -> Result<(), Box<dyn std::error::Error>> {
    let framework = StandardFramework::new()
        .configure(configure_bot)
        .group(&commands::GENERAL_GROUP);

    let token = crate::DISCORD_TOKEN;
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_INVITES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(token, intents)
        .event_handler(Handler::new())
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}

fn configure_bot(c: &mut Configuration) -> &mut Configuration {
    c.prefix(crate::BOT_PREFIX);

    c.allow_dm(false);

    c
}
