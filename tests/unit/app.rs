use super::*;
use crate::test_support::{Harness, Profile};

#[test]
fn application_manages_collections_requests_tabs_and_persistent_state() {
    let _profile = Profile::new();
    let mut app = RustApiApp::default();
    assert_eq!(app.collections.len(), 1);
    app.send_request();
    app.tab_manager.open_tab(ApiRequest::default(), None, None);
    app.send_request();
    assert!(!app.is_loading);
    app.save_current_request();
    app.add_request_to_collection(0);
    app.tab_manager.active_tab_mut().unwrap().request.name = "Saved name".into();
    app.save_current_request();
    assert_eq!(app.collections[0].requests[0].name, "Saved name");
    app.duplicate_request(0, 0);
    assert_eq!(app.collections[0].requests.len(), 2);
    app.delete_request(0, 0);
    assert_eq!(app.collections[0].requests.len(), 1);
    app.create_collection("Second".into());
    app.add_request_to_collection(1);
    app.delete_collection(1);
    assert_eq!(app.collections.len(), 1);
    app.delete_collection(99);
    app.add_request_to_collection(99);
    app.delete_request(99, 0);
    app.delete_request(0, 99);
    app.duplicate_request(99, 0);
    app.duplicate_request(0, 99);
    app.save_state();
    let reloaded = RustApiApp::default();
    assert!(!reloaded.collections.is_empty());
}

#[test]
fn application_renders_welcome_toolbar_editor_modals_and_history_actions() {
    let _profile = Profile::new();
    let mut h = Harness::new(
        RustApiApp::default(),
        |ctx: &egui::Context, app: &mut RustApiApp| app.render(ctx),
    );
    h.click("➕ New Request");
    assert!(h.state.tab_manager.has_tabs());
    h.click("➕");
    // The collection button precedes the tab-bar add button in the accessibility tree.
    if h.state.show_new_collection_modal {
        h.click("Cancel");
    }
    h.click("⚙");
    assert!(h.state.show_environment_editor);
    h.refresh();
    h.click("Close");
    h.click("📜 History");
    assert!(h.state.show_history_panel);
    h.refresh();
    h.click("Close");
    assert!(!h.state.show_history_panel);
    h.click("< >");
    assert!(h.state.show_code_gen_modal);
    h.refresh();
    h.click("Close");
    h.right_click("My Collection");
    h.click("➕ Add Request");
    assert_eq!(h.state.collections[0].requests.len(), 1);
    h.click("💾 Save");
    h.right_click_n("Request 1", 1);
    h.click("📋 Duplicate");
    assert_eq!(h.state.collections[0].requests.len(), 2);
    h.right_click_n("Request 1", 1);
    h.click("✏ Rename");
    assert!(h.state.show_rename_modal);
    h.refresh();
    h.click("Cancel");
    h.right_click("My Collection");
    h.click("✏ Rename");
    assert!(h.state.show_rename_modal);
    h.refresh();
    h.click("Cancel");
    h.click("Request 1");
    h.right_click_n("Request 1", 1);
    h.click("🗑 Delete");
    assert_eq!(h.state.collections[0].requests.len(), 1);
    h.right_click("My Collection");
    h.click("🗑 Delete");
    assert!(h.state.collections.is_empty());
    h.state.show_new_collection_modal = true;
    h.state.new_item_name = "Created".into();
    h.refresh();
    h.click("Create");
    assert_eq!(h.state.collections[0].name, "Created");
    h.state.history.add(
        ApiRequest {
            url: "http://test/history".into(),
            ..Default::default()
        },
        None,
    );
    h.state.show_history_panel = true;
    h.refresh();
    h.click("http://test/history");
    assert_eq!(
        h.state.tab_manager.active_tab().unwrap().request.url,
        "http://test/history"
    );
    h.click("🗑 Clear All");
    assert!(h.state.history.entries.is_empty());
    h.click("Close");
    h.state.show_code_gen_modal = true;
    h.state.tab_manager = TabManager::new();
    h.refresh();
}

#[test]
fn application_receives_network_results_and_saves_history() {
    let _profile = Profile::new();
    let mut app = RustApiApp::default();
    let env = Environment::new("test");
    app.active_environment_id = Some(env.id.clone());
    app.environments.push(env);
    assert!(app.get_active_environment().is_some());
    app.tab_manager.open_tab(
        ApiRequest {
            url: "://invalid".into(),
            ..Default::default()
        },
        None,
        None,
    );
    app.send_request();
    assert!(app.is_loading);
    let response = app
        .async_receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    app.async_sender.send(response).unwrap();
    let mut h = Harness::new(app, |ctx: &egui::Context, app: &mut RustApiApp| {
        app.render(ctx)
    });
    assert!(!h.state.is_loading);
    assert!(h
        .state
        .tab_manager
        .active_tab()
        .unwrap()
        .response
        .as_ref()
        .unwrap()
        .error
        .is_some());
    assert_eq!(h.state.history.entries.len(), 1);
    h.state
        .async_sender
        .send(AsyncMessage::ResponseReceived(ApiResponse::error(
            "closed tab",
        )))
        .unwrap();
    h.state.tab_manager = TabManager::new();
    h.refresh();
    h.state.is_loading = true;
    h.refresh();
}
