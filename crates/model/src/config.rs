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
    #[serde(default)]
    pub stream_notification_format: String,
    /// The Discord channel ID to use when sending debug messages.
    #[serde(default)]
    pub debug_channel: u64,
    #[serde(default)]
    pub roles_channel: u64,
    #[serde(default)]
    pub ad_hoc: HashMap<String, String>,
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
            stream_notification_format: Default::default(),
            debug_channel: u64::default(),
            roles_channel: u64::default(),
            ad_hoc: HashMap::new(),
        }
    }

    pub fn ad_hoc_command(&self, command: &String) -> Option<String> {
        self.ad_hoc.get(command).map(|x| x.to_string())
    }

    pub fn ad_hoc_commands(&self) -> Vec<String> {
        self.ad_hoc.iter().map(|(k, _)| k.to_string()).collect()
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
