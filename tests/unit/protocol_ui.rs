use super::*;
use crate::test_support::{Harness, Profile};
use dialogs::{Kind, Selections};
#[test]
fn protocol_profile_roundtrip_tabs_and_live_sse_connection() {
    let profile = Profile::new();
    let root = profile.directory.path();
    let server = rustapi::local_http::Server::start(0, |_| rustapi::local_http::Response {
        status: 200,
        headers: vec![("Content-Type".into(), "text/event-stream".into())],
        body: b"event: ready\ndata: live\n\n".to_vec(),
    })
    .unwrap();
    let file = root.join("session.json");
    let session = Session {
        protocol: Protocol::Sse,
        request: crate::models::ApiRequest {
            url: format!("http://{}", server.address),
            ..Default::default()
        },
        max_events: 1,
        ..Default::default()
    };
    std::fs::write(&file, serde_json::to_vec(&session).unwrap()).unwrap();
    let _dialogs = Selections::new(vec![
        (Kind::Open, Some(file.clone())),
        (Kind::Save, Some(file.clone())),
    ]);
    let service = Arc::new(HttpService::for_workspace(root));
    let mut state = ProtocolConsole::default();
    state.open = true;
    let mut h = Harness::new(state, |ctx, state| {
        state.window(ctx, service.clone(), &BTreeMap::new())
    });
    for tab in ["MQTT", "Socket.IO", "gRPC", "WebSocket", "SSE"] {
        h.tool_click("Live protocol console", tab);
    }
    h.tool_click("Live protocol console", "Load protocol profile");
    assert_eq!(h.state.profile.max_events, 1);
    h.tool_click("Live protocol console", "Save protocol profile");
    assert!(serde_json::from_slice::<Session>(&std::fs::read(file).unwrap()).is_ok());
    h.tool_click("Live protocol console", "Connect protocol");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state.cancel.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.refresh();
    }
    assert!(h.state.cancel.is_none(), "{}", h.state.result);
    assert!(
        h.state
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|entry| entry.preview.contains("live")),
        "{}",
        h.state.result
    );
}
