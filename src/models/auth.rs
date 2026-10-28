//! Authentication configuration models

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

/// Authentication type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AuthType {
    #[default]
    None,
    BearerToken,
    BasicAuth,
    ApiKey,
}

impl AuthType {
    pub fn all() -> &'static [AuthType] {
        &[
            AuthType::None,
            AuthType::BearerToken,
            AuthType::BasicAuth,
            AuthType::ApiKey,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::None => "No Auth",
            AuthType::BearerToken => "Bearer Token",
            AuthType::BasicAuth => "Basic Auth",
            AuthType::ApiKey => "API Key",
        }
    }
}

/// API Key location
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ApiKeyLocation {
    #[default]
    Header,
    QueryParam,
}

impl ApiKeyLocation {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiKeyLocation::Header => "Header",
            ApiKeyLocation::QueryParam => "Query Param",
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub auth_type: AuthType,
    // Bearer Token
    pub bearer_token: String,
    // Basic Auth
    pub basic_username: String,
    pub basic_password: String,
    // API Key
    pub api_key_name: String,
    pub api_key_value: String,
    pub api_key_location: ApiKeyLocation,
}

impl AuthConfig {
    /// Generate authorization header value if applicable
    pub fn get_auth_header(&self) -> Option<(String, String)> {
        match self.auth_type {
            AuthType::None => None,
            AuthType::BearerToken => {
                if self.bearer_token.is_empty() {
                    None
                } else {
                    Some((
                        "Authorization".to_string(),
                        format!("Bearer {}", self.bearer_token),
                    ))
                }
            }
            AuthType::BasicAuth => {
                if self.basic_username.is_empty() && self.basic_password.is_empty() {
                    None
                } else {
                    let credentials = format!("{}:{}", self.basic_username, self.basic_password);
                    let encoded = base64_encode(&credentials);
                    Some(("Authorization".to_string(), format!("Basic {}", encoded)))
                }
            }
            AuthType::ApiKey => {
                if self.api_key_name.is_empty() || self.api_key_value.is_empty() {
                    None
                } else if self.api_key_location == ApiKeyLocation::Header {
                    Some((self.api_key_name.clone(), self.api_key_value.clone()))
                } else {
                    None // Query param is handled separately
                }
            }
        }
    }

    /// Get API key as query parameter if applicable
    pub fn get_api_key_query_param(&self) -> Option<(String, String)> {
        if self.auth_type == AuthType::ApiKey
            && self.api_key_location == ApiKeyLocation::QueryParam
            && !self.api_key_name.is_empty()
            && !self.api_key_value.is_empty()
        {
            Some((self.api_key_name.clone(), self.api_key_value.clone()))
        } else {
            None
        }
    }
}

/// Encode credentials using the standard RFC 4648 Base64 alphabet.
fn base64_encode(input: &str) -> String {
    STANDARD.encode(input)
}
