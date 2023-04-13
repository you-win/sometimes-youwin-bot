use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{
    body::Body,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, Request},
    routing::post,
    Router,
};
use log::error;
use model::{
    config::Config,
    messages::{CentralMessage, ServerMessage},
};
use tokio::sync::{
    broadcast::{Receiver, Sender},
    RwLock,
};

const KEY_HEADER: &str = "A-Cool-Key";
const API_KEY: &str = env!("SYWB_SERVER_API_KEY");

enum Bail {
    No,
    YesWithResponse,
    YesIgnore,
}

#[derive(Debug)]
struct AppState {
    config: Arc<RwLock<Config>>,

    receiver: Receiver<CentralMessage>,
    sender: Sender<ServerMessage>,

    confused_actors: Vec<SocketAddr>,
    bad_actors: Vec<SocketAddr>,
    repeat_offenders: Vec<SocketAddr>,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            receiver: self.receiver.resubscribe(),
            sender: self.sender.clone(),
            confused_actors: self.confused_actors.clone(),
            bad_actors: self.bad_actors.clone(),
            repeat_offenders: self.repeat_offenders.clone(),
        }
    }
}

impl AppState {
    fn new(
        config: Arc<RwLock<Config>>,
        receiver: Receiver<CentralMessage>,
        sender: Sender<ServerMessage>,
    ) -> Self {
        Self {
            config,

            receiver,
            sender,

            confused_actors: Vec::new(),
            bad_actors: Vec::new(),
            repeat_offenders: Vec::new(),
        }
    }

    fn add_confused_actor(&mut self, info: SocketAddr) {
        if !self.confused_actors.contains(&info) {
            self.confused_actors.push(info);
        }
    }

    fn add_bad_actor(&mut self, info: SocketAddr) {
        if !self.bad_actors.contains(&info) {
            self.bad_actors.push(info);
        }
    }

    fn should_bail(&mut self, info: &SocketAddr) -> Bail {
        if self.repeat_offenders.contains(info) {
            return Bail::YesIgnore;
        }

        if self.confused_actors.contains(info) || self.bad_actors.contains(info) {
            self.repeat_offenders.push(info.clone());

            return Bail::YesWithResponse;
        }

        Bail::No
    }
}

pub async fn run(
    config: Arc<RwLock<Config>>,
    receiver: Receiver<CentralMessage>,
    sender: Sender<ServerMessage>,
) -> anyhow::Result<()> {
    let state = AppState::new(config, receiver, sender);

    let app = Router::new()
        .route("/", post(handle_command_direct))
        .route("/:bot", post(handle_command_indirect))
        .with_state(state);

    axum::Server::bind(&"127.0.0.1:8946".parse()?)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

async fn handle_command_direct(
    State(mut state): State<AppState>,
    ConnectInfo(info): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: String,
) {
    // match handle_command(&mut state, info, headers, body).await {
    //     Ok(v) => {}
    //     Err(e) => {}
    // }
    if let Ok(v) = handle_command(&mut state, info, headers, body).await {
        todo!()
    }
}

async fn handle_command_indirect(
    Path(bot_name): Path<String>,
    State(mut state): State<AppState>,
    ConnectInfo(info): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: String,
) {
    // match handle_command(&mut state, info, headers, body).await {
    //     Ok(v) => {}
    //     Err(e) => {}
    // }
    if let Ok(v) = handle_command(&mut state, info, headers, body).await {
        todo!()
    }
}

async fn handle_command(
    state: &mut AppState,
    info: SocketAddr,
    headers: HeaderMap,
    body: String,
) -> anyhow::Result<String> {
    match state.should_bail(&info) {
        Bail::No => {
            if let Some(key) = headers.get(KEY_HEADER) {
                if key.to_str()? != API_KEY {
                    state.add_bad_actor(info);
                    error!("BAD_ACTOR={}:{}", info.ip(), info.port());
                    anyhow::bail!("Invalid api key detected");
                }
            } else {
                state.add_confused_actor(info);
                error!("CONFUSED_ACTOR={}:{}", info.ip(), info.port());
                anyhow::bail!("No api key found, bailing out");
            }

            let response = commands::parse(
                body,
                commands::AdditionalInfo::None,
                &*state.config.read().await,
            );

            match response {
                commands::CommandOutput::Error { message, .. } => {
                    return Ok(message);
                }
                commands::CommandOutput::Command { value, .. }
                | commands::CommandOutput::AdminCommand { value, .. } => {
                    return Ok(value.unwrap_or("No output!".into()));
                }
            }
        }
        Bail::YesWithResponse => {
            return Ok("Fuck you".into());
        }
        Bail::YesIgnore => {
            anyhow::bail!("Bailing out");
        }
    }
}
