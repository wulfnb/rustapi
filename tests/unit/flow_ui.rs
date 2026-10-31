use super::*;
use crate::test_support::{Harness, Profile};
use dialogs::{Kind, Selections};
#[test]
fn visual_flow_build_edit_save_reload_and_execute() {
    let profile = Profile::new();
    let root = profile.directory.path();
    let mut collection = Collection::new("Flow GUI collection");
    collection
        .requests
        .push(crate::models::ApiRequest::new("Failure node"));
    crate::storage::save_collection(&collection, None).unwrap();
    let collections = vec![collection];
    let service = Arc::new(HttpService::for_workspace(root));
    let mut state = FlowEditor::default();
    state.open = true;
    let mut h = Harness::new(state, |ctx, state| {
        state.window(ctx, &collections, service.clone(), &BTreeMap::new())
    });
    h.tool_click("Visual request flows", "Build from a saved collection");
    h.tool_click("Visual request flows", "Flow GUI collection");
    assert_eq!(h.state.flow.nodes.len(), 1);
    h.tool_click("Visual request flows", "Add flow node");
    assert_eq!(h.state.flow.nodes.len(), 2);
    h.tool_click("Visual request flows", "Start at selected node");
    assert_eq!(h.state.flow.start, h.state.selected);
    h.tool_click("Visual request flows", "Delete selected node");
    assert_eq!(h.state.flow.nodes.len(), 1);
    h.tool_click("Visual request flows", "Save flow");
    assert!(h.state.result.contains("Flow saved"), "{}", h.state.result);
    let file = root.join("flows").join(format!("{}.json", h.state.flow.id));
    let _dialogs = Selections::new(vec![(Kind::Open, Some(file))]);
    h.tool_click("Visual request flows", "Load flow");
    h.tool_click("Visual request flows", "Refresh flow request names");
    assert_eq!(h.state.requests.len(), 1);
    h.tool_click("Visual request flows", "Run visual flow");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state.cancel.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.refresh();
    }
    assert!(h.state.cancel.is_none());
    assert!(
        h.state.result.contains("\"passed\": false"),
        "{}",
        h.state.result
    );
}
