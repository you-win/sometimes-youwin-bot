use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use twitch_api::twitch_oauth2;
use twitch_api::TwitchClient;
use twitch_oauth2::{
    tokens::errors::ValidationError, AccessToken, ClientId, ClientSecret, RefreshToken, UserToken,
};

pub struct TwitchBot {
    token: UserToken,
    next_refresh_time: Duration,
}

impl TwitchBot {
    pub fn new() -> Result<Self> {
        // let token = UserToken::from_existing(&client, std::env::var(key), refresh_token, client_secret)

        todo!()
    }
}

#[derive(Debug, Clone)]
pub enum BotMessage {
    Debug(String),

    Ready,

    Shutdown,
}

pub async fn create_bot(
    receiver: Receiver<crate::CentralMessage>,
) -> Result<
    (TwitchClient<'static, reqwest::Client>, Receiver<BotMessage>),
    Box<dyn std::error::Error>,
> {
    let mut refresh_token: RefreshToken = RefreshToken::new(crate::TWITCH_REFRESH_TOKEN.into());
    let client_id: ClientId = ClientId::new(crate::TWITCH_CLIENT_ID.into());
    let client_secret: ClientSecret = ClientSecret::new(crate::TWITCH_CLIENT_SECRET.into());

    let client: TwitchClient<reqwest::Client> = TwitchClient::default();

    let (access_token, duration, o_refresh_token) = refresh_token
        .refresh_token(&client, &client_id, &client_secret)
        .await?;
    if o_refresh_token.is_some() {
        refresh_token = o_refresh_token.unwrap();
    }

    let user_token = UserToken::from_existing(
        &client,
        access_token.clone(),
        refresh_token.clone(),
        client_secret.clone(),
    )
    .await?;

    // Ok(())
    todo!()
}

#[tokio::main]
pub async fn run_bot(
    client: TwitchClient<reqwest::Client>,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}

#[test]
fn test_create_access_token() {
    let dummy_token = "asdf";
    let access_token = AccessToken::new(dummy_token.to_string());

    assert_ne!(access_token.to_string(), dummy_token);
}
