//! File-based storage for collections, environments, and history

use crate::models::{Collection, Environment, RequestHistory};
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
}

/// Get the application data directory
pub fn get_app_data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("RUSTAPI_DATA_DIR").filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    dirs::data_dir()
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
    fs::write(&path, json)?;
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
    fs::write(&path, json)?;
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
        let safe_name = sanitize_filename(&collection.name);
        get_collections_dir().join(format!("{}.json", safe_name))
    };

    let json = serde_json::to_string_pretty(collection)?;

    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&file_path, json)?;
    Ok(file_path)
}

/// Load a collection from a file
pub fn load_collection(path: &Path) -> Result<Collection> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("Failed to read collection file: {:?}", path))?;

    let mut collection: Collection = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse collection file: {:?}", path))?;

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
    let safe_name = format!(
        "{}_{}",
        sanitize_filename(&environment.name),
        sanitize_filename(&environment.id.chars().take(8).collect::<String>())
    );
    let file_path = get_environments_dir().join(format!("{}.json", safe_name));

    let json = serde_json::to_string_pretty(environment)?;
    fs::write(&file_path, json)?;
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
    environments
}

/// Delete a collection file
pub fn delete_collection(collection: &Collection) -> Result<()> {
    if let Some(ref path) = collection.file_path {
        fs::remove_file(path)?;
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
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

/// Export a collection to a specified path
pub fn export_collection(collection: &Collection, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(collection)?;
    fs::write(path, json)?;
    Ok(())
}

/// Import a collection from a file (any location)
pub fn import_collection(path: &Path) -> Result<Collection> {
    load_collection(path)
}

/// Sanitize filename
fn sanitize_filename(name: &str) -> String {
    let safe = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string();
    let upper = safe.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && matches!(upper.as_bytes()[3], b'1'..=b'9'));
    if safe.is_empty() {
        "Untitled".into()
    } else if reserved {
        format!("_{safe}")
    } else {
        safe
    }
}
