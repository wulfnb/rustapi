//! Saved visual request graphs with shared variables, conditional edges and bounded loops.
use crate::{
    automation::{self, RunProfile},
    http::{Cancellation, HttpService},
    models::TestResult,
    variables::Variables,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Flow {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub profile: RunProfile,
    pub start: String,
    pub nodes: Vec<Node>,
    pub max_steps: usize,
    pub duration_seconds: u64,
}
impl Default for Flow {
    fn default() -> Self {
        Self {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            name: "Request flow".into(),
            profile: RunProfile::default(),
            start: String::new(),
            nodes: vec![],
            max_steps: 100,
            duration_seconds: 60,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub request_id: String,
    pub success: Option<String>,
    pub failure: Option<String>,
    pub position: [f32; 2],
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Step {
    pub node: String,
    pub request_id: String,
    pub status: u16,
    pub time_ms: u128,
    pub passed: bool,
    pub tests: Vec<TestResult>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct FlowReport {
    pub flow_id: String,
    pub started_at: String,
    pub steps: Vec<Step>,
    pub passed: bool,
    pub cancelled: bool,
    pub timed_out: bool,
    pub step_limit_reached: bool,
}
impl Flow {
    pub fn validate(&self) -> Result<()> {
        uuid::Uuid::parse_str(&self.id)?;
        ensure!(
            self.version == 1
                && !self.name.is_empty()
                && self.name.len() <= 256
                && !self.nodes.is_empty()
                && self.nodes.len() <= 200,
            "Flow requires 1–200 nodes and a valid name/version"
        );
        ensure!(
            (1..=1000).contains(&self.max_steps) && (1..=3600).contains(&self.duration_seconds),
            "Flow limits must be 1–1000 steps and 1–3600 seconds"
        );
        let mut ids = BTreeSet::new();
        for node in &self.nodes {
            ensure!(
                !node.id.is_empty()
                    && node.id.len() <= 128
                    && ids.insert(node.id.clone())
                    && node
                        .position
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 100000.),
                "Invalid/duplicate flow node"
            );
        }
        ensure!(ids.contains(&self.start), "Flow start node is missing");
        for node in &self.nodes {
            for target in [&node.success, &node.failure].into_iter().flatten() {
                ensure!(ids.contains(target), "Flow edge target is missing");
            }
        }
        Ok(())
    }
    pub fn save(&self, root: &Path) -> Result<std::path::PathBuf> {
        self.validate()?;
        automation::validate(root, &self.profile)?;
        let path = crate::storage::owned_path(root, &["flows", &format!("{}.json", self.id)])?;
        crate::workspace::atomic_write(&path, &serde_json::to_vec_pretty(self)?)?;
        Ok(path)
    }
}
pub fn run(
    root: &Path,
    service: &HttpService,
    flow: &Flow,
    secrets: BTreeMap<String, String>,
    cancel: &Cancellation,
    mut progress: impl FnMut(&Step),
) -> Result<FlowReport> {
    flow.validate()?;
    let (collection, environment, options) = automation::inputs(root, &flow.profile)?;
    let requests = collection.ordered_requests(options.folder.as_deref());
    ensure!(
        flow.nodes
            .iter()
            .all(|n| requests.iter().any(|r| r.id == n.request_id)),
        "Flow references an unavailable request"
    );
    let mut vars = Variables {
        global: crate::storage::GlobalVariables::load(root)?.variables,
        environment: environment
            .map(|e| {
                e.variables
                    .into_iter()
                    .filter(|v| v.enabled)
                    .map(|v| (v.key, v.value))
                    .collect()
            })
            .unwrap_or_default(),
        data: options.data.first().cloned().unwrap_or_default(),
        secrets,
        ..Default::default()
    };
    let local = crate::storage::LocalEnvironmentOverrides::load(root)?;
    if let Some(environment) = &flow.profile.environment {
        let path = automation::relative(root, environment)?;
        let imported = crate::import::path(&path)?;
        if let Some(environment) = imported.environment {
            vars.local_overrides = local
                .overrides
                .get(&environment.id)
                .cloned()
                .unwrap_or_default();
        }
    }
    let mut report = FlowReport {
        flow_id: flow.id.clone(),
        started_at: chrono::Utc::now().to_rfc3339(),
        steps: vec![],
        passed: false,
        cancelled: false,
        timed_out: false,
        step_limit_reached: false,
    };
    let start = Instant::now();
    let duration = Duration::from_secs(flow.duration_seconds);
    let work = Cancellation::default();
    let finished = Arc::new(AtomicBool::new(false));
    let signal = finished.clone();
    let parent = cancel.clone();
    let timer = work.clone();
    let mut next = Some(flow.start.clone());
    let mut completed = false;
    std::thread::scope(|scope| {
        scope.spawn(move || {
            while !signal.load(Ordering::SeqCst)
                && !parent.is_cancelled()
                && start.elapsed() < duration
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            timer.cancel();
        });
        for _ in 0..flow.max_steps {
            if work.is_cancelled() || cancel.is_cancelled() || start.elapsed() >= duration {
                break;
            }
            let Some(id) = next.take() else {
                break;
            };
            let node = flow.nodes.iter().find(|n| n.id == id).unwrap();
            let mut request = requests
                .iter()
                .find(|r| r.id == node.request_id)
                .unwrap()
                .clone();
            vars.collection = request.inherited_variables.clone().into_iter().collect();
            request.settings.timeout_ms = request
                .settings
                .timeout_ms
                .min(duration.saturating_sub(start.elapsed()).as_millis().max(1) as u64);
            let response = crate::runner::execute(service, &request, &mut vars, &work);
            let passed = response.error.is_none()
                && !response.skipped
                && response.tests.iter().all(|t| t.passed)
                && (response.is_success() || !response.tests.is_empty());
            let step = Step {
                node: node.id.clone(),
                request_id: request.id,
                status: response.status,
                time_ms: response.time_ms,
                passed,
                tests: response.tests,
                error: response
                    .error
                    .map(|_| "Request, script or extraction failed".into()),
            };
            progress(&step);
            report.steps.push(step);
            next = if passed {
                node.success.clone()
            } else {
                node.failure.clone()
            };
            if next.is_none() {
                completed = true;
                report.passed = passed;
                break;
            }
        }
        finished.store(true, Ordering::SeqCst);
    });
    report.cancelled = cancel.is_cancelled();
    report.timed_out = !completed && start.elapsed() >= duration;
    report.step_limit_reached = !completed && report.steps.len() >= flow.max_steps;
    report.passed &=
        completed && !report.cancelled && !report.timed_out && !report.step_limit_reached;
    let path = crate::storage::owned_path(
        root,
        &[
            ".local",
            "flow-runs",
            &format!(
                "{}-{}.json",
                chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
                uuid::Uuid::new_v4()
            ),
        ],
    )?;
    crate::workspace::atomic_write(&path, &serde_json::to_vec_pretty(&report)?)?;
    automation::retain_records(root, "flow-runs")?;
    Ok(report)
}
