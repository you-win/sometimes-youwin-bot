use std::time::Duration;

use log::{debug, error, info};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::Sender;
use twitch_api::{
    twitch_oauth2::{
        tokens::errors::ValidationError, AccessToken, ClientId, ClientSecret, RefreshToken,
        UserToken,
    },
    TwitchClient,
};
use twitchchat::connector;
use twitchchat::messages::Privmsg;
use twitchchat::AsyncRunner;
use twitchchat::UserConfig;

#[derive(Debug, Clone)]
pub enum BotMessage {
    Debug(String),

    Ready,

    Shutdown,
}

pub async fn create_user_token() -> Result<UserToken, Box<dyn std::error::Error>> {
    let mut refresh_token: RefreshToken = RefreshToken::new(crate::TWITCH_REFRESH_TOKEN.into());
    let client_id: ClientId = ClientId::new(crate::TWITCH_CLIENT_ID.into());
    let client_secret: ClientSecret = ClientSecret::new(crate::TWITCH_CLIENT_SECRET.into());

    let client: TwitchClient<reqwest::Client> = TwitchClient::default();

    // TODO this part is weird
    let (access_token, duration, o_refresh_token) = refresh_token
        .refresh_token(&client, &client_id, &client_secret)
        .await?;
    if o_refresh_token.is_some() {
        info!("Received new refresh token!");
        refresh_token = o_refresh_token.unwrap();
    }

    let user_token = UserToken::from_existing(
        &client,
        access_token.clone(),
        refresh_token.clone(),
        client_secret.clone(),
    )
    .await?;

    Ok(user_token)
}

pub async fn run_bot(
    central_receiver: Receiver<crate::CentralMessage>,
    twitch_sender: Sender<BotMessage>,
    user_token: UserToken,
) -> Result<(), Box<dyn std::error::Error>> {
    let irc_user_config = UserConfig::builder()
        .token(user_token.access_token.secret())
        .name("SometimesYouwinBot")
        .enable_all_capabilities()
        .build()
        .unwrap();

    let connector = connector::tokio::Connector::twitch()?;
    let mut irc_client = AsyncRunner::connect(connector, &irc_user_config).await?;

    let mut writer = irc_client.writer();
    let quit_handle = irc_client.quit_handle();

    tokio::spawn(async move {
        // TODO pull this value from the config.
        let mut interval = tokio::time::interval(Duration::from_secs_f32(0.5));

        loop {
            interval.tick().await;

            match irc_client.next_message().await {
                Ok(s) => {
                    match s {
                        twitchchat::Status::Message(m) => {
                            match m {
                                // twitchchat::messages::Commands::Raw(_) => todo!(),
                                twitchchat::messages::Commands::IrcReady(_) => todo!(),
                                twitchchat::messages::Commands::Ready(_) => todo!(),
                                // twitchchat::messages::Commands::Cap(_) => todo!(),
                                // twitchchat::messages::Commands::ClearChat(_) => todo!(),
                                // twitchchat::messages::Commands::ClearMsg(_) => todo!(),
                                // twitchchat::messages::Commands::GlobalUserState(_) => todo!(),
                                // twitchchat::messages::Commands::HostTarget(_) => todo!(),
                                // twitchchat::messages::Commands::Join(_) => todo!(),
                                twitchchat::messages::Commands::Notice(m) => info!(
                                    "{:?} - {:?}",
                                    m.msg_id()
                                        .unwrap_or(twitchchat::messages::MessageId::Unknown(
                                            "Unknown"
                                        )),
                                    m.message()
                                ),
                                // twitchchat::messages::Commands::Part(_) => todo!(),
                                // twitchchat::messages::Commands::Ping(_) => todo!(),
                                // twitchchat::messages::Commands::Pong(_) => todo!(),
                                twitchchat::messages::Commands::Privmsg(m) => {
                                    if let Err(e) = handle_privmsg(&m) {
                                        error!("{}", e);
                                        break;
                                    }
                                }
                                twitchchat::messages::Commands::Reconnect(_) => todo!(),
                                // twitchchat::messages::Commands::RoomState(_) => todo!(),
                                // twitchchat::messages::Commands::UserNotice(_) => todo!(),
                                // twitchchat::messages::Commands::UserState(_) => todo!(),
                                // twitchchat::messages::Commands::Whisper(_) => todo!(),
                                _ => {}
                            }
                        }
                        _ => break,
                    }
                }
                Err(e) => {
                    log::error!("{}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}

fn handle_privmsg(msg: &Privmsg) -> Result<(), Box<dyn std::error::Error>> {
    debug!("{}", msg.data());

    Ok(())
}

#[test]
fn test_create_access_token() {
    let dummy_token = "asdf";
    let access_token = AccessToken::new(dummy_token.to_string());

    assert_ne!(access_token.to_string(), dummy_token);
}
