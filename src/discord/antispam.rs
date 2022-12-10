use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serenity::model::user::User;

/// The default spam time to use if there is no configured spam time.
const DEFAULT_SPAM_TIME: u64 = 1;

/// Checks if a message from a user is spam based on time since the last message.
#[derive(Default)]
pub struct Antispam {
    min_non_spam_time: Duration,
    chatter_history: HashMap<User, Instant>,
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
    pub fn is_spam(&mut self, user: &User) -> bool {
        match self.chatter_history.get_mut(user) {
            Some(mut time) => {
                if time.elapsed() < self.min_non_spam_time {
                    return true;
                }

                time = &mut Instant::now();

                false
            }
            None => {
                println!("None!");
                let _ = self.chatter_history.insert(user.clone(), Instant::now());
                false
            }
        }
    }

    /// Clears the spam history. Needed so the history does not grow infinitely large.
    pub fn reset(&mut self) {
        self.chatter_history.clear();
    }
}
