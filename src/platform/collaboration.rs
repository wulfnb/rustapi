use crate::{
    local_http::{Request, Response, Server},
    models::Collection,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Viewer,
    Editor,
    Admin,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub token_hash: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}
impl User {
    pub fn issue(name: String, role: Role, days: u32) -> Result<(Self, String)> {
        ensure!(
            !name.is_empty() && name.len() <= 128 && (1..=365).contains(&days),
            "User requires a name and 1–365 day token lifetime"
        );
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let user = Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            role,
            token_hash: format!("{:x}", Sha256::digest(token.as_bytes())),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::days(days as i64)),
        };
        Ok((user, token))
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub port: u16,
    pub users: Vec<User>,
    pub public_documentation: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            port: 9090,
            users: vec![],
            public_documentation: false,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && !self.users.is_empty() && self.users.len() <= 1000,
            "Server needs 1–1000 configured users"
        );
        let mut ids = std::collections::BTreeSet::new();
        for user in &self.users {
            uuid::Uuid::parse_str(&user.id)?;
            ensure!(
                ids.insert(&user.id)
                    && user.token_hash.len() == 64
                    && user.token_hash.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid or duplicate server identity"
            );
        }
        ensure!(
            self.users.iter().any(|u| u.role == Role::Admin),
            "Configure at least one administrator"
        );
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Revision {
    pub collection: Collection,
    pub revision: String,
    pub updated_at: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub revision: String,
    pub updated_at: String,
    pub requests: usize,
}
pub struct TeamServer {
    pub server: Server,
    _lock: Arc<std::fs::File>,
}
fn artifact(root: &Path, id: &str) -> Result<std::path::PathBuf> {
    uuid::Uuid::parse_str(id).context("Collection ID must be a UUID")?;
    crate::storage::owned_path(root, &["collections", &format!("{id}.json")])
}
fn read(root: &Path, id: &str) -> Result<Revision> {
    let value: Revision = serde_json::from_str(&crate::tools_cli::read(
        &artifact(root, id)?,
        2 * 1024 * 1024,
    )?)?;
    ensure!(
        value.collection.id == id,
        "Server artifact identity mismatch"
    );
    Ok(value)
}
fn catalog(root: &Path) -> Result<Vec<CatalogEntry>> {
    let directory = crate::storage::owned_path(root, &["collections"])?;
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut entries = vec![];
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        ensure!(
            entries.len() < 1000,
            "Server catalog exceeds 1000 collections"
        );
        if entry.path().extension().is_none_or(|e| e != "json") {
            continue;
        }
        let id = entry
            .path()
            .file_stem()
            .and_then(|n| n.to_str())
            .context("Invalid server artifact")?
            .to_owned();
        let revision = read(root, &id)?;
        let requests = revision.collection.ordered_requests(None).len();
        entries.push(CatalogEntry {
            id,
            name: revision.collection.name,
            version: revision.collection.version,
            revision: revision.revision,
            updated_at: revision.updated_at,
            requests,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}
fn audit(root: &Path, user: &User, id: &str, revision: &str) -> Result<()> {
    use std::io::Write;
    let path = crate::storage::owned_path(root, &["audit.jsonl"])?;
    ensure!(
        !path.exists() || std::fs::metadata(&path)?.len() < 8 * 1024 * 1024,
        "Audit journal is full; archive it before accepting more writes"
    );
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    writeln!(
        file,
        "{}",
        serde_json::json!({"time":chrono::Utc::now().to_rfc3339(),"event":"write_intent","principal":user.id,"collection":id,"revision":revision})
    )?;
    file.sync_all()?;
    Ok(())
}
impl TeamServer {
    pub fn start(root: &Path, config: Config) -> Result<Self> {
        config.validate()?;
        std::fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        let lock_path = crate::storage::owned_path(&root, &["server.lock"])?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path)?;
        lock.try_lock()
            .context("Another team server owns this data directory")?;
        let lock = Arc::new(lock);
        let active_lease = lock.clone();
        let transaction = Arc::new(Mutex::new(()));
        let server = Server::start(config.port, move |request| {
            let _lease = &active_lease;
            let _transaction = transaction.lock().unwrap_or_else(|p| p.into_inner());
            handle(&root, &config, request)
        })?;
        Ok(Self {
            server,
            _lock: lock,
        })
    }
}
fn handle(root: &Path, config: &Config, request: Request) -> Response {
    let path = request.target.split('?').next().unwrap_or_default();
    let public =
        request.method == "GET" && config.public_documentation && path.starts_with("/v1/docs/");
    let user = request
        .headers
        .get("authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|token| {
            let hash = format!("{:x}", Sha256::digest(token.as_bytes()));
            config.users.iter().find(|user| {
                use subtle::ConstantTimeEq;
                user.token_hash.as_bytes().ct_eq(hash.as_bytes()).into()
                    && user
                        .expires_at
                        .is_none_or(|expires| expires > chrono::Utc::now())
            })
        });
    if user.is_none() && !public {
        return Response::json(401, serde_json::json!({"error":"Authentication required"}));
    }
    let result = (|| -> Result<Response> {
        if request.method == "GET" && path == "/v1/catalog" {
            return Ok(Response::json(200, serde_json::to_value(catalog(root)?)?));
        }
        if request.method == "GET" && path.starts_with("/v1/docs/") {
            let revision = read(root, path.trim_start_matches("/v1/docs/"))?;
            let docs = crate::documentation::generate(&revision.collection, false)?;
            return Ok(Response{status:200,headers:vec![("Content-Type".into(),"text/html; charset=utf-8".into()),("Content-Security-Policy".into(),"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'".into())],body:docs.html.into_bytes()});
        }
        let id = path
            .strip_prefix("/v1/collections/")
            .context("Unknown team endpoint")?;
        match request.method.as_str() {
            "GET" => {
                let revision = read(root, id)?;
                let mut response = Response::json(200, serde_json::to_value(&revision)?);
                response
                    .headers
                    .push(("ETag".into(), format!("\"{}\"", revision.revision)));
                Ok(response)
            }
            "PUT" => {
                let user = user.context("Authentication required")?;
                if user.role == Role::Viewer {
                    return Ok(Response::json(
                        403,
                        serde_json::json!({"error":"Editor role required"}),
                    ));
                }
                let path = artifact(root, id)?;
                let expected = if path.exists() {
                    read(root, id)?.revision
                } else {
                    "new".into()
                };
                if request.headers.get("if-match").map(|v| v.trim_matches('"'))
                    != Some(expected.as_str())
                {
                    return Ok(Response::json(
                        409,
                        serde_json::json!({"error":"Revision conflict; pull and review changes before pushing","revision":expected}),
                    ));
                }
                let collection: Collection = serde_json::from_slice(&request.body)?;
                crate::models::validate_collection(&collection)?;
                ensure!(
                    collection.id == id && collection.name.len() <= 256,
                    "Invalid collection identity/name"
                );
                let revision = Revision {
                    collection,
                    revision: uuid::Uuid::new_v4().to_string(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                let bytes = serde_json::to_vec(&revision)?;
                ensure!(
                    bytes.len() <= 1024 * 1024,
                    "Team collections are limited to 1 MiB"
                );
                audit(root, user, id, &revision.revision)?;
                crate::workspace::atomic_write(&path, &bytes)?;
                Ok(Response::json(
                    if expected == "new" { 201 } else { 200 },
                    serde_json::json!({"revision":revision.revision}),
                ))
            }
            _ => Ok(Response::json(
                405,
                serde_json::json!({"error":"Method not allowed"}),
            )),
        }
    })();
    result.unwrap_or_else(|_| {
        Response::json(
            400,
            serde_json::json!({"error":"Invalid or unavailable team resource"}),
        )
    })
}
pub struct Client {
    base: reqwest::Url,
    token: zeroize::Zeroizing<String>,
    client: reqwest::blocking::Client,
}
impl Client {
    pub fn for_workspace(root: &Path, endpoint: &str, token: &str) -> Result<Self> {
        crate::policy::Policy::load(root)?.url(endpoint)?;
        Self::new(endpoint, token)
    }
    pub fn new(endpoint: &str, token: &str) -> Result<Self> {
        let base = reqwest::Url::parse(&format!("{}/", endpoint.trim_end_matches('/')))?;
        ensure!(
            base.username().is_empty()
                && base.password().is_none()
                && base.query().is_none()
                && base.fragment().is_none()
                && (base.scheme() == "https"
                    || base.scheme() == "http"
                        && matches!(base.host_str(), Some("127.0.0.1" | "localhost"))),
            "Team servers require HTTPS or loopback HTTP without URL credentials"
        );
        ensure!(!token.is_empty(), "Team token missing");
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            base,
            token: zeroize::Zeroizing::new(token.into()),
            client,
        })
    }
    fn decode<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<T> {
        use std::io::Read;
        ensure!(
            response.status().is_success(),
            "Team server returned {}; pull and review conflicts before retrying",
            response.status()
        );
        let mut bytes = vec![];
        response.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 2 * 1024 * 1024,
            "Team response exceeds 2 MiB"
        );
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub fn catalog(&self) -> Result<Vec<CatalogEntry>> {
        self.decode(
            self.client
                .get(self.base.join("v1/catalog")?)
                .bearer_auth(&*self.token)
                .send()?,
        )
    }
    pub fn pull(&self, id: &str) -> Result<Revision> {
        uuid::Uuid::parse_str(id)?;
        self.decode(
            self.client
                .get(self.base.join(&format!("v1/collections/{id}"))?)
                .bearer_auth(&*self.token)
                .send()?,
        )
    }
    pub fn push(&self, collection: &Collection, revision: &str) -> Result<String> {
        uuid::Uuid::parse_str(&collection.id)?;
        let collection = crate::secrets::redact_collection(collection);
        let bytes = serde_json::to_vec(&collection)?;
        ensure!(
            bytes.len() <= 1024 * 1024,
            "Team collections are limited to 1 MiB"
        );
        let value: serde_json::Value = self.decode(
            self.client
                .put(
                    self.base
                        .join(&format!("v1/collections/{}", collection.id))?,
                )
                .bearer_auth(&*self.token)
                .header("If-Match", revision)
                .header("Content-Type", "application/json")
                .body(bytes)
                .send()?,
        )?;
        Ok(value["revision"]
            .as_str()
            .context("Missing team revision")?
            .into())
    }
}
