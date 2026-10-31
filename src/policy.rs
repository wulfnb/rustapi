//! Workspace-owned execution policy. Invalid policy files fail closed.
use crate::models::ApiRequest;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    pub require_encryption: bool,
    pub allow_insecure_tls: bool,
    pub allowed_hosts: Vec<String>,
    pub allow_scripts: bool,
    pub allow_external_secrets: bool,
    pub allow_ai: bool,
    pub allow_mcp_execution: bool,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            version: 1,
            require_encryption: false,
            allow_insecure_tls: true,
            allowed_hosts: vec![],
            allow_scripts: true,
            allow_external_secrets: true,
            allow_ai: true,
            allow_mcp_execution: true,
        }
    }
}
impl Policy {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && self.allowed_hosts.len() <= 1000,
            "Invalid workspace policy"
        );
        ensure!(
            self.allowed_hosts
                .iter()
                .all(|h| !h.is_empty() && h.len() <= 253 && !h.contains(['/', ':', '\r', '\n'])),
            "Policy hosts must be exact names or *.domain names"
        );
        Ok(())
    }
    pub fn load(root: &std::path::Path) -> Result<Self> {
        let path = crate::storage::owned_path(root, &["workspace-policy.json"])?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let policy: Self = serde_json::from_str(&crate::tools_cli::read(&path, 1024 * 1024)?)?;
        ensure!(
            policy.version == 1 && policy.allowed_hosts.len() <= 1000,
            "Invalid workspace policy"
        );
        ensure!(
            policy
                .allowed_hosts
                .iter()
                .all(|h| !h.is_empty() && h.len() <= 253 && !h.contains(['/', ':', '\r', '\n'])),
            "Policy hosts must be exact names or *.domain names"
        );
        Ok(policy)
    }
    pub fn request(&self, request: &ApiRequest) -> Result<()> {
        ensure!(
            self.allow_insecure_tls || request.settings.verify_tls,
            "Workspace policy requires certificate verification"
        );
        ensure!(
            self.allow_scripts
                || request.pre_script.trim().is_empty() && request.post_script.trim().is_empty(),
            "Workspace policy disables scripts"
        );
        self.url(&request.url)
    }
    pub fn url(&self, value: &str) -> Result<()> {
        let url = reqwest::Url::parse(value)?;
        ensure!(
            !self.require_encryption || ["https", "wss", "mqtts", "grpcs"].contains(&url.scheme()),
            "Workspace policy requires encrypted transport"
        );
        if !self.allowed_hosts.is_empty() {
            let host = url.host_str().unwrap_or_default().to_lowercase();
            ensure!(
                self.allowed_hosts.iter().any(|pattern| {
                    let pattern = pattern.to_lowercase();
                    if let Some(suffix) = pattern.strip_prefix("*.") {
                        host.ends_with(&format!(".{suffix}"))
                    } else {
                        host == pattern
                    }
                }),
                "Destination host is outside workspace policy"
            );
        }
        Ok(())
    }
}
