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
const HELP:&str="Usage: rustapi [--workspace DIR] [--version | --help | --smoke-test]\n       rustapi run COLLECTION.json [--workspace DIR] [--environment FILE] [--folder ID]\n           [--data FILE.csv|FILE.json] [--iterations N] [--stop-on-failure] [--delay MS]\n           [--report-json FILE] [--report-junit FILE] [--debug-report FILE]\n       rustapi import FILE.json [--workspace DIR] [--import-report] [--allow-lossy-import]\n       rustapi import --curl 'curl ...' [--workspace DIR]\n       rustapi export INPUT.json OUTPUT.json [--format v2.1|environment|native|v3] [--include-credentials] [--workspace DIR]\n       rustapi vault set NAME [--workspace DIR]\n       rustapi vault delete NAME [--workspace DIR]\nImports with known omissions require --allow-lossy-import (also available for run).\n--import-report prints JSON diagnostics instead of saved paths.\nVault commands use RUSTAPI_VAULT_PASSWORD; set reads the value from stdin.\n";
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
        ["run", "import", "export", "vault"].contains(&command),
        "Unknown command {command}\n{HELP}"
    );
    let mut options = BTreeMap::new();
    let mut positional = vec![];
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if [
            "--stop-on-failure",
            "--allow-lossy-import",
            "--import-report",
            "--include-credentials",
        ]
        .contains(&a.as_str())
        {
            anyhow::ensure!(
                options.insert(a.clone(), String::new()).is_none(),
                "Duplicate option {a}"
            );
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
                    "--debug-report",
                    "--curl",
                    "--format"
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
    anyhow::ensure!(
        command == "import" || !options.contains_key("--import-report"),
        "--import-report is only available for import"
    );
    anyhow::ensure!(
        command != "vault" || !options.contains_key("--allow-lossy-import"),
        "--allow-lossy-import is only available for import or run"
    );
    anyhow::ensure!(
        command == "export"
            || (!options.contains_key("--format")
                && !options.contains_key("--include-credentials")),
        "Export options are only available for export"
    );
    anyhow::ensure!(
        command == "run" || !options.contains_key("--debug-report"),
        "--debug-report is only available for run"
    );
    let allow_lossy = options.contains_key("--allow-lossy-import");
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
        "export" => {
            anyhow::ensure!(
                positional.len() == 2,
                "export needs an input and output path"
            );
            let source = absolute(&positional[0])?;
            let target = absolute(&positional[1])?;
            anyhow::ensure!(
                source.canonicalize().ok() != target.canonicalize().ok() || !target.exists(),
                "Export output must differ from input"
            );
            if options.get("--format").is_some_and(|format| format == "v3") {
                let imported = crate::import::path(&source)?;
                review_import(&imported, allow_lossy, false)?;
                let collection = imported.collection.context("Expected collection")?;
                for note in crate::v3::export_directory(
                    &collection,
                    &target,
                    options.contains_key("--include-credentials"),
                )? {
                    eprintln!("Export note: {note}");
                }
                println!("{}", target.display());
                return Ok(Some(0));
            }
            let include = options.contains_key("--include-credentials");
            let format = options
                .get("--format")
                .map(String::as_str)
                .unwrap_or("v2.1");
            let exported = if format == "environment" {
                let bytes = std::fs::read(&source)?;
                let source_value: serde_json::Value = serde_json::from_slice(&bytes)?;
                let environment = if source_value["values"].is_array() {
                    let imported = crate::import::json(&bytes)?;
                    review_import(&imported, allow_lossy, false)?;
                    imported.environment.context("Expected environment")?
                } else {
                    serde_json::from_value(source_value)?
                };
                crate::export::environment(&environment, include)?
            } else {
                let imported = crate::import::path(&source)?;
                review_import(&imported, allow_lossy, false)?;
                let collection = imported.collection.context("Expected collection")?;
                match format {
                    "v2.1" => crate::export::collection(&collection, include)?,
                    "native" => crate::export::PortableExport {
                        document: serde_json::to_value(if include {
                            collection
                        } else {
                            crate::secrets::redact_collection(&collection)
                        })?,
                        warnings: vec![],
                    },
                    _ => bail!("Unknown export format; choose v2.1, environment or native"),
                }
            };
            workspace::atomic_write(&target, &serde_json::to_vec_pretty(&exported.document)?)?;
            for warning in exported.warnings {
                eprintln!("Export note: {warning}");
            }
            println!("{}", target.display());
            Ok(Some(0))
        }
        "run" => {
            anyhow::ensure!(
                positional.len() == 1,
                "run needs one collection file\n{HELP}"
            );
            let path = absolute(&positional[0])?;
            let imported = crate::import::path(&path)?;
            review_import(&imported, allow_lossy, false)?;
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
                        let imported = crate::import::json(&bytes)?;
                        review_import(&imported, allow_lossy, false)?;
                        imported.environment.context("Expected an environment")
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
            let service = HttpService::default();
            let report = runner::run(&service, &c, env.as_ref(), &settings, secrets, &cancel)?;
            let json = serde_json::to_vec_pretty(&report)?;
            if let Some(p) = options.get("--report-json") {
                workspace::atomic_write(&absolute(p)?, &json)?;
            }
            if let Some(p) = options.get("--report-junit") {
                workspace::atomic_write(&absolute(p)?, report.junit().as_bytes())?;
            }
            if let Some(path) = options.get("--debug-report") {
                workspace::atomic_write(
                    &absolute(path)?,
                    &serde_json::to_vec_pretty(
                        &serde_json::json!({"schema_version":1,"events":service.debug.snapshot()}),
                    )?,
                )?;
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
                crate::import::path(&absolute(&positional[0])?)?
            };
            let machine_report = options.contains_key("--import-report");
            review_import(&imported, allow_lossy, machine_report)?;
            // Parsing, validation and omission review finish before project data is written.
            for warning in imported.warnings {
                eprintln!("Import warning: {warning}");
            }
            if let Some(c) = imported.collection {
                let saved = storage::save_collection(&c, None)?;
                if !machine_report {
                    println!("{}", saved.display());
                }
            }
            if let Some(e) = imported.environment {
                let saved = storage::save_environment(&e)?;
                if !machine_report {
                    println!("{}", saved.display());
                }
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

fn review_import(
    imported: &crate::import::ImportResult,
    allow_lossy: bool,
    machine_report: bool,
) -> Result<()> {
    if machine_report {
        println!("{}", serde_json::to_string_pretty(&imported.report())?);
    }
    for diagnostic in &imported.diagnostics {
        eprintln!(
            "Import warning [{}] at {}: {}",
            diagnostic.feature, diagnostic.path, diagnostic.message
        );
    }
    anyhow::ensure!(!imported.requires_confirmation() || allow_lossy,
        "Import contains known omissions; review the diagnostics and use --allow-lossy-import to continue");
    Ok(())
}
