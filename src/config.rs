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
    #[serde(default = "job_tick_duration_default")]
    pub job_tick_duration: f32,
}

impl Config {
    pub fn new() -> Self {
        Self {
            reaction_roles: HashMap::new(),
            max_message_width: 36,
            timeout_role_id: u64::default(),
            job_tick_duration: job_tick_duration_default(),
        }
    }

    pub fn from(&mut self, other: &Config) {
        other.reaction_roles.clone_into(&mut self.reaction_roles);
    }
}

fn job_tick_duration_default() -> f32 {
    0.5
}
