use crate::config::Config;

#[derive(Debug, Clone)]
pub enum CentralMessage {
    Discord(DiscordMessage),
    Twitch(TwitchMessage),
    Server(ServerMessage),

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
    ChannelLive {
        channel: String,
        title: String,
        url: String,
    },

    TokenExpired,
}

#[derive(Debug, Clone)]
pub enum ServerMessage {
    Debug(String),
    Error(String),

    Ready,
}
