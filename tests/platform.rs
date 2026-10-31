mod support;
use rustapi::{
    flows::{Flow, Node},
    http::{Cancellation, HttpService},
    models::*,
    platform::{
        ai,
        collaboration::{Client, Config, Role, TeamServer, User},
        external_secrets, mcp,
    },
    policy::Policy,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
fn root() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    rustapi::workspace::open_workspace(temp.path()).unwrap();
    temp
}
#[test]
fn stdio_mcp_remains_responsive_and_cancels_an_executing_profile() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
        time::Duration,
    };
    let temp = root();
    let root = temp.path();
    let (started, arrived) = std::sync::mpsc::channel();
    let server = support::Server::new(1, move |_, _| {
        started.send(()).unwrap();
        let mut reply = support::Reply::json("{}");
        reply.delay_ms = 1200;
        reply
    });
    let mut collection = Collection::new("MCP cancellable run");
    let mut request = ApiRequest::new("Slow request");
    request.url = server.url.clone();
    collection.requests.push(request);
    let filename = format!("{}.json", collection.id);
    std::fs::write(
        root.join("collections").join(&filename),
        serde_json::to_vec(&collection).unwrap(),
    )
    .unwrap();
    let profile = rustapi::automation::RunProfile {
        collection: format!("collections/{filename}"),
        ..Default::default()
    };
    rustapi::automation::save(root, &profile).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .args(["mcp", "serve", "--allow-execution", "--workspace"])
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (sender, received) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if sender
                .send(serde_json::from_str::<serde_json::Value>(&line.unwrap()).unwrap())
                .is_err()
            {
                break;
            }
        }
    });
    let send = |input: &mut std::process::ChildStdin, value: serde_json::Value| {
        writeln!(input, "{value}").unwrap();
        input.flush().unwrap();
    };
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}),
    );
    assert_eq!(
        received.recv_timeout(Duration::from_secs(5)).unwrap()["result"]["resultType"],
        "complete"
    );
    send(
        &mut input,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_profile","arguments":{"id":profile.id}}}),
    );
    arrived.recv_timeout(Duration::from_secs(5)).unwrap();
    send(&mut input, json!({"jsonrpc":"2.0","id":3,"method":"ping"}));
    assert_eq!(
        received.recv_timeout(Duration::from_secs(1)).unwrap()["id"],
        3
    );
    send(
        &mut input,
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2,"reason":"User cancelled"}}),
    );
    // Cancelled MCP calls produce no reply. The run still records its cancellation.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let history = rustapi::automation::history(root).unwrap();
        if let Some(record) = history.first() {
            assert!(record.report.cancelled);
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Cancelled run was not recorded"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    send(&mut input, json!({"jsonrpc":"2.0","id":4,"method":"ping"}));
    assert_eq!(
        received.recv_timeout(Duration::from_secs(2)).unwrap()["id"],
        4
    );
    drop(input);
    assert!(child.wait().unwrap().success());
    reader.join().unwrap();
    server.finish();
}
fn write(path: &std::path::Path, value: &impl serde::Serialize) {
    rustapi::workspace::atomic_write(path, &serde_json::to_vec(value).unwrap()).unwrap();
}
#[test]
fn aws_secret_provider_signs_real_http_and_cli_outputs_aliases_only() {
    let temp = root();
    let server = support::Server::new(1, |_, request| {
        let lower = request.head.to_ascii_lowercase();
        assert!(lower.contains("authorization: aws4-hmac-sha256"));
        assert!(lower.contains("/us-east-1/secretsmanager/aws4_request"));
        assert!(lower.contains("x-amz-target: secretsmanager.getsecretvalue"));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["SecretId"],
            "service/api"
        );
        support::Reply::json(r#"{"SecretString":"{\"token\":\"aws-provider-private-value\"}"}"#)
    });
    write(
        &temp.path().join("secret-providers.json"),
        &json!({"providers":[{"name":"aws","kind":"AwsSecretsManager","endpoint":server.url,"region":"us-east-1"}],"bindings":[{"alias":"aws-api","provider":"aws","secret_id":"service/api","pointer":"/token"}]}),
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_rustapi"))
        .args(["secrets", "check", "--workspace"])
        .arg(temp.path())
        .env("AWS_ACCESS_KEY_ID", "EXAMPLE_PROVIDER_ACCESS")
        .env(
            "AWS_SECRET_ACCESS_KEY",
            "example-provider-secret-with-sufficient-length",
        )
        .env_remove("AWS_SESSION_TOKEN")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ext/aws-api"));
    assert!(!stdout.contains("aws-provider-private-value"));
    server.finish();
}
#[test]
fn team_roles_revision_conflicts_expiry_audit_and_opt_in_documentation() {
    let directory = root();
    let (admin, admin_token) = User::issue("admin".into(), Role::Admin, 30).unwrap();
    let (viewer, viewer_token) = User::issue("viewer".into(), Role::Viewer, 30).unwrap();
    let (mut expired, expired_token) = User::issue("expired".into(), Role::Editor, 1).unwrap();
    expired.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(1));
    let server = TeamServer::start(
        directory.path(),
        Config {
            port: 0,
            users: vec![admin.clone(), viewer, expired],
            public_documentation: true,
            ..Default::default()
        },
    )
    .unwrap();
    let endpoint = format!("http://{}", server.server.address);
    let client = Client::new(&endpoint, &admin_token).unwrap();
    let mut collection = Collection::new("Team <script>bad</script>");
    let mut request = ApiRequest::new("Request");
    request.url = "https://example.test".into();
    request.auth.auth_type = AuthType::BearerToken;
    request.auth.bearer_token = "{{vault:access_token}}".into();
    request.body_type = BodyType::Json;
    request.body = r#"{"password":"private-literal"}"#.into();
    collection.requests.push(request);
    let revision = client.push(&collection, "new").unwrap();
    assert_eq!(client.catalog().unwrap().len(), 1);
    let pulled = client.pull(&collection.id).unwrap();
    assert_eq!(
        pulled.collection.requests[0].auth.bearer_token,
        "{{vault:access_token}}"
    );
    assert!(!serde_json::to_string(&pulled)
        .unwrap()
        .contains("private-literal"));
    assert!(client.push(&collection, "new").is_err());
    let next = client.push(&collection, &revision).unwrap();
    assert_ne!(revision, next);
    assert!(client.push(&collection, &revision).is_err());
    let viewer = Client::new(&endpoint, &viewer_token).unwrap();
    assert_eq!(viewer.catalog().unwrap().len(), 1);
    assert!(viewer.push(&collection, &next).is_err());
    assert!(Client::new(&endpoint, &expired_token)
        .unwrap()
        .catalog()
        .is_err());
    assert!(Client::new(&endpoint, "invalid")
        .unwrap()
        .catalog()
        .is_err());
    let audit = std::fs::read_to_string(directory.path().join("audit.jsonl")).unwrap();
    assert!(audit.contains(&admin.id) && audit.contains("write_intent"));
    for secret in [&admin_token, &viewer_token, &expired_token] {
        assert!(!audit.contains(secret));
    }
    let html = reqwest::blocking::get(format!("{endpoint}/v1/docs/{}", collection.id))
        .unwrap()
        .text()
        .unwrap();
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>") && !html.contains("private-literal"));
    assert!(TeamServer::start(
        directory.path(),
        Config {
            port: 0,
            users: vec![admin],
            ..Default::default()
        }
    )
    .is_err());
    assert!(Client::new("http://remote.example.test", "token").is_err());
}
#[test]
fn policy_blocks_scripts_tls_overrides_redirect_escapes_and_malformed_files() {
    let directory = root();
    let path = directory.path().join("workspace-policy.json");
    let policy = Policy {
        require_encryption: true,
        allow_insecure_tls: false,
        allow_scripts: false,
        allowed_hosts: vec!["api.example.test".into()],
        ..Default::default()
    };
    write(&path, &policy);
    assert!(Client::for_workspace(
        directory.path(),
        "https://outside.example.test",
        "synthetic-token"
    )
    .is_err());
    assert!(
        Client::for_workspace(directory.path(), "http://127.0.0.1:1", "synthetic-token").is_err()
    );
    assert!(Client::for_workspace(
        directory.path(),
        "https://api.example.test",
        "synthetic-token"
    )
    .is_ok());
    let service = HttpService::for_workspace(directory.path());
    let request = ApiRequest {
        url: "http://api.example.test".into(),
        ..Default::default()
    };
    assert!(service
        .execute(&request, &Cancellation::default())
        .error
        .unwrap()
        .contains("encrypted"));
    let mut request = ApiRequest {
        url: "https://api.example.test".into(),
        pre_script: "pm.variables.set('must_not_run','yes');".into(),
        ..Default::default()
    };
    let mut vars = rustapi::variables::Variables::default();
    assert!(
        rustapi::runner::execute(&service, &request, &mut vars, &Cancellation::default())
            .error
            .is_some()
    );
    assert!(!vars.local.contains_key("must_not_run"));
    request.pre_script.clear();
    request.settings.verify_tls = false;
    assert!(service
        .execute(&request, &Cancellation::default())
        .error
        .unwrap()
        .contains("certificate"));
    let server = support::Server::new(1, |_, _| support::Reply {
        status: 302,
        headers: vec![("Location".into(), "http://localhost:1/disallowed".into())],
        body: vec![],
        delay_ms: 0,
    });
    write(
        &path,
        &Policy {
            allowed_hosts: vec!["127.0.0.1".into()],
            ..Default::default()
        },
    );
    service.reload_policy(directory.path()).unwrap();
    request = ApiRequest {
        url: server.url.clone(),
        ..Default::default()
    };
    assert!(service
        .execute(&request, &Cancellation::default())
        .error
        .unwrap()
        .contains("policy"));
    server.finish();
    std::fs::write(&path, "{").unwrap();
    assert!(service.reload_policy(directory.path()).is_err());
    assert!(service
        .execute(&request, &Cancellation::default())
        .error
        .unwrap()
        .contains("invalid"));
    let policy = Policy {
        allowed_hosts: vec!["*.example.test".into()],
        ..Default::default()
    };
    assert!(policy.url("https://api.example.test").is_ok());
    assert!(policy.url("https://example.test").is_err());
    assert!(policy.url("https://badexample.test").is_err());
}
#[test]
fn vault_provider_reads_only_into_memory_and_never_logs_values() {
    let directory = root();
    let variable = format!(
        "RUSTAPI_TEST_VAULT_{}",
        uuid::Uuid::new_v4().simple().to_string().to_uppercase()
    );
    std::env::set_var(&variable, "provider-token");
    let server = support::Server::new(1, |_, request| {
        assert!(request.head.starts_with("GET /v1/secret/data/apps/api "));
        assert!(request.head.contains("provider-token"));
        support::Reply::json(r#"{"data":{"data":{"api_key":"fetched-private-value"}}}"#)
    });
    let config = external_secrets::Config {
        providers: vec![external_secrets::Provider {
            name: "vault".into(),
            endpoint: server.url.clone(),
            token_env: variable.clone(),
            ..Default::default()
        }],
        bindings: vec![external_secrets::Binding {
            alias: "api".into(),
            provider: "vault".into(),
            secret_id: "apps/api".into(),
            pointer: "/api_key".into(),
        }],
    };
    let path = directory.path().join("secret-providers.json");
    write(&path, &config);
    let service = HttpService::for_workspace(directory.path());
    let values =
        external_secrets::load(directory.path(), &service, &Cancellation::default()).unwrap();
    assert_eq!(values["ext/api"], "fetched-private-value");
    let vars = rustapi::variables::Variables {
        secrets: values,
        ..Default::default()
    };
    assert_eq!(
        vars.resolve("{{vault:ext/api}}").unwrap(),
        "fetched-private-value"
    );
    assert!(!std::fs::read_to_string(path)
        .unwrap()
        .contains("fetched-private-value"));
    assert!(!directory.path().join(".local/vault.json").exists());
    let logs = serde_json::to_string(&service.debug.snapshot()).unwrap();
    assert!(!logs.contains("provider-token") && !logs.contains("fetched-private-value"));
    server.finish();
    std::env::remove_var(variable);
}
#[test]
fn ai_context_is_redacted_and_generated_requests_are_unsent_safe_drafts() {
    let server = support::Server::new(2, |index, request| {
        if index == 0 {
            return support::Reply::json(r#"{"models":[{"name":"fixture-model"}]}"#);
        }
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["stream"], false);
        assert_eq!(body["model"], "fixture-model");
        assert!(!body["prompt"]
            .as_str()
            .unwrap()
            .contains("private-credential"));
        support::Reply::json(&json!({"response":json!({"name":"Draft","url":"https://example.test/api","pre_script":"throw Error('must not run')","binary_file":"/etc/passwd","settings":{"verify_tls":false,"proxy":"http://evil.test"}}).to_string()}).to_string())
    });
    let directory = root();
    let service = HttpService::for_workspace(directory.path());
    assert_eq!(
        ai::models(&service, &server.url, &Cancellation::default()).unwrap(),
        vec!["fixture-model"]
    );
    let mut collection = Collection::new("Context");
    collection.auth.auth_type = AuthType::BearerToken;
    collection.auth.bearer_token = "private-credential".into();
    let draft = ai::draft(
        &service,
        &server.url,
        "fixture-model",
        "Make a GET request",
        Some(&collection),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(
        draft.pre_script.is_empty()
            && draft.binary_file.is_empty()
            && draft.settings.proxy.is_empty()
            && draft.settings.verify_tls
    );
    assert_eq!(draft.auth.auth_type, AuthType::None);
    server.finish();
}
#[test]
fn mcp_discovery_redaction_and_execution_permissions_cover_both_protocol_eras() {
    let directory = root();
    let mut collection = Collection::new("MCP");
    let mut request = ApiRequest::new("Saved");
    request.url = "https://example.test".into();
    request.auth.auth_type = AuthType::BearerToken;
    request.auth.bearer_token = "private-tool-token".into();
    let id = request.id.clone();
    collection.requests.push(request);
    write(&directory.path().join("collections/mcp.json"), &collection);
    let service = HttpService::for_workspace(directory.path());
    let secrets = Default::default();
    let call = |value| mcp::handle(directory.path(), &service, &secrets, false, value).unwrap();
    let legacy = call(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}),
    );
    assert_eq!(legacy["result"]["protocolVersion"], "2025-11-25");
    let modern = call(
        json!({"jsonrpc":"2.0","id":2,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}),
    );
    assert_eq!(modern["result"]["resultType"], "complete");
    assert!(modern["result"]["supportedVersions"]
        .as_array()
        .unwrap()
        .contains(&json!("2026-07-28")));
    let list = call(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}));
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 2);
    let described = call(
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"describe_request","arguments":{"collection":collection.id,"request":id}}}),
    );
    assert!(!described.to_string().contains("private-tool-token"));
    assert_eq!(described["result"]["isError"], false);
    let denied = call(
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"run_profile","arguments":{"id":"unavailable"}}}),
    );
    assert_eq!(denied["result"]["isError"], true);
    let unknown = call(
        json!({"jsonrpc":"2.0","id":6,"method":"ping","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2099-01-01"}}}),
    );
    assert_eq!(unknown["error"]["code"], -32022);
    assert!(mcp::handle(
        directory.path(),
        &service,
        &secrets,
        false,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .is_none());
}
#[test]
fn visual_flow_follows_failure_edges_extracts_values_and_bounds_cycles() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let server = support::Server::new(3, move |index, request| {
        count.fetch_add(1, Ordering::SeqCst);
        if index == 0 {
            support::Reply {
                status: 401,
                headers: vec![],
                body: vec![],
                delay_ms: 0,
            }
        } else if index == 1 {
            support::Reply::json(r#"{"token":"flow-private-token"}"#)
        } else {
            assert!(request.head.contains("Bearer flow-private-token"));
            support::Reply::json("{}")
        }
    });
    let directory = root();
    let mut collection = Collection::new("Flow");
    let first = ApiRequest {
        url: server.url.clone(),
        ..Default::default()
    };
    let mut login = first.duplicate();
    login.extractors.push(Extractor {
        variable: "token".into(),
        pointer: "/token".into(),
    });
    let mut final_request = first.duplicate();
    final_request.auth.auth_type = AuthType::BearerToken;
    final_request.auth.bearer_token = "{{token}}".into();
    let ids = [first.id.clone(), login.id.clone(), final_request.id.clone()];
    collection.requests = vec![first, login, final_request];
    write(&directory.path().join("collections/flow.json"), &collection);
    let mut flow = Flow {
        profile: rustapi::automation::RunProfile {
            collection: "collections/flow.json".into(),
            ..Default::default()
        },
        start: "first".into(),
        nodes: vec![
            Node {
                id: "first".into(),
                request_id: ids[0].clone(),
                success: None,
                failure: Some("login".into()),
                position: [0., 0.],
            },
            Node {
                id: "login".into(),
                request_id: ids[1].clone(),
                success: Some("final".into()),
                failure: None,
                position: [100., 0.],
            },
            Node {
                id: "final".into(),
                request_id: ids[2].clone(),
                success: None,
                failure: None,
                position: [200., 0.],
            },
        ],
        ..Default::default()
    };
    flow.save(directory.path()).unwrap();
    let service = HttpService::for_workspace(directory.path());
    let report = rustapi::flows::run(
        directory.path(),
        &service,
        &flow,
        Default::default(),
        &Cancellation::default(),
        |_| {},
    )
    .unwrap();
    assert!(report.passed);
    assert_eq!(report.steps.len(), 3);
    assert!(!serde_json::to_string(&report)
        .unwrap()
        .contains("flow-private-token"));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    server.finish();
    flow.nodes[0].failure = Some("missing".into());
    assert!(flow.validate().is_err());
    flow.nodes[0].failure = Some("first".into());
    flow.max_steps = 0;
    assert!(flow.validate().is_err());
}
