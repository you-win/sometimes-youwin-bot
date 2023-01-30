use std::io::Write;
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
use twitchchat::writer::AsyncWriter;
use twitchchat::writer::MpscWriter;
use twitchchat::AsyncRunner;
use twitchchat::UserConfig;

use crate::commands;

#[derive(Debug, Clone)]
pub enum BotMessage {
    Debug(String),

    Ready,
    ChannelLive,
    Shutdown,
}

pub async fn run_bot(
    central_receiver: Receiver<crate::CentralMessage>,
    twitch_sender: Sender<BotMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let irc_user_config = match UserConfig::builder()
        .token(format!("oauth:{}", user_token.access_token.secret()))
        .name("sometimes_youwin")
        .enable_all_capabilities()
        .build()
    {
        Ok(v) => v,
        Err(e) => {
            panic!("{e}")
        }
    };

    let mut bot_config = crate::Config::new();

    let connector = connector::tokio::Connector::twitch()?;
    let mut irc_client = AsyncRunner::connect(connector, &irc_user_config).await?;

    irc_client.join(crate::TWITCH_CHANNEL_NAME).await?;

    let mut writer = irc_client.writer();
    let quit_handle = irc_client.quit_handle();

    let mut receiver = central_receiver.resubscribe();
    let sender = twitch_sender.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(*crate::TICK_DURATION);

        // TODO this sucks
        let mut ticks: u16 = 0;
        const CHECK_LIVE_TICKS: u16 = 120;

        loop {
            interval.tick().await;

            ticks += 1;
            if ticks >= CHECK_LIVE_TICKS {
                ticks = 0;

                let req_param = &[twitch_api::types::UserNameRef::from_str(
                    crate::TWITCH_CHANNEL_NAME,
                )];
                if let Ok(response) = client
                    .helix
                    .req_get(
                        twitch_api::helix::streams::GetStreamsRequest::user_logins(&req_param[..]),
                        &user_token,
                    )
                    .await
                {
                    if !response.data.is_empty() {
                        if let Err(e) = sender.send(BotMessage::ChannelLive) {
                            error!("{:?}", e);
                        }
                    }
                }
            }

            match irc_client.next_message().await {
                Ok(s) => match s {
                    twitchchat::Status::Message(m) => {
                        debug!("{:?}", &m);
                        match m {
                            twitchchat::messages::Commands::IrcReady(_) => {
                                debug!("irc ready received!");
                                if let Err(e) = sender.send(BotMessage::Ready) {
                                    error!("{}", e);
                                }
                            }
                            twitchchat::messages::Commands::Ready(_) => {
                                debug!("regular ready");

                                writer
                                    .encode(twitchchat::commands::privmsg(
                                        crate::TWITCH_CHANNEL_NAME,
                                        "Bot ready!",
                                    ))
                                    .await
                                    .unwrap();
                            }
                            twitchchat::messages::Commands::Notice(m) => info!(
                                "{:?} - {:?}",
                                m.msg_id()
                                    .unwrap_or(twitchchat::messages::MessageId::Unknown("Unknown")),
                                m.message()
                            ),
                            twitchchat::messages::Commands::Privmsg(m) => {
                                if let Err(e) = handle_privmsg(&mut writer, &m).await {
                                    error!("{}", e);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => break,
                },
                Err(e) => {
                    log::error!("{}", e);
                    break;
                }
            }

            match receiver.try_recv() {
                Ok(m) => match m {
                    crate::CentralMessage::ConfigUpdated(c) => {
                        bot_config = c.clone();
                    }
                    crate::CentralMessage::Discord(_) => {
                        // TODO stub
                    }
                    crate::CentralMessage::Shutdown => {
                        info!("Shutdown received!");
                        break;
                    }
                    _ => {}
                },
                Err(e) => match e {
                    tokio::sync::broadcast::error::TryRecvError::Closed => {
                        error!("Channel closed");
                        break;
                    }
                    tokio::sync::broadcast::error::TryRecvError::Lagged(n) => {
                        debug!("Channel lagged by {} messages", n);
                    }
                    _ => {}
                },
            }
        }

        quit_handle.notify().await;
    });

    Ok(())
}

async fn handle_privmsg(
    writer: &mut AsyncWriter<MpscWriter>,
    msg: &Privmsg<'_>,
) -> anyhow::Result<()> {
    let id = msg.tags().get("id").unwrap_or_default();
    if id.is_empty() {
        debug!("No id found for message {}", msg.data());
        return Ok(());
    }

    let text = msg.data();

    if !text.starts_with(crate::BOT_PREFIX) {
        return Ok(());
    }

    let cmd = text.strip_prefix(crate::BOT_PREFIX).unwrap_or_default();

    let (cmd, args) = cmd.split_once(" ").unwrap_or((cmd, ""));

    let output: String = match cmd {
        "ping" => commands::ping(),
        "whoami" => commands::whoami(&msg.name().into()),
        "high-five" => commands::high_five(),
        "ferris-say" | "ferrissay" | "cowsay" => {
            format!("{} is not supported in Twitch chat", cmd).into()
            // commands::ferris_say(&args.into()).await.unwrap()
        }
        "roll" => {
            let num: u64 = match args.parse() {
                Ok(v) => v,
                Err(_) => 6,
            };

            commands::roll(&num.into()).to_string()
        }
        // "config" => commands::config().await.replace("\n", " "),
        "help" => {
            format!("?ping, ?whoami, ?high-five, ?roll <number>")
        }
        _ => return Ok(()),
    };

    writer
        .encode(twitchchat::commands::reply(
            crate::TWITCH_CHANNEL_NAME,
            id,
            &output,
        ))
        .await?;

    Ok(())
}

#[test]
fn test_create_access_token() {
    let dummy_token = "asdf";
    let access_token = AccessToken::new(dummy_token.to_string());

    assert_ne!(access_token.to_string(), dummy_token);
}
