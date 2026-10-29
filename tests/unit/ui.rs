use super::*;
use crate::models::*;
use crate::test_support::{Harness, Profile};
use egui::accesskit::Role;

#[test]
fn request_editor_switches_tabs_body_types_and_edits_rows() {
    let _profile = Profile::new();
    let state = (ApiRequest::default(), RequestTab::Params, false);
    let mut h = Harness::new(
        state,
        |ctx: &egui::Context, state: &mut (ApiRequest, RequestTab, bool)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_request_editor(ui, &mut state.0, &mut state.1, &mut state.2, true)
            });
        },
    );
    h.click("➕ Add");
    assert_eq!(h.state.0.query_params.len(), 1);
    h.refresh();
    h.click("🗑");
    assert!(h.state.0.query_params.is_empty());
    for (label, expected) in [
        ("Headers", RequestTab::Headers),
        ("Body", RequestTab::Body),
        ("Auth", RequestTab::Auth),
        ("Params", RequestTab::Params),
    ] {
        h.click(label);
        assert_eq!(h.state.1, expected);
    }
    h.click("Body");
    for kind in BodyType::all() {
        h.refresh();
        h.click(kind.as_str());
        assert_eq!(&h.state.0.body_type, kind);
    }
    h.state.0.body_type = BodyType::Binary;
    h.refresh();
    h.click("📁 Select File");
    h.click("💾 Save");
    assert!(h.state.2);
}

#[test]
fn auth_editor_selects_every_auth_type_and_api_key_location() {
    let mut h = Harness::new(
        AuthConfig::default(),
        |ctx: &egui::Context, state: &mut AuthConfig| {
            egui::CentralPanel::default()
                .show(ctx, |ui| super::auth_editor::render_auth_editor(ui, state));
        },
    );
    for kind in [
        AuthType::BearerToken,
        AuthType::BasicAuth,
        AuthType::ApiKey,
        AuthType::None,
    ] {
        h.combo(0);
        h.refresh();
        h.click(kind.as_str());
        assert_eq!(h.state.auth_type, kind);
        h.refresh();
        if kind == AuthType::ApiKey {
            h.combo(1);
            h.refresh();
            h.click("Query Param");
            assert_eq!(h.state.api_key_location, ApiKeyLocation::QueryParam);
            h.refresh();
            h.combo(1);
            h.click("Header");
            assert_eq!(h.state.api_key_location, ApiKeyLocation::Header);
        }
    }
}

#[test]
fn url_bar_selects_methods_sends_on_enter_and_disables_loading_button() {
    let mut h = Harness::new(
        (ApiRequest::default(), false, false, false),
        |ctx: &egui::Context, s: &mut (ApiRequest, bool, bool, bool)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_url_bar(ui, &mut s.0, s.1, &mut s.2, &mut s.3)
            });
        },
    );
    for method in [
        HttpMethod::POST,
        HttpMethod::PUT,
        HttpMethod::DELETE,
        HttpMethod::PATCH,
        HttpMethod::HEAD,
        HttpMethod::OPTIONS,
        HttpMethod::GET,
    ] {
        h.combo(0);
        h.refresh();
        h.click(method.as_str());
        assert_eq!(h.state.0.method, method);
    }
    h.click("< >");
    assert!(h.state.3);
    h.click("▶ Send");
    assert!(h.state.2);
    h.state.2 = false;
    h.edit(Role::TextInput, 0, "http://localhost/example");
    h.key(egui::Key::Enter);
    assert!(h.state.2);
    assert_eq!(h.state.0.url, "http://localhost/example");
    h.state.1 = true;
    h.state.2 = false;
    h.refresh();
    h.click("⏳ Sending...");
    assert!(!h.state.2);
}

#[test]
fn response_viewer_handles_empty_loading_error_headers_and_copy() {
    let mut h = Harness::new(
        (None, false, ResponseTab::Body, true),
        |ctx: &egui::Context, s: &mut (Option<ApiResponse>, bool, ResponseTab, bool)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_response_viewer(ui, s.0.as_ref(), s.1, &mut s.2, &mut s.3)
            });
        },
    );
    h.state.1 = true;
    h.refresh();
    h.state.0 = Some(ApiResponse::error("Network unavailable"));
    h.refresh();
    assert!(h
        .nodes()
        .iter()
        .any(|(_, n)| n.name() == Some("Network unavailable")));
    for size in [42, 2048, 2097152] {
        h.state.0 = Some(ApiResponse {
            status: 200,
            status_text: "OK".into(),
            body: "{\"ok\":true}".into(),
            headers: vec![("x-test".into(), "value".into())],
            size,
            ..Default::default()
        });
        h.refresh();
    }
    h.click("📋 Copy");
    assert_eq!(h.output.platform_output.copied_text, "{\"ok\":true}");
    h.click("Headers");
    assert_eq!(h.state.2, ResponseTab::Headers);
    h.refresh();
    assert!(h.nodes().iter().any(|(_, n)| n.name() == Some("x-test")));
    h.click("Body");
    assert_eq!(h.state.2, ResponseTab::Body);
    h.click("Format JSON");
    assert!(!h.state.3);
    h.refresh();
}

