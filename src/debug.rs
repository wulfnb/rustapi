//! Session-local bounded diagnostics. Request bodies and transport header values are never recorded.
use crate::{models::*, variables::Variables};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::VecDeque, sync::Mutex};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugEvent {
    pub time: String,
    pub kind: String,
    pub level: String,
    pub details: Value,
}
#[derive(Default)]
pub struct DebugLog(Mutex<VecDeque<DebugEvent>>);
impl DebugLog {
    pub fn push(&self, kind: &str, level: &str, details: Value) {
        let details = if serde_json::to_vec(&details).map_or(true, |bytes| bytes.len() > 4096) {
            json!("[large diagnostic omitted]")
        } else {
            details
        };
        let mut queue = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if queue.len() >= 256 {
            queue.pop_front();
        }
        queue.push_back(DebugEvent {
            time: chrono::Utc::now().to_rfc3339(),
            kind: kind.into(),
            level: level.into(),
            details,
        });
    }
    pub fn snapshot(&self) -> Vec<DebugEvent> {
        self.0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .iter()
            .cloned()
            .collect()
    }
    pub fn clear(&self) {
        self.0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clear();
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptLog {
    pub level: String,
    pub args: Vec<Value>,
}
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ScriptFailure {
    pub message: String,
    pub logs: Vec<ScriptLog>,
}
pub struct LogRedactor {
    values: Vec<String>,
    mask_all: bool,
}
impl LogRedactor {
    pub fn new(
        vars: &Variables,
        request: Option<&ApiRequest>,
        response: Option<&ApiResponse>,
    ) -> Self {
        let mut redactor = Self {
            values: vec![],
            mask_all: false,
        };
        for value in vars.merged().values() {
            redactor.add(value);
        }
        if let Some(request) = request {
            for row in request
                .headers
                .iter()
                .chain(&request.query_params)
                .chain(&request.path_params)
                .chain(&request.form_data)
            {
                redactor.add(&row.value);
            }
            for value in [
                &request.auth.bearer_token,
                &request.auth.basic_username,
                &request.auth.basic_password,
                &request.auth.api_key_value,
                &request.auth.oauth.client_secret,
                &request.auth.oauth.code,
                &request.auth.oauth.verifier,
            ] {
                redactor.add(value);
            }
            redactor.add_json(
                &serde_json::from_str(&request.body).unwrap_or(Value::String(request.body.clone())),
            );
            if let Ok(url) = reqwest::Url::parse(&request.url) {
                for (_, value) in url.query_pairs() {
                    redactor.add(&value);
                }
                redactor.add(url.password().unwrap_or_default());
            }
        }
        if let Some(response) = response {
            for (_, value) in &response.headers {
                redactor.add(value);
            }
            redactor.add_json(
                &serde_json::from_str(&response.body)
                    .unwrap_or(Value::String(response.body.clone())),
            );
        }
        redactor
    }
    fn add(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        if self.values.len() >= 5000 {
            self.mask_all = true;
            return;
        }
        self.values.push(value.chars().take(64).collect());
    }
    pub fn add_json(&mut self, value: &Value) {
        match value {
            Value::String(value) => self.add(value),
            Value::Number(value) => self.add(&value.to_string()),
            Value::Bool(value) => self.add(&value.to_string()),
            Value::Object(map) => {
                for value in map.values() {
                    self.add_json(value);
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.add_json(value);
                }
            }
            _ => (),
        }
    }
    fn redact(&self, value: &mut Value, depth: usize, left: &mut usize) {
        if depth > 8 || *left == 0 {
            *value = json!("[bounded log omitted]");
            return;
        }
        *left -= 1;
        match value {
            Value::Object(map) => {
                for (index, (key, mut item)) in std::mem::take(map).into_iter().enumerate() {
                    if crate::secrets::sensitive(&key) {
                        item = json!("[redacted]");
                    } else {
                        self.redact(&mut item, depth + 1, left);
                    }
                    let key =
                        if self.mask_all || self.values.iter().any(|value| key.contains(value)) {
                            format!("[redacted key {index}]")
                        } else {
                            key
                        };
                    map.insert(key, item);
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.redact(value, depth + 1, left);
                }
            }
            Value::String(text) => {
                if let Ok(mut parsed) = serde_json::from_str::<Value>(text) {
                    if parsed.is_object() || parsed.is_array() {
                        self.redact(&mut parsed, depth + 1, left);
                        *text = parsed.to_string();
                        return;
                    }
                }
                if self.mask_all || self.values.iter().any(|value| text.contains(value)) {
                    *text = "[redacted]".into();
                }
            }
            Value::Number(number)
                if self.mask_all
                    || self.values.iter().any(|value| value == &number.to_string()) =>
            {
                *value = json!("[redacted]");
            }
            Value::Bool(boolean)
                if self.mask_all
                    || self
                        .values
                        .iter()
                        .any(|value| value == &boolean.to_string()) =>
            {
                *value = json!("[redacted]");
            }
            _ => (),
        }
    }
    pub fn logs(&self, raw: &Value) -> Vec<ScriptLog> {
        raw.as_array()
            .into_iter()
            .flatten()
            .take(100)
            .map(|log| {
                let mut args = log["args"].clone();
                self.redact(&mut args, 0, &mut 200);
                ScriptLog {
                    level: match log["level"].as_str() {
                        Some("warn") => "warn",
                        Some("error") => "error",
                        _ => "log",
                    }
                    .into(),
                    args: args.as_array().cloned().unwrap_or_default(),
                }
            })
            .collect()
    }
}
pub fn safe_url(url: &str) -> String {
    reqwest::Url::parse(url)
        .map(|url| format!("{}/[path omitted]", url.origin().ascii_serialization()))
        .unwrap_or_else(|_| "[invalid or unresolved URL]".into())
}
