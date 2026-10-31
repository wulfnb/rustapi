use super::*;
use crate::test_support::{Harness, Profile};
use dialogs::{Kind, Selections};
#[test]
fn self_hosted_server_token_catalog_push_pull_and_policy_controls() {
    let profile = Profile::new();
    let root = profile.directory.path();
    let collections = vec![Collection::new("Shared GUI API")];
    let service = Arc::new(HttpService::for_workspace(root));
    let config = root.join("team.json");
    let _dialogs = Selections::new(vec![
        (Kind::Save, Some(config.clone())),
        (Kind::Open, Some(config.clone())),
    ]);
    let mut state = PlatformTools::default();
    state.open = true;
    state.config.port = 0;
    let mut secrets = BTreeMap::new();
    let mut actions = vec![];
    let mut h = Harness::new(state, |ctx, state| {
        if let Some(action) = state.window(ctx, &collections, service.clone(), &secrets) {
            match action {
                PlatformAction::Secrets(values) => secrets.extend(values),
                other => actions.push(other),
            }
        }
    });
    let title = "Optional self-hosted tools";
    h.tool_click(title, "Host a team server");
    h.tool_click(title, "Create 30-day token user");
    assert_eq!(h.state.config.users.len(), 1);
    h.tool_click(title, "Save server configuration");
    let saved = std::fs::read_to_string(&config).unwrap();
    assert!(!saved.contains(h.state.new_token.as_str()));
    h.tool_click(title, "Load server configuration");
    h.tool_click(title, "Use new token for this session");
    h.tool_click(title, "Start self-hosted server");
    assert!(h.state.server.is_some(), "{}", h.state.result);
    h.tool_click(title, "Use this server URL");
    h.tool_click(title, "Host a team server");
    h.tool_click(title, "Team collaboration and API catalog");
    h.tool_click(title, "Push selected collection");
    wait(&mut h);
    assert_eq!(h.state.result, "Completed");
    h.tool_click(title, "Refresh team catalog");
    wait(&mut h);
    assert_eq!(h.state.catalog.len(), 1);
    h.state.selected_remote = h.state.catalog[0].id.clone();
    h.tool_click(title, "Import remote as local copy");
    wait(&mut h);
    h.tool_click(title, "Team collaboration and API catalog");
    h.tool_click(title, "Workspace execution policy");
    h.tool_click(title, "Load workspace policy");
    h.tool_click(title, "Allow AI endpoint calls");
    h.tool_click(title, "Save and apply workspace policy");
    assert!(!service.policy().unwrap().allow_ai);
    h.tool_click(title, "Workspace execution policy");
    h.tool_click(title, "Host a team server");
    h.tool_click(title, "Stop team server");
    assert!(h.state.server.is_none());
    drop(h);
    assert!(actions
        .iter()
        .any(|a| matches!(a, PlatformAction::Linked { .. })));
    assert!(actions
        .iter()
        .any(|a| matches!(a, PlatformAction::Import(_))));
}
fn wait<F: FnMut(&egui::Context, &mut PlatformTools)>(h: &mut Harness<PlatformTools, F>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state.cancel.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.refresh();
    }
    assert!(h.state.cancel.is_none(), "{}", h.state.result);
}