#[test]
fn code_generation_dialog_switches_languages_copies_and_closes() {
    let mut h = Harness::new(
        (
            true,
            ApiRequest::new("sample"),
            crate::http::CodeGenLanguage::Curl,
        ),
        |ctx: &egui::Context, s: &mut (bool, ApiRequest, crate::http::CodeGenLanguage)| {
            render_code_gen_modal(ctx, &mut s.0, &s.1, None, &mut s.2);
        },
    );
    for language in crate::http::CodeGenLanguage::all() {
        h.click(language.as_str());
        assert_eq!(&h.state.2, language);
        h.refresh();
    }
    h.click("📋 Copy to Clipboard");
    assert!(h.output.platform_output.copied_text.contains("reqwest"));
    h.click("Close");
    assert!(!h.state.0);
    h.refresh();
}

#[test]
fn top_bar_changes_environment_and_emits_toolbar_actions() {
    let env = Environment::new("Production");
    let env_id = env.id.clone();
    let mut h = Harness::new(
        (vec![env], None, false, false, false),
        |ctx: &egui::Context, s: &mut (Vec<Environment>, Option<String>, bool, bool, bool)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_top_bar(ui, &s.0, &mut s.1, &mut s.2, &mut s.3, &mut s.4)
            });
        },
    );
    h.combo(0);
    h.click("Production");
    assert_eq!(h.state.1, Some(env_id));
    h.refresh();
    h.combo(0);
    h.click("No Environment");
    assert_eq!(h.state.1, None);
    h.click("⚙");
    h.click("📜 History");
    h.click("📁 Import");
    assert!(h.state.2 && h.state.3 && h.state.4);
}

#[test]
fn sidebar_search_selection_expansion_and_context_actions() {
    let mut collection = Collection::new("Example collection");
    let mut request = ApiRequest::new("Find users");
    request.url = "http://test/users".into();
    collection.requests.push(request);
    let mut h = Harness::new(
        (vec![collection], String::new(), Vec::new()),
        |ctx: &egui::Context, s: &mut (Vec<Collection>, String, Vec<SidebarAction>)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(action) = render_sidebar(ui, &mut s.0, &mut s.1, Some(0), Some(0)) {
                    s.2.push(action);
                }
            });
        },
    );
    h.click("➕");
    assert!(matches!(
        h.state.2.last(),
        Some(SidebarAction::NewCollection)
    ));
    h.click("Find users");
    assert!(matches!(
        h.state.2.last(),
        Some(SidebarAction::SelectRequest(0, 0))
    ));
    for (name, expected) in [
        ("✏ Rename", "rename"),
        ("📋 Duplicate", "duplicate"),
        ("🗑 Delete", "delete"),
    ] {
        h.right_click("Find users");
        h.click(name);
        assert!(matches!(
            (expected, h.state.2.last().unwrap()),
            ("rename", SidebarAction::RenameRequest(0, 0))
                | ("duplicate", SidebarAction::DuplicateRequest(0, 0))
                | ("delete", SidebarAction::DeleteRequest(0, 0))
        ));
        h.refresh();
    }
    for (name, expected) in [
        ("➕ Add Request", "add"),
        ("✏ Rename", "rename"),
        ("📤 Export", "export"),
        ("🗑 Delete", "delete"),
    ] {
        h.right_click("Example collection");
        h.click(name);
        assert!(matches!(
            (expected, h.state.2.last().unwrap()),
            ("add", SidebarAction::AddRequest(0))
                | ("rename", SidebarAction::RenameCollection(0))
                | ("export", SidebarAction::ExportCollection(0))
                | ("delete", SidebarAction::DeleteCollection(0))
        ));
        h.refresh();
    }
    h.state.1 = "missing".into();
    h.refresh();
    assert!(!h
        .nodes()
        .iter()
        .any(|(_, n)| n.name() == Some("Find users")));
    h.state.1 = "USERS".into();
    h.refresh();
    assert!(h
        .nodes()
        .iter()
        .any(|(_, n)| n.name() == Some("Find users")));
    h.click("▼");
    assert!(!h.state.0[0].expanded);
    h.refresh();
    h.click("Example collection");
    assert!(h.state.0[0].expanded);
}

#[test]
fn new_collection_dialog_creates_and_cancels() {
    let mut h = Harness::new(
        (false, String::new(), None),
        |ctx: &egui::Context, s: &mut (bool, String, Option<String>)| {
            if let Some(name) = render_new_collection_modal(ctx, &mut s.0, &mut s.1) {
                s.2 = Some(name);
            }
        },
    );
    h.state.0 = true;
    h.refresh();
    h.click("Create");
    assert!(h.state.0);
    h.edit(Role::TextInput, 0, "New API");
    h.click("Create");
    assert_eq!(h.state.2.as_deref(), Some("New API"));
    assert!(!h.state.0 && h.state.1.is_empty());
    h.state.0 = true;
    h.state.1 = "discard".into();
    h.refresh();
    h.click("Cancel");
    assert!(!h.state.0 && h.state.1.is_empty());
}

