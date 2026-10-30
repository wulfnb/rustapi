use rustapi::storage::*;
#[test]
fn documents_are_portable_bounded_and_cannot_escape_workspace() {
    let root = tempfile::tempdir().unwrap();
    assert!(list_documents(root.path()).unwrap().is_empty());
    write_document(
        root.path(),
        "WORKSPACE-README.md",
        "# Local API\n\nOffline documentation α",
    )
    .unwrap();
    write_document(root.path(), "Usage.md", "Updated").unwrap();
    assert_eq!(
        list_documents(root.path()).unwrap(),
        vec!["Usage.md", "WORKSPACE-README.md"]
    );
    assert!(read_document(root.path(), "WORKSPACE-README.md")
        .unwrap()
        .contains('α'));
    for name in [
        "../escape.md",
        "/escape.md",
        "..\\escape.md",
        ".hidden.md",
        "file.json",
    ] {
        assert!(write_document(root.path(), name, "bad").is_err());
    }
    assert!(write_document(root.path(), "large.md", &"a".repeat(1024 * 1024 + 1)).is_err());
    std::fs::write(
        root.path().join("documents/large.md"),
        vec![0; 1024 * 1024 + 1],
    )
    .unwrap();
    assert!(read_document(root.path(), "large.md").is_err());
}
#[cfg(unix)]
#[test]
fn document_storage_refuses_symlink_files_and_directories() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("documents")).unwrap();
    assert!(write_document(root.path(), "escape.md", "bad").is_err());
    assert!(list_documents(root.path()).is_err());
    std::fs::remove_file(root.path().join("documents")).unwrap();
    std::fs::create_dir(root.path().join("documents")).unwrap();
    std::fs::write(outside.path().join("private.md"), "unchanged").unwrap();
    symlink(
        outside.path().join("private.md"),
        root.path().join("documents/link.md"),
    )
    .unwrap();
    assert!(read_document(root.path(), "link.md").is_err());
    assert!(write_document(root.path(), "link.md", "bad").is_err());
    assert_eq!(
        std::fs::read_to_string(outside.path().join("private.md")).unwrap(),
        "unchanged"
    );
}
