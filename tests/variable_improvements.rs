use rustapi::{http::Cancellation, storage::*, variables::*};
use std::collections::BTreeMap;
use tempfile::tempdir;

#[test]
fn scope_precedence_from_global_to_vault() {
    let mut vars = Variables::default();

    // 1. Global level
    vars.global.insert("host".into(), "global.test".into());
    vars.global
        .insert("only_global".into(), "val_global".into());
    assert_eq!(vars.resolve("{{host}}").unwrap(), "global.test");
    assert_eq!(vars.resolve("{{only_global}}").unwrap(), "val_global");

    // 2. Collection overrides Global
    vars.collection
        .insert("host".into(), "collection.test".into());
    assert_eq!(vars.resolve("{{host}}").unwrap(), "collection.test");

    // 3. Environment overrides Collection
    vars.environment.insert("host".into(), "env.test".into());
    assert_eq!(vars.resolve("{{host}}").unwrap(), "env.test");

    // 4. Local Environment Override wins within Environment scope
    vars.local_overrides
        .insert("host".into(), "override.test".into());
    assert_eq!(vars.resolve("{{host}}").unwrap(), "override.test");

    // 5. Iteration Data overrides Local Environment Overrides
    vars.data.insert("host".into(), "data.test".into());
    assert_eq!(vars.resolve("{{host}}").unwrap(), "data.test");

    // 6. Script Local overrides Iteration Data
    vars.local.insert("host".into(), "local.test".into());
    assert_eq!(vars.resolve("{{host}}").unwrap(), "local.test");

    // 7. Vault references are top-precedence
    vars.secrets.insert("host".into(), "secret.test".into());
    assert_eq!(vars.resolve("{{vault:host}}").unwrap(), "secret.test");
}

#[test]
fn dynamic_variables_resolve_valid_values() {
    let vars = Variables::default();

    // $guid / $uuid
    let guid = vars.resolve("{{$guid}}").unwrap();
    assert!(uuid::Uuid::parse_str(&guid).is_ok());
    let uuid = vars.resolve("{{$uuid}}").unwrap();
    assert!(uuid::Uuid::parse_str(&uuid).is_ok());

    // $timestamp
    let ts_str = vars.resolve("{{$timestamp}}").unwrap();
    let ts: i64 = ts_str.parse().unwrap();
    assert!(ts > 1_700_000_000);

    // $isoTimestamp
    let iso = vars.resolve("{{$isoTimestamp}}").unwrap();
    assert!(chrono::DateTime::parse_from_rfc3339(&iso).is_ok());

    // $randomInt
    let rand_int_str = vars.resolve("{{$randomInt}}").unwrap();
    let rand_int: u32 = rand_int_str.parse().unwrap();
    assert!((1..=1000).contains(&rand_int));

    // $randomEmail
    let email = vars.resolve("{{$randomEmail}}").unwrap();
    assert!(email.starts_with("user-"));
    assert!(email.ends_with("@example.com"));

    // Variable in URL
    let url = vars
        .resolve("https://example.com/item/{{$guid}}?time={{$timestamp}}")
        .unwrap();
    assert!(url.starts_with("https://example.com/item/"));
    assert!(url.contains("?time="));
}

#[test]
fn scripts_can_read_and_mutate_pm_globals() {
    let mut vars = Variables::default();
    vars.global.insert("initial_global".into(), "100".into());

    let script = r#"
        const current = pm.globals.get("initial_global");
        pm.globals.set("initial_global", String(Number(current) + 50));
        pm.globals.set("new_global", "created_by_script");
    "#;

    let cancel = Cancellation::default();
    let result = rustapi::scripts::execute(script, &mut vars, None, &cancel);
    assert!(result.is_ok(), "Script execution error: {:?}", result.err());

    // Check that globals were mutated and returned
    assert_eq!(vars.global.get("initial_global"), Some(&"150".to_string()));
    assert_eq!(
        vars.global.get("new_global"),
        Some(&"created_by_script".to_string())
    );
}

#[test]
fn persistent_storage_for_globals_and_local_overrides() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".local")).unwrap();

    // 1. GlobalVariables persistence
    let mut globals = GlobalVariables::default();
    globals
        .variables
        .insert("api_key".into(), "global_secret".into());
    globals
        .variables
        .insert("endpoint".into(), "https://api.test".into());
    globals.save(root).unwrap();

    let reloaded_globals = GlobalVariables::load(root).unwrap();
    assert_eq!(
        reloaded_globals.variables.get("api_key"),
        Some(&"global_secret".to_string())
    );
    assert_eq!(
        reloaded_globals.variables.get("endpoint"),
        Some(&"https://api.test".to_string())
    );

    // 2. LocalEnvironmentOverrides persistence
    let mut overrides = LocalEnvironmentOverrides::default();
    let mut dev_vars = BTreeMap::new();
    dev_vars.insert("base_url".into(), "http://localhost:8080".into());
    overrides.overrides.insert("env_dev_123".into(), dev_vars);
    overrides.save(root).unwrap();

    let reloaded_overrides = LocalEnvironmentOverrides::load(root).unwrap();
    let dev_overrides = reloaded_overrides.get_overrides("env_dev_123", "Development");
    assert_eq!(
        dev_overrides.get("base_url"),
        Some(&"http://localhost:8080".to_string())
    );
}

#[test]
fn variable_description_and_hover_lookup() {
    let mut vars = Variables::default();
    vars.global.insert("api_host".into(), "global.host".into());
    vars.collection.insert("api_key".into(), "col_key".into());

    let (scope, val, is_secret) = describe_variable("api_host", &vars, None).unwrap();
    assert_eq!(scope, "Global");
    assert_eq!(val, "global.host");
    assert!(!is_secret);

    let (scope, val, _) = describe_variable("api_key", &vars, None).unwrap();
    assert_eq!(scope, "Collection/Folder");
    assert_eq!(val, "col_key");

    let (scope, _, _) = describe_variable("$guid", &vars, None).unwrap();
    assert_eq!(scope, "Dynamic");
}
