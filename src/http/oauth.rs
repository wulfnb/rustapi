use crate::models::{KeyValue, OAuthClientAuth, OAuthConfig};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
#[derive(Clone)]
pub struct Token {
    pub access: String,
    pub refresh: String,
    pub expires: Instant,
    pub persistent: bool,
    pub label: String,
}
pub(crate) struct TokenStore {
    path: std::path::PathBuf,
    password: zeroize::Zeroizing<String>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredToken {
    access: String,
    refresh: String,
    expires_at: u64,
    label: String,
}
#[derive(serde::Serialize)]
pub struct TokenSummary {
    pub id: String,
    pub label: String,
    pub valid_for_seconds: u64,
    pub refreshable: bool,
    pub persistent: bool,
}
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn save_tokens(
    store: &TokenStore,
    tokens: &std::collections::HashMap<String, Token>,
) -> Result<()> {
    let values = tokens
        .iter()
        .filter(|(_, token)| token.persistent)
        .map(|(key, token)| {
            let stored = StoredToken {
                access: token.access.clone(),
                refresh: token.refresh.clone(),
                expires_at: unix_now().saturating_add(
                    token
                        .expires
                        .saturating_duration_since(Instant::now())
                        .as_secs(),
                ),
                label: token.label.clone(),
            };
            Ok((key.clone(), serde_json::to_string(&stored)?))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    crate::secrets::save(&store.path, &store.password, &values)
}
impl super::HttpService {
    /// The password lives only in memory until lock_tokens; token values never enter reports.
    pub fn unlock_tokens(&self, path: &std::path::Path, password: &str) -> Result<()> {
        let mut store = self.token_store.lock().unwrap_or_else(|p| p.into_inner());
        anyhow::ensure!(
            !path.exists() || std::fs::metadata(path)?.len() <= 4 * 1024 * 1024,
            "OAuth token store exceeds 4 MiB"
        );
        let values = crate::secrets::load(path, password)?;
        anyhow::ensure!(values.len() <= 1000, "Too many saved OAuth tokens");
        let mut loaded = std::collections::HashMap::new();
        for (key, value) in values {
            let token: StoredToken =
                serde_json::from_str(&value).context("Invalid OAuth token store")?;
            loaded.insert(
                key,
                Token {
                    access: token.access,
                    refresh: token.refresh,
                    expires: Instant::now()
                        + Duration::from_secs(
                            token.expires_at.saturating_sub(unix_now()).min(86400 * 30),
                        ),
                    persistent: true,
                    label: token.label,
                },
            );
        }
        self.tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .extend(loaded);
        *store = Some(TokenStore {
            path: path.into(),
            password: zeroize::Zeroizing::new(password.into()),
        });
        Ok(())
    }
    pub fn lock_tokens(&self) {
        let mut store = self.token_store.lock().unwrap_or_else(|p| p.into_inner());
        *store = None;
        self.tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }
    pub fn token_summaries(&self) -> Vec<TokenSummary> {
        let tokens = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        let mut output = tokens
            .iter()
            .map(|(id, t)| TokenSummary {
                id: id.clone(),
                label: t.label.clone(),
                valid_for_seconds: t
                    .expires
                    .saturating_duration_since(Instant::now())
                    .as_secs(),
                refreshable: !t.refresh.is_empty(),
                persistent: t.persistent,
            })
            .collect::<Vec<_>>();
        output.sort_by(|a, b| a.id.cmp(&b.id));
        output
    }
    pub fn forget_token(&self, id: &str) -> Result<()> {
        let store = self.token_store.lock().unwrap_or_else(|p| p.into_inner());
        let mut tokens = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        let mut next = tokens.clone();
        next.remove(id);
        if let Some(store) = store.as_ref() {
            save_tokens(store, &next)?;
        }
        *tokens = next;
        Ok(())
    }
    fn remember_token(&self, id: String, token: Token) -> Result<()> {
        let store = self.token_store.lock().unwrap_or_else(|p| p.into_inner());
        let mut tokens = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        let mut next = tokens.clone();
        next.insert(id, token.clone());
        if token.persistent {
            save_tokens(
                store
                    .as_ref()
                    .context("Unlock the vault to persist OAuth tokens")?,
                &next,
            )?;
        }
        *tokens = next;
        Ok(())
    }
}
fn cache_key(config: &OAuthConfig) -> Result<String> {
    let mut config = config.clone();
    config.code.clear();
    config.verifier.clear();
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&config)?)
    ))
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
const RESERVED: &[&str] = &[
    "response_type",
    "client_id",
    "client_secret",
    "redirect_uri",
    "scope",
    "state",
    "code_challenge",
    "code_challenge_method",
    "code",
    "grant_type",
    "refresh_token",
    "code_verifier",
    "response_mode",
];
fn endpoint(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value)?;
    anyhow::ensure!(
        ["http", "https"].contains(&url.scheme()) && url.host_str().is_some(),
        "OAuth endpoint must be HTTP(S)"
    );
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none() && url.fragment().is_none(),
        "OAuth endpoint cannot contain userinfo or fragments"
    );
    anyhow::ensure!(
        url.query_pairs().all(|(key, _)| !RESERVED
            .iter()
            .any(|reserved| key.eq_ignore_ascii_case(reserved))),
        "OAuth endpoint query cannot override managed parameters"
    );
    Ok(url)
}
fn extra(rows: &[KeyValue]) -> Result<Vec<(String, String)>> {
    anyhow::ensure!(rows.len() <= 100, "At most 100 OAuth provider parameters");
    let mut output = vec![];
    for row in rows.iter().filter(|row| row.enabled) {
        anyhow::ensure!(
            !row.key.is_empty() && row.key.len() <= 256 && row.value.len() <= 16384 && !row.file,
            "Invalid OAuth provider parameter"
        );
        anyhow::ensure!(
            !RESERVED
                .iter()
                .any(|reserved| row.key.eq_ignore_ascii_case(reserved)),
            "Provider parameters cannot override managed OAuth/PKCE fields"
        );
        output.push((row.key.clone(), row.value.clone()));
    }
    Ok(output)
}
pub fn authorization_url(config: &OAuthConfig, state: &str, challenge: &str) -> Result<String> {
    anyhow::ensure!(
        !config.client_id.is_empty() && !config.redirect_uri.is_empty(),
        "OAuth client ID and redirect URI are required"
    );
    let mut url = endpoint(&config.authorization_url)?;
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", config.client_id.as_str()),
        ("redirect_uri", config.redirect_uri.as_str()),
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
    ]);
    if !config.scope.is_empty() {
        url.query_pairs_mut().append_pair("scope", &config.scope);
    }
    url.query_pairs_mut()
        .extend_pairs(extra(&config.authorization_params)?);
    Ok(url.into())
}
fn basic_component(value: &str) -> Result<String> {
    Ok(serde_urlencoded::to_string([("", value)])?
        .strip_prefix('=')
        .unwrap_or_default()
        .to_owned())
}
pub async fn access_token(
    service: &super::HttpService,
    client: &reqwest::Client,
    c: &OAuthConfig,
) -> Result<String> {
    let key = cache_key(c)?;
    if c.persist_tokens {
        anyhow::ensure!(
            service
                .token_store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_some(),
            "Unlock the vault to persist OAuth tokens"
        );
    }
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
    anyhow::ensure!(!c.client_id.is_empty(), "OAuth client ID is required");
    anyhow::ensure!(
        c.grant.is_empty()
            || ["client_credentials", "authorization_code"].contains(&c.grant.as_str()),
        "Unsupported OAuth grant"
    );
    let refreshing = previous
        .as_ref()
        .is_some_and(|token| !token.refresh.is_empty());
    let mut fields = vec![];
    if c.client_auth != OAuthClientAuth::Basic {
        fields.push(("client_id".to_owned(), c.client_id.clone()));
    }
    if c.client_auth == OAuthClientAuth::Body && !c.client_secret.is_empty() {
        fields.push(("client_secret".into(), c.client_secret.clone()));
    }
    if refreshing {
        fields.extend([
            ("grant_type".into(), "refresh_token".into()),
            (
                "refresh_token".into(),
                previous.as_ref().unwrap().refresh.clone(),
            ),
        ]);
        if !c.refresh_scope.is_empty() {
            fields.push(("scope".into(), c.refresh_scope.clone()));
        }
        fields.extend(extra(&c.refresh_params)?);
    } else if c.grant == "authorization_code" {
        anyhow::ensure!(
            !c.code.is_empty() && (43..=128).contains(&c.verifier.len()),
            "OAuth authorization code and PKCE verifier required"
        );
        fields.extend([
            ("grant_type".into(), "authorization_code".into()),
            ("code".into(), c.code.clone()),
            ("code_verifier".into(), c.verifier.clone()),
            ("redirect_uri".into(), c.redirect_uri.clone()),
        ]);
        fields.extend(extra(&c.token_params)?);
    } else {
        anyhow::ensure!(
            c.client_auth != OAuthClientAuth::Public,
            "Client credentials grant requires confidential client authentication"
        );
        fields.extend([("grant_type".into(), "client_credentials".into())]);
        if !c.scope.is_empty() {
            fields.push(("scope".into(), c.scope.clone()));
        }
        fields.extend(extra(&c.token_params)?);
    }
    let url = endpoint(if refreshing && !c.refresh_url.is_empty() {
        &c.refresh_url
    } else {
        &c.token_url
    })?;
    service.policy()?.url(url.as_str())?;
    let mut request = client.post(url.clone()).form(&fields);
    if c.client_auth == OAuthClientAuth::Basic {
        request = request.basic_auth(
            basic_component(&c.client_id)?,
            Some(basic_component(&c.client_secret)?),
        );
    }
    service.debug.push("oauth","info",serde_json::json!({"event":"exchange","refresh":refreshing,"url":crate::debug::safe_url(url.as_str()),"client_auth":c.client_auth}));
    let mut response = request.send().await.context("OAuth token exchange")?;
    service.debug.push(
        "oauth",
        if response.status().is_success() {
            "info"
        } else {
            "error"
        },
        serde_json::json!({"event":"exchange response","status":response.status().as_u16()}),
    );
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
    anyhow::ensure!(
        !access.is_empty()
            && access
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/=".contains(&byte)),
        "Invalid OAuth Bearer token"
    );
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
    service.remember_token(
        key,
        Token {
            access: access.clone(),
            refresh,
            expires: Instant::now() + Duration::from_secs(expires),
            persistent: c.persist_tokens,
            label: format!("{} ({})", crate::debug::safe_url(&c.token_url), c.client_id),
        },
    )?;
    Ok(access)
}

