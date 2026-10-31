use rustapi::{
    automation::{self, RunProfile},
    http::{Cancellation, HttpService},
    local_http::{Response, Server},
    models::*,
    performance::{self, LoadProfile},
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
fn workspace(url: &str) -> (tempfile::TempDir, RunProfile) {
    let temp = tempfile::tempdir().unwrap();
    rustapi::workspace::open_workspace(temp.path()).unwrap();
    let mut collection = Collection::new("Automated");
    let mut request = ApiRequest::new("Check");
    request.url = url.into();
    request.checks.push(Assertion::default());
    collection.requests.push(request);
    rustapi::workspace::atomic_write(
        &temp.path().join("collections/test.json"),
        &serde_json::to_vec(&collection).unwrap(),
    )
    .unwrap();
    (
        temp,
        RunProfile {
            collection: "collections/test.json".into(),
            ..Default::default()
        },
    )
}
#[test]
fn profiles_datasets_history_and_comparison_survive_reload_and_confine_paths() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let server = Server::start(0, move |_| {
        Response::json(
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                200
            } else {
                500
            },
            serde_json::json!({"token":"private-response"}),
        )
    })
    .unwrap();
    let (temp, mut profile) = workspace(&format!("http://{}/", server.address));
    automation::save(temp.path(), &profile).unwrap();
    assert_eq!(automation::profiles(temp.path()).unwrap().len(), 1);
    let source = temp.path().join("source.csv");
    std::fs::write(&source, "id,name\n1,Ada\n2,Grace\n").unwrap();
    automation::import_dataset(temp.path(), &source, "users.csv").unwrap();
    assert!(automation::import_dataset(temp.path(), &source, "users.csv").is_err());
    profile.dataset = Some("datasets/users.csv".into());
    let (_, _, options) = automation::inputs(temp.path(), &profile).unwrap();
    assert_eq!(options.data.len(), 2);
    profile.dataset = None;
    let service = HttpService::default();
    let first = automation::run(
        temp.path(),
        &service,
        &profile,
        Default::default(),
        &Cancellation::default(),
    )
    .unwrap();
    let second = automation::run(
        temp.path(),
        &service,
        &profile,
        Default::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(first.report.passed());
    assert!(!second.report.passed());
    let difference = automation::compare(&first, &second);
    assert_eq!(difference.changed.len(), 1);
    let records = automation::history(temp.path()).unwrap();
    assert_eq!(records.len(), 2);
    assert!(!serde_json::to_string(&records)
        .unwrap()
        .contains("private-response"));
    assert!(automation::relative(temp.path(), "../outside.json").is_err());
    assert!(automation::relative(temp.path(), "/tmp/outside.json").is_err());
    profile.interval_seconds = Some(1);
    assert!(automation::save(temp.path(), &profile).is_err());
}
#[test]
fn scheduler_checkpoints_next_due_does_not_repeat_after_restart_and_enforces_one_owner() {
    let server = Server::start(0, |_| Response::json(200, serde_json::json!({}))).unwrap();
    let (temp, mut profile) = workspace(&format!("http://{}/", server.address));
    profile.interval_seconds = Some(60);
    automation::save(temp.path(), &profile).unwrap();
    let cancel = Cancellation::default();
    let stop = cancel.clone();
    let service = HttpService::default();
    automation::watch(
        temp.path(),
        &service,
        Default::default(),
        &cancel,
        |record| {
            assert!(record.unwrap().report.passed());
            stop.cancel();
        },
    )
    .unwrap();
    assert_eq!(automation::history(temp.path()).unwrap().len(), 1);
    let cancel = Cancellation::default();
    let stop = cancel.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        stop.cancel();
    });
    automation::watch(temp.path(), &service, Default::default(), &cancel, |_| {
        panic!("Schedule repeated before due")
    })
    .unwrap();
    thread.join().unwrap();
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .open(temp.path().join(".local/scheduler.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(automation::watch(
        temp.path(),
        &service,
        Default::default(),
        &Cancellation::default(),
        |_| {}
    )
    .is_err());
}
#[test]
fn percentiles_use_nearest_rank_and_load_is_concurrent_capped_and_session_isolated() {
    let values = (1..=100).map(|n| (n, n % 10 == 0)).collect::<Vec<_>>();
    let stats = performance::metrics(&values);
    assert_eq!(stats.p50_ms, 50);
    assert_eq!(stats.p95_ms, 95);
    assert_eq!(stats.p99_ms, 99);
    assert_eq!(stats.errors, 10);
    assert_eq!(stats.mean_ms, 50.5);
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(AtomicUsize::new(0));
    let counters = (active.clone(), peak.clone(), requests.clone());
    let server = Server::start(0, move |request| {
        let user = request.headers.get("x-vu").unwrap();
        if let Some(cookie) = request.headers.get("cookie") {
            assert_eq!(cookie, &format!("vu={user}"));
        }
        counters.2.fetch_add(1, Ordering::SeqCst);
        let count = counters.0.fetch_add(1, Ordering::SeqCst) + 1;
        counters.1.fetch_max(count, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(100));
        counters.0.fetch_sub(1, Ordering::SeqCst);
        Response {
            status: 200,
            headers: vec![("Set-Cookie".into(), format!("vu={user}; Path=/"))],
            body: b"{}".to_vec(),
        }
    })
    .unwrap();
    let (temp, profile) = workspace(&format!("http://{}/", server.address));
    let path = temp.path().join("collections/test.json");
    let mut collection: Collection =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    collection.requests[0]
        .headers
        .push(KeyValue::new("X-Vu", "{{__rustapi_vu}}"));
    // Script loops cannot exceed the shared execution gate.
    collection.requests[0].post_script = "pm.execution.setNextRequest('Check');".into();
    rustapi::workspace::atomic_write(&path, &serde_json::to_vec(&collection).unwrap()).unwrap();
    let load = LoadProfile {
        run: profile,
        users: 3,
        duration_seconds: 5,
        max_requests: 9,
        max_error_rate: 0.0,
        ..Default::default()
    };
    let report = performance::run(
        temp.path(),
        &load,
        Default::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(report.passed, "{report:?}");
    assert_eq!(report.metrics.requests, 9);
    assert_eq!(requests.load(Ordering::SeqCst), 9);
    assert!(peak.load(Ordering::SeqCst) >= 2);
    assert!(report.elapsed_seconds < 3.0);
    assert!(report.metrics.p95_ms >= 100);
}
#[test]
fn load_cancellation_and_invalid_unbounded_profiles_fail_predictably() {
    let server = Server::start(0, |_| {
        std::thread::sleep(Duration::from_millis(300));
        Response::json(500, serde_json::json!({}))
    })
    .unwrap();
    let (temp, profile) = workspace(&format!("http://{}/", server.address));
    let mut load = LoadProfile {
        run: profile,
        users: 2,
        duration_seconds: 10,
        ..Default::default()
    };
    let cancel = Cancellation::default();
    let stop = cancel.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        stop.cancel();
    });
    let report = performance::run(temp.path(), &load, Default::default(), &cancel).unwrap();
    thread.join().unwrap();
    assert!(report.cancelled && !report.passed);
    assert!(report.elapsed_seconds < 2.0);
    load.users = 100000;
    assert!(load.validate().is_err());
    load.users = 1;
    load.stages = vec![performance::Stage {
        users: 1,
        duration_seconds: u64::MAX,
    }];
    assert!(load.validate().is_err());
}
