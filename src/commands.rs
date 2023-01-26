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

#[derive(Debug, thiserror::Error)]
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
pub enum CommandInput {
    UInt(u64),
    String(String),
    Vec(Vec<Value>),
    Map(HashMap<String, Value>),
    Unknown,
}

impl From<&[CommandDataOption]> for CommandInput {
    fn from(options: &[CommandDataOption]) -> Self {
        Self::Vec(
            options
                .into_iter()
                .map(|v| Value::from(v))
                .collect::<Vec<Value>>(),
        )
    }
}

impl From<String> for CommandInput {
    fn from(s: String) -> Self {
        Self::String(s.clone())
    }
}

impl From<&str> for CommandInput {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<u32> for CommandInput {
    fn from(n: u32) -> Self {
        Self::UInt(n.into())
    }
}

impl From<u64> for CommandInput {
    fn from(n: u64) -> Self {
        Self::UInt(n.into())
    }
}

/// Ping pong.
pub fn ping() -> String {
    "pong".into()
}

/// Receive a username and reply appropriately.
pub fn whoami(command: &CommandInput) -> String {
    match command {
        CommandInput::String(s) => format!("You are {}!", s),
        _ => unreachable!(),
    }
}

/// Reply with a clap emoji.
pub fn high_five() -> String {
    "👏".into()
}

/// Similar to how `cowsay` works, take a message and make it fancy.
pub async fn ferris_say(command: &CommandInput) -> Result<String> {
    let message = match command {
        CommandInput::String(s) => s,
        _ => unreachable!(),
    };
    // let max_width = crate::CONFIG.read().await.max_message_width;
    let max_width: usize = 36;

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
pub fn roll(command: &CommandInput) -> u32 {
    let mut rng = rand::thread_rng();

    let mut sides = match command {
        CommandInput::UInt(u) => *u,
        _ => unreachable!(),
    };
    if sides < 1 {
        sides += 1;
    }

    rng.gen_range(1..=(sides as u32))
}

/// Returns public fields from the config.
// pub async fn config() -> String {
//     // let config = crate::CONFIG.read().await;

//     // format!(
//     //     "max_message_width: {:?}\nreaction_roles: {:?}",
//     //     config.max_message_width, config.reaction_roles
//     // )

//     format!("max_message_width: {:?}\nreaction_roles: {:?}", 36, "eh")
// }

pub fn reload_config() {
    //
}
