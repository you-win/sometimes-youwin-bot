pub mod commands;
pub mod debug;
pub mod discord;
pub mod twitch;
pub mod utils;

const DISCORD_TOKEN: &str = env!("DISCORD_TOKEN");
const DISCORD_GUILD_ID: &str = env!("DISCORD_GUILD_ID");

const TWITCH_CLIENT_ID: &str = env!("TWITCH_CLIENT_ID");
const TWITCH_CLIENT_SECRET: &str = env!("TWITCH_CLIENT_SECRET");
const TWITCH_REFRESH_TOKEN: &str = env!("TWITCH_REFRESH_TOKEN");

const BOT_PREFIX: &str = "?";
