pub mod commands;
pub mod debug;
pub mod discord;
pub mod twitch;
pub mod utils;

pub const BUILD_NAME: &str = env!("BUILD_NAME");
pub const GIT_REV: &str = env!("GIT_REV");
pub const LOG_LEVEL: &str = env!("LOG_LEVEL");

const DISCORD_TOKEN: &str = env!("DISCORD_TOKEN");
const DISCORD_GUILD_ID: &str = env!("DISCORD_GUILD_ID");
const DISCORD_BOT_DATA_CHANNEL_ID: &str = env!("DISCORD_BOT_DATA_CHANNEL_ID");
const DISCORD_BOT_ID: &str = env!("DISCORD_BOT_ID");
const DISCORD_ADMIN_ID: &str = env!("DISCORD_ADMIN_ID");

const TWITCH_CLIENT_ID: &str = env!("TWITCH_CLIENT_ID");
const TWITCH_CLIENT_SECRET: &str = env!("TWITCH_CLIENT_SECRET");
const TWITCH_REFRESH_TOKEN: &str = env!("TWITCH_REFRESH_TOKEN");

const BOT_PREFIX: &str = "?";
