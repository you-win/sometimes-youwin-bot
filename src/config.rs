use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub reaction_roles: HashMap<String, String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            reaction_roles: HashMap::new(),
        }
    }

    pub fn from(&mut self, other: &Config) {
        other.reaction_roles.clone_into(&mut self.reaction_roles);
    }
}
