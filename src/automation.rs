//! Saved run profiles, managed datasets, history and an opt-in local scheduler.
use crate::{
    http::{Cancellation, HttpService},
    runner::{RunOptions, RunReport},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunProfile {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub collection: String,
    pub environment: Option<String>,
    pub dataset: Option<String>,
    pub options: RunOptions,
    pub interval_seconds: Option<u64>,
    pub alert_url_env: Option<String>,
}
impl Default for RunProfile {
    fn default() -> Self {
        Self {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            name: "Saved run".into(),
            collection: String::new(),
            environment: None,
            dataset: None,
            options: RunOptions::default(),
            interval_seconds: None,
            alert_url_env: None,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub version: u32,
    pub id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub started_at: String,
    pub report: RunReport,
    pub alert_error: Option<String>,
}
#[derive(Serialize)]
pub struct Comparison {
    pub before_passed: bool,
    pub after_passed: bool,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}
pub fn relative(root: &Path, value: &str) -> Result<PathBuf> {
    ensure!(
        !value.is_empty() && !Path::new(value).is_absolute(),
        "Profile paths must be workspace-relative"
    );
    crate::storage::owned_path(root, &value.split('/').collect::<Vec<_>>())
}
fn profile_path(root: &Path, id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id).context("Profile ID must be a UUID")?;
    crate::storage::owned_path(root, &["runs", "profiles", &format!("{id}.json")])
}
pub fn validate(root: &Path, profile: &RunProfile) -> Result<()> {
    ensure!(
        profile.version == 1 && !profile.name.trim().is_empty() && profile.name.len() <= 256,
        "Invalid run profile version/name"
    );
    profile_path(root, &profile.id)?;
    ensure!(
        relative(root, &profile.collection)?.exists(),
        "Profile collection is missing"
    );
    if let Some(path) = &profile.environment {
        relative(root, path)?;
    }
    if let Some(path) = &profile.dataset {
        relative(root, path)?;
        ensure!(
            profile.options.data.is_empty(),
            "Choose a managed dataset or inline data"
        );
    }
    if let Some(interval) = profile.interval_seconds {
        ensure!(
            (60..=86400 * 30).contains(&interval),
            "Schedule interval must be 60 seconds to 30 days"
        );
    }
    if let Some(name) = &profile.alert_url_env {
        ensure!(
            !name.is_empty()
                && name.len() <= 128
                && name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
            "Alert URL must refer to an environment variable"
        );
    }
    ensure!(
        profile.options.iterations <= 10000 && profile.options.data.len() <= 10000,
        "Run profile exceeds iteration/data limits"
    );
    Ok(())
}
pub fn save(root: &Path, profile: &RunProfile) -> Result<PathBuf> {
    validate(root, profile)?;
    let path = profile_path(root, &profile.id)?;
    crate::workspace::atomic_write(&path, &serde_json::to_vec_pretty(profile)?)?;
    Ok(path)
}
pub fn profiles(root: &Path) -> Result<Vec<RunProfile>> {
    let directory = crate::storage::owned_path(root, &["runs", "profiles"])?;
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut profiles = vec![];
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.path().extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        ensure!(profiles.len() < 100, "At most 100 saved run profiles");
        let path = profile_path(
            root,
            entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .context("Invalid profile filename")?,
        )?;
        let text = crate::tools_cli::read(&path, 8 * 1024 * 1024)?;
        let profile: RunProfile = serde_json::from_str(&text)?;
        ensure!(
            profile_path(root, &profile.id)? == path,
            "Profile identity does not match filename"
        );
        profiles.push(profile);
    }
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(profiles)
}
pub fn import_dataset(root: &Path, source: &Path, name: &str) -> Result<PathBuf> {
    ensure!(
        std::fs::metadata(source)?.len() <= 8 * 1024 * 1024,
        "Dataset exceeds 8 MiB"
    );
    ensure!(
        name.ends_with(".csv") || name.ends_with(".json"),
        "Dataset filename must end in .csv or .json"
    );
    ensure!(
        source.extension() == Path::new(name).extension(),
        "Dataset extension must match its format"
    );
    let rows = crate::runner::load_data(source)?;
    ensure!(
        !rows.is_empty() && rows.len() <= 10000,
        "Dataset requires 1–10000 rows"
    );
    let path = crate::storage::owned_path(root, &["datasets", name])?;
    ensure!(!path.exists(), "Managed dataset already exists");
    crate::workspace::atomic_write(&path, &std::fs::read(source)?)?;
    Ok(path)
}
pub fn inputs(
    root: &Path,
    profile: &RunProfile,
) -> Result<(
    crate::models::Collection,
    Option<crate::models::Environment>,
    RunOptions,
)> {
    validate(root, profile)?;
    let collection = crate::tools_cli::collection(&relative(root, &profile.collection)?, false)?;
    let environment = profile
        .environment
        .as_ref()
        .map(|path| -> Result<_> {
            let imported = crate::import::path(&relative(root, path)?)?;
            ensure!(
                !imported.requires_confirmation(),
                "Environment import needs omission review"
            );
            imported.environment.context("Expected environment")
        })
        .transpose()?;
    let mut options = profile.options.clone();
    options.workspace_root = Some(root.into());
    if let Some(path) = &profile.dataset {
        let path = relative(root, path)?;
        ensure!(
            std::fs::metadata(&path)?.len() <= 8 * 1024 * 1024,
            "Dataset exceeds 8 MiB"
        );
        options.data = crate::runner::load_data(&path)?;
        ensure!(
            !options.data.is_empty() && options.data.len() <= 10000,
            "Dataset requires 1–10000 rows"
        );
    }
    Ok((collection, environment, options))
}
pub fn run(
    root: &Path,
    service: &HttpService,
    profile: &RunProfile,
    secrets: BTreeMap<String, String>,
    cancel: &Cancellation,
) -> Result<RunRecord> {
    let (collection, environment, options) = inputs(root, profile)?;
    let started = chrono::Utc::now();
    let report = crate::runner::run(
        service,
        &collection,
        environment.as_ref(),
        &options,
        secrets,
        cancel,
    )?;
    let mut record = RunRecord {
        version: 1,
        id: format!(
            "{}-{}",
            started.format("%Y%m%dT%H%M%SZ"),
            uuid::Uuid::new_v4()
        ),
        profile_id: profile.id.clone(),
        profile_name: profile.name.clone(),
        started_at: started.to_rfc3339(),
        report,
        alert_error: None,
    };
    save_record(root, &record)?;
    if !record.report.passed() && !cancel.is_cancelled() {
        if let Some(variable) = &profile.alert_url_env {
            if alert(service, variable, &record).is_err() {
                record.alert_error = Some(
                    "Alert delivery failed; verify its URL environment variable and endpoint"
                        .into(),
                );
                save_record(root, &record)?;
            }
        }
    }
    Ok(record)
}
fn save_record(root: &Path, record: &RunRecord) -> Result<()> {
    let path =
        crate::storage::owned_path(root, &[".local", "runs", &format!("{}.json", record.id)])?;
    let bytes = serde_json::to_vec_pretty(record)?;
    ensure!(bytes.len() <= 32 * 1024 * 1024, "Run record exceeds 32 MiB");
    crate::workspace::atomic_write(&path, &bytes)?;
    retain_records(root, "runs")
}
pub(crate) fn retain_records(root: &Path, directory: &str) -> Result<()> {
    let path = crate::storage::owned_path(root, &[".local", directory])?;
    // Keep the newest 200 records. Never traverse symlinks or delete unrelated files.
    let mut files = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| valid_record_name(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect::<Vec<_>>();
    files.sort();
    let excess = files.len().saturating_sub(200);
    for old in files.into_iter().take(excess) {
        std::fs::remove_file(old)?;
    }
    Ok(())
}
fn valid_record_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".json") else {
        return false;
    };
    stem.is_ascii()
        && stem.len() == 53
        && chrono::NaiveDateTime::parse_from_str(&stem[..16], "%Y%m%dT%H%M%SZ").is_ok()
        && stem.as_bytes()[16] == b'-'
        && uuid::Uuid::parse_str(&stem[17..]).is_ok()
}
pub fn history(root: &Path) -> Result<Vec<RunRecord>> {
    let path = crate::storage::owned_path(root, &[".local", "runs"])?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let mut records = vec![];
    let mut total = 0_u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !valid_record_name(&name) {
            continue;
        }
        ensure!(records.len() < 1000, "Run history exceeds 1000 records");
        let path = crate::storage::owned_path(root, &[".local", "runs", &name])?;
        total = total.saturating_add(std::fs::metadata(&path)?.len());
        ensure!(
            total <= 128 * 1024 * 1024,
            "Run history exceeds 128 MiB; archive older records before loading"
        );
        let record: RunRecord =
            serde_json::from_str(&crate::tools_cli::read(&path, 32 * 1024 * 1024)?)?;
        ensure!(
            record.version == 1 && format!("{}.json", record.id) == name,
            "Invalid run record"
        );
        records.push(record);
    }
    records.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(records)
}
pub fn compare(before: &RunRecord, after: &RunRecord) -> Comparison {
    fn entries(record: &RunRecord) -> BTreeMap<(usize, String), &crate::runner::RunEntry> {
        record
            .report
            .entries
            .iter()
            .map(|entry| ((entry.iteration, entry.request_id.clone()), entry))
            .collect::<BTreeMap<_, _>>()
    }
    let a = entries(before);
    let b = entries(after);
    let mut result = Comparison {
        before_passed: before.report.passed(),
        after_passed: after.report.passed(),
        added: vec![],
        removed: vec![],
        changed: vec![],
    };
    for (key, old) in &a {
        match b.get(key) {
            None => result.removed.push(old.name.clone()),
            Some(new) if old.passed() != new.passed() || old.status != new.status => {
                result.changed.push(format!(
                    "{}: status {} → {}, passed {} → {}",
                    old.name,
                    old.status,
                    new.status,
                    old.passed(),
                    new.passed()
                ))
            }
            _ => {}
        }
    }
    for (key, new) in &b {
        if !a.contains_key(key) {
            result.added.push(new.name.clone());
        }
    }
    result
}
fn alert(service: &HttpService, variable: &str, record: &RunRecord) -> Result<()> {
    let url = reqwest::Url::parse(&std::env::var(variable).context("Missing alert URL")?)?;
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && (url.scheme() == "https"
                || url.scheme() == "http"
                    && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))),
        "Alerts require HTTPS or loopback HTTP"
    );
    service.policy()?.url(url.as_str())?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response=client.post(url).json(&serde_json::json!({"event":"rustapi.run.failed","run_id":record.id,"profile_id":record.profile_id,"passed":false,"requests":record.report.entries.len(),"failed":record.report.entries.iter().filter(|e|!e.passed()).count()})).send()?;
    ensure!(
        response.status().is_success(),
        "Alert endpoint rejected delivery"
    );
    Ok(())
}
#[derive(Default, Serialize, Deserialize)]
struct ScheduleState {
    due: BTreeMap<String, u64>,
}
/// Run with a single process-wide workspace lock; missed intervals are coalesced.
pub fn watch(
    root: &Path,
    service: &HttpService,
    secrets: BTreeMap<String, String>,
    cancel: &Cancellation,
    mut event: impl FnMut(Result<RunRecord>),
) -> Result<()> {
    std::fs::create_dir_all(root.join(".local"))?;
    let lock_path = crate::storage::owned_path(root, &[".local", "scheduler.lock"])?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    lock.try_lock()
        .context("Another scheduler is active for this workspace")?;
    let state_path = crate::storage::owned_path(root, &[".local", "schedule-state.json"])?;
    let mut state: ScheduleState = if state_path.exists() {
        serde_json::from_str(&crate::tools_cli::read(&state_path, 1024 * 1024)?)?
    } else {
        ScheduleState::default()
    };
    while !cancel.is_cancelled() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        for profile in profiles(root)? {
            if cancel.is_cancelled() {
                break;
            }
            let Some(interval) = profile.interval_seconds else {
                continue;
            };
            ensure!(
                (60..=86400 * 30).contains(&interval),
                "Invalid schedule interval"
            );
            let due = *state.due.entry(profile.id.clone()).or_insert(now);
            if now < due {
                continue;
            }
            state
                .due
                .insert(profile.id.clone(), now.saturating_add(interval));
            crate::workspace::atomic_write(&state_path, &serde_json::to_vec(&state)?)?;
            event(run(root, service, &profile, secrets.clone(), cancel));
            let next = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs()
                .saturating_add(interval);
            state.due.insert(profile.id, next);
            crate::workspace::atomic_write(&state_path, &serde_json::to_vec(&state)?)?;
        }
        for _ in 0..10 {
            if cancel.is_cancelled() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}
