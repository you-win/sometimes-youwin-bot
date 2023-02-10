use std::fmt::Display;

use clap::{Args, CommandFactory, Parser, Subcommand};

use super::commands;

#[derive(Debug, Parser)]
#[command(name = "sometimes-youwin-bot")]
#[command(about = "A multibot made by youwin.")]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    /// Ping the bot.
    Ping,
    /// Receive a username and reply appropriately.
    Whoami,
    /// Reply with a clap emoji.
    HighFive,
    /// Have Ferris say something.
    #[command(aliases = ["cowsay", "ferrissay"])]
    FerrisSay {
        /// The text to say.
        #[arg(num_args = 0..)]
        text: String,
    },
    /// Generate a random number from 1 - input.
    Roll {
        /// The max number that can be rolled.
        sides: u64,
    },
    Admin(Admin),
}

impl Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Commands::Ping => write!(f, "ping"),
            Commands::Whoami => write!(f, "whoami"),
            Commands::HighFive => write!(f, "high-five"),
            Commands::FerrisSay { text } => write!(f, "ferris-say {}", text),
            Commands::Roll { sides } => write!(f, "roll {}", sides),
            Commands::Admin(admin) => write!(f, "admin {}", &admin.command),
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct Admin {
    #[command(subcommand)]
    command: AdminCommands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AdminCommands {
    ReloadConfig,
}

impl Display for AdminCommands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReloadConfig => write!(f, "reload-config"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CommandOutput {
    Error(String),
    Command {
        value: Option<String>,
        command: Commands,
    },
    AdminCommand {
        value: Option<String>,
        command: AdminCommands,
    },
}

impl CommandOutput {
    pub fn get_value(&self) -> Option<String> {
        match self {
            Self::Command { value, .. } => value.clone(),
            Self::AdminCommand { value, .. } => value.clone(),
            Self::Error(s) => Some(s.clone()),
        }
    }

    pub fn get_command_name(&self) -> String {
        match self {
            Self::Command { command, .. } => command.to_string(),
            Self::AdminCommand { command, .. } => command.to_string(),
            Self::Error(s) => s.to_string(),
        }
    }
}

impl From<(Commands, Option<String>)> for CommandOutput {
    fn from(value: (Commands, Option<String>)) -> Self {
        Self::Command {
            value: value.1,
            command: value.0,
        }
    }
}

impl From<(AdminCommands, Option<String>)> for CommandOutput {
    fn from(value: (AdminCommands, Option<String>)) -> Self {
        Self::AdminCommand {
            value: value.1,
            command: value.0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AdditionalInfo {
    None,
    Discord {
        name: String,
        user_id: u64,
        channel_id: u64,
    },
    Twitch {
        name: String,
        is_vip: bool,
    },
}

pub fn parse(text: impl ToString, info: AdditionalInfo) -> CommandOutput {
    let args = match Cli::try_parse_from(text.to_string().split(' ')) {
        Ok(args) => args,
        Err(e) => return CommandOutput::Error(e.render().to_string()),
    };

    let output = match args.command {
        Commands::Ping => Some(commands::ping()),
        Commands::Whoami => {
            let name = match info {
                AdditionalInfo::Discord { name, .. } => Some(name),
                AdditionalInfo::Twitch { name, .. } => Some(name),
                _ => None,
            };

            match name {
                Some(name) => Some(commands::whoami(&name)),
                None => Some(show_help()),
            }
        }
        Commands::HighFive => Some(commands::high_five()),
        Commands::FerrisSay { ref text } => Some(commands::ferris_say(text)),
        Commands::Roll { sides } => Some(commands::roll(sides).to_string()),
        Commands::Admin(ref admin) => match admin.command {
            AdminCommands::ReloadConfig => None,
            _ => Some(show_help()),
        },
        _ => Some(show_help()),
    };

    CommandOutput::from((args.command, output))
}

fn show_help() -> String {
    Cli::command().render_long_help().to_string()
}
