use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use log::debug;
use serenity::model::user::User;

/// The default spam time to use if there is no configured spam time.
const DEFAULT_SPAM_TIME: u64 = 1;

const MAX_STRIKES: u8 = 3;

#[derive(Default)]
struct History {
    last_timestamp: Instant,
    strikes: u8,
}

impl History {
    /// Wrapper around `Instant::elapsed`.
    fn elapsed(&self) -> &Duration {
        &self.last_timestamp.elapsed()
    }

    fn update_timestamp(&mut self) {
        self.last_timestamp = Instant::now();
    }

    fn add_strike(&mut self) {
        self.strikes += 1;
    }

    fn reset_strikes(&mut self) {
        self.strikes = 0;
    }
}

/// Checks if a message from a user is spam based on time since the last message.
#[derive(Default)]
pub struct Antispam {
    min_non_spam_time: Duration,
    chatter_history: HashMap<u64, History>,
}

impl Antispam {
    /// Create a new instance of `Antispam`.
    pub fn new() -> Self {
        Self {
            min_non_spam_time: Duration::from_secs(DEFAULT_SPAM_TIME),
            chatter_history: HashMap::new(),
        }
    }

    /// Check if the given `user` is spamming.
    pub fn is_spam(&mut self, user_id: &u64) -> bool {
        match self.chatter_history.get_mut(user_id) {
            Some(mut history) => {
                let history = *history;

                let r = if history.elapsed() < &self.min_non_spam_time {
                    history.add_strike();
                    true
                } else {
                    history.reset_strikes();
                    false
                };

                history.update_timestamp();

                r
            }
            None => {
                let _ = self.chatter_history.insert(*user_id, History::default());
                false
            }
        }
    }

    pub fn should_timeout(&self, user_id: &u64) -> bool {
        match self.chatter_history.get(user_id) {
            Some(h) => h.strikes > MAX_STRIKES,
            None => false,
        }
    }

    /// Clears the spam history. Needed so the history does not grow infinitely large.
    pub fn reset(&mut self) {
        self.chatter_history.clear();
    }
}
