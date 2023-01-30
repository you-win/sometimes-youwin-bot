use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Role name to emoji.
    #[serde(default)]
    pub reaction_roles: HashMap<String, String>,
    /// The max width to use for any message, if the message has a configurable width.
    #[serde(default)]
    pub max_message_width: u16,
    /// The Discord role ID to use when silencing a user. A value of 0 means there is no role.
    #[serde(default)]
    pub timeout_role_id: u64,
    #[serde(default)]
    pub stream_notification_channel: u64,
    #[serde(default)]
    pub min_stream_notification_secs: u64,
    #[serde(default)]
    pub debug_channel: u64,
}

impl Config {
    pub fn new() -> Self {
        Self {
            reaction_roles: HashMap::new(),
            max_message_width: 36,
            timeout_role_id: u64::default(),
            stream_notification_channel: u64::default(),
            min_stream_notification_secs: 21600,
            debug_channel: u64::default(),
        }
    }

    pub fn from(&mut self, other: &Config) {
        other.reaction_roles.clone_into(&mut self.reaction_roles);
        self.max_message_width = other.max_message_width;
        self.timeout_role_id = other.timeout_role_id;
        self.stream_notification_channel = other.stream_notification_channel;
        self.min_stream_notification_secs = other.min_stream_notification_secs;
        self.debug_channel = other.debug_channel;
    }
}
