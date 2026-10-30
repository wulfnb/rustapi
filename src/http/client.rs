//! Shared, cancellable HTTP service used by GUI sends and collection runs.
use crate::models::*;
use anyhow::{bail, Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
#[derive(Default)]
pub struct CookieJar(Mutex<cookie_store::CookieStore>);
impl reqwest::cookie::CookieStore for CookieJar {
    fn set_cookies(&self, headers: &mut dyn Iterator<Item = &HeaderValue>, url: &reqwest::Url) {
        let mut jar = self.0.lock().unwrap_or_else(|p| p.into_inner());
        for h in headers {
            if let Ok(s) = h.to_str() {
                let _ = jar.parse(s, url);
            }
        }
    }
    fn cookies(&self, url: &reqwest::Url) -> Option<HeaderValue> {
        let jar = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let text = jar
            .get_request_values(url)
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        if text.is_empty() {
            None
        } else {
            HeaderValue::from_str(&text).ok()
        }
    }
}
impl CookieJar {
    pub fn clear(&self) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clear();
    }
    pub fn list(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter_any()
            .map(|c| {
                format!(
                    "{}={}; domain={:?}; path={:?}",
                    c.name(),
                    c.value(),
                    c.domain,
                    c.path
                )
            })
            .collect()
    }
    pub fn set(&self, url: &str, cookie: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .parse(cookie, &reqwest::Url::parse(url)?)
            .map_err(|_| anyhow::anyhow!("Invalid cookie"))?;
        Ok(())
    }
}
#[derive(Default)]
pub struct HttpService {
    clients: Mutex<HashMap<String, Client>>,
    runtime: std::sync::OnceLock<std::result::Result<tokio::runtime::Runtime, String>>,
    pub cookies: Arc<CookieJar>,
    pub(crate) tokens: Mutex<HashMap<String, super::oauth::Token>>,
}
impl HttpService {
    pub fn client(&self, s: &HttpSettings) -> Result<Client> {
        anyhow::ensure!(
            s.timeout_ms > 0 && s.timeout_ms <= 3_600_000,
            "Timeout must be between 1 and 3600000 ms"
        );
        anyhow::ensure!(
            s.max_response_bytes > 0 && s.max_response_bytes <= 256 * 1024 * 1024,
            "Response limit must be between 1 byte and 256 MiB"
        );
        let key = serde_json::to_string(s)?;
        let mut clients = self.clients.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = clients.get(&key) {
            return Ok(c.clone());
        }
        let mut b = Client::builder()
            .use_rustls_tls()
            .danger_accept_invalid_certs(!s.verify_tls)
            .timeout(Duration::from_millis(s.timeout_ms))
            .redirect(if s.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            });
        if s.use_cookies {
            b = b.cookie_provider(self.cookies.clone());
        }
        if !s.proxy.is_empty() {
            b = b.proxy(reqwest::Proxy::all(&s.proxy).context("Invalid proxy")?);
        }
        if !s.ca_file.is_empty() {
            for cert in reqwest::Certificate::from_pem_bundle(
                &std::fs::read(&s.ca_file).context("Read custom CA")?,
            )? {
                b = b.add_root_certificate(cert);
            }
        }
        if !s.identity_file.is_empty() {
            b = b.identity(reqwest::Identity::from_pem(
                &std::fs::read(&s.identity_file).context("Read client certificate/key PEM")?,
            )?);
        }
        let c = b.build()?;
        if clients.len() > 32 {
            clients.clear();
        }
        clients.insert(key, c.clone());
        Ok(c)
    }
    pub fn execute(&self, request: &ApiRequest, cancel: &Cancellation) -> ApiResponse {
        let start = Instant::now();
        let run = || -> Result<ApiResponse> {
            if cancel.is_cancelled() {
                bail!("Request cancelled");
            }
            let rt = self
                .runtime
                .get_or_init(|| {
                    tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .enable_all()
                        .build()
                        .map_err(|e| e.to_string())
                })
                .as_ref()
                .map_err(|e| anyhow::anyhow!(e.clone()))?;
            rt.block_on(async{
                tokio::select! {
                    biased;
                    _=async {while !cancel.is_cancelled(){tokio::time::sleep(Duration::from_millis(20)).await;}}=>bail!("Request cancelled"),
                    result=self.execute_async(request,start)=>result,
                }
            })
        };
        match run() {
            Ok(r) => r,
            Err(e) => ApiResponse {
                time_ms: start.elapsed().as_millis(),
                error: Some(format!("Request failed: {e:#}")),
                ..Default::default()
            },
        }
    }
    async fn execute_async(&self, r: &ApiRequest, start: Instant) -> Result<ApiResponse> {
        let client = self.client(&r.settings)?;
        let mut r = r.clone();
        if r.auth.auth_type == AuthType::OAuth2 {
            r.auth.bearer_token = super::oauth::access_token(self, &client, &r.auth.oauth).await?;
            r.auth.auth_type = AuthType::BearerToken;
        }
        if matches!(r.auth.auth_type, AuthType::Inherit | AuthType::Unsupported) {
            bail!("No inherited authentication context; choose an explicit auth type");
        }
        let mut params = r.query_params.clone();
        if let Some((k, v)) = r.auth.get_api_key_query_param() {
            params.push(KeyValue::new(k, v));
        }
        let path_url = crate::variables::resolve_path_variables(&r.url, &r.path_params)?;
        let url = build_url_with_params(&path_url, &params, None);
        let mut headers = HeaderMap::new();
        if let Some((k, v)) = r.auth.get_auth_header() {
            headers.insert(
                HeaderName::try_from(k.as_str()).context("Invalid auth header name")?,
                HeaderValue::try_from(v).context("Invalid auth header value")?,
            );
        }
        for h in &r.headers {
            if h.enabled && !h.key.is_empty() {
                headers.insert(
                    HeaderName::try_from(h.key.as_str()).context("Invalid header name")?,
                    HeaderValue::try_from(h.value.as_str()).context("Invalid header value")?,
                );
            }
        }
        let mut b = client.request(
            reqwest::Method::from_bytes(r.method.as_str().as_bytes())?,
            &url,
        );
        match r.body_type {
            BodyType::None => {}
            BodyType::Json | BodyType::Text => {
                if !headers.contains_key("content-type") {
                    headers.insert(
                        reqwest::header::CONTENT_TYPE,
                        HeaderValue::from_static(if r.body_type == BodyType::Json {
                            "application/json"
                        } else {
                            "text/plain"
                        }),
                    );
                }
                b = b.body(r.body);
            }
            BodyType::Binary => {
                let bytes = if r.binary_file.is_empty() {
                    r.body.into_bytes()
                } else {
                    std::fs::read(&r.binary_file).context("Read binary upload")?
                };
                if !headers.contains_key("content-type") {
                    headers.insert(
                        reqwest::header::CONTENT_TYPE,
                        HeaderValue::from_static("application/octet-stream"),
                    );
                }
                b = b.body(bytes);
            }
            BodyType::UrlEncoded => {
                b = b.form(
                    &r.form_data
                        .iter()
                        .filter(|v| v.enabled && !v.key.is_empty())
                        .map(|v| (v.key.clone(), v.value.clone()))
                        .collect::<Vec<_>>(),
                );
            }
            BodyType::FormData => {
                // The encoder owns Content-Type and its boundary.
                headers.remove(reqwest::header::CONTENT_TYPE);
                let mut form = reqwest::multipart::Form::new();
                for v in &r.form_data {
                    if v.enabled && !v.key.is_empty() {
                        let mut part = if v.file {
                            let bytes = std::fs::read(&v.value).context("Read multipart upload")?;
                            let name = std::path::Path::new(&v.value)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();
                            reqwest::multipart::Part::bytes(bytes).file_name(name)
                        } else {
                            reqwest::multipart::Part::text(v.value.clone())
                        };
                        if !v.content_type.is_empty() {
                            part = part
                                .mime_str(&v.content_type)
                                .context("Invalid part content type")?;
                        }
                        form = form.part(v.key.clone(), part);
                    }
                }
                b = b.multipart(form);
            }
        }
        let mut response = b
            .headers(headers)
            .send()
            .await
            .context("Send HTTP request")?;
        let status = response.status();
        let headers_time_ms = start.elapsed().as_millis();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
            .collect();
        let mut raw_body = vec![];
        while let Some(chunk) = response.chunk().await.context("Read response body")? {
            if raw_body.len().saturating_add(chunk.len()) > r.settings.max_response_bytes {
                bail!(
                    "Response exceeds configured {} byte limit",
                    r.settings.max_response_bytes
                );
            }
            raw_body.extend_from_slice(&chunk);
        }
        let size = raw_body.len();
        let preview = &raw_body[..raw_body.len().min(1024 * 1024)];
        let body = String::from_utf8_lossy(preview).into_owned();
        Ok(ApiResponse {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("Unknown").into(),
            headers,
            body,
            size,
            time_ms: start.elapsed().as_millis(),
            headers_time_ms,
            raw_body,
            error: None,
            tests: vec![],
        })
    }
}
/// Compatibility entry point; interactive sends and runs reuse an HttpService.
pub fn execute_request(request: &ApiRequest, environment: Option<&Environment>) -> ApiResponse {
    let vars = crate::variables::Variables::for_request(request, environment);
    match vars.resolve_request(request) {
        Ok(r) => HttpService::default().execute(&r, &Cancellation::default()),
        Err(e) => ApiResponse::error(e.to_string()),
    }
}
/// Build URL with query parameters
pub(crate) fn build_url_with_params(
    base_url: &str,
    params: &[KeyValue],
    environment: Option<&Environment>,
) -> String {
    let enabled_params: Vec<_> = params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .collect();

    if enabled_params.is_empty() {
        return base_url.to_string();
    }

    let query_string: String = enabled_params
        .iter()
        .map(|p| {
            let value = if let Some(env) = environment {
                env.substitute(&p.value)
            } else {
                p.value.clone()
            };
            format!(
                "{}={}",
                urlencoding::encode(&p.key),
                urlencoding::encode(&value)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    let (base, fragment) = base_url.split_once('#').unwrap_or((base_url, ""));
    let separator = if base.ends_with('?') || (base.contains('?') && base.ends_with('&')) {
        ""
    } else if base.contains('?') {
        "&"
    } else {
        "?"
    };
    let suffix = if base_url.contains('#') {
        format!("#{fragment}")
    } else {
        String::new()
    };
    format!("{base}{separator}{query_string}{suffix}")
}

/// Format JSON body for display (pretty print)
pub fn format_json(json_str: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_else(|_| json_str.to_string()),
        Err(_) => json_str.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! query_case {
        ($name:ident, $base:expr, $expected:expr) => {
            #[test]
            fn $name() {
                assert_eq!(
                    build_url_with_params($base, &[KeyValue::new("key", "a b")], None),
                    $expected
                );
            }
        };
    }
    query_case!(
        query_on_plain_url,
        "https://test/path",
        "https://test/path?key=a%20b"
    );
    query_case!(
        query_before_fragment,
        "https://test/path#section",
        "https://test/path?key=a%20b#section"
    );
    query_case!(
        query_after_existing_param,
        "https://test/path?first=1",
        "https://test/path?first=1&key=a%20b"
    );
    query_case!(
        query_after_empty_query,
        "https://test/path?",
        "https://test/path?key=a%20b"
    );
    query_case!(
        query_after_trailing_separator,
        "https://test/path?first=1&",
        "https://test/path?first=1&key=a%20b"
    );
    query_case!(
        ampersand_in_path_is_not_a_query,
        "https://test/path&",
        "https://test/path&?key=a%20b"
    );

    #[test]
    fn no_enabled_params_preserve_url_exactly() {
        let base = "https://test/path?existing=1#fragment";
        assert_eq!(build_url_with_params(base, &[], None), base);
        assert_eq!(
            build_url_with_params(
                base,
                &[KeyValue {
                    enabled: false,
                    ..KeyValue::new("key", "value")
                }],
                None
            ),
            base
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/http.rs"]
mod request_tests;

#[cfg(test)]
mod configuration_tests {
    use super::*;
    #[test]
    fn client_configuration_failure_becomes_a_transport_error() {
        let r = ApiRequest {
            url: "http://127.0.0.1:1".into(),
            settings: HttpSettings {
                timeout_ms: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(HttpService::default()
            .execute(&r, &Cancellation::default())
            .error
            .unwrap()
            .contains("Timeout must"));
    }
}
