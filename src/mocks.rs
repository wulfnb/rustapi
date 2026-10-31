//! Local example-backed mocks with explicit ordered matching and bounded faults.
use crate::{
    http::Cancellation,
    local_http::{Request, Response, Server},
    models::*,
};
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MockConfig {
    pub port: u16,
    pub rules: Vec<Rule>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
    pub name: String,
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body_contains: String,
    pub json: BTreeMap<String, serde_json::Value>,
    pub status: u16,
    pub response_headers: Vec<(String, String)>,
    pub body_base64: String,
    pub delay_ms: u64,
    pub disconnect: bool,
}
impl Default for Rule {
    fn default() -> Self {
        Self {
            name: "Example".into(),
            method: "GET".into(),
            path: "/".into(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body_contains: String::new(),
            json: BTreeMap::new(),
            status: 200,
            response_headers: vec![],
            body_base64: String::new(),
            delay_ms: 0,
            disconnect: false,
        }
    }
}
pub fn path_matches(template: &str, path: &str) -> bool {
    let a = template.split('/').collect::<Vec<_>>();
    let b = path.split('/').collect::<Vec<_>>();
    for (index, part) in a.iter().enumerate() {
        if *part == "*" && index + 1 == a.len() {
            return true;
        }
        let Some(actual) = b.get(index) else {
            return false;
        };
        if !(part.starts_with(':') || part.starts_with('{') && part.ends_with('}'))
            && part != actual
        {
            return false;
        }
        if actual.is_empty() && part.starts_with(':') {
            return false;
        }
    }
    a.len() == b.len()
}
impl MockConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.rules.is_empty() && self.rules.len() <= 1000,
            "Mock needs 1–1000 rules"
        );
        for rule in &self.rules {
            ensure!(
                rule.path.starts_with('/') && rule.path.len() <= 4096,
                "Invalid mock path"
            );
            reqwest::Method::from_bytes(rule.method.as_bytes())?;
            ensure!(
                (200..=599).contains(&rule.status) && rule.delay_ms <= 30000,
                "Mock status/delay out of range"
            );
            ensure!(
                rule.body_base64.len() <= 1_398_104
                    && STANDARD.decode(&rule.body_base64)?.len() <= 1024 * 1024,
                "Mock body exceeds 1 MiB"
            );
            for (key, value) in rule
                .response_headers
                .iter()
                .map(|(k, v)| (k, v))
                .chain(rule.headers.iter())
            {
                reqwest::header::HeaderName::from_bytes(key.as_bytes())?;
                reqwest::header::HeaderValue::from_str(value)?;
            }
        }
        Ok(())
    }
    pub fn from_examples(collection: &Collection) -> Result<Self> {
        let source = crate::secrets::redact_collection(collection);
        let mut rules = vec![];
        for request in source.ordered_requests(None) {
            if request.examples.is_empty() {
                continue;
            }
            let vars = crate::variables::Variables {
                collection: request.inherited_variables.clone().into_iter().collect(),
                ..Default::default()
            };
            let mut raw = vars.resolve(&request.url).unwrap_or(request.url.clone());
            if raw.starts_with("{{") {
                if let Some(end) = raw.find("}}") {
                    raw = format!("http://example.invalid{}", &raw[end + 2..]);
                }
            }
            let url = reqwest::Url::parse(&raw)
                .context("Mock request URL needs a resolvable host/base URL")?;
            for example in request.examples {
                let response = example.response()?;
                rules.push(Rule {
                    name: example.name,
                    method: request.method.as_str().into(),
                    path: url.path().into(),
                    query: url
                        .query_pairs()
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect(),
                    status: response.status,
                    response_headers: response.headers,
                    body_base64: STANDARD.encode(response.raw_body),
                    ..Default::default()
                });
            }
        }
        let config = Self { port: 0, rules };
        config.validate()?;
        Ok(config)
    }
}
pub struct MockServer {
    pub server: Server,
    pub requests: Arc<AtomicUsize>,
    pub unmatched: Arc<AtomicUsize>,
    cancel: Cancellation,
}
impl Drop for MockServer {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
impl MockServer {
    pub fn start(config: MockConfig) -> Result<Self> {
        config.validate()?;
        let requests = Arc::new(AtomicUsize::new(0));
        let unmatched = Arc::new(AtomicUsize::new(0));
        let cancel = Cancellation::default();
        let counters = (requests.clone(), unmatched.clone());
        let signal = cancel.clone();
        let server = Server::start(config.port, move |request| {
            counters.0.fetch_add(1, Ordering::Relaxed);
            let url = match reqwest::Url::parse(&format!("http://127.0.0.1{}", request.target)) {
                Ok(url) => url,
                Err(_) => {
                    return Response::json(400, serde_json::json!({"error":"Invalid target"}))
                }
            };
            let selected = config
                .rules
                .iter()
                .find(|rule| matches(rule, &request, &url));
            let Some(rule) = selected else {
                counters.1.fetch_add(1, Ordering::Relaxed);
                return Response::json(404, serde_json::json!({"error":"No matching example"}));
            };
            let until = Instant::now() + Duration::from_millis(rule.delay_ms);
            while Instant::now() < until && !signal.is_cancelled() {
                std::thread::sleep(Duration::from_millis(10));
            }
            if signal.is_cancelled() {
                return Response::json(503, serde_json::json!({"error":"Mock stopped"}));
            }
            Response {
                status: if rule.disconnect { 0 } else { rule.status },
                headers: rule.response_headers.clone(),
                body: if request.method == "HEAD" {
                    vec![]
                } else {
                    STANDARD.decode(&rule.body_base64).unwrap_or_default()
                },
            }
        })?;
        Ok(Self {
            server,
            requests,
            unmatched,
            cancel,
        })
    }
}
fn matches(rule: &Rule, request: &Request, url: &reqwest::Url) -> bool {
    if !rule.method.eq_ignore_ascii_case(&request.method) || !path_matches(&rule.path, url.path()) {
        return false;
    }
    if request
        .headers
        .get("x-rustapi-example")
        .is_some_and(|name| name != &rule.name)
    {
        return false;
    }
    if !rule.query.iter().all(|(key, value)| {
        url.query_pairs()
            .any(|(k, v)| k == key.as_str() && v == value.as_str())
    }) {
        return false;
    }
    if !rule
        .headers
        .iter()
        .all(|(k, v)| request.headers.get(&k.to_ascii_lowercase()) == Some(v))
    {
        return false;
    }
    if !rule.body_contains.is_empty()
        && !request
            .body
            .windows(rule.body_contains.len())
            .any(|part| part == rule.body_contains.as_bytes())
    {
        return false;
    }
    if !rule.json.is_empty() {
        let Ok(body) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
            return false;
        };
        if !rule
            .json
            .iter()
            .all(|(pointer, value)| body.pointer(pointer) == Some(value))
        {
            return false;
        }
    }
    true
}
