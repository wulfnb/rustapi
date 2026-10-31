//! Bounded concurrent virtual users. Each user has its own HTTP/cookie/token state.
use crate::{
    automation::RunProfile,
    http::{Cancellation, HttpService},
    runner::RunEntry,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoadProfile {
    pub run: RunProfile,
    pub users: usize,
    pub duration_seconds: u64,
    pub ramp_seconds: u64,
    pub pacing_ms: u64,
    pub max_requests: usize,
    pub stages: Vec<Stage>,
    pub max_error_rate: f64,
    pub max_p95_ms: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub users: usize,
    pub duration_seconds: u64,
}
impl Default for LoadProfile {
    fn default() -> Self {
        Self {
            run: RunProfile::default(),
            users: 4,
            duration_seconds: 10,
            ramp_seconds: 0,
            pacing_ms: 0,
            max_requests: 10000,
            stages: vec![],
            max_error_rate: 0.01,
            max_p95_ms: None,
        }
    }
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub requests: usize,
    pub errors: usize,
    pub min_ms: u128,
    pub mean_ms: f64,
    pub p50_ms: u128,
    pub p90_ms: u128,
    pub p95_ms: u128,
    pub p99_ms: u128,
    pub max_ms: u128,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct LoadReport {
    pub version: u32,
    pub id: String,
    pub profile_id: String,
    pub configuration_hash: String,
    pub started_at: String,
    pub elapsed_seconds: f64,
    pub cancelled: bool,
    pub throughput: f64,
    pub metrics: Metrics,
    pub endpoints: BTreeMap<String, Metrics>,
    pub passed: bool,
}
pub fn metrics(values: &[(u128, bool)]) -> Metrics {
    if values.is_empty() {
        return Metrics::default();
    }
    let mut times = values.iter().map(|v| v.0).collect::<Vec<_>>();
    times.sort_unstable();
    let percentile = |p: usize| {
        times[(times.len() * p)
            .div_ceil(100)
            .saturating_sub(1)
            .min(times.len() - 1)]
    };
    Metrics {
        requests: values.len(),
        errors: values.iter().filter(|v| v.1).count(),
        min_ms: times[0],
        mean_ms: times.iter().map(|v| *v as f64).sum::<f64>() / times.len() as f64,
        p50_ms: percentile(50),
        p90_ms: percentile(90),
        p95_ms: percentile(95),
        p99_ms: percentile(99),
        max_ms: *times.last().unwrap(),
    }
}
impl LoadProfile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=64).contains(&self.users) && self.stages.len() <= 100,
            "Load supports 1–64 users and at most 100 stages"
        );
        ensure!(
            (1..=3600).contains(&self.duration_seconds)
                && self.ramp_seconds <= self.duration_seconds,
            "Load duration must be 1–3600 seconds; ramp cannot exceed it"
        );
        ensure!(
            self.stages
                .iter()
                .all(|s| (1..=64).contains(&s.users) && (1..=3600).contains(&s.duration_seconds))
                && self.stages.iter().map(|s| s.duration_seconds).sum::<u64>() <= 3600,
            "Invalid load stages"
        );
        ensure!(
            (1..=100000).contains(&self.max_requests) && self.pacing_ms <= 60000,
            "Load supports 1–100000 requests and at most 60000 ms pacing"
        );
        ensure!(
            self.max_error_rate.is_finite() && (0.0..=1.0).contains(&self.max_error_rate),
            "Error-rate threshold must be 0–1"
        );
        Ok(())
    }
    fn duration(&self) -> u64 {
        if self.stages.is_empty() {
            self.duration_seconds
        } else {
            self.stages.iter().map(|s| s.duration_seconds).sum()
        }
    }
    fn users_at(&self, elapsed: Duration) -> usize {
        if !self.stages.is_empty() {
            let mut end = 0;
            for stage in &self.stages {
                end += stage.duration_seconds;
                if elapsed.as_secs() < end {
                    return stage.users;
                }
            }
            return 0;
        }
        if self.ramp_seconds == 0 {
            return self.users;
        }
        ((elapsed.as_secs_f64() / self.ramp_seconds as f64 * self.users as f64).ceil() as usize)
            .clamp(1, self.users)
    }
}
pub fn run(
    root: &Path,
    profile: &LoadProfile,
    secrets: BTreeMap<String, String>,
    cancel: &Cancellation,
) -> Result<LoadReport> {
    profile.validate()?;
    let (collection, environment, options) = crate::automation::inputs(root, &profile.run)?;
    // A saved login must be reauthorized per user; parallel refreshes cannot share one persisted pair.
    ensure!(
        collection
            .ordered_requests(options.folder.as_deref())
            .iter()
            .all(|r| !r.auth.oauth.persist_tokens),
        "Load profiles require session OAuth tokens; disable persistent OAuth for isolated users"
    );
    let users = profile
        .stages
        .iter()
        .map(|s| s.users)
        .max()
        .unwrap_or(profile.users);
    let started_at = chrono::Utc::now().to_rfc3339();
    let start = Instant::now();
    let stop = Cancellation::default();
    let claimed = Arc::new(AtomicUsize::new(0));
    let entries = Arc::new(Mutex::new(Vec::<RunEntry>::new()));
    let failed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    std::thread::scope(|scope| {
        let stop_monitor = stop.clone();
        let parent = cancel.clone();
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = finished.clone();
        scope.spawn(move || {
            while !done.load(Ordering::SeqCst)
                && !parent.is_cancelled()
                && start.elapsed() < Duration::from_secs(profile.duration())
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            stop_monitor.cancel();
        });
        let mut workers = vec![];
        for user in 0..users {
            let failed = failed.clone();
            let entries = entries.clone();
            let claimed = claimed.clone();
            let stop = stop.clone();
            let secrets = secrets.clone();
            let collection = &collection;
            let environment = &environment;
            let options = &options;
            workers.push(scope.spawn(move || {
                let service = HttpService::for_workspace(root);
                let mut iteration = 0;
                while !stop.is_cancelled() {
                    if user >= profile.users_at(start.elapsed()) {
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    let mut options = options.clone();
                    options.iterations = 1;
                    if !options.data.is_empty() {
                        options.data =
                            vec![options.data[(user + iteration) % options.data.len()].clone()];
                    }
                    if options.data.is_empty() {
                        options.data.push(BTreeMap::new());
                    }
                    options.data[0].insert("__rustapi_vu".into(), (user + 1).to_string());
                    options.data[0]
                        .insert("__rustapi_iteration".into(), (iteration + 1).to_string());
                    if claimed.load(Ordering::SeqCst) >= profile.max_requests {
                        break;
                    }
                    let gate = || {
                        claimed
                            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                                used.checked_add(1).filter(|n| *n <= profile.max_requests)
                            })
                            .is_ok()
                    };
                    match crate::runner::run_with_gate(
                        &service,
                        collection,
                        environment.as_ref(),
                        &options,
                        secrets.clone(),
                        &stop,
                        (|_| {}, gate),
                    ) {
                        Ok(report) => {
                            let mut output = entries.lock().unwrap_or_else(|p| p.into_inner());
                            for entry in report.entries {
                                if !(stop.is_cancelled() && entry.error.is_some())
                                    && output.len() < profile.max_requests
                                {
                                    output.push(entry);
                                }
                            }
                        }
                        Err(_) => {
                            failed.store(true, Ordering::SeqCst);
                            stop.cancel();
                            break;
                        }
                    }
                    iteration += 1;
                    let until = Instant::now() + Duration::from_millis(profile.pacing_ms);
                    while Instant::now() < until && !stop.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }));
        }
        for worker in workers {
            if worker.join().is_err() {
                failed.store(true, Ordering::SeqCst);
                stop.cancel();
            }
        }
        finished.store(true, Ordering::SeqCst);
    });
    let entries = entries.lock().unwrap_or_else(|p| p.into_inner());
    let mut all = vec![];
    let mut by_endpoint = BTreeMap::<String, Vec<(u128, bool)>>::new();
    for entry in entries.iter().filter(|e| !e.skipped) {
        let value = (
            entry.time_ms,
            entry.error.is_some() || entry.status >= 400 || !entry.passed(),
        );
        all.push(value);
        by_endpoint
            .entry(entry.request_id.clone())
            .or_default()
            .push(value);
    }
    let elapsed = start.elapsed().as_secs_f64();
    let metrics = metrics(&all);
    let error_rate = if metrics.requests == 0 {
        1.0
    } else {
        metrics.errors as f64 / metrics.requests as f64
    };
    let passed = !cancel.is_cancelled()
        && !failed.load(Ordering::SeqCst)
        && metrics.requests > 0
        && error_rate <= profile.max_error_rate
        && profile
            .max_p95_ms
            .is_none_or(|limit| metrics.p95_ms <= u128::from(limit));
    use sha2::Digest;
    let report = LoadReport {
        version: 1,
        id: format!(
            "{}-{}",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
            uuid::Uuid::new_v4()
        ),
        profile_id: profile.run.id.clone(),
        configuration_hash: format!("{:x}", sha2::Sha256::digest(serde_json::to_vec(profile)?)),
        started_at,
        elapsed_seconds: elapsed,
        cancelled: cancel.is_cancelled(),
        throughput: metrics.requests as f64 / elapsed.max(0.001),
        metrics,
        endpoints: by_endpoint
            .into_iter()
            .map(|(id, values)| (id, self::metrics(&values)))
            .collect(),
        passed,
    };
    let path = crate::storage::owned_path(
        root,
        &[".local", "load-runs", &format!("{}.json", report.id)],
    )?;
    crate::workspace::atomic_write(&path, &serde_json::to_vec_pretty(&report)?)?;
    crate::automation::retain_records(root, "load-runs")?;
    Ok(report)
}
pub fn compare(before: &LoadReport, after: &LoadReport) -> serde_json::Value {
    serde_json::json!({"same_configuration":before.configuration_hash==after.configuration_hash,
        "before_passed":before.passed,"after_passed":after.passed,"requests_delta":after.metrics.requests as i64-before.metrics.requests as i64,
        "error_fraction_before":before.metrics.errors as f64/before.metrics.requests.max(1) as f64,
        "error_fraction_after":after.metrics.errors as f64/after.metrics.requests.max(1) as f64,
        "p95_ms_delta":after.metrics.p95_ms as i128-before.metrics.p95_ms as i128,"throughput_delta":after.throughput-before.throughput})
}
