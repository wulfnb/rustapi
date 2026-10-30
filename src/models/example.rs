//! Saved examples are snapshots; they never execute or overwrite their parent.
use super::{ApiRequest, ApiResponse};
use anyhow::{ensure, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestExample {
    pub id: String,
    pub name: String,
    pub request: Box<ApiRequest>,
    pub response: ApiResponse,
    #[serde(default)]
    pub body_base64: String,
    #[serde(default)]
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}
impl RequestExample {
    pub fn capture(name: String, request: &ApiRequest, response: &ApiResponse) -> Result<Self> {
        ensure!(
            (100..=599).contains(&response.status) && response.error.is_none() && !response.skipped,
            "Only completed HTTP responses can be saved as examples"
        );
        let bytes = if response.raw_body.is_empty() {
            response.body.as_bytes()
        } else {
            &response.raw_body
        };
        ensure!(
            bytes.len() <= 1024 * 1024,
            "Examples are limited to 1 MiB; download the complete response instead"
        );
        let mut request = request.clone();
        request.examples.clear();
        let mut snapshot = response.clone();
        snapshot.raw_body.clear();
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            request: Box::new(request),
            response: snapshot,
            body_base64: STANDARD.encode(bytes),
            metadata: Default::default(),
        })
    }
    pub fn response(&self) -> Result<ApiResponse> {
        let mut response = self.response.clone();
        response.raw_body = STANDARD.decode(&self.body_base64)?;
        response.size = response.raw_body.len();
        Ok(response)
    }
    pub fn set_text(&mut self, body: String) {
        self.body_base64 = STANDARD.encode(body.as_bytes());
        self.response.size = body.len();
        self.response.body = body;
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.id.is_empty(), "Example ID is empty");
        ensure!(
            (100..=599).contains(&self.response.status)
                && !self.response.skipped
                && self.response.error.is_none(),
            "Example must contain a completed HTTP response"
        );
        ensure!(
            self.response.body.len() <= 1024 * 1024,
            "Example text exceeds 1 MiB"
        );
        ensure!(
            self.request.examples.is_empty(),
            "Nested examples are unsupported"
        );
        ensure!(self.body_base64.len() <= 1_398_104, "Example exceeds 1 MiB");
        ensure!(
            STANDARD.decode(&self.body_base64)?.len() <= 1024 * 1024,
            "Example exceeds 1 MiB"
        );
        Ok(())
    }
}
