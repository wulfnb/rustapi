//! Portable workspace manifest and crash-safe JSON replacement.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("Could not replace {}", path.display()))?;
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    pub schema_version: u32,
    pub name: String,
}
pub fn open_workspace(path: &Path) -> Result<PathBuf> {
    fs::create_dir_all(path)?;
    let manifest = path.join("rustapi.json");
    if manifest.exists() {
        let m: WorkspaceManifest = serde_json::from_slice(&fs::read(&manifest)?)?;
        if m.schema_version != 1 {
            bail!(
                "Unsupported workspace schema {}; expected 1",
                m.schema_version
            );
        }
    } else {
        atomic_write(
            &manifest,
            &serde_json::to_vec_pretty(&WorkspaceManifest {
                schema_version: 1,
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
            })?,
        )?;
    }
    for dir in ["collections", "environments", ".local"] {
        fs::create_dir_all(path.join(dir))?;
    }
    let ignore = path.join(".gitignore");
    let mut contents = fs::read_to_string(&ignore).unwrap_or_default();
    for entry in ["/.local/", "/history.json", "/state.json", "/drafts.json"] {
        if !contents.lines().any(|l| l == entry) {
            contents.push_str(&format!("\n{entry}\n"));
        }
    }
    atomic_write(&ignore, contents.as_bytes())?;
    path.canonicalize().context("Resolve workspace path")
}
