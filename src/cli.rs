//! Headless entry points. Exit 1: failed run/cancel; exit 2: configuration/import error.
use crate::{
    http::{Cancellation, HttpService},
    models::*,
    runner::{self, RunOptions},
    storage, workspace,
};
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
const HELP:&str="Usage: rustapi [--workspace DIR] [--version | --help | --smoke-test]\n       rustapi run COLLECTION.json [--workspace DIR] [--environment FILE] [--folder ID]\n           [--data FILE.csv|FILE.json] [--iterations N] [--stop-on-failure] [--delay MS]\n           [--report-json FILE] [--report-junit FILE]\n       rustapi import FILE.json [--workspace DIR]\n       rustapi import --curl 'curl ...' [--workspace DIR]\n       rustapi vault set NAME [--workspace DIR]\n       rustapi vault delete NAME [--workspace DIR]\nVault commands use RUSTAPI_VAULT_PASSWORD; set reads the value from stdin.\n";
pub fn dispatch(args: &[String]) -> Result<Option<i32>> {
    if args == ["--version"] {
        println!("RustAPI {}", env!("CARGO_PKG_VERSION"));
        return Ok(Some(0));
    }
    if args == ["--help"] {
        print!("{HELP}");
        return Ok(Some(0));
    }
    if args.is_empty() || args == ["--smoke-test"] {
        return Ok(None);
    }
    if args.first().is_some_and(|a| a == "--workspace") && args.len() == 2 {
        let path = workspace::open_workspace(Path::new(&args[1]))?;
        std::env::set_var("RUSTAPI_DATA_DIR", path);
        return Ok(None);
    }
    let command = args.first().context("Missing command")?.as_str();
    anyhow::ensure!(
        ["run", "import", "vault"].contains(&command),
        "Unknown command {command}\n{HELP}"
    );
    let mut options = BTreeMap::new();
    let mut positional = vec![];
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if a == "--stop-on-failure" {
            options.insert(a.clone(), String::new());
        } else if a.starts_with("--") {
            anyhow::ensure!(
                [
                    "--workspace",
                    "--environment",
                    "--folder",
                    "--data",
                    "--iterations",
                    "--delay",
                    "--report-json",
                    "--report-junit",
                    "--curl"
                ]
                .contains(&a.as_str()),
                "Unknown option {a}\n{HELP}"
            );
            i += 1;
            let value = args.get(i).context("Missing option value")?;
            anyhow::ensure!(!value.starts_with("--"), "Missing option value");
            anyhow::ensure!(
                options.insert(a.clone(), value.clone()).is_none(),
                "Duplicate option {a}"
            );
        } else {
            positional.push(a.clone());
        }
        i += 1;
    }
    let root = if let Some(path) = options.get("--workspace") {
        workspace::open_workspace(Path::new(path))?
    } else {
        storage::get_app_data_dir()
    };
    let absolute = |v: &str| -> Result<PathBuf> {
        let p = Path::new(v);
        if p.is_absolute() {
            Ok(p.into())
        } else {
            Ok(std::env::current_dir()?.join(p))
        }
    };
    std::env::set_var("RUSTAPI_DATA_DIR", &root);
    storage::init_storage()?;
    match command {
        "run" => {
            anyhow::ensure!(
                positional.len() == 1,
                "run needs one collection file\n{HELP}"
            );
            let path = absolute(&positional[0])?;
            let imported = crate::import::json(&std::fs::read(&path)?)?;
            let mut c = imported.collection.context("Expected a collection")?;
            c.file_path = Some(path.to_string_lossy().into());
            for w in imported.warnings {
                eprintln!("Import warning: {w}");
            }
            let env = options
                .get("--environment")
                .map(|p| -> Result<Environment> {
                    let bytes = std::fs::read(absolute(p)?)?;
                    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
                    if v["values"].is_array() {
                        crate::import::json(&bytes)?
                            .environment
                            .context("Expected an environment")
                    } else {
                        Ok(serde_json::from_value(v)?)
                    }
                })
                .transpose()?;
            let data = options
                .get("--data")
                .map(|p| runner::load_data(&absolute(p)?))
                .transpose()?
                .unwrap_or_default();
            let settings = RunOptions {
                folder: options.get("--folder").cloned(),
                iterations: options
                    .get("--iterations")
                    .map(|n| n.parse())
                    .transpose()?
                    .unwrap_or(0),
                stop_on_failure: options.contains_key("--stop-on-failure"),
                delay_ms: options
                    .get("--delay")
                    .map(|n| n.parse())
                    .transpose()?
                    .unwrap_or(0),
                data,
            };
            let secrets = load_secrets(&root)?;
            let cancel = Cancellation::default();
            let signal = cancel.clone();
            ctrlc::set_handler(move || signal.cancel()).context("Install interrupt handler")?;
            let report = runner::run(
                &HttpService::default(),
                &c,
                env.as_ref(),
                &settings,
                secrets,
                &cancel,
            )?;
            let json = serde_json::to_vec_pretty(&report)?;
            if let Some(p) = options.get("--report-json") {
                workspace::atomic_write(&absolute(p)?, &json)?;
            }
            if let Some(p) = options.get("--report-junit") {
                workspace::atomic_write(&absolute(p)?, report.junit().as_bytes())?;
            }
            println!("{}", String::from_utf8(json)?);
            Ok(Some(if report.passed() { 0 } else { 1 }))
        }
        "import" => {
            let imported = if let Some(curl) = options.get("--curl") {
                anyhow::ensure!(positional.is_empty(), "Choose a JSON file or --curl");
                crate::import::curl(curl)?
            } else {
                anyhow::ensure!(positional.len() == 1, "import needs one JSON file\n{HELP}");
                crate::import::json(&std::fs::read(absolute(&positional[0])?)?)?
            };
            // Parsing and validation finish before any collection/environment file is written.
            for warning in imported.warnings {
                eprintln!("Import warning: {warning}");
            }
            if let Some(c) = imported.collection {
                println!("{}", storage::save_collection(&c, None)?.display());
            }
            if let Some(e) = imported.environment {
                println!("{}", storage::save_environment(&e)?.display());
            }
            Ok(Some(0))
        }
        "vault" => {
            anyhow::ensure!(
                positional.len() == 2,
                "vault needs set/delete and a name\n{HELP}"
            );
            let password = std::env::var("RUSTAPI_VAULT_PASSWORD")
                .context("Set RUSTAPI_VAULT_PASSWORD to unlock the vault")?;
            let path = root.join(".local/vault.json");
            let mut values = crate::secrets::load(&path, &password)?;
            match positional[0].as_str() {
                "set" => {
                    use std::io::Read;
                    let mut value = String::new();
                    std::io::stdin()
                        .take(1024 * 1024)
                        .read_to_string(&mut value)?;
                    values.insert(
                        positional[1].clone(),
                        value.trim_end_matches(['\r', '\n']).into(),
                    );
                }
                "delete" => {
                    values.remove(&positional[1]);
                }
                _ => bail!("Unknown vault command\n{HELP}"),
            }
            crate::secrets::save(&path, &password, &values)?;
            Ok(Some(0))
        }
        _ => bail!("Unknown command {command}\n{HELP}"),
    }
}
fn load_secrets(root: &Path) -> Result<BTreeMap<String, String>> {
    let path = root.join(".local/vault.json");
    match std::env::var("RUSTAPI_VAULT_PASSWORD") {
        Ok(password) => crate::secrets::load(&path, &password),
        Err(_) => Ok(BTreeMap::new()),
    }
}
