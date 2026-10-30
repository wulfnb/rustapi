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
        .any(|(_, n)| n.label() == Some("📁 Select File") || n.value() == Some("📁 Select File")));
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
        .any(|(_, n)| n.label() == Some("Network unavailable")
            || n.value() == Some("Network unavailable")));
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
    assert!(h
        .nodes()
        .iter()
        .any(|(_, n)| n.label() == Some("x-test") || n.value() == Some("x-test")));
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

#[test]
fn description_controls_edit_content_without_discarding_import_metadata() {
    let request = ApiRequest {
        description: "Original request description".into(),
        description_metadata: Some(std::collections::BTreeMap::from([(
            "type".into(),
            serde_json::json!("text/markdown"),
        )])),
        ..Default::default()
    };
    let mut h = Harness::new(
        (request, RequestTab::Description, false),
        |ctx: &egui::Context, s: &mut (ApiRequest, RequestTab, bool)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_request_editor(ui, &mut s.0, &mut s.1, &mut s.2, true)
            });
        },
    );
    h.edit(
        egui::accesskit::Role::MultilineTextInput,
        0,
        "Edited Markdown",
    );
    assert_eq!(h.state.0.description, "Edited Markdown");
    assert_eq!(
        h.state.0.description_metadata.as_ref().unwrap()["type"],
        "text/markdown"
    );
}

#[test]
fn bulk_editor_is_atomic_and_keeps_separate_context_drafts() {
    let row = KeyValue::new("X-Old", "original");
    let mut h = Harness::new(
        ("first".to_owned(), vec![row]),
        |ctx: &egui::Context, s: &mut (String, Vec<KeyValue>)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.push_id(&s.0, |ui| render_bulk_rows(ui, &mut s.1));
            });
        },
    );
    h.click("Bulk edit JSON");
    h.edit(egui::accesskit::Role::MultilineTextInput, 0, "[invalid");
    h.click("Apply bulk rows");
    assert_eq!(h.state.1[0].value, "original");
    h.state.0 = "second".into();
    h.refresh();
    h.click("Bulk edit JSON");
    let rows = vec![KeyValue {
        description: "kept".into(),
        enabled: false,
        ..KeyValue::new("X-New", "updated")
    }];
    h.edit(
        egui::accesskit::Role::MultilineTextInput,
        0,
        &serde_json::to_string(&rows).unwrap(),
    );
    h.click("Apply bulk rows");
    assert_eq!(h.state.1[0].key, "X-New");
    assert!(!h.state.1[0].enabled);
    assert_eq!(h.state.1[0].description, "kept");
    h.state.0 = "first".into();
    h.refresh();
    assert!(h
        .nodes()
        .iter()
        .any(|(_, node)| node.value() == Some("[invalid")));
}

#[test]
fn header_preset_ui_persists_and_replaces_headers() {
    let _profile = Profile::new();
    let mut row = KeyValue::new("Accept", "application/json");
    row.description = "Retained metadata".into();
    let mut h = Harness::new(
        vec![row],
        |ctx: &egui::Context, rows: &mut Vec<KeyValue>| {
            egui::CentralPanel::default().show(ctx, |ui| render_header_presets(ui, rows));
        },
    );
    h.click("Header presets");
    h.edit(egui::accesskit::Role::TextInput, 0, "JSON headers");
    h.click("Save headers as preset");
    assert_eq!(
        crate::storage::HeaderPresets::load(&crate::storage::get_app_data_dir())
            .unwrap()
            .presets
            .len(),
        1
    );
    h.state.clear();
    h.click("Replace headers");
    assert_eq!(h.state[0].description, "Retained metadata");
    h.click("Append preset");
    assert_eq!(h.state.len(), 2);
    h.click("Delete preset");
    assert!(
        crate::storage::HeaderPresets::load(&crate::storage::get_app_data_dir())
            .unwrap()
            .presets
            .is_empty()
    );
}

