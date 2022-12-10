use crate::{commands as c, utils};
use serenity::{
    async_trait,
    framework::{
        standard::{
            macros::{command, group},
            CommandResult, Configuration,
        },
        StandardFramework,
    },
    model::channel::Message,
    prelude::*,
    Client,
};

#[group]
#[commands(ping, whoami, high_five, ferris_say, roll)]
struct General;

#[command]
async fn ping(ctx: &Context, message: &Message) -> CommandResult {
    message.reply(ctx, c::ping()).await?;

    Ok(())
}

#[command]
async fn whoami(ctx: &Context, message: &Message) -> CommandResult {
    message.reply(ctx, c::whoami(&message.author.name)).await?;

    Ok(())
}

#[command]
#[aliases("high-five")]
async fn high_five(ctx: &Context, message: &Message) -> CommandResult {
    message.reply(ctx, c::high_five()).await?;

    Ok(())
}

#[command]
#[aliases("ferris-say", "ferrissay", "cowsay")]
async fn ferris_say(ctx: &Context, message: &Message) -> CommandResult {
    let val = match c::ferris_say(&utils::strip_command_prefix(&message.content), 36) {
        Ok(v) => v,
        Err(e) => e.to_string(),
    };

    message.reply(ctx, format!("```\n{}```", val)).await?;

    Ok(())
}

#[command]
async fn roll(ctx: &Context, message: &Message) -> CommandResult {
    let sides = match utils::strip_command_prefix(&message.content).parse() {
        Ok(v) => v,
        Err(_) => 6,
    };

    let val = c::roll(sides);

    message.reply(ctx, val).await?;

    Ok(())
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

#[tokio::main]
pub async fn run_bot() -> Result<(), Box<dyn std::error::Error>> {
    let framework = StandardFramework::new()
        .configure(configure_bot)
        .group(&GENERAL_GROUP);

    let token = crate::DISCORD_TOKEN;
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_INVITES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
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
