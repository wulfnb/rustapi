//! Headless companion tools; services are also shared by the desktop tools window.
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};
pub const HELP: &str = r#"Additional tools:
  rustapi spec lint SPEC.json|yaml
  rustapi spec format INPUT OUTPUT.json
  rustapi spec drift OLD NEW
  rustapi docs COLLECTION OUTPUT_DIR [--include-credentials]
  rustapi mock create COLLECTION CONFIG.json
  rustapi mock serve CONFIG.json
  rustapi automation save PROFILE.json | list | run ID | history | compare BEFORE AFTER | watch
  rustapi automation dataset INPUT.csv|json NAME.csv|json
  rustapi load run PROFILE.json | compare BEFORE.json AFTER.json
  rustapi protocol run SESSION.json
  rustapi flow save|run FLOW.json
  rustapi team init CONFIG | token CONFIG NAME ROLE DAYS | serve CONFIG DATA_DIR
  rustapi team catalog ENDPOINT | pull ENDPOINT ID OUTPUT | push ENDPOINT COLLECTION REVISION
  rustapi ai models ENDPOINT | draft ENDPOINT MODEL PROMPT.txt OUTPUT.json
  rustapi mcp serve [--allow-execution]
  rustapi secrets check
  rustapi policy template OUTPUT | validate
