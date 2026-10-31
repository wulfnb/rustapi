//! Explicit, paired browser capture/sync. No browser databases are opened.
use super::CookieJar;
use crate::local_http::{Request, Response, Server};
use anyhow::Result;
use std::sync::Arc;
pub struct CookieBridge {
    pub server: Server,
    pub pairing: String,
}
impl CookieBridge {
    pub fn start(jar: Arc<CookieJar>) -> Result<Self> {
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let expected = token.clone();
        let server = Server::start(0, move |request| handle(request, &expected, &jar))?;
        let pairing = format!("http://{}/#{}", server.address, token);
        Ok(Self { server, pairing })
    }
}
pub fn export_connector(parent: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    let directory = parent.join(format!(
        "rustapi-cookie-connector-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir(&directory)?;
    for (name, content) in [
        (
            "manifest.json",
            include_str!("../../browser-connector/manifest.json"),
        ),
        (
            "popup.html",
            include_str!("../../browser-connector/popup.html"),
        ),
        ("popup.js", include_str!("../../browser-connector/popup.js")),
        (
            "README.md",
            include_str!("../../browser-connector/README.md"),
        ),
    ] {
        crate::workspace::atomic_write(&directory.join(name), content.as_bytes())?;
    }
    Ok(directory)
}
fn extension_origin(value: &str) -> bool {
    ["chrome-extension://", "moz-extension://"]
        .iter()
        .any(|prefix| {
            value.strip_prefix(prefix).is_some_and(|id| {
                !id.is_empty()
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
        })
}
fn handle(request: Request, token: &str, jar: &CookieJar) -> Response {
    let origin = request
        .headers
        .get("origin")
        .map(String::as_str)
        .unwrap_or_default();
    if !extension_origin(origin) {
        return Response::json(
            403,
            serde_json::json!({"error":"Browser extension origin required"}),
        );
    }
    let mut response = if request.method == "OPTIONS" {
        Response::json(200, serde_json::json!({}))
    } else if request
        .headers
        .get("authorization")
        .is_none_or(|value| value != &format!("Bearer {token}"))
    {
        Response::json(401, serde_json::json!({"error":"Pairing required"}))
    } else {
        let result = (|| -> Result<serde_json::Value> {
            let url = reqwest::Url::parse(&format!("http://127.0.0.1{}", request.target))?;
            anyhow::ensure!(url.path() == "/cookies", "Unknown bridge endpoint");
            match request.method.as_str() {
                "GET" => {
                    let origin = url
                        .query_pairs()
                        .find(|(k, _)| k == "origin")
                        .map(|(_, v)| v.into_owned())
                        .ok_or_else(|| anyhow::anyhow!("Capture origin missing"))?;
                    Ok(serde_json::json!({"cookies":jar.for_origin(&origin)?}))
                }
                "POST" => {
                    #[derive(serde::Deserialize)]
                    struct Capture {
                        origin: String,
                        cookies: Vec<super::BrowserCookie>,
                    }
                    let capture: Capture = serde_json::from_slice(&request.body)?;
                    let count = jar.import_browser(&capture.origin, &capture.cookies)?;
                    Ok(serde_json::json!({"count":count}))
                }
                _ => anyhow::bail!("Unsupported bridge method"),
            }
        })();
        match result {
            Ok(value) => Response::json(200, value),
            Err(_) => Response::json(
                400,
                serde_json::json!({"error":"Invalid cookie capture; check origin, cookie attributes and partitioning"}),
            ),
        }
    };
    response.headers.extend([
        ("Access-Control-Allow-Origin".into(), origin.into()),
        ("Vary".into(), "Origin".into()),
        (
            "Access-Control-Allow-Methods".into(),
            "GET, POST, OPTIONS".into(),
        ),
        (
            "Access-Control-Allow-Headers".into(),
            "authorization, content-type".into(),
        ),
    ]);
    response
}
