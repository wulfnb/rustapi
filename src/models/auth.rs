//! Authentication configuration models

use serde::{Deserialize, Serialize};
use std::io::Write;

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

/// Simple base64 encoding
fn base64_encode(input: &str) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    {
        let mut encoder = Base64Encoder::new(&mut buf);
        encoder.write_all(input.as_bytes()).unwrap();
    }
    String::from_utf8(buf).unwrap()
}

/// Simple base64 encoder
struct Base64Encoder<W: std::io::Write> {
    writer: W,
    buffer: [u8; 3],
    buffer_len: usize,
}

impl<W: std::io::Write> Base64Encoder<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            buffer: [0; 3],
            buffer_len: 0,
        }
    }
}

impl<W: std::io::Write> std::io::Write for Base64Encoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        let mut written = 0;
        for &byte in buf {
            self.buffer[self.buffer_len] = byte;
            self.buffer_len += 1;

            if self.buffer_len == 3 {
                let out = [
                    CHARS[(self.buffer[0] >> 2) as usize],
                    CHARS[(((self.buffer[0] & 0x03) << 4) | (self.buffer[1] >> 4)) as usize],
                    CHARS[(((self.buffer[1] & 0x0f) << 2) | (self.buffer[2] >> 6)) as usize],
                    CHARS[(self.buffer[2] & 0x3f) as usize],
                ];
                self.writer.write_all(&out)?;
                self.buffer_len = 0;
            }
            written += 1;
        }
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        if self.buffer_len > 0 {
            let out = match self.buffer_len {
                1 => [
                    CHARS[(self.buffer[0] >> 2) as usize],
                    CHARS[((self.buffer[0] & 0x03) << 4) as usize],
                    b'=',
                    b'=',
                ],
                2 => [
                    CHARS[(self.buffer[0] >> 2) as usize],
                    CHARS[(((self.buffer[0] & 0x03) << 4) | (self.buffer[1] >> 4)) as usize],
                    CHARS[((self.buffer[1] & 0x0f) << 2) as usize],
                    b'=',
                ],
                _ => unreachable!(),
            };
            self.writer.write_all(&out)?;
            self.buffer_len = 0;
        }
        self.writer.flush()
    }
}

impl<W: std::io::Write> Drop for Base64Encoder<W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
