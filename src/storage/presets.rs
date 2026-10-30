use crate::models::{ApiRequest, KeyValue};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub fn parse_bulk_rows(text: &str) -> Result<Vec<KeyValue>> {
    ensure!(text.len() <= 1024 * 1024, "Bulk rows exceed 1 MiB");
    let rows: Vec<KeyValue> = serde_json::from_str(text)?;
    ensure!(rows.len() <= 1000, "At most 1000 bulk rows");
    Ok(rows)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderPreset {
    pub id: String,
    pub name: String,
    pub headers: Vec<KeyValue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderPresets {
    pub schema_version: u32,
    pub presets: Vec<HeaderPreset>,
}
impl Default for HeaderPresets {
    fn default() -> Self {
        Self {
            schema_version: 1,
            presets: vec![],
        }
    }
}
impl HeaderPresets {
    fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "Unsupported header preset schema");
        ensure!(self.presets.len() <= 100, "At most 100 header presets");
        let mut ids = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for preset in &self.presets {
            ensure!(
                !preset.id.is_empty() && ids.insert(&preset.id),
                "Empty or duplicate preset ID"
            );
            ensure!(
                !preset.name.trim().is_empty()
                    && preset.name.len() <= 120
                    && names.insert(&preset.name),
                "Empty, duplicate or oversized preset name"
            );
            ensure!(preset.headers.len() <= 1000, "At most 1000 preset rows");
            for row in &preset.headers {
                if crate::secrets::sensitive(&row.key)
                    || row.value_type.eq_ignore_ascii_case("secret")
                {
                    let value = row.value.trim();
                    let reference = value
                        .strip_prefix("Bearer ")
                        .or_else(|| value.strip_prefix("Basic "))
                        .unwrap_or(value)
                        .trim();
                    let valid = reference
                        .strip_prefix("{{")
                        .and_then(|s| s.strip_suffix("}}"))
                        .is_some_and(|key| !key.trim().is_empty() && !key.contains(['{', '}']));
                    ensure!(
                        value.is_empty() || valid,
                        "Use variable references instead of literal credentials in header presets"
                    );
                }
            }
            crate::http::effective_headers(&ApiRequest {
                headers: preset.headers.clone(),
                ..Default::default()
            })?;
        }
        Ok(())
    }
    pub fn load(root: &Path) -> Result<Self> {
        let path = super::owned_path(root, &[".local", "header-presets.json"])?;
        if !path.exists() {
            return Ok(Self::default());
        }
        ensure!(
            std::fs::metadata(&path)?.len() <= 1024 * 1024,
            "Header presets exceed 1 MiB"
        );
        let presets: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        presets.validate()?;
        Ok(presets)
    }
    pub fn save(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        ensure!(bytes.len() <= 1024 * 1024, "Header presets exceed 1 MiB");
        crate::workspace::atomic_write(
            &super::owned_path(root, &[".local", "header-presets.json"])?,
            &bytes,
        )
    }
    pub fn add(&mut self, name: &str, headers: &[KeyValue]) -> Result<()> {
        let mut next = self.clone();
        next.presets.push(HeaderPreset {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().into(),
            headers: headers.to_vec(),
        });
        next.validate()?;
        *self = next;
        Ok(())
    }
}
