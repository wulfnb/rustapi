//! Collection and Folder models

use super::request::ApiRequest;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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
