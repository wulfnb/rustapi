//! One execution pipeline for interactive requests and headless/GUI collection runs.
use crate::{
    http::{Cancellation, HttpService},
    models::*,
    variables::Variables,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RunOptions {
    #[serde(skip)]
    pub workspace_root: Option<std::path::PathBuf>,
    pub folder: Option<String>,
    pub iterations: usize,
    pub stop_on_failure: bool,
    pub delay_ms: u64,
    pub data: Vec<BTreeMap<String, String>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEntry {
    pub iteration: usize,
    pub request_id: String,
    pub name: String,
    #[serde(default)]
    pub skipped: bool,
    pub status: u16,
    pub time_ms: u128,
    pub tests: Vec<TestResult>,
    pub error: Option<String>,
}
impl RunEntry {
    pub fn passed(&self) -> bool {
        self.error.is_none() && self.tests.iter().all(|t| t.passed)
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunReport {
    pub entries: Vec<RunEntry>,
    pub cancelled: bool,
}
impl RunReport {
    pub fn passed(&self) -> bool {
        !self.cancelled && !self.entries.is_empty() && self.entries.iter().all(RunEntry::passed)
    }
    pub fn junit(&self) -> String {
        fn xml(s: &str) -> String {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;")
        }
        let failures = self.entries.iter().filter(|e| !e.passed()).count();
        let mut out=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuite name=\"RustAPI\" tests=\"{}\" failures=\"{}\" errors=\"{}\">",self.entries.len(),failures,usize::from(self.cancelled));
        for e in &self.entries {
            out.push_str(&format!(
                "<testcase name=\"{} (iteration {})\" time=\"{}\">",
                xml(&e.name),
                e.iteration,
                e.time_ms as f64 / 1000.
            ));
            if e.skipped && e.passed() {
                out.push_str("<skipped/>");
            }
            if !e.passed() {
                out.push_str(&format!(
                    "<failure>{}</failure>",
                    xml(&e.error.clone().unwrap_or_else(|| e
                        .tests
                        .iter()
                        .filter(|t| !t.passed)
                        .map(|t| format!("{}: {}", t.name, t.message))
                        .collect::<Vec<_>>()
                        .join("; ")))
                ));
            }
            out.push_str("</testcase>");
        }
        if self.cancelled {
            out.push_str("<system-err>Run cancelled</system-err>");
        }
        out.push_str("</testsuite>");
        out
    }
}
pub fn execute(
    service: &HttpService,
    r: &ApiRequest,
    vars: &mut Variables,
    cancel: &Cancellation,
) -> ApiResponse {
    execute_with_flow(service, r, vars, cancel).0
}
fn execute_with_flow(
    service: &HttpService,
    r: &ApiRequest,
    vars: &mut Variables,
    cancel: &Cancellation,
) -> (ApiResponse, crate::scripts::Workflow) {
    let mut workflow = crate::scripts::Workflow::default();
    let work = || -> Result<ApiResponse> {
        anyhow::ensure!(
            service.policy()?.allow_scripts
                || r.pre_script.trim().is_empty() && r.post_script.trim().is_empty(),
            "Workspace policy disables scripts"
        );
        let mut draft = r.clone();
        let result =
            crate::scripts::execute_request(&r.pre_script, vars, Some(&mut draft), None, cancel)?;
        for log in result.logs {
            service.debug.push(
                "script",
                &log.level,
                serde_json::json!({"phase":"beforeRequest","args":log.args}),
            );
        }
        workflow = result.workflow;
        let mut pre = result.tests;
        if workflow.skip_request {
            service
                .debug
                .push("execution", "info", serde_json::json!({"event":"skipped"}));
            return Ok(ApiResponse {
                skipped: true,
                status_text: "Skipped".into(),
                tests: pre,
                ..Default::default()
            });
        }
        let mut request = vars.resolve_request(&draft)?;
        let mut response = service.execute(&request, cancel);
        if response.error.is_some() {
            return Ok(response);
        }
        if request.graphql.is_some() {
            let value =
                serde_json::from_slice::<serde_json::Value>(if response.raw_body.is_empty() {
                    response.body.as_bytes()
                } else {
                    &response.raw_body
                })
                .ok();
            let passed = value.as_ref().is_some_and(|v| {
                v.get("data").is_some()
                    && v.get("errors")
                        .is_none_or(|e| e.as_array().is_some_and(Vec::is_empty))
            });
            pre.push(TestResult {
                name: "GraphQL response".into(),
                passed,
                message: if passed {
                    String::new()
                } else {
                    "GraphQL response is invalid or contains errors".into()
                },
            });
        }
        let body_json = if response.raw_body.is_empty() {
            serde_json::from_str::<serde_json::Value>(&response.body).ok()
        } else {
            serde_json::from_slice::<serde_json::Value>(&response.raw_body).ok()
        };
        for check in &r.checks {
            if check.kind == AssertionKind::OpenApi {
                let result = crate::openapi::response_contract(&check.expected, &response);
                let (passed, message) = match result {
                    Ok(errors) => (errors.is_empty(), errors.join("; ")),
                    Err(error) => (false, error.to_string()),
                };
                pre.push(TestResult {
                    name: check.name.clone(),
                    passed,
                    message,
                });
                continue;
            }
            if check.kind == AssertionKind::JsonSchema {
                let result = body_json
                    .as_ref()
                    .and_then(|body| body.pointer(&check.target))
                    .ok_or_else(|| {
                        anyhow::anyhow!("Schema target is missing or response is not JSON")
                    })
                    .and_then(|instance| crate::schema::validate(&check.expected, instance));
                let (passed, message) = match result {
                    Ok(errors) => (errors.is_empty(), errors.join("; ")),
                    Err(error) => (false, error.to_string()),
                };
                pre.push(TestResult {
                    name: check.name.clone(),
                    passed,
                    message,
                });
                continue;
            }
            let value = match check.kind {
                AssertionKind::Status => Some(response.status.to_string()),
                AssertionKind::Header => response
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&check.target))
                    .map(|(_, v)| v.clone()),
                AssertionKind::Json => body_json
                    .as_ref()
                    .and_then(|v| v.pointer(&check.target))
                    .map(|v| {
                        v.as_str()
                            .map(String::from)
                            .unwrap_or_else(|| v.to_string())
                    }),
                AssertionKind::BodyContains => {
                    let needle = vars.resolve(&check.expected)?;
                    let haystack = if response.raw_body.is_empty() {
                        response.body.as_bytes()
                    } else {
                        response.raw_body.as_slice()
                    };
                    Some(
                        (needle.is_empty()
                            || haystack
                                .windows(needle.len())
                                .any(|part| part == needle.as_bytes()))
                        .to_string(),
                    )
                }
                AssertionKind::JsonSchema | AssertionKind::OpenApi => {
                    unreachable!("Handled before ordinary assertions")
                }
                AssertionKind::MaxTime => Some(
                    (response.time_ms
                        <= check
                            .expected
                            .parse::<u128>()
                            .context("Invalid time assertion")?)
                    .to_string(),
                ),
            };
            let expected = if matches!(
                check.kind,
                AssertionKind::BodyContains | AssertionKind::MaxTime
            ) {
                "true".into()
            } else {
                vars.resolve(&check.expected)?
            };
            let passed = value.as_deref() == Some(&expected);
            pre.push(TestResult {
                name: check.name.clone(),
                passed,
                message: if passed {
                    String::new()
                } else {
                    "Assertion did not match".into()
                },
            });
        }
        for extractor in &r.extractors {
            let value = body_json
                .as_ref()
                .and_then(|v| v.pointer(&extractor.pointer))
                .with_context(|| format!("Missing extraction path {}", extractor.pointer))?;
            vars.local.insert(
                extractor.variable.clone(),
                value
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| value.to_string()),
            );
        }
        let result = crate::scripts::execute_request(
            &r.post_script,
            vars,
            Some(&mut request),
            Some(&response),
            cancel,
        )?;
        for log in result.logs {
            service.debug.push(
                "script",
                &log.level,
                serde_json::json!({"phase":"afterResponse","args":log.args}),
            );
        }
        if result.workflow.next_set {
            workflow.next_set = true;
            workflow.next_request = result.workflow.next_request;
        }
        pre.extend(result.tests);
        response.tests = pre;
        Ok(response)
    };
    let mut work = work;
    match work() {
        Ok(response) => (response, workflow),
        Err(e) => {
            if let Some(failure) = e.downcast_ref::<crate::debug::ScriptFailure>() {
                for log in &failure.logs {
                    service
                        .debug
                        .push("script", &log.level, serde_json::json!({"args":log.args}));
                }
            }
            service.debug.push(
                "execution",
                "error",
                serde_json::json!({"event":"request/script/extraction failed"}),
            );
            (ApiResponse::error(e.to_string()), Default::default())
        }
    }
}
pub fn run(
    service: &HttpService,
    c: &Collection,
    env: Option<&Environment>,
    options: &RunOptions,
    secrets: BTreeMap<String, String>,
    cancel: &Cancellation,
) -> Result<RunReport> {
    run_with_progress(service, c, env, options, secrets, cancel, |_| {})
}
pub fn run_with_progress(
    service: &HttpService,
    c: &Collection,
    env: Option<&Environment>,
    options: &RunOptions,
    secrets: BTreeMap<String, String>,
    cancel: &Cancellation,
    progress: impl FnMut(&RunEntry),
) -> Result<RunReport> {
    run_with_gate(
        service,
        c,
        env,
        options,
        secrets,
        cancel,
        (progress, || true),
    )
}
/// A gate is checked before every execution, including scripted workflow jumps.
pub fn run_with_gate(
    service: &HttpService,
    c: &Collection,
    env: Option<&Environment>,
    options: &RunOptions,
    secrets: BTreeMap<String, String>,
    cancel: &Cancellation,
    callbacks: (impl FnMut(&RunEntry), impl FnMut() -> bool),
) -> Result<RunReport> {
    let (mut progress, mut gate) = callbacks;
    let requests = c.ordered_requests(options.folder.as_deref());
    let root = options
        .workspace_root
        .clone()
        .unwrap_or_else(crate::storage::get_app_data_dir);
    let mut global = if root.exists() {
        crate::storage::GlobalVariables::load(&root)?.variables
    } else {
        BTreeMap::new()
    };
    let local_overrides = if root.exists() {
        crate::storage::LocalEnvironmentOverrides::load(&root)?
    } else {
        Default::default()
    };
    anyhow::ensure!(!requests.is_empty(), "No requests selected");
    let iterations = if options.iterations == 0 {
        options.data.len().max(1)
    } else {
        options.iterations
    };
    anyhow::ensure!(iterations <= 100_000, "Iteration limit exceeded");
    let mut report = RunReport::default();
    for i in 0..iterations {
        let mut vars = Variables {
            global: global.clone(),
            local_overrides: env
                .and_then(|e| local_overrides.overrides.get(&e.id))
                .cloned()
                .unwrap_or_default(),
            environment: env
                .map(|e| {
                    e.variables
                        .iter()
                        .filter(|v| v.enabled)
                        .map(|v| (v.key.clone(), v.value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            data: options
                .data
                .get(i % options.data.len().max(1))
                .cloned()
                .unwrap_or_default(),
            secrets: secrets.clone(),
            ..Default::default()
        };
        let mut collection_changes = BTreeMap::new();
        let mut collection_removed = std::collections::BTreeSet::new();
        let mut index = 0;
        let mut steps = 0;
        while index < requests.len() {
            let r = &requests[index];
            steps += 1;
            if cancel.is_cancelled() {
                report.cancelled = true;
                return Ok(report);
            }
            if !gate() {
                return Ok(report);
            }
            vars.collection = r.inherited_variables.clone().into_iter().collect();
            vars.collection.extend(collection_changes.clone());
            for key in &collection_removed {
                vars.collection.remove(key);
            }
            let before = vars.collection.clone();
            let (response, flow) = execute_with_flow(service, r, &mut vars, cancel);
            for key in before.keys() {
                if !vars.collection.contains_key(key) {
                    collection_removed.insert(key.clone());
                    collection_changes.remove(key);
                }
            }
            for (k, v) in &vars.collection {
                if before.get(k) != Some(v) {
                    collection_removed.remove(k);
                    collection_changes.insert(k.clone(), v.clone());
                }
            }
            let mut entry=RunEntry{iteration:i+1,request_id:r.id.clone(),name:r.name.clone(),status:response.status,skipped:response.skipped,time_ms:response.time_ms,tests:response.tests,error:response.error.map(|_|"Request, script, or extraction failed (details available on interactive send)".into())};
            let mut next = Some(index + 1);
            if flow.next_set {
                next = if let Some(target) = &flow.next_request {
                    let by_id = requests.iter().position(|request| request.id == *target);
                    let by_name: Vec<_> = requests
                        .iter()
                        .enumerate()
                        .filter(|(_, request)| request.name == *target)
                        .map(|(index, _)| index)
                        .collect();
                    if let Some(index) = by_id {
                        Some(index)
                    } else if by_name.len() == 1 {
                        Some(by_name[0])
                    } else {
                        entry.error = Some(
                            "Unknown or ambiguous workflow target in selected requests".into(),
                        );
                        None
                    }
                } else {
                    None
                };
            }
            let more = next.is_some_and(|index| index < requests.len());
            let limit_reached = (steps >= 1000 && more)
                || (report.entries.len() >= 99_999 && (more || i + 1 < iterations));
            if limit_reached {
                entry.error = Some(
                    "Workflow step limit exceeded (1000 per iteration, 100000 per run)".into(),
                );
                next = None;
            }
            index = next.unwrap_or(requests.len());
            let failed = !entry.passed();
            progress(&entry);
            report.entries.push(entry);
            if limit_reached {
                return Ok(report);
            }
            if cancel.is_cancelled() {
                report.cancelled = true;
                return Ok(report);
            }
            if failed && options.stop_on_failure {
                return Ok(report);
            }
            let mut delay = options.delay_ms.min(60_000);
            while delay > 0 && !cancel.is_cancelled() {
                let step = delay.min(20);
                std::thread::sleep(std::time::Duration::from_millis(step));
                delay -= step;
            }
        }
        global = vars.global;
    }
    Ok(report)
}
pub fn load_data(path: &Path) -> Result<Vec<BTreeMap<String, String>>> {
    if path.extension().is_some_and(|e| e == "csv") {
        let mut reader = csv::Reader::from_path(path)?;
        let headers = reader.headers()?.clone();
        reader
            .records()
            .map(|r| {
                Ok(headers
                    .iter()
                    .zip(r?.iter())
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect())
            })
            .collect()
    } else {
        let rows: Vec<BTreeMap<String, serde_json::Value>> =
            serde_json::from_slice(&std::fs::read(path)?)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            v.as_str()
                                .map(String::from)
                                .unwrap_or_else(|| v.to_string()),
                        )
                    })
                    .collect()
            })
            .collect())
    }
}
