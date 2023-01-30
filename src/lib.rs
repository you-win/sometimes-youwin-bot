pub mod commands;
pub mod config;
pub mod debug;
pub mod discord;
pub mod twitch;
pub mod utils;

use std::{sync::atomic::AtomicBool, time::Duration};

use lazy_static::lazy_static;

use config::Config;

pub const BUILD_NAME: &str = env!("BUILD_NAME");
pub const GIT_REV: &str = env!("GIT_REV");
pub const LOG_LEVEL: &str = env!("LOG_LEVEL");

const DISCORD_TOKEN: &str = env!("DISCORD_TOKEN");
const DISCORD_GUILD_ID: &str = env!("DISCORD_GUILD_ID");
const DISCORD_BOT_DATA_CHANNEL_ID: &str = env!("DISCORD_BOT_DATA_CHANNEL_ID");
const DISCORD_BOT_CONTROLLER_CHANNEL_ID: &str = env!("DISCORD_BOT_CONTROLLER_CHANNEL_ID");
const DISCORD_BOT_ID: &str = env!("DISCORD_BOT_ID");
const DISCORD_ADMIN_ID: &str = env!("DISCORD_ADMIN_ID");
const DISCORD_ROLES_CHANNEL_ID: &str = env!("DISCORD_ROLES_CHANNEL_ID");

const TWITCH_CLIENT_ID: &str = env!("TWITCH_CLIENT_ID");
const TWITCH_CLIENT_SECRET: &str = env!("TWITCH_CLIENT_SECRET");
const TWITCH_REFRESH_TOKEN: &str = env!("TWITCH_REFRESH_TOKEN");
const TWITCH_CHANNEL_NAME: &str = env!("TWITCH_CHANNEL_NAME");

const BOT_PREFIX: &str = "?";

// pub const TICK_DURATION: f32 = 0.5;
lazy_static! {
    pub static ref TICK_DURATION: Duration = Duration::from_secs_f32(0.5);
}

pub static IS_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub enum CentralMessage {
    Discord(discord::BotMessage),
    Twitch(twitch::BotMessage),

    ConfigUpdated(Config),

    Shutdown,
}

impl From<discord::BotMessage> for CentralMessage {
    fn from(m: discord::BotMessage) -> Self {
        Self::Discord(m)
    }
}

impl From<twitch::BotMessage> for CentralMessage {
    fn from(m: twitch::BotMessage) -> Self {
        Self::Twitch(m)
    }
}
