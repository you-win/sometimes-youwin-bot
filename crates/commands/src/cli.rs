use std::fmt::Display;

use clap::{error::ErrorKind, Args, CommandFactory, Parser, Subcommand};
use model::config::Config;
use strum::{EnumIter, IntoEnumIterator};

use super::commands;

#[derive(Debug, Parser)]
#[command(name = "bot?")]
#[command(about = "A multibot made by youwin.")]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Subcommand, EnumIter)]
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
        #[arg(num_args = 1..)]
        text: Vec<String>,
    },
    /// Generate a random number from 1 - input.
    Roll {
        /// The max number that can be rolled.
        sides: String,
    },
    /// An ad hoc command that only returns a String value.
    #[command(aliases = ["adhoc"])]
    AdHoc {
        /// The ad-hoc command to run.
        text: String,
    },
    /// Run a Rhai script in safe mode.
    Rhai {
        /// The script. Must be properly formatted using rhai <script> and triple backticks.
        script: Vec<String>,
    },
    Admin(Admin),
}

impl Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ping => write!(f, "ping"),
            Self::Whoami => write!(f, "whoami"),
            Self::HighFive => write!(f, "high-five"),
            Self::FerrisSay { text } => write!(f, "ferris-say {}", text.join(" ")),
            Self::Roll { sides } => write!(f, "roll {}", sides),
            Self::AdHoc { text } => write!(f, "ad-hoc {}", text),
            Self::Rhai { script } => write!(f, "rhai {}", script.join(" ")),
            Self::Admin(admin) => write!(f, "admin {}", &admin.command),
        }
    }
}

impl Commands {
    pub fn commands() -> Vec<String> {
        Commands::iter()
            .map(|x| format!("{:?}", x))
            .collect::<Vec<String>>()
    }
}

#[derive(Debug, Clone, Args)]
pub struct Admin {
    #[command(subcommand)]
    command: AdminCommands,
}

impl Default for Admin {
    fn default() -> Self {
        Self {
            command: AdminCommands::Test,
        }
    }
}

#[derive(Debug, Clone, Subcommand, EnumIter)]
pub enum AdminCommands {
    Test,
    ReloadConfig,
}

impl Display for AdminCommands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Test => write!(f, "test"),
            Self::ReloadConfig => write!(f, "reload-config"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CommandOutput {
    Error {
        message: String,
        is_help: bool,
    },
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
            Self::Error { message, .. } => Some(message.clone()),
        }
    }

    pub fn get_command_name(&self) -> String {
        match self {
            Self::Command { command, .. } => command.to_string(),
            Self::AdminCommand { command, .. } => command.to_string(),
            Self::Error { message, .. } => message.to_string(),
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

pub fn parse(input: impl Display, info: AdditionalInfo, config: &Config) -> CommandOutput {
    let args = match Cli::try_parse_from(format!("{input}",).split(' ')) {
        Ok(args) => args,
        Err(e) => {
            let ad_hoc_val = config.ad_hoc_command(
                &input
                    .to_string()
                    .split_once(" ")
                    .unwrap_or_default()
                    .1
                    .to_string(),
            );

            if ad_hoc_val.is_some() {
                return CommandOutput::Command {
                    value: ad_hoc_val,
                    command: Commands::AdHoc {
                        text: input.to_string(),
                    },
                };
            }

            let mut is_help = false;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    is_help = true;
                }
                _ => {}
            }
            return CommandOutput::Error {
                message: e.render().to_string(),
                is_help,
            };
        }
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
        Commands::FerrisSay { ref text } => {
            if let AdditionalInfo::Discord { .. } = info {
                Some(format!("```{}```", commands::ferris_say(&text.join(" "))))
            } else {
                Some("This does not work in Twitch chat!".to_string())
            }
        }
        Commands::Roll { ref sides } => {
            Some(commands::roll(sides.parse().unwrap_or(6)).to_string())
        }
        Commands::AdHoc { ref text } => {
            let ad_hoc_val = config.ad_hoc_command(text);

            if ad_hoc_val.is_some() {
                ad_hoc_val
            } else {
                Some(show_help())
            }
        }
        Commands::Rhai { ref script } => {
            if matches!(info, AdditionalInfo::Twitch { .. }) {
                return CommandOutput::from((
                    args.command,
                    Some("This does not work in Twitch chat!".to_string()),
                ));
            }

            let script = script.join(" ");
            if (!script.starts_with("```rhai") && !script.starts_with("```rust"))
                || !script.ends_with("```")
            {
                return CommandOutput::from((
                    args.command,
                    Some("Improperly formatted script, declining to run.".to_string()),
                ));
            }

            match scripting::execute(
                script
                    .replace("```rhai", "")
                    .replace("```rust", "")
                    .strip_suffix("```")
                    .unwrap_or_default(),
            ) {
                Ok(v) => Some(format!("```{v}```")),
                Err(e) => Some(e.to_string()),
            }
        }
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
