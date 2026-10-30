//! API Request model

use super::auth::AuthConfig;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
            HttpMethod::GET => egui::Color32::from_rgb(97, 175, 239), // Blue
            HttpMethod::POST => egui::Color32::from_rgb(152, 195, 121), // Green
            HttpMethod::PUT => egui::Color32::from_rgb(229, 192, 123), // Yellow
            HttpMethod::DELETE => egui::Color32::from_rgb(224, 108, 117), // Red
            HttpMethod::PATCH => egui::Color32::from_rgb(198, 120, 221), // Purple
            HttpMethod::HEAD => egui::Color32::from_rgb(86, 182, 194), // Cyan
            HttpMethod::OPTIONS => egui::Color32::from_rgb(190, 190, 190), // Gray
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
            BodyType::Binary,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_metadata: Option<super::DescriptionMetadata>,
    #[serde(default)]
    pub file: bool,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub value_type: String,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extensions: std::collections::BTreeMap<String, serde_json::Value>,
}

impl KeyValue {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
            description: String::new(),
            description_metadata: None,
            file: false,
            content_type: String::new(),
            value_type: String::new(),
            extensions: Default::default(),
        }
    }
}

/// An API request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_metadata: Option<super::DescriptionMetadata>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub imported_settings: std::collections::BTreeMap<String, serde_json::Value>,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub query_params: Vec<KeyValue>,
    #[serde(default)]
    pub path_params: Vec<KeyValue>,
    pub body_type: BodyType,
    pub body: String,
    pub form_data: Vec<KeyValue>,
    pub auth: AuthConfig,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub settings: super::HttpSettings,
    #[serde(default)]
    pub checks: Vec<super::Assertion>,
    #[serde(default)]
    pub extractors: Vec<super::Extractor>,
    #[serde(default)]
    pub pre_script: String,
    #[serde(default)]
    pub post_script: String,
    #[serde(default)]
    pub binary_file: String,
    #[serde(skip)]
    pub inherited_variables: std::collections::HashMap<String, String>,
    #[serde(skip)]
    pub file_base: std::path::PathBuf,
}

impl Default for ApiRequest {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::from("New Request"),
            description: String::new(),
            description_metadata: None,
            imported_settings: Default::default(),
            method: HttpMethod::GET,
            url: String::new(),
            headers: vec![],
            query_params: vec![],
            path_params: vec![],
            body_type: BodyType::None,
            body: String::new(),
            form_data: vec![],
            auth: AuthConfig::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            settings: Default::default(),
            checks: vec![],
            extractors: vec![],
            pre_script: String::new(),
            post_script: String::new(),
            binary_file: String::new(),
            inherited_variables: Default::default(),
            file_base: Default::default(),
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

    /// Create a duplicate of this request with a new ID
    pub fn duplicate(&self) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: format!("{} (copy)", self.name),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            ..self.clone()
        }
    }
}