#[test]
fn rename_dialog_persists_collection_and_request_and_can_cancel() {
    let _profile = Profile::new();
    let mut collection = Collection::new("Example");
    collection.requests.push(ApiRequest::new("Original"));
    let mut h = Harness::new(
        (
            false,
            RenameTarget::None,
            String::new(),
            vec![collection],
            String::from("Original"),
        ),
        |ctx: &egui::Context, s: &mut (bool, RenameTarget, String, Vec<Collection>, String)| {
            render_rename_modal(ctx, &mut s.0, &s.1, &mut s.2, &mut s.3, Some(&mut s.4));
        },
    );
    h.state.0 = true;
    h.refresh();
    for (target, name) in [
        (RenameTarget::Collection(0), "Renamed collection"),
        (RenameTarget::Request(0, 0), "Renamed request"),
    ] {
        h.state.0 = true;
        h.state.1 = target;
        h.state.2 = name.into();
        h.refresh();
        h.click("Save");
        assert!(!h.state.0 && h.state.2.is_empty());
    }
    assert_eq!(h.state.3[0].name, "Renamed collection");
    assert_eq!(h.state.3[0].requests[0].name, "Renamed request");
    assert_eq!(h.state.4, "Renamed request");
    h.state.0 = true;
    h.state.2 = "discard".into();
    h.refresh();
    h.click("Cancel");
    assert!(!h.state.0 && h.state.2.is_empty());
    for target in [RenameTarget::Request(0, 99), RenameTarget::Collection(99)] {
        h.state.0 = true;
        h.state.1 = target;
        h.state.2 = "invalid target".into();
        h.refresh();
        h.click("Save");
        assert!(!h.state.0);
    }
}

#[test]
fn environment_editor_adds_edits_saves_and_deletes_active_environment() {
    let profile = Profile::new();
    assert!(profile.directory.path().join("environments").is_dir());
    let mut h = Harness::new(
        (false, Vec::new(), None),
        |ctx: &egui::Context, s: &mut (bool, Vec<Environment>, Option<String>)| {
            render_environment_editor(ctx, &mut s.0, &mut s.1, &mut s.2)
        },
    );
    h.state.0 = true;
    h.refresh();
    h.click("➕ New Environment");
    assert_eq!(h.state.1.len(), 1);
    h.refresh();
    h.click("Environment 1");
    h.refresh();
    h.click("➕ Add Variable");
    assert_eq!(h.state.1[0].variables.len(), 1);
    h.refresh();
    h.edit(Role::TextInput, 1, "token");
    h.key(egui::Key::Tab);
    h.edit(Role::TextInput, 2, "secret");
    h.key(egui::Key::Tab);
    assert_eq!(h.state.1[0].variables[0].key, "token");
    assert_eq!(h.state.1[0].variables[0].value, "secret");
    h.click_rect(h.role_rect(Role::CheckBox, 0), egui::PointerButton::Primary);
    assert!(h.state.1[0].variables[0].enabled);
    h.click("💾 Save");
    h.click("Save All");
    assert_eq!(
        crate::storage::load_all_environments()[0].variables[0].value,
        "secret"
    );
    h.edit(Role::TextInput, 0, "Renamed environment");
    h.key(egui::Key::Tab);
    assert_eq!(h.state.1[0].name, "Renamed environment");
    h.click("🗑");
    assert!(h.state.1[0].variables.is_empty());
    h.state.2 = Some(h.state.1[0].id.clone());
    h.click("🗑 Delete Environment");
    assert!(h.state.1.is_empty() && h.state.2.is_none());
    assert!(crate::storage::load_all_environments().is_empty());
    h.refresh();
    h.click("➕ New Environment");
    h.refresh();
    h.click("Close");
    assert!(!h.state.0);
}

#[test]
fn history_panel_loads_and_clears_entries() {
    let mut history = RequestHistory::new(10);
    history.add(
        ApiRequest {
            url: "http://test/history".into(),
            ..Default::default()
        },
        Some(ApiResponse {
            status: 200,
            ..Default::default()
        }),
    );
    let mut h = Harness::new(
        (true, history, Vec::new()),
        |ctx: &egui::Context, s: &mut (bool, RequestHistory, Vec<HistoryAction>)| {
            if let Some(action) = render_history_panel(ctx, &mut s.0, &s.1) {
                s.2.push(action);
            }
        },
    );
    h.click("http://test/history");
    assert!(matches!(
        h.state.2.last(),
        Some(HistoryAction::LoadRequest(_))
    ));
    h.click("🗑 Clear All");
    assert!(matches!(
        h.state.2.last(),
        Some(HistoryAction::ClearHistory)
    ));
    h.state.1.clear();
    h.refresh();
    h.click("Close");
    assert!(!h.state.0);
    h.refresh();
}
