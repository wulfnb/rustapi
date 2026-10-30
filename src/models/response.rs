//! API Response model

use serde::{Deserialize, Serialize};

/// HTTP Response from a request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiResponse {
    #[serde(default)]
    pub skipped: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub size: usize,
    pub time_ms: u128,
    pub error: Option<String>,
    #[serde(default, skip_serializing)]
    pub raw_body: Vec<u8>,
    #[serde(default)]
    pub headers_time_ms: u128,
    #[serde(default)]
    pub tests: Vec<super::TestResult>,
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

    pub fn is_success(&self) -> bool {
        self.error.is_none() && (200..300).contains(&self.status)
    }
}
