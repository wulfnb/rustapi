//! Data models for API requests, collections, and environments

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// HTTP Methods supported by the client
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HttpMethod {
    #[default]
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
}

impl HttpMethod {
    pub fn all() -> &'static [HttpMethod] {
        &[
            HttpMethod::GET,
            HttpMethod::POST,
            HttpMethod::PUT,
            HttpMethod::DELETE,
            HttpMethod::PATCH,
            HttpMethod::HEAD,
            HttpMethod::OPTIONS,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::PATCH => "PATCH",
            HttpMethod::HEAD => "HEAD",
            HttpMethod::OPTIONS => "OPTIONS",
        }
    }

    pub fn color(&self) -> egui::Color32 {
        match self {
            HttpMethod::GET => egui::Color32::from_rgb(97, 175, 239),    // Blue
            HttpMethod::POST => egui::Color32::from_rgb(152, 195, 121),  // Green
            HttpMethod::PUT => egui::Color32::from_rgb(229, 192, 123),   // Yellow
            HttpMethod::DELETE => egui::Color32::from_rgb(224, 108, 117),// Red
            HttpMethod::PATCH => egui::Color32::from_rgb(198, 120, 221), // Purple
            HttpMethod::HEAD => egui::Color32::from_rgb(86, 182, 194),   // Cyan
            HttpMethod::OPTIONS => egui::Color32::from_rgb(190, 190, 190),// Gray
        }
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Body content type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BodyType {
    #[default]
    None,
    Json,
    Text,
    FormData,
    UrlEncoded,
    Binary,
}

impl BodyType {
    pub fn all() -> &'static [BodyType] {
        &[
            BodyType::None,
            BodyType::Json,
            BodyType::Text,
            BodyType::FormData,
            BodyType::UrlEncoded,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BodyType::None => "None",
            BodyType::Json => "JSON",
            BodyType::Text => "Text",
            BodyType::FormData => "Form Data",
            BodyType::UrlEncoded => "URL Encoded",
            BodyType::Binary => "Binary",
        }
    }

    pub fn content_type(&self) -> Option<&'static str> {
        match self {
            BodyType::None => None,
            BodyType::Json => Some("application/json"),
            BodyType::Text => Some("text/plain"),
            BodyType::FormData => Some("multipart/form-data"),
            BodyType::UrlEncoded => Some("application/x-www-form-urlencoded"),
            BodyType::Binary => Some("application/octet-stream"),
        }
    }
}

/// A key-value pair with enabled flag
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub enabled: bool,
    pub description: String,
}

impl KeyValue {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
            description: String::new(),
        }
    }
}

/// An API request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub id: String,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub query_params: Vec<KeyValue>,
    pub body_type: BodyType,
    pub body: String,
    pub form_data: Vec<KeyValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for ApiRequest {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::from("New Request"),
            method: HttpMethod::GET,
            url: String::new(),
            headers: vec![],
            query_params: vec![],
            body_type: BodyType::None,
            body: String::new(),
            form_data: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl ApiRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn with_method(mut self, method: HttpMethod) -> Self {
        self.method = method;
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }
}

/// A folder containing requests
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub requests: Vec<ApiRequest>,
    pub folders: Vec<Folder>,
    pub expanded: bool,
}

impl Folder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            requests: vec![],
            folders: vec![],
            expanded: true,
        }
    }
}

/// A collection of API requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub requests: Vec<ApiRequest>,
    pub folders: Vec<Folder>,
    pub variables: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub file_path: Option<String>,
    #[serde(skip)]
    pub expanded: bool,
}

impl Default for Collection {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::from("New Collection"),
            description: String::new(),
            version: String::from("1.0.0"),
            requests: vec![],
            folders: vec![],
            variables: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            file_path: None,
            expanded: true,
        }
    }
}

impl Collection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

/// An environment with variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub variables: Vec<KeyValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::from("New Environment"),
            variables: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Environment {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Get a variable value by key
    pub fn get(&self, key: &str) -> Option<&str> {
        self.variables
            .iter()
            .find(|kv| kv.enabled && kv.key == key)
            .map(|kv| kv.value.as_str())
    }

    /// Substitute variables in a string using {{variable}} syntax
    pub fn substitute(&self, input: &str) -> String {
        let mut result = input.to_string();
        for kv in &self.variables {
            if kv.enabled {
                let pattern = format!("{{{{{}}}}}", kv.key);
                result = result.replace(&pattern, &kv.value);
            }
        }
        result
    }
}

/// HTTP Response from a request
#[derive(Debug, Clone, Default)]
pub struct ApiResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub size: usize,
    pub time_ms: u128,
    pub error: Option<String>,
}

impl ApiResponse {
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            error: Some(msg.into()),
            ..Default::default()
        }
    }

    pub fn status_color(&self) -> egui::Color32 {
        match self.status {
            200..=299 => egui::Color32::from_rgb(152, 195, 121), // Green
            300..=399 => egui::Color32::from_rgb(229, 192, 123), // Yellow
            400..=499 => egui::Color32::from_rgb(224, 108, 117), // Red
            500..=599 => egui::Color32::from_rgb(198, 120, 221), // Purple
            _ => egui::Color32::GRAY,
        }
    }
}

/// Application state for persistence
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub collections: Vec<Collection>,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<String>,
    pub recent_files: Vec<String>,
    pub window_size: Option<(f32, f32)>,
}

