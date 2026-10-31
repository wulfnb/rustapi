use super::*;
use anyhow::{bail, Context};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    protocol::{Message, WebSocketConfig},
};
pub(super) async fn run(
    service: &HttpService,
    profile: &Session,
    commands: Option<&Receiver<String>>,
    sink: &mut Sink<'_>,
) -> Result<()> {
    let mut request = profile.request.clone();
    super::auth(service, &mut request).await?;
    ensure!(
        !matches!(
            request.auth.auth_type,
            crate::models::AuthType::AwsSigV4 | crate::models::AuthType::Digest
        ),
        "WebSocket handshake signing/Digest is unsupported; use explicit token/header auth"
    );
    ensure!(request.settings.verify_tls&&request.settings.ca_file.is_empty()&&request.settings.identity_file.is_empty()&&request.settings.proxy.is_empty(),"WebSocket currently requires normal verified TLS without custom CA, identity or proxy settings");
    let mut params = request.query_params.clone();
    if let Some((key, value)) = request.auth.get_api_key_query_param() {
        params.push(crate::models::KeyValue::new(key, value));
    }
    let mut url = reqwest::Url::parse(&crate::http::build_url_with_params(
        &request.url,
        &params,
        None,
    ))?;
    if profile.protocol == Protocol::SocketIo {
        ensure!(
            ["http", "https", "ws", "wss"].contains(&url.scheme()),
            "Socket.IO URL must be HTTP(S)/WS(S)"
        );
        let scheme = if ["https", "wss"].contains(&url.scheme()) {
            "wss"
        } else {
            "ws"
        };
        url.set_scheme(scheme)
            .map_err(|_| anyhow::anyhow!("Invalid WebSocket scheme"))?;
        if url.path() == "/" {
            url.set_path("/socket.io/");
        }
        url.query_pairs_mut()
            .append_pair("EIO", "4")
            .append_pair("transport", "websocket");
        ensure!(
            profile.socket_namespace.starts_with('/')
                && !profile.socket_namespace.contains([',', '\r', '\n']),
            "Invalid Socket.IO namespace"
        );
    }
    ensure!(
        ["ws", "wss"].contains(&url.scheme()),
        "WebSocket URL must use ws:// or wss://"
    );
    let mut handshake = url.as_str().into_client_request()?;
    for (key, value) in crate::http::effective_headers(&request)? {
        ensure!(
            ![
                "host",
                "connection",
                "upgrade",
                "sec-websocket-key",
                "sec-websocket-version",
                "content-length",
                "transfer-encoding"
            ]
            .contains(&key.as_str()),
            "WebSocket transport headers cannot be overridden"
        );
        handshake.headers_mut().append(
            tokio_tungstenite::tungstenite::http::header::HeaderName::from_bytes(key.as_bytes())?,
            value.parse()?,
        );
    }
    if request.settings.use_cookies && !handshake.headers().contains_key("cookie") {
        let mut cookie_url = url.clone();
        let scheme = if url.scheme() == "wss" {
            "https"
        } else {
            "http"
        };
        let _ = cookie_url.set_scheme(scheme);
        if let Some(value) =
            reqwest::cookie::CookieStore::cookies(service.cookies.as_ref(), &cookie_url)
        {
            handshake.headers_mut().insert("cookie", value);
        }
    }
    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(1024 * 1024);
    config.max_frame_size = Some(1024 * 1024);
    config.max_write_buffer_size = 2 * 1024 * 1024;
    let (mut socket, response) =
        tokio_tungstenite::connect_async_with_config(handshake, Some(config), false)
            .await
            .context("WebSocket handshake")?;
    if request.settings.use_cookies {
        let mut cookie_url = url.clone();
        let scheme = if url.scheme() == "wss" {
            "https"
        } else {
            "http"
        };
        let _ = cookie_url.set_scheme(scheme);
        let mut headers = response.headers().get_all("set-cookie").iter();
        reqwest::cookie::CookieStore::set_cookies(
            service.cookies.as_ref(),
            &mut headers,
            &cookie_url,
        );
    }
    let mut connected = profile.protocol == Protocol::WebSocket;
    let mut sent = false;
    if connected {
        sink.send("connected", json!({"transport":"websocket"}))?;
    }
    let mut poll = tokio::time::interval(Duration::from_millis(20));
    loop {
        if connected && !sent {
            for message in &profile.messages {
                socket
                    .send(Message::Text(super::outgoing(profile, message)?.into()))
                    .await?;
            }
            sent = true;
        }
        tokio::select! {
            message=socket.next()=>{
                let Some(message)=message else{return Ok(());};
                match message? {
                    Message::Text(text) if profile.protocol==Protocol::SocketIo=> {
                        if let Some(open)=text.strip_prefix('0'){let _:Value=serde_json::from_str(open).context("Invalid Engine.IO open packet")?;let auth:Value=serde_json::from_str(&profile.socket_auth).context("Socket.IO auth must be JSON")?;ensure!(auth.is_object(),"Socket.IO auth must be an object");let namespace=if profile.socket_namespace=="/"{String::new()}else{format!("{},",profile.socket_namespace)};socket.send(Message::Text(format!("40{namespace}{auth}").into())).await?;}
                        else if text=="2"{socket.send(Message::Text("3".into())).await?;}
                        else if text.starts_with("40"){connected=true;sink.send("connected",json!({"transport":"socket.io","namespace":profile.socket_namespace}))?;}
                        else if let Some(event)=text.strip_prefix("42"){let event=if event.starts_with('/'){event.split_once(',').context("Invalid Socket.IO event namespace")?.1}else{event};let value:Value=serde_json::from_str(event).context("Socket.IO acknowledgements/binary events are unsupported")?;if sink.send("event",value)?{return Ok(());}}
                        else if text.starts_with("44"){bail!("Socket.IO server rejected namespace authentication");}
                        else if text.starts_with("41")||text=="1"{return Ok(());}
                        else if text.starts_with("45")||text.starts_with("46"){bail!("Binary Socket.IO events are unsupported");}
                    },
                    Message::Text(text)=>if sink.send("message",json!({"text":text.as_str()}))?{return Ok(());},
                    Message::Binary(bytes)=>{use base64::Engine;if sink.send("message",json!({"base64":base64::engine::general_purpose::STANDARD.encode(bytes)}))?{return Ok(());}},
                    Message::Ping(bytes)=>socket.send(Message::Pong(bytes)).await?,
                    Message::Close(_)=>return Ok(()),_=>{},
                }
            },
            _=poll.tick()=>if connected{if let Some(commands)=commands{if let Ok(message)=commands.try_recv(){socket.send(Message::Text(super::outgoing(profile,&message)?.into())).await?;}}}
        }
    }
}
