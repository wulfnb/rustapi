//! Flat, bounded workspace documents; source paths cannot escape their workspace.
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
const LIMIT: u64 = 1024 * 1024;

/// Reject symlink components rather than following them for workspace-owned data.
pub fn owned_path(root: &Path, components: &[&str]) -> Result<PathBuf> {
    let mut path = root.canonicalize().context("Resolve workspace")?;
    for component in components {
        ensure!(
            !component.is_empty()
                && *component != "."
                && *component != ".."
                && !component.contains(['/', '\\']),
            "Invalid workspace filename"
        );
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "Workspace data cannot use symlink components"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(path)
}
fn document_path(root: &Path, name: &str) -> Result<PathBuf> {
    ensure!(
        name.len() <= 120
            && name.ends_with(".md")
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_. ".contains(c))
            && !name.starts_with('.'),
        "Use a Markdown filename with letters, numbers, spaces, dots, hyphens or underscores"
    );
    owned_path(root, &["documents", name])
}
pub fn list_documents(root: &Path) -> Result<Vec<String>> {
    let path = owned_path(root, &["documents"])?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(name) = entry.file_name().to_str() {
                if document_path(root, name).is_ok() {
                    names.push(name.to_owned());
                }
            }
        }
        ensure!(names.len() <= 1000, "More than 1000 workspace documents");
    }
    names.sort();
    Ok(names)
}
pub fn read_document(root: &Path, name: &str) -> Result<String> {
    let path = document_path(root, name)?;
    ensure!(
        fs::metadata(&path)?.len() <= LIMIT,
        "Document exceeds 1 MiB"
    );
    fs::read_to_string(path).context("Read UTF-8 Markdown document")
}
pub fn write_document(root: &Path, name: &str, content: &str) -> Result<()> {
    ensure!(content.len() as u64 <= LIMIT, "Document exceeds 1 MiB");
    let path = document_path(root, name)?;
    crate::workspace::atomic_write(&path, content.as_bytes())
}
