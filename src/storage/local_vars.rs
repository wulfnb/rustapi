use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Persistent global variables stored locally in `.local/globals.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalVariables {
    pub schema_version: u32,
    pub variables: BTreeMap<String, String>,
}

impl Default for GlobalVariables {
    fn default() -> Self {
        Self {
            schema_version: 1,
            variables: BTreeMap::new(),
        }
    }
}

impl GlobalVariables {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "Unsupported global variables schema"
        );
        ensure!(
            self.variables.len() <= 1000,
            "At most 1000 global variables"
        );
        for (k, v) in &self.variables {
            ensure!(
                !k.trim().is_empty() && k.len() <= 256,
                "Invalid global variable name"
            );
            ensure!(v.len() <= 64 * 1024, "Global variable value exceeds 64 KiB");
        }
        Ok(())
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = super::owned_path(root, &[".local", "globals.json"])?;
        if !path.exists() {
            return Ok(Self::default());
        }
        ensure!(
            std::fs::metadata(&path)?.len() <= 1024 * 1024,
            "Global variables file exceeds 1 MiB"
        );
        let globals: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        globals.validate()?;
        Ok(globals)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        ensure!(
            bytes.len() <= 1024 * 1024,
            "Global variables file exceeds 1 MiB"
        );
        crate::workspace::atomic_write(
            &super::owned_path(root, &[".local", "globals.json"])?,
            &bytes,
        )
    }
}

/// Persistent local environment overrides stored locally in `.local/environment-overrides.json`
/// Keyed by environment ID or name, mapping variable keys to override values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalEnvironmentOverrides {
    pub schema_version: u32,
    pub overrides: BTreeMap<String, BTreeMap<String, String>>,
}

impl Default for LocalEnvironmentOverrides {
    fn default() -> Self {
        Self {
            schema_version: 1,
            overrides: BTreeMap::new(),
        }
    }
}

impl LocalEnvironmentOverrides {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "Unsupported environment overrides schema"
        );
        ensure!(
            self.overrides.len() <= 100,
            "At most 100 environment overrides"
        );
        for (env, vars) in &self.overrides {
            ensure!(
                !env.trim().is_empty() && env.len() <= 256,
                "Invalid environment identifier"
            );
            ensure!(vars.len() <= 1000, "At most 1000 overrides per environment");
            for (k, v) in vars {
                ensure!(
                    !k.trim().is_empty() && k.len() <= 256,
                    "Invalid variable name"
                );
                ensure!(v.len() <= 64 * 1024, "Override value exceeds 64 KiB");
            }
        }
        Ok(())
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = super::owned_path(root, &[".local", "environment-overrides.json"])?;
        if !path.exists() {
            return Ok(Self::default());
        }
        ensure!(
            std::fs::metadata(&path)?.len() <= 1024 * 1024,
            "Environment overrides file exceeds 1 MiB"
        );
        let overrides: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        overrides.validate()?;
        Ok(overrides)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        ensure!(
            bytes.len() <= 1024 * 1024,
            "Environment overrides file exceeds 1 MiB"
        );
        crate::workspace::atomic_write(
            &super::owned_path(root, &[".local", "environment-overrides.json"])?,
            &bytes,
        )
    }

    pub fn get_overrides(&self, env_id: &str, env_name: &str) -> BTreeMap<String, String> {
        if let Some(map) = self.overrides.get(env_id) {
            return map.clone();
        }
        if let Some(map) = self.overrides.get(env_name) {
            return map.clone();
        }
        BTreeMap::new()
    }
}