pub fn authorization_binding(config: &OAuthConfig) -> Result<String> {
    let mut config = config.clone();
    config.code.clear();
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&config)?)
    ))
}
pub struct Authorization {
    pub url: String,
    pub listener: std::net::TcpListener,
    pub state: String,
    expected_path: String,
}
pub fn begin_authorization(config: &mut OAuthConfig) -> Result<Authorization> {
    let listener = if config.redirect_uri.is_empty() {
        std::net::TcpListener::bind("127.0.0.1:0")?
    } else {
        let uri = reqwest::Url::parse(&config.redirect_uri)?;
        anyhow::ensure!(
            uri.scheme() == "http"
                && uri.host_str() == Some("127.0.0.1")
                && uri.username().is_empty()
                && uri.password().is_none()
                && uri.fragment().is_none()
                && uri
                    .query_pairs()
                    .all(|(key, _)| !["state", "code", "error"].contains(&key.as_ref())),
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
        expected_path: reqwest::Url::parse(&config.redirect_uri)?.path().to_owned(),
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
                    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                    let connection = Instant::now();
                    let mut bytes = vec![];
                    let mut buffer = [0; 1024];
                    while bytes.len() < 16384
                        && !bytes.windows(4).any(|part| part == b"\r\n\r\n")
                        && connection.elapsed() < Duration::from_secs(2)
                        && start.elapsed() < Duration::from_secs(120)
                        && !cancel.is_cancelled()
                    {
                        match stream.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                            Err(error)
                                if [
                                    std::io::ErrorKind::WouldBlock,
                                    std::io::ErrorKind::TimedOut,
                                ]
                                .contains(&error.kind()) => {}
                            Err(_) => break,
                        }
                    }
                    let text = String::from_utf8_lossy(&bytes);
                    let first = text
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .split_whitespace()
                        .collect::<Vec<_>>();
                    let parsed = if first.len() == 3
                        && first[0] == "GET"
                        && first[1].starts_with('/')
                        && bytes.windows(4).any(|part| part == b"\r\n\r\n")
                    {
                        reqwest::Url::parse(&format!("http://127.0.0.1{}", first[1])).ok()
                    } else {
                        None
                    };
                    let mut fields = std::collections::HashMap::new();
                    let mut duplicate = false;
                    if let Some(url) = &parsed {
                        for (key, value) in url.query_pairs() {
                            if fields
                                .insert(key.into_owned(), value.into_owned())
                                .is_some()
                            {
                                duplicate = true;
                            }
                        }
                    }
                    if duplicate
                        || parsed
                            .as_ref()
                            .is_none_or(|url| url.path() != self.expected_path)
                        || fields.get("state") != Some(&self.state)
                    {
                        let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 16\r\nConnection: close\r\n\r\nInvalid callback");
                        continue;
                    }
                    if fields.contains_key("error") {
                        let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 20\r\nConnection: close\r\n\r\nAuthorization denied");
                        bail!("OAuth provider denied authorization");
                    }
                    let Some(code) = fields.get("code").filter(|code| !code.is_empty()) else {
                        let _=stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 16\r\nConnection: close\r\n\r\nInvalid callback");
                        continue;
                    };
                    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 36\r\nConnection: close\r\n\r\nAuthorized. You can return to RustAPI")?;
                    return Ok(code.clone());
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
