use crate::config::Config;

#[derive(Debug, Clone)]
pub enum CentralMessage {
    Discord(DiscordMessage),
    Twitch(TwitchMessage),

    ConfigUpdated,

    Shutdown,
}

#[derive(Debug, Clone)]
pub enum DiscordMessage {
    Debug(String),
    Error(String),

    ConfigUpdated(Config),

    Ready,
}

#[derive(Debug, Clone)]
pub enum TwitchMessage {
    Debug(String),
    Error(String),

    Ready,
    ChannelLive { channel: String, title: String },
}