//! Bounded live protocol sessions, separate from buffered REST responses.
use crate::{
    http::{Cancellation, HttpService},
    models::ApiRequest,
    variables::Variables,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};
mod sse;
pub use sse::*;
mod grpc;
mod mqtt;
mod sockets;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Protocol {
    #[default]
    WebSocket,
    Sse,
    Mqtt,
    SocketIo,
    Grpc,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Session {
    pub protocol: Protocol,
    pub request: ApiRequest,
    pub duration_seconds: u64,
    pub max_events: usize,
    pub messages: Vec<String>,
    pub topic: String,
    pub mqtt_username: String,
    pub mqtt_password: String,
    pub mqtt_client_id: String,
    pub qos: u8,
    pub retain: bool,
    pub subscribe: bool,
    pub socket_event: String,
    pub socket_namespace: String,
    pub socket_auth: String,
    pub grpc_method: String,
    pub grpc_action: String,
    pub grpc_descriptor: String,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            protocol: Protocol::WebSocket,
            request: ApiRequest::default(),
            duration_seconds: 60,
            max_events: 1000,
            messages: vec![],
            topic: "rustapi/test".into(),
            mqtt_username: String::new(),
            mqtt_password: String::new(),
            mqtt_client_id: String::new(),
            qos: 0,
            retain: false,
            subscribe: true,
            socket_event: "message".into(),
            socket_namespace: "/".into(),
            socket_auth: "{}".into(),
            grpc_method: String::new(),
            grpc_action: "invoke".into(),
            grpc_descriptor: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub elapsed_ms: u128,
    pub kind: String,
    pub data: Value,
}
#[derive(Serialize, Debug)]
pub struct Summary {
    pub events: usize,
    pub bytes: usize,
    pub reason: String,
}
pub(crate) struct Sink<'a> {
    start: Instant,
    events: usize,
    bytes: usize,
    max: usize,
    connected: bool,
    emit: &'a mut dyn FnMut(Event),
}
impl Sink<'_> {
    pub fn send(&mut self, kind: &str, data: Value) -> Result<bool> {
        if kind == "connected" {
            self.connected = true;
        }
        let bytes = serde_json::to_vec(&data)?.len();
        ensure!(
            bytes <= 2 * 1024 * 1024 && self.bytes.saturating_add(bytes) <= 16 * 1024 * 1024,
            "Protocol session exceeds payload limits"
        );
        self.bytes += bytes;
        if kind != "connected" {
            self.events += 1;
        }
        (self.emit)(Event {
            elapsed_ms: self.start.elapsed().as_millis(),
            kind: kind.into(),
            data,
        });
        Ok(self.events >= self.max)
    }
}
pub fn run(
    service: &HttpService,
    profile: &Session,
    variables: &Variables,
    cancel: &Cancellation,
    commands: Option<&Receiver<String>>,
    mut emit: impl FnMut(Event),
) -> Result<Summary> {
    ensure!(
        (1..=3600).contains(&profile.duration_seconds) && (1..=10000).contains(&profile.max_events),
        "Protocol sessions require 1–3600 seconds and 1–10000 events"
    );
    ensure!(
        profile.messages.len() <= 1000 && profile.messages.iter().all(|m| m.len() <= 1024 * 1024),
        "Protocol outgoing messages exceed limits"
    );
    ensure!(
        profile.messages.iter().map(String::len).sum::<usize>() <= 8 * 1024 * 1024,
        "Initial protocol messages exceed 8 MiB"
    );
    let mut profile = profile.clone();
    profile.request = variables.resolve_request(&profile.request)?;
    service.policy()?.request(&profile.request)?;
    for field in [
        &mut profile.topic,
        &mut profile.mqtt_username,
        &mut profile.mqtt_password,
        &mut profile.mqtt_client_id,
        &mut profile.socket_auth,
    ] {
        *field = variables.resolve(field)?;
    }
    for message in &mut profile.messages {
        *message = variables.resolve(message)?;
    }
    let url = reqwest::Url::parse(&profile.request.url)?;
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Protocol URLs cannot contain credentials; use auth fields"
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut sink = Sink {
        start: Instant::now(),
        events: 0,
        bytes: 0,
        max: profile.max_events,
        connected: false,
        emit: &mut emit,
    };
    let reason=rt.block_on(async {
        tokio::select!{
            biased;
            _=async{while !cancel.is_cancelled(){tokio::time::sleep(Duration::from_millis(20)).await;}}=>Ok("cancelled".to_owned()),
            result=tokio::time::timeout(Duration::from_secs(profile.duration_seconds),async {
                match profile.protocol {
                    Protocol::Sse=>sse::run(service,&profile,&mut sink).await,
                    Protocol::WebSocket|Protocol::SocketIo=>sockets::run(service,&profile,commands,&mut sink).await,
                    Protocol::Mqtt=>mqtt::run(&profile,commands,&mut sink).await,
                    Protocol::Grpc=>grpc::run(service,&profile,&mut sink).await,
                }
            })=>match result{Ok(result)=>result.map(|_|"completed".to_owned()),Err(_)=>Ok("duration reached".to_owned())}
        }
    })?;
    ensure!(
        reason != "duration reached" || sink.connected,
        "Protocol connection did not become ready before deadline"
    );
    Ok(Summary {
        events: sink.events,
        bytes: sink.bytes,
        reason,
    })
}
pub(crate) async fn auth(service: &HttpService, request: &mut ApiRequest) -> Result<()> {
    if request.auth.auth_type == crate::models::AuthType::OAuth2 {
        let mut settings = request.settings.clone();
        settings.follow_redirects = false;
        request.auth.bearer_token = crate::http::oauth::access_token(
            service,
            &service.client(&settings)?,
            &request.auth.oauth,
        )
        .await?;
        request.auth.auth_type = crate::models::AuthType::BearerToken;
    }
    ensure!(
        !matches!(
            request.auth.auth_type,
            crate::models::AuthType::Unsupported | crate::models::AuthType::Inherit
        ),
        "Choose explicit supported protocol authentication"
    );
    Ok(())
}
pub(crate) fn outgoing(profile: &Session, message: &str) -> Result<String> {
    ensure!(
        message.len() <= 1024 * 1024,
        "Outgoing message exceeds 1 MiB"
    );
    if profile.protocol == Protocol::SocketIo {
        let value: Value = serde_json::from_str(message)
            .map_err(|_| anyhow::anyhow!("Socket.IO payload must be JSON"))?;
        let namespace = if profile.socket_namespace == "/" {
            String::new()
        } else {
            format!("{},", profile.socket_namespace)
        };
        Ok(format!(
            "42{namespace}{}",
            json!([profile.socket_event, value])
        ))
    } else {
        Ok(message.into())
    }
}