#[test]
fn variable_inspector_masks_secret_aliases_and_shows_resolution_errors() {
    let mut vars = rustapi::variables::Variables::default();
    vars.environment
        .insert("credential".into(), "private-value".into());
    vars.local
        .insert("alias".into(), "prefix {{ credential }}".into());
    vars.local.insert("broken".into(), "{{missing}}".into());
    let env = crate::models::Environment {
        variables: vec![KeyValue {
            value_type: "secret".into(),
            ..KeyValue::new("credential", "private-value")
        }],
        ..Default::default()
    };
    let mut h = Harness::new(false, |ctx: &egui::Context, reveal: &mut bool| {
        egui::CentralPanel::default().show(ctx, |ui| {
            render_variable_inspector(ui, &vars, Default::default(), Some(&env), *reveal)
        });
    });
    assert!(!h.nodes().iter().any(|(_, node)| node
        .label()
        .or(node.value())
        .is_some_and(|text| text.contains("private-value"))));
    assert!(h.nodes().iter().any(|(_, node)| node
        .label()
        .or(node.value())
        .is_some_and(|text| text.contains("Unresolved variable: missing"))));
    h.state = true;
    h.refresh();
    assert!(h.nodes().iter().any(|(_, node)| node
        .label()
        .or(node.value())
        .is_some_and(|text| text.contains("prefix private-value"))));
}

#[test]
fn response_tree_pointer_raw_timing_and_inert_preview_controls() {
    let response = ApiResponse {
        status: 200,
        body: "{\"user\":{\"name\":\"Example\"}}".into(),
        headers: vec![("content-type".into(), "application/json".into())],
        time_ms: 35,
        headers_time_ms: 10,
        ..Default::default()
    };
    let mut h = Harness::new(
        (ResponseTab::Body, true, String::new()),
        |ctx: &egui::Context, s: &mut (ResponseTab, bool, String)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_response_viewer(ui, Some(&response), false, &mut s.0, &mut s.1, &mut s.2)
            });
        },
    );
    h.click("JSON tree");
    h.edit(egui::accesskit::Role::TextInput, 1, "/user/name");
    assert!(h.nodes().iter().any(|(_, node)| node
        .label()
        .or(node.value())
        .is_some_and(|text| text.contains("Example"))));
    h.click("Raw");
    assert_eq!(h.state.0, ResponseTab::Raw);
    h.click("Timing");
    assert!(h
        .nodes()
        .iter()
        .any(|(_, node)| node.label().or(node.value()) == Some("Body transfer: 25 ms")));
    h.click("Preview");
    assert_eq!(h.state.0, ResponseTab::Preview);
    assert_eq!(response_hex(&[0, 255, 128]), "00000000  00 ff 80");
}

#[test]
fn response_image_decoder_preserves_pixels_and_enforces_preview_limits() {
    let source = image::RgbaImage::from_raw(1, 1, vec![255, 0, 0, 255]).unwrap();
    let mut bytes = std::io::Cursor::new(vec![]);
    source
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let image = decode_response_image(bytes.get_ref()).unwrap();
    assert_eq!(image.size, [1, 1]);
    assert_eq!(image.pixels[0], egui::Color32::RED);
    assert!(decode_response_image(b"<html><script>bad()</script></html>").is_err());
    assert!(decode_response_image(&vec![0; 8 * 1024 * 1024 + 1]).is_err());
    let source = image::RgbaImage::new(4097, 1);
    let mut bytes = std::io::Cursor::new(vec![]);
    source
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    assert!(decode_response_image(bytes.get_ref()).is_err());
}

#[test]
fn schema_assertion_editor_selects_schema_and_accepts_multiline_json() {
    let request = ApiRequest {
        checks: vec![Assertion::default()],
        ..Default::default()
    };
    let mut h = Harness::new(
        (request, RequestTab::Tests, false),
        |ctx: &egui::Context, s: &mut (ApiRequest, RequestTab, bool)| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_request_editor(ui, &mut s.0, &mut s.1, &mut s.2, true)
            });
        },
    );
    h.combo(0);
    h.click("JsonSchema");
    h.edit(
        egui::accesskit::Role::MultilineTextInput,
        0,
        "{\n  \"type\": \"object\"\n}",
    );
    assert_eq!(h.state.0.checks[0].kind, AssertionKind::JsonSchema);
    assert!(h.state.0.checks[0].expected.contains("object"));
}
