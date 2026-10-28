//! Request history model

use super::request::ApiRequest;
use super::response::ApiResponse;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A history entry for a sent request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub request: ApiRequest,
    pub response: Option<ApiResponse>,
    pub timestamp: DateTime<Utc>,
}

impl HistoryEntry {
    pub fn new(request: ApiRequest, response: Option<ApiResponse>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            request,
            response,
            timestamp: Utc::now(),
        }
    }

    /// Get a short display string for the history entry
    pub fn display_string(&self) -> String {
        let status = self
            .response
            .as_ref()
            .map(|r| format!("{}", r.status))
            .unwrap_or_else(|| "ERR".to_string());

        let time = self.timestamp.format("%H:%M:%S").to_string();

        format!(
            "[{}] {} {} - {}",
            time,
            self.request.method.as_str(),
            truncate_url(&self.request.url, 40),
            status
        )
    }
}

/// Request history storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestHistory {
    pub entries: Vec<HistoryEntry>,
    pub max_entries: usize,
}

impl RequestHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: vec![],
            max_entries,
        }
    }

    /// Add a new entry to history
    pub fn add(&mut self, request: ApiRequest, response: Option<ApiResponse>) {
        let entry = HistoryEntry::new(request, response);
        self.entries.insert(0, entry);

        // Trim to max entries
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get recent entries
    pub fn recent(&self, count: usize) -> &[HistoryEntry] {
        let end = count.min(self.entries.len());
        &self.entries[..end]
    }
}

/// Truncate URL for display
fn truncate_url(url: &str, max_len: usize) -> String {
    crate::utils::truncate_str(url, max_len)
}
