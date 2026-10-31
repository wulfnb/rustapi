use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpSettings {
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    pub verify_tls: bool,
    pub use_cookies: bool,
    pub proxy: String,
    pub ca_file: String,
    pub identity_file: String,
    pub max_response_bytes: usize,
}
impl Default for HttpSettings {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            follow_redirects: true,
            verify_tls: true,
            use_cookies: true,
            proxy: String::new(),
            ca_file: String::new(),
            identity_file: String::new(),
            max_response_bytes: 32 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum OAuthClientAuth {
    #[default]
    Body,
    Basic,
    Public,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OAuthConfig {
    pub persist_tokens: bool,
    pub authorization_url: String,
    pub token_url: String,
    pub refresh_url: String,
    pub refresh_scope: String,
    pub client_auth: OAuthClientAuth,
    pub authorization_params: Vec<super::KeyValue>,
    pub token_params: Vec<super::KeyValue>,
    pub refresh_params: Vec<super::KeyValue>,
    pub client_id: String,
    pub client_secret: String,
    pub scope: String,
    pub redirect_uri: String,
    pub grant: String,
    pub code: String,
    pub verifier: String,
    #[serde(skip)]
    pub authorization_requested: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssertionKind {
    Status,
    Header,
    Json,
    BodyContains,
    MaxTime,
    JsonSchema,
    OpenApi,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    pub name: String,
    pub kind: AssertionKind,
    pub target: String,
    pub expected: String,
}
impl Default for Assertion {
    fn default() -> Self {
        Self {
            name: "Status is 200".into(),
            kind: AssertionKind::Status,
            target: String::new(),
            expected: "200".into(),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Extractor {
    pub variable: String,
    pub pointer: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
}
