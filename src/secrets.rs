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
    ]
    .iter()
    .any(|k| n.contains(k))
}
pub fn redact_request(r: &crate::models::ApiRequest) -> crate::models::ApiRequest {
    let mut r = r.clone();
    r.auth.bearer_token = "[redacted]".into();
    r.auth.basic_password = "[redacted]".into();
    r.auth.api_key_value = "[redacted]".into();
    r.auth.oauth.client_secret = "[redacted]".into();
    r.auth.oauth.code.clear();
    r.auth.oauth.verifier.clear();
    for v in r
        .headers
        .iter_mut()
        .chain(&mut r.query_params)
        .chain(&mut r.form_data)
    {
        if sensitive(&v.key) {
            v.value = "[redacted]".into();
        }
    }
    if let Ok(mut url) = reqwest::Url::parse(&r.url) {
        let pairs: Vec<_> = url
            .query_pairs()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    if sensitive(&k) {
                        "[redacted]".into()
                    } else {
                        v.to_string()
                    },
                )
            })
            .collect();
        if !pairs.is_empty() {
            url.query_pairs_mut().clear().extend_pairs(&pairs);
        }
        let _ = url.set_password(None);
        let _ = url.set_username("");
        r.url = url.to_string();
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
    c
}
