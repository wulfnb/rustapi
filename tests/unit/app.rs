use super::*;
use crate::test_support::{Harness, Profile};

struct ChosenFile(Option<std::path::PathBuf>);
impl CollectionDialogs for ChosenFile {
    fn import(&self) -> Option<std::path::PathBuf> {
        self.0.clone()
    }
    fn export(&self, _: &str) -> Option<std::path::PathBuf> {
        self.0.clone()
    }
}
fn app_with_collection() -> RustApiApp {
    let mut app = RustApiApp::default();
    assert!(app.commit_collection(Collection::new("Managed")));
    app
}
#[test]
fn file_dialog_commands_handle_chosen_files_and_cancellation() {
    let profile = Profile::new();
    let source = profile.directory.path().join("input.json");
    let original = serde_json::to_vec(&Collection::new("Imported")).unwrap();
    std::fs::write(&source, &original).unwrap();
    let app = RustApiApp {
        dialogs: Box::new(ChosenFile(Some(source.clone()))),
        ..Default::default()
    };
    let mut h = Harness::new(app, |ctx: &egui::Context, app: &mut RustApiApp| {
        app.render(ctx)
    });
    h.click("Import");
    h.click("Choose collection or environment JSON");
    assert_eq!(h.state.collections[0].name, "Imported");
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let id = h.state.collections[0].id.clone();
    h.state.dialogs = Box::new(ChosenFile(None));
    h.click("Choose collection or environment JSON");
    assert_eq!(h.state.collections.len(), 1);
    h.state.action(Action::Export(id.clone()));
    let exported = profile.directory.path().join("export.json");
    h.state.dialogs = Box::new(ChosenFile(Some(exported.clone())));
    h.state.action(Action::Export(id));
    assert_eq!(
        storage::load_collection(&exported).unwrap().name,
        "Imported"
    );
}
#[test]
fn application_manages_collections_requests_tabs_and_persistent_state() {
    let _profile = Profile::new();
    let mut app = app_with_collection();
    let cid = app.collections[0].id.clone();
    app.send_request();
    assert!(!app.save_tab("missing", None));
    app.action(Action::Add(cid.clone(), None));
    let tid = app.tab_manager.active_tab().unwrap().id.clone();
    let rid = app.tab_manager.active_tab().unwrap().request.id.clone();
    app.tab_manager.active_tab_mut().unwrap().request.name = "Saved".into();
    assert!(app.save_tab(&tid, None));
    app.action(Action::Duplicate(cid.clone(), rid.clone()));
    assert_eq!(app.collections[0].requests.len(), 2);
    app.action(Action::Delete(cid.clone(), rid));
    assert_eq!(app.collections[0].requests.len(), 1);
    assert!(app
        .tab_manager
        .active_tab()
        .unwrap()
        .collection_id
        .is_none());
    app.action(Action::NewFolder(cid.clone(), None));
    let fid = app.collections[0].folders[0].id.clone();
    app.action(Action::Add(cid.clone(), Some(fid.clone())));
    assert_eq!(app.collections[0].folders[0].requests.len(), 1);
    app.action(Action::DeleteFolder(cid.clone(), fid));
    app.persist_drafts();
    assert!(!RustApiApp::default().tab_manager.tabs.is_empty());
    app.action(Action::DeleteCollection(cid));
    assert!(app.collections.is_empty());
}
#[test]
fn application_renders_welcome_toolbar_editor_modals_and_history_actions() {
    let _profile = Profile::new();
    let mut h = Harness::new(
        app_with_collection(),
        |ctx: &egui::Context, app: &mut RustApiApp| app.render(ctx),
    );
    h.right_click("Managed");
    h.click("Add request");
    assert_eq!(h.state.collections[0].requests.len(), 1);
    h.click("+ New Request");
    assert!(h.state.tab_manager.has_tabs());
    for (label, kind) in [
        ("Tests", RequestTab::Tests),
        ("Scripts", RequestTab::Scripts),
        ("Settings", RequestTab::Settings),
    ] {
        h.click(label);
        assert_eq!(h.state.request_tab, kind);
    }
    h.click("Environments");
    assert!(h.state.show_environments);
    h.click("Save all and close");
    h.click("History");
    h.click("Close");
    assert!(!h.state.show_history);
    h.click("< >");
    assert!(h.state.show_code);
    h.click("Python");
    h.click("Copy");
    assert!(!h.output.platform_output.copied_text.is_empty());
    h.click("Close");
    h.click("+");
    h.state.new_name = "Created".into();
    h.refresh();
    h.click("Create");
    assert!(h.state.collections.iter().any(|c| c.name == "Created"));
    h.state.tab_manager.active_tab_mut().unwrap().is_modified = true;
    h.click_n("×", 1);
    assert!(h.state.close_pending.is_some());
    h.click("Cancel");
    assert!(h.state.close_pending.is_none());
    let mut history = RequestHistory::new(10);
    history.add(
        ApiRequest {
            url: "http://test/history".into(),
            ..Default::default()
        },
        None,
    );
    storage::save_history(&history).unwrap();
    h.click("History");
    h.click("http://test/history");
    assert_eq!(
        h.state.tab_manager.active_tab().unwrap().request.url,
        "http://test/history"
    );
    h.click("🗑 Clear All");
    assert!(storage::load_history().entries.is_empty());
}
#[test]
fn application_receives_network_results_and_saves_history() {
    let _profile = Profile::new();
    let mut app = RustApiApp::default();
    app.tab_manager.open_tab(
        ApiRequest {
            url: "://invalid".into(),
            ..Default::default()
        },
        None,
    );
    app.send_request();
    assert!(app.tab_manager.active_tab().unwrap().pending.is_some());
    let message = app.receiver.recv_timeout(Duration::from_secs(10)).unwrap();
    app.sender.send(message).unwrap();
    let h = Harness::new(app, |ctx: &egui::Context, app: &mut RustApiApp| {
        app.render(ctx)
    });
    assert!(h.state.tab_manager.active_tab().unwrap().pending.is_none());
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
    assert_eq!(storage::load_history().entries.len(), 1);
}
#[test]
fn imported_collections_survive_restart_without_modifying_the_source_file() {
    let profile = Profile::new();
    let source = profile.directory.path().join("external.json");
    let original = serde_json::to_vec(&Collection::new("Imported API")).unwrap();
    std::fs::write(&source, &original).unwrap();
    let mut app = RustApiApp {
        dialogs: Box::new(ChosenFile(Some(source.clone()))),
        ..Default::default()
    };
    app.choose_import();
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert!(app.collections[0]
        .file_path
        .as_ref()
        .unwrap()
        .contains("collections"));
    assert_eq!(RustApiApp::default().collections[0].name, "Imported API");
    std::fs::write(&source, "corrupt").unwrap();
    app.choose_import();
    assert!(app.error.is_some());
    assert_eq!(app.collections.len(), 1);
    std::fs::write(&source, &original).unwrap();
    std::fs::remove_dir_all(storage::get_collections_dir()).unwrap();
    std::fs::write(storage::get_collections_dir(), "blocked storage").unwrap();
    app.choose_import();
    assert_eq!(app.collections.len(), 1);
}
#[test]
fn sidebar_commands_reject_stale_ids_and_keep_background_rename_target() {
    let _profile = Profile::new();
    let mut app = app_with_collection();
    let cid = app.collections[0].id.clone();
    for action in [
        Action::Add("missing".into(), None),
        Action::Delete(cid.clone(), "missing".into()),
        Action::Duplicate(cid.clone(), "missing".into()),
        Action::Rename(cid.clone(), "missing".into(), false),
        Action::Export("missing".into()),
        Action::DeleteCollection("missing".into()),
    ] {
        app.action(action);
    }
    assert!(!app.tab_manager.has_tabs());
    app.action(Action::Add(cid.clone(), None));
    let rid = app.tab_manager.active_tab().unwrap().request.id.clone();
    app.tab_manager.open_tab(ApiRequest::new("Different"), None);
    app.action(Action::Rename(cid, rid.clone(), false));
    assert_eq!(app.rename.as_ref().unwrap().1, rid);
    assert_eq!(
        app.tab_manager.active_tab().unwrap().request.name,
        "Different"
    );
}
#[test]
fn desktop_windows_persist_environment_and_collection_edits() {
    let _profile = Profile::new();
    let mut h = Harness::new(
        app_with_collection(),
        |ctx: &egui::Context, app: &mut RustApiApp| app.render(ctx),
    );
    let cid = h.state.collections[0].id.clone();
    h.state.action(Action::Edit(cid.clone(), None));
    h.refresh();
    h.state.edit_draft.as_mut().unwrap().name = "Renamed".into();
    h.refresh();
    h.click("Save");
    assert_eq!(storage::load_all_collections()[0].name, "Renamed");
    h.click("Environments");
    h.click("New environment");
    let id = h.state.environments[0].id.clone();
    h.state.environments[0].name = "Development".into();
    h.click("Save all and close");
    assert_eq!(storage::load_all_environments()[0].id, id);
    h.click("Runner");
    assert!(h.state.show_runner);
    h.click("Close");
    h.click("Vault");
    assert!(h.state.show_vault);
    h.click("Close");
    h.click("Cookies");
    h.click("Clear all cookies");
    h.click("Close");
}
