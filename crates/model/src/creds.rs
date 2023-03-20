pub trait BotCreds {
    fn bot_prefix(&self) -> &str {
        "bot?"
    }
}

impl BotCreds for TwitchCreds {}
impl BotCreds for DiscordCreds {}

#[derive(Clone)]
pub struct TwitchCreds {
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,

    pub bot_name: String,
    pub channel_name: String,
}

impl TwitchCreds {
    pub fn new(
        refresh_token: &str,
        client_id: &str,
        client_secret: &str,
        bot_name: &str,
        channel_name: &str,
    ) -> Self {
        Self {
            refresh_token: refresh_token.to_string(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            bot_name: bot_name.to_string(),
            channel_name: channel_name.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct DiscordCreds {
    pub token: String,

    pub bot_id: u64,
    pub admin_id: u64,
    pub guild_id: u64,

    pub data_channel: u64,
}

impl DiscordCreds {
    pub fn new(
        token: &str,
        bot_id: &str,
        admin_id: &str,
        guild_id: &str,
        data_channel: &str,
    ) -> anyhow::Result<Self> {
        let bot_id = bot_id.parse()?;
        let admin_id = admin_id.parse()?;
        let guild_id = guild_id.parse()?;
        let data_channel = data_channel.parse()?;

        Ok(Self {
            token: token.to_string(),
            bot_id,
            admin_id,
            guild_id,
            data_channel,
        })
    }
}
