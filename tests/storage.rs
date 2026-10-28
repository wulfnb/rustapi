use rustapi::{models::*, storage::*};
use std::fs;
use tempfile::tempdir;

// This test owns the process-wide override. Other tests use explicit paths only.
#[test]
fn profile_lifecycle_uses_only_a_temporary_data_directory() {
    let root = tempdir().unwrap();
    std::env::set_var("RUSTAPI_DATA_DIR", root.path());
    assert!(load_all_collections().is_empty());
    assert!(load_all_environments().is_empty());
    delete_environment(&Environment::new("absent")).unwrap();
    init_storage().unwrap();
    assert_eq!(get_app_data_dir(), root.path());
    assert!(get_collections_dir().is_dir());
    assert!(get_environments_dir().is_dir());
    assert!(load_app_state().unwrap().active_environment_id.is_none());
    assert_eq!(load_history().max_entries, 100);

    let mut env = Environment::new("Dev / test");
    // Imported environments need not have a UUID-length identifier.
    env.id = "x".into();
    env.variables.push(KeyValue::new("token", "abc"));
    let env_path = save_environment(&env).unwrap();
    env.name = "Renamed environment".into();
    assert_eq!(save_environment(&env).unwrap(), env_path);
    assert_eq!(load_all_environments().len(), 1);
    assert_eq!(env_path.parent().unwrap(), get_environments_dir());
    assert_eq!(
        load_environment(&env_path).unwrap().variables[0].value,
        "abc"
    );
    assert_eq!(load_all_environments().len(), 1);

    let state = AppState {
        active_environment_id: Some(env.id.clone()),
        recent_files: vec!["a.json".into()],
        window_size: Some((800.0, 600.0)),
    };
    save_app_state(&state).unwrap();
    assert_eq!(
        load_app_state().unwrap().active_environment_id,
        Some(env.id.clone())
    );
    assert_eq!(load_app_state().unwrap().window_size, Some((800.0, 600.0)));
    let mut history = RequestHistory::new(5);
    history.add(
        ApiRequest::new("Saved request"),
        Some(ApiResponse {
            status: 201,
            ..Default::default()
        }),
    );
    save_history(&history).unwrap();
    assert_eq!(
        load_history().entries[0].response.as_ref().unwrap().status,
        201
    );

    let collection = Collection::new("API / test");
    let path = save_collection(&collection, None).unwrap();
    assert_eq!(path.file_name().unwrap(), "API _ test.json");
    fs::write(get_collections_dir().join("broken.json"), "broken").unwrap();
    fs::write(get_environments_dir().join("broken.json"), "broken").unwrap();
    fs::write(get_environments_dir().join("ignored.txt"), "ignored").unwrap();
    fs::write(get_collections_dir().join("ignored.txt"), "{}").unwrap();
    assert_eq!(load_all_collections().len(), 1);
    assert_eq!(load_all_environments().len(), 1);
    let loaded = load_collection(&path).unwrap();
    assert_eq!(save_collection(&loaded, None).unwrap(), path);
    delete_collection(&loaded).unwrap();
    assert!(!path.exists());
    delete_environment(&env).unwrap();
    assert!(load_all_environments().is_empty());
    delete_environment(&env).unwrap();

    for name in ["CON", "prn", "NUL", "COM1", "LPT9", "   "] {
        let path = save_collection(&Collection::new(name), None).unwrap();
        assert_eq!(path.parent().unwrap(), get_collections_dir());
        assert!(path.exists());
    }

    fs::write(get_history_file(), "corrupt").unwrap();
    assert!(load_history().entries.is_empty());
    fs::remove_file(get_history_file()).unwrap();
    fs::create_dir(get_history_file()).unwrap();
    assert!(load_history().entries.is_empty());
    fs::write(get_state_file(), "corrupt").unwrap();
    assert!(load_app_state().is_err());
    for directory in [get_collections_dir(), get_environments_dir()] {
        fs::remove_dir_all(&directory).unwrap();
        fs::write(directory, "a file in place of a directory").unwrap();
    }
    assert!(load_all_collections().is_empty());
    assert!(load_all_environments().is_empty());
    delete_environment(&env).unwrap();
    std::env::remove_var("RUSTAPI_DATA_DIR");
}

#[test]
fn collection_export_import_and_nested_save_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested").join("collection.json");
    let mut collection = Collection::new("🦀 Collection");
    collection.requests.push(ApiRequest::new("GET users"));
    save_collection(&collection, Some(&path)).unwrap();
    let loaded = import_collection(&path).unwrap();
    assert_eq!(loaded.requests[0].id, collection.requests[0].id);
    assert_eq!(loaded.file_path.as_deref(), path.to_str());
    let exported = dir.path().join("export.json");
    export_collection(&loaded, &exported).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        fs::read_to_string(&exported).unwrap()
    );
}

#[test]
fn missing_and_malformed_files_return_contextual_errors() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing.json");
    assert!(load_collection(&path)
        .unwrap_err()
        .to_string()
        .contains("Failed to read collection"));
    assert!(load_environment(&path).is_err());
    fs::write(&path, "not json").unwrap();
    assert!(load_collection(&path)
        .unwrap_err()
        .to_string()
        .contains("Failed to parse collection"));
    assert!(load_environment(&path)
        .unwrap_err()
        .to_string()
        .contains("Failed to parse environment"));
}

#[test]
fn save_to_a_directory_reports_io_error() {
    let dir = tempdir().unwrap();
    assert!(save_collection(&Collection::new("test"), Some(dir.path())).is_err());
    assert!(export_collection(&Collection::new("test"), dir.path()).is_err());
    assert!(save_collection(&Collection::new("test"), Some(std::path::Path::new("/"))).is_err());
}