All tools accept --workspace DIR. Import review uses --allow-lossy-import.
Team clients use RUSTAPI_TEAM_TOKEN; encrypted state uses RUSTAPI_VAULT_PASSWORD.
See docs/advanced-tools.md for profiles, configuration and compatibility limits.
"#;
pub fn read(path: &Path, limit: u64) -> Result<String> {
    ensure!(
        std::fs::metadata(path)?.len() <= limit,
        "Input exceeds size limit"
    );
    std::fs::read_to_string(path).context("Read UTF-8 input")
}
pub fn collection(path: &Path, allow_lossy: bool) -> Result<crate::models::Collection> {
    let imported = crate::import::path(path)?;
    ensure!(allow_lossy||!imported.requires_confirmation(),"Import has omissions; review with rustapi import --import-report and use --allow-lossy-import to accept");
    imported.collection.context("Expected a collection")
}
pub fn dispatch(args: &[String]) -> Result<Option<i32>> {
    let Some(command) = args.first().map(String::as_str).filter(|c| {
        [
            "spec",
            "docs",
            "mock",
            "automation",
            "load",
            "protocol",
            "team",
            "flow",
            "ai",
            "mcp",
            "secrets",
            "policy",
        ]
        .contains(c)
    }) else {
        return Ok(None);
    };
    let mut positional = vec![];
    let mut options = BTreeMap::new();
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if [
            "--include-credentials",
            "--allow-lossy-import",
            "--allow-execution",
        ]
        .contains(&a.as_str())
        {
            ensure!(
                options.insert(a.clone(), String::new()).is_none(),
                "Duplicate option"
            );
        } else if a == "--workspace" {
            i += 1;
            let path = args.get(i).context("Missing workspace")?;
            ensure!(
                options.insert(a.clone(), path.clone()).is_none(),
                "Duplicate workspace"
            );
        } else {
            ensure!(!a.starts_with("--"), "Unknown option {a}");
            positional.push(a.clone());
        }
        i += 1;
    }
    if let Some(root) = options.get("--workspace") {
        let root = crate::workspace::open_workspace(Path::new(root))?;
        std::env::set_var("RUSTAPI_DATA_DIR", root);
    }
    let allow = options.contains_key("--allow-lossy-import");
    let path = |index: usize| -> Result<PathBuf> {
        positional
            .get(index)
            .map(PathBuf::from)
            .context("Missing input path")
    };
    let code = match command {
        "team" | "flow" | "ai" | "mcp" | "secrets" | "policy" => {
            let root = crate::workspace::open_workspace(&crate::storage::get_app_data_dir())?;
            crate::platform::cli::run(
                command,
                &positional,
                &root,
                options.contains_key("--allow-execution"),
            )?
        }
        "protocol" => {
            ensure!(
                positional.len() == 2 && positional[0] == "run",
                "Usage: rustapi protocol run SESSION.json [--workspace DIR]"
            );
            let root = crate::workspace::open_workspace(&crate::storage::get_app_data_dir())?;
            let profile = serde_json::from_str(&read(&path(1)?, 8 * 1024 * 1024)?)?;
            let (service, secrets) = unlocked(&root)?;
            let variables = crate::variables::Variables {
                global: crate::storage::GlobalVariables::load(&root)?.variables,
                secrets,
                ..Default::default()
            };
            let cancel = crate::http::Cancellation::default();
            let signal = cancel.clone();
            ctrlc::set_handler(move || signal.cancel())?;
            let summary =
                crate::protocols::run(&service, &profile, &variables, &cancel, None, |event| {
                    println!("{}", serde_json::to_string(&event).unwrap_or_default())
                })?;
            eprintln!("{}", serde_json::to_string(&summary)?);
            i32::from(summary.reason == "cancelled")
        }
        "load" => match positional.first().map(String::as_str) {
            Some("run") if positional.len() == 2 => {
                let root = crate::workspace::open_workspace(&crate::storage::get_app_data_dir())?;
                let profile = serde_json::from_str(&read(&path(1)?, 8 * 1024 * 1024)?)?;
                let (_, secrets) = unlocked(&root)?;
                let cancel = crate::http::Cancellation::default();
                let signal = cancel.clone();
                ctrlc::set_handler(move || signal.cancel())?;
                let report = crate::performance::run(&root, &profile, secrets, &cancel)?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                i32::from(!report.passed)
            }
            Some("compare") if positional.len() == 3 => {
                let before = serde_json::from_str(&read(&path(1)?, 8 * 1024 * 1024)?)?;
                let after = serde_json::from_str(&read(&path(2)?, 8 * 1024 * 1024)?)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&crate::performance::compare(&before, &after))?
                );
                0
            }
            _ => bail!("{HELP}"),
        },
        "automation" => {
            let root = crate::workspace::open_workspace(&crate::storage::get_app_data_dir())?;
            match positional.first().map(String::as_str) {
                Some("save") if positional.len() == 2 => {
                    let profile = serde_json::from_str(&read(&path(1)?, 8 * 1024 * 1024)?)?;
                    println!("{}", crate::automation::save(&root, &profile)?.display());
                    0
                }
                Some("list") if positional.len() == 1 => {
                    let profiles = crate::automation::profiles(&root)?;
                    println!("{}",serde_json::to_string_pretty(&profiles.iter().map(|p|serde_json::json!({"id":p.id,"name":p.name,"interval_seconds":p.interval_seconds})).collect::<Vec<_>>())?);
                    0
                }
                Some("dataset") if positional.len() == 3 => {
                    println!(
                        "{}",
                        crate::automation::import_dataset(&root, &path(1)?, &positional[2])?
                            .display()
                    );
                    0
                }
                Some("history") if positional.len() == 1 => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&crate::automation::history(&root)?)?
                    );
                    0
                }
                Some("compare") if positional.len() == 3 => {
                    let history = crate::automation::history(&root)?;
                    let before = history
                        .iter()
                        .find(|r| r.id == positional[1])
                        .context("Before run missing")?;
                    let after = history
                        .iter()
                        .find(|r| r.id == positional[2])
                        .context("After run missing")?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&crate::automation::compare(before, after))?
                    );
                    0
                }
                Some("run" | "watch") => {
                    let (service, secrets) = unlocked(&root)?;
                    let cancel = crate::http::Cancellation::default();
                    let signal = cancel.clone();
                    ctrlc::set_handler(move || signal.cancel())?;
                    if positional[0] == "watch" {
                        ensure!(positional.len() == 1, "{HELP}");
                        crate::automation::watch(&root, &service, secrets, &cancel, |event| {
                            match event{Ok(record)=>println!("{}",serde_json::to_string(&record).unwrap_or_default()),Err(_)=>eprintln!("Scheduled run failed; check profile inputs and vault configuration")}
                        })?;
                        0
                    } else {
                        ensure!(positional.len() == 2, "{HELP}");
                        let profile = crate::automation::profiles(&root)?
                            .into_iter()
                            .find(|p| p.id == positional[1])
                            .context("Run profile missing")?;
                        let record =
                            crate::automation::run(&root, &service, &profile, secrets, &cancel)?;
                        println!("{}", serde_json::to_string_pretty(&record)?);
                        i32::from(!record.report.passed())
                    }
                }
                _ => bail!("{HELP}"),
            }
        }
        "spec" => match positional.first().map(String::as_str) {
            Some("lint") if positional.len() == 2 => {
                let spec =
                    crate::openapi::Specification::parse(&read(&path(1)?, 8 * 1024 * 1024)?)?;
                let issues = spec.lint();
                println!("{}", serde_json::to_string_pretty(&issues)?);
                i32::from(issues.iter().any(|issue| issue.severity == "error"))
            }
            Some("format") if positional.len() == 3 => {
                let spec =
                    crate::openapi::Specification::parse(&read(&path(1)?, 8 * 1024 * 1024)?)?;
                ensure!(
                    path(1)?.canonicalize().ok() != path(2)?.canonicalize().ok()
                        || !path(2)?.exists(),
                    "Output must differ from input"
                );
                crate::workspace::atomic_write(
                    &path(2)?,
                    &serde_json::to_vec_pretty(&spec.value)?,
                )?;
                0
            }
            Some("drift") if positional.len() == 3 => {
                let old = crate::openapi::Specification::parse(&read(&path(1)?, 8 * 1024 * 1024)?)?;
                let new = crate::openapi::Specification::parse(&read(&path(2)?, 8 * 1024 * 1024)?)?;
                let changes = old.drift(&new)?;
                println!("{}", serde_json::to_string_pretty(&changes)?);
                i32::from(changes.iter().any(|c| c.requires_review))
            }
            _ => bail!("{HELP}"),
        },
        "docs" => {
            ensure!(positional.len() == 2, "{HELP}");
            crate::documentation::write(
                &collection(&path(0)?, allow)?,
                &path(1)?,
                options.contains_key("--include-credentials"),
            )?;
            println!("{}", path(1)?.display());
            0
        }
        "mock" => match positional.first().map(String::as_str) {
            Some("create") if positional.len() == 3 => {
                ensure!(!path(2)?.exists(), "Mock config already exists");
                let config =
                    crate::mocks::MockConfig::from_examples(&collection(&path(1)?, allow)?)?;
                crate::workspace::atomic_write(&path(2)?, &serde_json::to_vec_pretty(&config)?)?;
                0
            }
            Some("serve") if positional.len() == 2 => {
                let config = serde_json::from_str(&read(&path(1)?, 16 * 1024 * 1024)?)?;
                let server = crate::mocks::MockServer::start(config)?;
                let cancel = crate::http::Cancellation::default();
                let signal = cancel.clone();
                ctrlc::set_handler(move || signal.cancel())?;
                println!("Mock listening on http://{}", server.server.address);
                while !cancel.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(100));
                }
                0
            }
            _ => bail!("{HELP}"),
        },
        _ => unreachable!(),
    };
    Ok(Some(code))
}
pub fn unlocked(root: &Path) -> Result<(crate::http::HttpService, BTreeMap<String, String>)> {
    let service = crate::http::HttpService::for_workspace(root);
    let mut secrets = BTreeMap::new();
    if let Ok(password) = std::env::var("RUSTAPI_VAULT_PASSWORD") {
        secrets = crate::secrets::load(&root.join(".local/vault.json"), &password)?;
        service.unlock_tokens(&root.join(".local/oauth-tokens.json"), &password)?;
        let path = root.join(".local/cookies.json");
        if path.exists() {
            service.cookies.unlock(&path, &password)?;
        }
    }
    secrets.extend(crate::platform::external_secrets::load(
        root,
        &service,
        &crate::http::Cancellation::default(),
    )?);
    Ok((service, secrets))
}
