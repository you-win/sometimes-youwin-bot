use std::{fmt::Display, io::BufWriter, string::FromUtf8Error};

use rand::Rng;

use crate::utils;

pub enum CommandError {
    InvalidInput,
    StringConversion(String),
    Other(String),
}

impl Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::InvalidInput => write!(f, "{}", "InvalidInput"),
            CommandError::StringConversion(t) | CommandError::Other(t) => write!(f, "{}", t),
        }
    }
}

impl From<FromUtf8Error> for CommandError {
    fn from(e: FromUtf8Error) -> Self {
        Self::StringConversion(e.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        Self::StringConversion(e.to_string())
    }
}

type Result<T> = std::result::Result<T, CommandError>;

/// Ping pong.
pub fn ping() -> String {
    "pong".into()
}

/// Receive a username and reply appropriately.
pub fn whoami(username: &String) -> String {
    format!("You are {}!", username)
}

/// Reply with a clap emoji.
pub fn high_five() -> String {
    "👏".into()
}

/// Similar to how `cowsay` works, take a message and make it fancy.
pub fn ferris_say(message: &String, max_width: usize) -> Result<String> {
    let mut buffer = vec![];
    let mut writer = BufWriter::new(buffer);

    ferris_says::say(message.as_bytes(), max_width, &mut writer)?;

    match String::from_utf8(writer.buffer().to_vec()) {
        Ok(v) => Ok(v),
        Err(e) => Err(CommandError::from(e)),
    }
}

/// Roll a dice with the given number of sides. The number of sides must always
/// be equal to or greater than 1.
pub fn roll(sides: u32) -> u32 {
    let mut rng = rand::thread_rng();

    let mut sides = sides;
    if sides < 1 {
        sides += 1;
    }

    rng.gen_range(1..=sides)
}
