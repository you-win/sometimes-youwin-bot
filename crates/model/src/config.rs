use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The duration to wait between bot ticks.
    #[serde(default = "default_tick_duration")]
    pub tick_duration: f32,
    /// Ticks to elapse before the bot checks if the configured Twitch stream is live.
    #[serde(default = "default_check_live_ticks")]
    pub check_live_ticks: u64,
    /// Role name to emoji.
    #[serde(default)]
    pub reaction_roles: HashMap<String, String>,
    /// The max width to use for any message if the message has a configurable width.
    #[serde(default = "default_max_message_width")]
    pub max_message_width: u16,
    /// The Discord role ID to use when silencing a user. A value of 0 means there is no role.
    #[serde(default)]
    pub timeout_role_id: u64,
    /// The Discord channel ID to use when sending stream notifications.
    #[serde(default)]
    pub stream_notification_channel: u64,
    /// The minimum duration between stream notifications in seconds.
    #[serde(default = "default_min_stream_notification_secs")]
    pub min_stream_notification_secs: u64,
    /// The Discord channel ID to use when sending debug messages.
    #[serde(default)]
    pub debug_channel: u64,
}

impl Config {
    pub fn new() -> Self {
        Self {
            tick_duration: default_tick_duration(),
            check_live_ticks: default_check_live_ticks(),
            reaction_roles: HashMap::new(),
            max_message_width: default_max_message_width(),
            timeout_role_id: u64::default(),
            stream_notification_channel: u64::default(),
            min_stream_notification_secs: default_min_stream_notification_secs(),
            debug_channel: u64::default(),
        }
    }
}

pub fn default_tick_duration() -> f32 {
    0.5
}

pub fn default_check_live_ticks() -> u64 {
    240
}

fn default_max_message_width() -> u16 {
    36
}

fn default_min_stream_notification_secs() -> u64 {
    21600
}
