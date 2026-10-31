//! Workspace jar, encrypted opt-in persistence and browser cookie exchange.
use anyhow::{Context, Result};
use reqwest::{header::HeaderValue, Url};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};
#[derive(Default)]
pub struct CookieJar(Mutex<State>);
#[derive(Default)]
struct State {
    jar: cookie_store::CookieStore,
    persistence: Option<(PathBuf, zeroize::Zeroizing<String>)>,
    error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub host_only: bool,
    #[serde(default)]
    pub same_site: String,
    #[serde(default)]
    pub expiration_date: Option<f64>,
    #[serde(default)]
    pub partition_key: Option<serde_json::Value>,
}
fn origin(value: &str) -> Result<Url> {
    let url = Url::parse(value)?;
    anyhow::ensure!(
        ["http", "https"].contains(&url.scheme())
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none(),
        "Cookie origin must be HTTP(S) without credentials"
    );
    Ok(url)
}
fn persist(state: &State) -> Result<()> {
    if let Some((path, password)) = &state.persistence {
        let mut json = vec![];
        cookie_store::serde::json::save_incl_expired_and_nonpersistent(&state.jar, &mut json)
            .map_err(|_| anyhow::anyhow!("Encode cookie jar"))?;
        crate::secrets::save(
            path,
            password,
            &BTreeMap::from([("cookies".into(), String::from_utf8(json)?)]),
        )?;
    }
    Ok(())
}
impl reqwest::cookie::CookieStore for CookieJar {
    fn set_cookies(&self, headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        for header in headers {
            if let Ok(text) = header.to_str() {
                let _ = state.jar.parse(text, url);
            }
        }
        state.error = persist(&state)
            .err()
            .map(|_| "Could not save encrypted cookie jar".into());
    }
    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let text = state
            .jar
            .get_request_values(url)
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        if text.is_empty() {
            None
        } else {
            let mut value = HeaderValue::from_str(&text).ok()?;
            value.set_sensitive(true);
            Some(value)
        }
    }
}
impl CookieJar {
    pub fn unlock(&self, path: &Path, password: &str) -> Result<()> {
        anyhow::ensure!(
            !path.exists() || std::fs::metadata(path)?.len() <= 8 * 1024 * 1024,
            "Cookie jar exceeds 8 MiB"
        );
        let values = crate::secrets::load(path, password)?;
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let jar = values
            .get("cookies")
            .map(|text| {
                cookie_store::serde::json::load(text.as_bytes())
                    .map_err(|_| anyhow::anyhow!("Invalid encrypted cookie jar"))
            })
            .transpose()?;
        if let Some(jar) = jar {
            state.jar = jar;
        }
        state.persistence = Some((path.into(), zeroize::Zeroizing::new(password.into())));
        persist(&state)?;
        Ok(())
    }
    pub fn lock(&self) {
        let mut s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        s.persistence = None;
        s.jar.clear();
        s.error = None;
    }
    pub fn persistent(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .persistence
            .is_some()
    }
    pub fn last_error(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .error
            .clone()
    }
    pub fn clear(&self) {
        let mut s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        s.jar.clear();
        s.error = persist(&s)
            .err()
            .map(|_| "Could not save cleared cookie jar".into());
    }
    pub fn list(&self) -> Vec<String> {
        self.entries()
            .into_iter()
            .map(|c| {
                format!(
                    "{}={}; domain={}; path={}; Secure={}; HttpOnly={}; SameSite={}",
                    c.name, c.value, c.domain, c.path, c.secure, c.http_only, c.same_site
                )
            })
            .collect()
    }
    pub fn entries(&self) -> Vec<BrowserCookie> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .jar
            .iter_unexpired()
            .map(|c| BrowserCookie {
                name: c.name().into(),
                value: c.value().into(),
                domain: c.domain.as_cow().unwrap_or_default().into_owned(),
                path: c.path.to_string(),
                secure: c.secure().unwrap_or(false),
                http_only: c.http_only().unwrap_or(false),
                host_only: matches!(c.domain, cookie_store::CookieDomain::HostOnly(_)),
                same_site: c
                    .same_site()
                    .map(|s| s.to_string().to_lowercase())
                    .unwrap_or_default(),
                expiration_date: match c.expires {
                    cookie_store::CookieExpiration::AtUtc(time) => {
                        Some(time.unix_timestamp() as f64)
                    }
                    _ => None,
                },
                partition_key: None,
            })
            .collect()
    }
    pub fn for_origin(&self, value: &str) -> Result<Vec<BrowserCookie>> {
        let url = origin(value)?;
        let domains = self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .jar
            .iter_unexpired()
            .filter(|c| c.domain.matches(&url))
            .map(|c| {
                (
                    c.name().to_owned(),
                    c.domain.as_cow().unwrap_or_default().into_owned(),
                    c.path.to_string(),
                )
            })
            .collect::<Vec<_>>();
        Ok(self
            .entries()
            .into_iter()
            .filter(|c| domains.contains(&(c.name.clone(), c.domain.clone(), c.path.clone())))
            .collect())
    }
    pub fn set(&self, value: &str, cookie: &str) -> Result<()> {
        let url = origin(value)?;
        anyhow::ensure!(
            cookie.len() <= 16384 && !cookie.bytes().any(|b| b < 32 || b == 127),
            "Invalid cookie text"
        );
        let mut s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        // Expired is a successful deletion in cookie_store.
        match s.jar.parse(cookie, &url) {
            Ok(_) | Err(cookie_store::CookieError::Expired) => {}
            Err(_) => anyhow::bail!("Invalid cookie for this origin"),
        }
        persist(&s)
    }
    pub fn remove(&self, domain: &str, path: &str, name: &str) -> Result<()> {
        let mut s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        s.jar.remove(domain, path, name);
        persist(&s)
    }
    /// Merge a browser snapshot transactionally; never accept cookies for unrelated domains.
    pub fn import_browser(&self, value: &str, cookies: &[BrowserCookie]) -> Result<usize> {
        let url = origin(value)?;
        anyhow::ensure!(cookies.len() <= 1000, "At most 1000 cookies per capture");
        let mut s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let mut next = s.jar.clone();
        for c in cookies {
            anyhow::ensure!(
                c.partition_key.is_none(),
                "Partitioned browser cookies are unsupported"
            );
            anyhow::ensure!(
                !c.name.is_empty()
                    && !c
                        .name
                        .bytes()
                        .any(|b| b <= 32 || b >= 127 || b"()<>@,;:\\\"/[]?={}".contains(&b)),
                "Invalid cookie name"
            );
            anyhow::ensure!(
                [c.value.as_str(), c.path.as_str(), c.domain.as_str()]
                    .iter()
                    .all(
                        |v| v.len() <= 16384 && !v.bytes().any(|b| b < 32 || b == 127 || b == b';')
                    ),
                "Invalid cookie field"
            );
            anyhow::ensure!(c.path.starts_with('/'), "Cookie path must start with /");
            if c.host_only {
                anyhow::ensure!(
                    Some(c.domain.trim_start_matches('.')) == url.host_str(),
                    "Host-only cookie belongs to another host"
                );
            }
            let mut text = format!("{}={}; Path={}", c.name, c.value, c.path);
            if !c.host_only {
                text.push_str(&format!("; Domain={}", c.domain));
            }
            if c.secure {
                text.push_str("; Secure");
            }
            if c.http_only {
                text.push_str("; HttpOnly");
            }
            match c.same_site.as_str() {
                "strict" => text.push_str("; SameSite=Strict"),
                "lax" => text.push_str("; SameSite=Lax"),
                "none" | "no_restriction" => text.push_str("; SameSite=None"),
                "" | "unspecified" => {}
                _ => anyhow::bail!("Invalid SameSite"),
            }
            if let Some(time) = c.expiration_date {
                anyhow::ensure!(time.is_finite(), "Invalid cookie expiration");
                let date = chrono::DateTime::from_timestamp(time as i64, 0)
                    .context("Cookie expiration out of range")?;
                text.push_str(&format!(
                    "; Expires={}",
                    date.format("%a, %d %b %Y %H:%M:%S GMT")
                ));
            }
            match next.parse(&text, &url) {
                Ok(_) | Err(cookie_store::CookieError::Expired) => {}
                Err(_) => anyhow::bail!("Cookie does not match the capture origin"),
            }
        }
        let previous = std::mem::replace(&mut s.jar, next);
        if let Err(error) = persist(&s) {
            s.jar = previous;
            return Err(error);
        }
        Ok(cookies.len())
    }
}
