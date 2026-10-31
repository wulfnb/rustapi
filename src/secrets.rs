//! Password-unlocked encrypted local vault. No key or plaintext is written to disk.
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    salt: Vec<u8>,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}
fn key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    anyhow::ensure!(
        password.len() >= 12,
        "Vault password must contain at least 12 characters"
    );
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| anyhow::anyhow!("Deriving vault key failed"))?;
    Ok(key)
}
pub fn save(path: &Path, password: &str, secrets: &BTreeMap<String, String>) -> Result<()> {
    let salt = uuid::Uuid::new_v4().as_bytes().to_vec();
    let nonce = uuid::Uuid::new_v4().as_bytes()[..12].to_vec();
    let cipher = Aes256Gcm::new_from_slice(&key(password, &salt)?)
        .map_err(|_| anyhow::anyhow!("Invalid vault key"))?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            serde_json::to_vec(secrets)?.as_ref(),
        )
        .map_err(|_| anyhow::anyhow!("Encrypting vault failed"))?;
    crate::workspace::atomic_write(
        path,
        &serde_json::to_vec(&Envelope {
            version: 1,
            salt,
            nonce,
            ciphertext,
        })?,
    )
}
pub fn load(path: &Path, password: &str) -> Result<BTreeMap<String, String>> {
    if !path.exists() {
        let _ = key(password, &[0; 16])?;
        return Ok(BTreeMap::new());
    }
    let e: Envelope = serde_json::from_slice(&std::fs::read(path)?)?;
    if e.version != 1 || e.nonce.len() != 12 || e.salt.len() != 16 {
        bail!("Invalid vault format");
    }
    let cipher = Aes256Gcm::new_from_slice(&key(password, &e.salt)?)
        .map_err(|_| anyhow::anyhow!("Invalid vault key"))?;
    let data = cipher
        .decrypt(Nonce::from_slice(&e.nonce), e.ciphertext.as_ref())
        .map_err(|_| anyhow::anyhow!("Vault password incorrect or file damaged"))?;
    serde_json::from_slice(&data).context("Decode vault")
}
pub fn sensitive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [
        "authorization",
        "cookie",
        "token",
        "password",
        "secret",
        "api_key",
        "api-key",
        "apikey",
        "assertion",
    ]
    .iter()
    .any(|k| n.contains(k))
}
/// Keep portable vault references while masking literal credentials.
pub fn mask_credential(value: &str) -> String {
    let reference = value
        .strip_prefix("{{vault:")
        .and_then(|v| v.strip_suffix("}}"));
    if reference.is_some_and(|v| {
        !v.is_empty()
            && v.len() <= 256
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-/.:".contains(&b))
    }) {
        value.into()
    } else {
        "[redacted]".into()
    }
}
pub fn redact_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if sensitive(key) {
                    *value = "[redacted]".into();
                } else {
                    redact_json(value);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                redact_json(value);
            }
        }
        _ => {}
    }
}
fn redact_metadata(map: &mut BTreeMap<String, serde_json::Value>) {
    for (key, value) in map {
        if sensitive(key) {
            *value = "[redacted]".into();
        } else {
            redact_json(value);
        }
    }
}
fn redact_rows(rows: &mut [crate::models::KeyValue]) {
    for row in rows {
        if sensitive(&row.key) || row.value_type.eq_ignore_ascii_case("secret") {
            row.value = mask_credential(&row.value);
        }
        redact_metadata(&mut row.extensions);
        if let Some(metadata) = &mut row.description_metadata {
            redact_metadata(metadata);
        }
    }
}
fn redact_scope(
    values: &mut std::collections::HashMap<String, String>,
    rows: &mut [crate::models::KeyValue],
) {
    let typed: std::collections::HashSet<_> = rows
        .iter()
        .filter(|row| row.value_type.eq_ignore_ascii_case("secret"))
        .map(|row| row.key.clone())
        .collect();
    for (key, value) in values {
        if sensitive(key) || typed.contains(key) {
            *value = "[redacted]".into();
        }
    }
    redact_rows(rows);
}
/// Preserve unrelated raw query text and template references while masking credentials.
pub fn redact_url(input: &str) -> String {
    let mut input = input.to_owned();
    if let Ok(mut url) = reqwest::Url::parse(&input) {
        if !url.username().is_empty() || url.password().is_some() {
            let _ = url.set_password(None);
            let _ = url.set_username("");
            input = url.to_string();
        }
    }
    let (without_fragment, fragment) = input
        .split_once('#')
        .map(|(url, fragment)| (url, Some(fragment)))
        .unwrap_or((&input, None));
    let Some((base, query)) = without_fragment.split_once('?') else {
        return input;
    };
    let query = query
        .split('&')
        .map(|pair| {
            let (key, _) = pair.split_once('=').unwrap_or((pair, ""));
            let decoded = urlencoding::decode(key).unwrap_or_else(|_| key.into());
            if sensitive(&decoded) {
                format!("{key}=%5Bredacted%5D")
            } else {
                pair.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("&");
    format!(
        "{base}?{query}{}",
        fragment
            .map(|fragment| format!("#{fragment}"))
            .unwrap_or_default()
    )
}
pub fn redact_request(r: &crate::models::ApiRequest) -> crate::models::ApiRequest {
    let mut r = r.clone();
    if let Some(graphql) = &mut r.graphql {
        if let Ok(mut variables) = serde_json::from_str::<serde_json::Value>(&graphql.variables) {
            redact_json(&mut variables);
            graphql.variables = variables.to_string();
        }
    }
    r.auth.bearer_token = mask_credential(&r.auth.bearer_token);
    r.auth.basic_password = mask_credential(&r.auth.basic_password);
    r.auth.api_key_value = mask_credential(&r.auth.api_key_value);
    r.auth.jwt.key = mask_credential(&r.auth.jwt.key);
    r.auth.jwt.claims = "{}".into();
    r.auth.aws.access_key = mask_credential(&r.auth.aws.access_key);
    r.auth.aws.secret_key = mask_credential(&r.auth.aws.secret_key);
    r.auth.aws.session_token = mask_credential(&r.auth.aws.session_token);
    r.auth.oauth.client_secret = mask_credential(&r.auth.oauth.client_secret);
    for rows in [
        &mut r.auth.oauth.authorization_params,
        &mut r.auth.oauth.token_params,
        &mut r.auth.oauth.refresh_params,
    ] {
        redact_rows(rows);
        for row in rows {
            if row.key.to_ascii_lowercase().contains("assertion") {
                row.value = mask_credential(&row.value);
            }
        }
    }
    r.auth.oauth.code.clear();
    r.auth.oauth.verifier.clear();
    for v in r
        .headers
        .iter_mut()
        .chain(&mut r.query_params)
        .chain(&mut r.path_params)
        .chain(&mut r.form_data)
    {
        if sensitive(&v.key) || v.value_type.eq_ignore_ascii_case("secret") {
            v.value = "[redacted]".into();
        }
        redact_metadata(&mut v.extensions);
        if let Some(metadata) = &mut v.description_metadata {
            redact_metadata(metadata);
        }
    }
    redact_metadata(&mut r.imported_settings);
    if let Some(metadata) = &mut r.description_metadata {
        redact_metadata(metadata);
    }
    r.url = redact_url(&r.url);
    for endpoint in [
        &mut r.auth.oauth.authorization_url,
        &mut r.auth.oauth.token_url,
        &mut r.auth.oauth.refresh_url,
    ] {
        *endpoint = redact_url(endpoint);
    }
    if r.body_type == crate::models::BodyType::Json {
        fn scrub(v: &mut serde_json::Value) {
            match v {
                serde_json::Value::Object(map) => {
                    for (k, v) in map {
                        if sensitive(k) {
                            *v = "[redacted]".into();
                        } else {
                            scrub(v);
                        }
                    }
                }
                serde_json::Value::Array(a) => {
                    for v in a {
                        scrub(v)
                    }
                }
                _ => {}
            }
        }
        if let Ok(mut body) = serde_json::from_str::<serde_json::Value>(&r.body) {
            scrub(&mut body);
            r.body = body.to_string();
        }
    }
    for example in &mut r.examples {
        *example.request = redact_request(&example.request);
        for (key, value) in &mut example.response.headers {
            if sensitive(key) {
                *value = "[redacted]".into();
            }
        }
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&example.response.body) {
            redact_json(&mut value);
            example.set_text(value.to_string());
        }
        redact_metadata(&mut example.metadata);
    }
    r
}
pub fn redact_collection(c: &crate::models::Collection) -> crate::models::Collection {
    let mut c = c.clone();
    fn walk(c: &mut [crate::models::ApiRequest], folders: &mut [crate::models::Folder]) {
        for r in c {
            *r = redact_request(r);
        }
        for f in folders {
            let r = crate::models::ApiRequest {
                auth: f.auth.clone(),
                ..Default::default()
            };
            f.auth = redact_request(&r).auth;
            for (k, v) in &mut f.variables {
                if sensitive(k) {
                    *v = "[redacted]".into();
                }
            }
            redact_scope(&mut f.variables, &mut f.variable_metadata);
            redact_metadata(&mut f.imported_settings);
            if let Some(metadata) = &mut f.description_metadata {
                redact_metadata(metadata);
            }
            walk(&mut f.requests, &mut f.folders);
        }
    }
    walk(&mut c.requests, &mut c.folders);
    c.auth = redact_request(&crate::models::ApiRequest {
        auth: c.auth.clone(),
        ..Default::default()
    })
    .auth;
    for (k, v) in &mut c.variables {
        if sensitive(k) {
            *v = "[redacted]".into();
        }
    }
    redact_scope(&mut c.variables, &mut c.variable_metadata);
    redact_metadata(&mut c.imported_settings);
    if let Some(metadata) = &mut c.description_metadata {
        redact_metadata(metadata);
    }
    c
}

pub fn redact_environment(environment: &crate::models::Environment) -> crate::models::Environment {
    let mut environment = environment.clone();
    redact_rows(&mut environment.variables);
    environment
}
