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
    assert!(h
        .nodes()
        .iter()
        .any(|(_, n)| n.name() == Some("📁 Select File")));
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
        AuthType::Inherit,
        AuthType::OAuth2,
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
        (None, false, ResponseTab::Body, true, String::new()),
        |ctx: &egui::Context, s: &mut (Option<ApiResponse>, bool, ResponseTab, bool, String)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_response_viewer(ui, s.0.as_ref(), s.1, &mut s.2, &mut s.3, &mut s.4)
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
