use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Role name to emoji
    pub reaction_roles: HashMap<String, String>,
    pub max_message_width: u16,
}

impl Config {
    pub fn new() -> Self {
        Self {
            reaction_roles: HashMap::new(),
            max_message_width: 36,
        }
    }

    pub fn from(&mut self, other: &Config) {
        other.reaction_roles.clone_into(&mut self.reaction_roles);
    }
}
