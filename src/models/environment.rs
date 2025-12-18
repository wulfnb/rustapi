//! Environment model

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use super::request::KeyValue;

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

