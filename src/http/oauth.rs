use crate::models::OAuthConfig;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
#[derive(Clone)]
pub struct Token {
    pub access: String,
    pub refresh: String,
    pub expires: Instant,
}
pub fn pkce() -> (String, String) {
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}
pub fn authorization_url(config: &OAuthConfig, state: &str, challenge: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(&config.authorization_url)?;
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", config.client_id.as_str()),
        ("redirect_uri", config.redirect_uri.as_str()),
        ("scope", config.scope.as_str()),
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
    ]);
    Ok(url.into())
}
pub async fn access_token(
    service: &super::HttpService,
    client: &reqwest::Client,
    c: &OAuthConfig,
) -> Result<String> {
    let key = serde_json::to_string(c)?;
    let previous = service
        .tokens
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&key)
        .cloned();
    if let Some(t) = &previous {
        if t.expires > Instant::now() {
            return Ok(t.access.clone());
        }
    }
    let mut fields = vec![("client_id", c.client_id.clone())];
    if !c.client_secret.is_empty() {
        fields.push(("client_secret", c.client_secret.clone()));
    }
    if let Some(t) = &previous {
        if !t.refresh.is_empty() {
            fields.extend([
                ("grant_type", "refresh_token".into()),
                ("refresh_token", t.refresh.clone()),
            ]);
        }
    }
    if fields.iter().all(|(k, _)| *k != "grant_type") {
        if c.grant == "authorization_code" {
            anyhow::ensure!(
                !c.code.is_empty() && c.verifier.len() >= 43,
                "OAuth authorization code and PKCE verifier required"
            );
            fields.extend([
                ("grant_type", "authorization_code".into()),
                ("code", c.code.clone()),
                ("code_verifier", c.verifier.clone()),
                ("redirect_uri", c.redirect_uri.clone()),
            ]);
        } else {
            fields.extend([
                ("grant_type", "client_credentials".into()),
                ("scope", c.scope.clone()),
            ]);
        }
    }
    let mut response = client
        .post(&c.token_url)
        .form(&fields)
        .send()
        .await
        .context("OAuth token exchange")?;
    if !response.status().is_success() {
        bail!("OAuth token endpoint returned {}", response.status());
    }
    let mut bytes = vec![];
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            bytes.len() + chunk.len() <= 1024 * 1024,
            "OAuth response too large"
        );
        bytes.extend_from_slice(&chunk);
    }

    let json: serde_json::Value = serde_json::from_slice(&bytes)?;
    let access = json["access_token"]
        .as_str()
        .context("OAuth response has no access_token")?
        .to_string();
    let token_type = json["token_type"].as_str().unwrap_or("Bearer");
    anyhow::ensure!(
        token_type.eq_ignore_ascii_case("Bearer"),
        "Only Bearer OAuth tokens are supported"
    );
    let expires = json["expires_in"]
        .as_u64()
        .unwrap_or(300)
        .min(86400 * 30)
        .saturating_sub(5);
    let refresh = json["refresh_token"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| previous.map(|t| t.refresh).unwrap_or_default());
    service
        .tokens
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(
            key,
            Token {
                access: access.clone(),
                refresh,
                expires: Instant::now() + Duration::from_secs(expires),
            },
        );
    Ok(access)
}

pub struct Authorization {
    pub url: String,
    pub listener: std::net::TcpListener,
    pub state: String,
}
pub fn begin_authorization(config: &mut OAuthConfig) -> Result<Authorization> {
    let listener = if config.redirect_uri.is_empty() {
        std::net::TcpListener::bind("127.0.0.1:0")?
    } else {
        let uri = reqwest::Url::parse(&config.redirect_uri)?;
        anyhow::ensure!(
            uri.scheme() == "http" && uri.host_str() == Some("127.0.0.1"),
            "Browser OAuth requires an http://127.0.0.1:<port>/callback redirect"
        );
        std::net::TcpListener::bind((
            "127.0.0.1",
            uri.port().context("Redirect URI requires a port")?,
        ))?
    };
    if config.redirect_uri.is_empty() {
        config.redirect_uri = format!("http://{}/callback", listener.local_addr()?);
    }
    let (verifier, challenge) = pkce();
    config.verifier = verifier;
    config.code.clear();
    let state = uuid::Uuid::new_v4().to_string();
    let url = authorization_url(config, &state, &challenge)?;
    listener.set_nonblocking(true)?;
    Ok(Authorization {
        url,
        listener,
        state,
    })
}
impl Authorization {
    pub fn wait(self, cancel: &super::Cancellation) -> Result<String> {
        use std::io::{Read, Write};
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(120) && !cancel.is_cancelled() {
            match self.listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false)?;
                    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                    let mut buf = [0; 16384];
                    let n = stream.read(&mut buf)?;
                    let text = String::from_utf8_lossy(&buf[..n]);
                    let path = text
                        .lines()
                        .next()
                        .and_then(|l| l.split_whitespace().nth(1))
                        .unwrap_or("");
                    let url = reqwest::Url::parse(&format!("http://127.0.0.1{path}"))?;
                    let fields: std::collections::HashMap<_, _> = url
                        .query_pairs()
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect();
                    if fields.get("state") != Some(&self.state) {
                        let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 13\r\nConnection: close\r\n\r\nInvalid state");
                        continue;
                    }
                    let code = fields
                        .get("code")
                        .context("OAuth provider denied authorization")?
                        .clone();
                    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 36\r\nConnection: close\r\n\r\nAuthorized. You can return to RustAPI")?;
                    return Ok(code);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Err(e) => return Err(e.into()),
            }
        }
        bail!("OAuth authorization cancelled or timed out")
    }
}
