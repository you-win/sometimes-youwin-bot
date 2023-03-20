use std::{collections::HashMap, time::Duration};

use tokio::time::Instant;

const DEFAULT_SPAM_TIME: f32 = 0.75;
const MAX_STRIKES: u8 = 3;
const SILENT_DELETE_AMOUNT: u8 = 4;

struct History {
    last_timestamp: Instant,
    strikes: u8,

    silent_delete: bool,
}

impl History {
    /// Create a new `History`.
    fn new() -> Self {
        Self {
            last_timestamp: Instant::now(),
            strikes: 0,
            silent_delete: false,
        }
    }

    /// Wrapper around `Instant::elapsed`.
    fn elapsed(&self) -> Duration {
        self.last_timestamp.elapsed()
    }

    fn update_timestamp(&mut self) {
        self.last_timestamp = Instant::now();
    }

    fn add_strike(&mut self) {
        self.strikes += 1;
        if self.strikes > SILENT_DELETE_AMOUNT {
            self.silent_delete = true;
        }
    }

    fn reset_strikes(&mut self) {
        self.strikes = 0;
        self.silent_delete = false;
    }
}

pub struct Antispam {
    min_non_spam_time: Duration,
    chatter_history: HashMap<u64, History>,
}

impl Antispam {
    /// Create a new instance of `Antispam`.
    pub fn new() -> Self {
        Self {
            min_non_spam_time: Duration::from_secs_f32(DEFAULT_SPAM_TIME),
            chatter_history: HashMap::new(),
        }
    }

    /// Check if the given `user` is spamming.
    pub fn is_spam(&mut self, user_id: &u64) -> bool {
        match self.chatter_history.get_mut(&user_id) {
            Some(history) => {
                let r = if history.elapsed() < self.min_non_spam_time {
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
                let _ = self.chatter_history.insert(*user_id, History::new());
                false
            }
        }
    }

    pub fn too_many_strikes(&self, user_id: &u64) -> bool {
        match self.chatter_history.get(user_id) {
            Some(h) => h.strikes > MAX_STRIKES,
            None => false,
        }
    }

    pub fn should_silent_delete(&self, user_id: &u64) -> bool {
        if let Some(h) = self.chatter_history.get(user_id) {
            h.silent_delete
        } else {
            false
        }
    }

    /// Clears the spam history. Needed so the history does not grow infinitely large.
    pub fn reset(&mut self) {
        self.chatter_history.clear();
    }
}
