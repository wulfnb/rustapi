use super::*;
use crate::test_support::{Harness, Profile};
use dialogs::{Kind, Selections};
use rustapi::{http::HttpService, models::*};
use std::{collections::BTreeMap, sync::Arc};

#[test]
fn specification_editor_import_docs_mock_and_draft_recovery() {
    let profile = Profile::new();
    let root = profile.directory.path();
    let source = root.join("api.json");
    let spec = r#"{"openapi":"3.0.3","info":{"title":"GUI API","version":"1"},"paths":{"/ping":{"get":{"operationId":"ping","responses":{"200":{"description":"OK"}}}}}}"#;
    std::fs::write(&source, spec).unwrap();
    let mut collection = Collection::new("GUI examples");
    let mut request = ApiRequest::new("Ping");
    request.url = "https://example.test/ping".into();
    request.examples.push(
        RequestExample::capture(
            "Success".into(),
            &request,
            &ApiResponse {
                status: 200,
                body: "ok".into(),
                ..Default::default()
            },
        )
        .unwrap(),
    );
    collection.requests.push(request);
    let collections = vec![collection];
    let service = Arc::new(HttpService::for_workspace(root));
    let mut imported = None;
    let mut h = Harness::new(
        ApiTools {
            open: true,
            ..Default::default()
        },
        |ctx, state| {
            if let Some(value) = state.window(ctx, &collections, service.clone(), &BTreeMap::new())
            {
                imported = Some(value);
            }
        },
    );
    let _dialogs = Selections::new(vec![
        (Kind::Open, Some(source.clone())),
        (Kind::Save, Some(source.clone())),
        (Kind::Save, Some(source.clone())),
        (Kind::Folder, Some(root.to_owned())),
        (Kind::Save, Some(root.join("mock.json"))),
        (Kind::Open, Some(root.join("mock.json"))),
    ]);
    h.tool_click("API tools", "Open specification");
    assert_eq!(h.state.spec, spec);
    h.tool_click("API tools", "Lint specification");
    assert_eq!(h.state.result, "[]");
    h.tool_click("API tools", "Import operations");
    h.state.spec = h.state.spec.replace("GUI API", "Edited API");
    h.refresh();
    assert_eq!(ApiTools::recover().spec, h.state.spec);
    std::fs::write(&source, "external edit").unwrap();
    h.tool_click("API tools", "Save specification as");
    assert!(h.state.result.contains("changed on disk"));
    std::fs::write(&source, spec).unwrap();
    h.tool_click("API tools", "Save specification as");
    assert!(std::fs::read_to_string(&source)
        .unwrap()
        .contains("Edited API"));
    h.tool_click(
        "API tools",
        "Generate offline Markdown + HTML documentation",
    );
    assert!(h.state.result.contains("Documentation written"));
    h.tool_click("API tools", "Create mock rules from saved examples");
    h.tool_click(
        "API tools",
        "Mock matching rules, response delays and faults",
    );
    h.tool_click("API tools", "Save mock profile");
    h.state.mock_config.clear();
    h.tool_click("API tools", "Load mock profile");
    h.tool_click("API tools", "Start local mock");
    assert!(h.state.mock.is_some(), "{}", h.state.result);
    h.tool_click("API tools", "Stop local mock");
    assert!(h.state.mock.is_none());
    drop(h);
    assert_eq!(
        imported
            .unwrap()
            .collection
            .unwrap()
            .ordered_requests(None)
            .len(),
        1
    );
}
