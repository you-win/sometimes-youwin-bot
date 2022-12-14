use std::{collections::HashMap, fmt::Display, io::BufWriter, string::FromUtf8Error};

use rand::Rng;
use serenity::model::{
    prelude::{
        command,
        interaction::application_command::{CommandDataOption, CommandDataOptionValue},
        Attachment, PartialChannel, Role,
    },
    user::User,
};

use crate::{config::Config, utils};

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

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Integer(i64),
    Boolean(bool),
    Number(f64),

    DiscordObject(DiscordObject),
    TwitchObject(TwitchObject),

    Unknown,
}

impl From<&CommandDataOption> for Value {
    fn from(option: &CommandDataOption) -> Self {
        match option.clone().resolved {
            Some(v) => match v {
                CommandDataOptionValue::String(v) => Self::String(v),
                CommandDataOptionValue::Integer(v) => Self::Integer(v),
                CommandDataOptionValue::Boolean(v) => Self::Boolean(v),
                CommandDataOptionValue::User(v, _) => Self::DiscordObject(DiscordObject::User(v)),
                CommandDataOptionValue::Channel(v) => {
                    Self::DiscordObject(DiscordObject::Channel(v))
                }
                CommandDataOptionValue::Role(v) => Self::DiscordObject(DiscordObject::Role(v)),
                CommandDataOptionValue::Number(v) => Self::Number(v),
                CommandDataOptionValue::Attachment(v) => {
                    Self::DiscordObject(DiscordObject::Attachment(v))
                }
                _ => Self::Unknown,
            },
            None => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DiscordObject {
    User(User),
    Channel(PartialChannel),
    Role(Role),
    Attachment(Attachment),
}

#[derive(Debug, Clone)]
pub enum TwitchObject {}

#[derive(Debug, Clone)]
pub enum CommandInput<T, S> {
    UInt(u32),
    String(String),
    Tuple(T, S),
    Vec(Vec<Value>),
    Map(HashMap<String, Value>),
    Unknown,
}

impl<T, S> From<&[CommandDataOption]> for CommandInput<T, S> {
    fn from(options: &[CommandDataOption]) -> Self {
        Self::Vec(
            options
                .into_iter()
                .map(|v| Value::from(v))
                .collect::<Vec<Value>>(),
        )
    }
}

impl<T, S> From<&String> for CommandInput<T, S> {
    fn from(s: &String) -> Self {
        Self::String(s.clone())
    }
}

impl<T, S> From<&u32> for CommandInput<T, S> {
    fn from(n: &u32) -> Self {
        Self::UInt(n.clone())
    }
}

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
    let buffer = vec![];
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

/// Returns public fields from the config.
pub fn config() -> String {
    let config = crate::CONFIG.lock().unwrap();

    format!("reaction_roles: {:?}", config.reaction_roles)
}
