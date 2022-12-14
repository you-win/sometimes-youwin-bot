use crate::{commands as c, utils};
use serenity::{
    framework::standard::{
        macros::{command, group},
        CommandResult,
    },
    model::prelude::Message,
    prelude::*,
};

#[group]
#[commands(ping, whoami, high_five, ferris_say, roll, config)]
pub struct General;

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

#[command]
async fn config(ctx: &Context, message: &Message) -> CommandResult {
    message
        .reply(ctx, format!("```\n{}```", c::config()))
        .await?;

    Ok(())
}
