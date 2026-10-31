//! File-based storage for collections, environments, and history

use crate::models::{Collection, Environment, RequestHistory};
use crate::workspace::atomic_write;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Application state for persistence
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub active_environment_id: Option<String>,
    pub recent_files: Vec<String>,
    pub window_size: Option<(f32, f32)>,
    #[serde(default)]
    pub theme: ThemeChoice,
}

/// Colour scheme the user picked in View → Theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    System,
    #[default]
    Dark,
    Light,
}

/// Get the application data directory
pub fn get_app_data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("RUSTAPI_DATA_DIR").filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    default_app_data_dir(dirs::data_dir())
}

fn default_app_data_dir(platform_dir: Option<PathBuf>) -> PathBuf {
    platform_dir
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rustapi")
}

/// Get the collections directory
pub fn get_collections_dir() -> PathBuf {
    get_app_data_dir().join("collections")
}

/// Get the environments directory
pub fn get_environments_dir() -> PathBuf {
    get_app_data_dir().join("environments")
}

/// Get the state file path
pub fn get_state_file() -> PathBuf {
    get_app_data_dir().join("state.json")
}

/// Get the history file path
pub fn get_history_file() -> PathBuf {
    get_app_data_dir().join("history.json")
}

/// Initialize storage directories
pub fn init_storage() -> Result<()> {
    fs::create_dir_all(get_collections_dir())?;
    fs::create_dir_all(get_environments_dir())?;
    Ok(())
}

/// Save app state
pub fn save_app_state(state: &AppState) -> Result<()> {
    let path = get_state_file();
    let json = serde_json::to_string_pretty(state)?;
    atomic_write(&path, json.as_bytes())?;
    Ok(())
}

/// Load app state
pub fn load_app_state() -> Result<AppState> {
    let path = get_state_file();
    if path.exists() {
        let json = fs::read_to_string(&path)?;
        let state: AppState = serde_json::from_str(&json)?;
        Ok(state)
    } else {
        Ok(AppState::default())
    }
}

/// Save request history
pub fn save_history(history: &RequestHistory) -> Result<()> {
    let path = get_history_file();
    let json = serde_json::to_string_pretty(history)?;
    atomic_write(&path, json.as_bytes())?;
    Ok(())
}

/// Load request history
pub fn load_history() -> RequestHistory {
    let path = get_history_file();
    if path.exists() {
        if let Ok(json) = fs::read_to_string(&path) {
            if let Ok(history) = serde_json::from_str(&json) {
                return history;
            }
        }
    }
    RequestHistory::new(100)
}

/// Save a collection to a file
pub fn save_collection(collection: &Collection, path: Option<&Path>) -> Result<PathBuf> {
    let file_path = if let Some(p) = path {
        p.to_path_buf()
    } else if let Some(ref existing_path) = collection.file_path {
        PathBuf::from(existing_path)
    } else {
        get_collections_dir().join(format!("{}.json", storage_id(&collection.id)?))
    };

    crate::models::validate_collection(collection)?;
    let json = serde_json::to_string_pretty(collection)?;

    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    atomic_write(&file_path, json.as_bytes())?;
    Ok(file_path)
}

/// Load a collection from a file
pub fn load_collection(path: &Path) -> Result<Collection> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("Failed to read collection file: {:?}", path))?;

    let mut collection: Collection = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse collection file: {:?}", path))?;

    anyhow::ensure!(
        collection.schema_version == 1,
        "Unsupported collection schema {}",
        collection.schema_version
    );
    crate::models::validate_collection(&collection)?;
    collection.file_path = Some(path.to_string_lossy().to_string());
    Ok(collection)
}

/// Load all collections from the collections directory
pub fn load_all_collections() -> Vec<Collection> {
    let dir = get_collections_dir();
    if !dir.exists() {
        return vec![];
    }

    let mut collections = vec![];
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(collection) = load_collection(&path) {
                    collections.push(collection);
                }
            }
        }
    }
    collections
}

/// Save an environment to a file
pub fn save_environment(environment: &Environment) -> Result<PathBuf> {
    fs::create_dir_all(get_environments_dir())?;
    let file_path = get_environments_dir().join(format!("{}.json", storage_id(&environment.id)?));

    let json = serde_json::to_string_pretty(environment)?;
    atomic_write(&file_path, json.as_bytes())?;
    // Preserve legacy versions before retiring name-derived duplicate paths.
    for entry in fs::read_dir(get_environments_dir())?.flatten() {
        let old = entry.path();
        if old != file_path
            && old.extension().is_some_and(|e| e == "json")
            && load_environment(&old).is_ok_and(|e| e.id == environment.id)
        {
            let backup = get_app_data_dir()
                .join(".local/migrations")
                .join(format!("{}.json", uuid::Uuid::new_v4()));
            atomic_write(&backup, &fs::read(&old)?)?;
            fs::remove_file(old)?;
        }
    }

    Ok(file_path)
}

/// Load an environment from a file
pub fn load_environment(path: &Path) -> Result<Environment> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("Failed to read environment file: {:?}", path))?;

    let environment: Environment = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse environment file: {:?}", path))?;

    Ok(environment)
}

/// Load all environments from the environments directory
pub fn load_all_environments() -> Vec<Environment> {
    let dir = get_environments_dir();
    if !dir.exists() {
        return vec![];
    }

    let mut environments = vec![];
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(env) = load_environment(&path) {
                    environments.push(env);
                }
            }
        }
    }
    environments.sort_by_key(|a| std::cmp::Reverse(a.updated_at));
    let mut seen = std::collections::HashSet::new();
    environments.retain(|e| seen.insert(e.id.clone()));
    environments
}

/// Delete a collection file
pub fn delete_collection(collection: &Collection) -> Result<()> {
    if let Some(ref path) = collection.file_path {
        if Path::new(path).exists() {
            fs::remove_file(path)?;
        }
    } else {
        let path = get_collections_dir().join(format!("{}.json", storage_id(&collection.id)?));
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Delete an environment file
pub fn delete_environment(environment: &Environment) -> Result<()> {
    let dir = get_environments_dir();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(env) = load_environment(&path) {
                    if env.id == environment.id {
                        fs::remove_file(path)?;
                        continue;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Export a collection to a specified path
pub fn export_collection(collection: &Collection, path: &Path) -> Result<()> {
    crate::models::validate_collection(collection)?;
    let json = serde_json::to_string_pretty(collection)?;
    atomic_write(path, json.as_bytes())?;
    Ok(())
}

/// Import a collection from a file (any location)
pub fn import_collection(path: &Path) -> Result<Collection> {
    let mut collection = load_collection(path)?;
    collection.file_path = None;
    Ok(collection)
}

fn storage_id(id: &str) -> Result<&str> {
    anyhow::ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
        "Invalid storage identifier"
    );
    Ok(id)
}

pub fn save_drafts<T: Serialize>(value: &T) -> Result<()> {
    atomic_write(
        &get_app_data_dir().join("drafts.json"),
        &serde_json::to_vec_pretty(value)?,
    )
}
pub fn load_drafts<T: serde::de::DeserializeOwned + Default>() -> Result<T> {
    let path = get_app_data_dir().join("drafts.json");
    if path.exists() {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    } else {
        Ok(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_paths_are_namespaced_with_a_relative_fallback() {
        assert_eq!(
            default_app_data_dir(None),
            PathBuf::from(".").join("rustapi")
        );
        assert_eq!(
            default_app_data_dir(Some(PathBuf::from("platform"))),
            PathBuf::from("platform/rustapi")
        );
    }
}
#[cfg(test)]
#[path = "../../tests/unit/storage.rs"]
mod storage_tests;
