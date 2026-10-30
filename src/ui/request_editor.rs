//! Request editor UI component

use super::auth_editor::render_auth_editor;
use crate::models::{ApiRequest, BodyType, HttpMethod, KeyValue};
use egui::Ui;

/// Request editor tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestTab {
    #[default]
    Params,
    Headers,
    Body,
    Auth,
    Tests,
    Scripts,
    Settings,
}

/// Render the request editor
pub fn render_request_editor(
    ui: &mut Ui,
    request: &mut ApiRequest,
    current_tab: &mut RequestTab,
    on_save: &mut bool,
    _has_save_target: bool,
) {
    // Tabs
    ui.horizontal(|ui| {
        if ui
            .selectable_label(*current_tab == RequestTab::Params, "Params")
            .clicked()
        {
            *current_tab = RequestTab::Params;
        }
        if ui
            .selectable_label(*current_tab == RequestTab::Headers, "Headers")
            .clicked()
        {
            *current_tab = RequestTab::Headers;
        }
        if ui
            .selectable_label(*current_tab == RequestTab::Body, "Body")
            .clicked()
        {
            *current_tab = RequestTab::Body;
        }
        if ui
            .selectable_label(*current_tab == RequestTab::Auth, "Auth")
            .clicked()
        {
            *current_tab = RequestTab::Auth;
        }

        for (tab, label) in [
            (RequestTab::Tests, "Tests"),
            (RequestTab::Scripts, "Scripts"),
            (RequestTab::Settings, "Settings"),
        ] {
            ui.selectable_value(current_tab, tab, label);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("💾 Save").clicked() {
                *on_save = true;
            }
        });
    });

    ui.separator();
    ui.add_space(8.0);

    // Tab content
    egui::ScrollArea::vertical().show(ui, |ui| match current_tab {
        RequestTab::Params => {
            ui.label("Query parameters");
            ui.push_id("query_parameters", |ui| render_key_value_editor(ui, &mut request.query_params));
            ui.separator();
            ui.label("Path variables (:name segments)");
            ui.push_id("path_parameters", |ui| render_key_value_editor(ui, &mut request.path_params));
        }
        RequestTab::Headers => {
            render_key_value_editor(ui, &mut request.headers);
        }
        RequestTab::Body => {
            render_body_editor(ui, request);
        }
        RequestTab::Auth => { render_auth_editor(ui, &mut request.auth); }
        RequestTab::Tests => {
            ui.label("All enabled checks must pass. JSON targets use JSON Pointer, e.g. /user/id.");
            let mut remove=None;
            for (i,c) in request.checks.iter_mut().enumerate() { ui.push_id(i,|ui| {
                ui.text_edit_singleline(&mut c.name);
                egui::ComboBox::from_id_salt("kind").selected_text(format!("{:?}",c.kind)).show_ui(ui,|ui|{for k in [crate::models::AssertionKind::Status,crate::models::AssertionKind::Header,crate::models::AssertionKind::Json,crate::models::AssertionKind::BodyContains,crate::models::AssertionKind::MaxTime]{let label=format!("{k:?}");ui.selectable_value(&mut c.kind,k,label);}});
                ui.horizontal(|ui|{ui.label("Target");ui.text_edit_singleline(&mut c.target);ui.label("Expected");ui.text_edit_singleline(&mut c.expected);if ui.small_button("Remove").clicked(){remove=Some(i);}});
            }); }
            if let Some(i)=remove {request.checks.remove(i);}
            if ui.button("Add assertion").clicked(){request.checks.push(Default::default());}
            ui.separator();ui.label("Extract JSON values into local variables for later requests");
            let mut remove=None;for(i,e)in request.extractors.iter_mut().enumerate(){ui.horizontal(|ui|{ui.text_edit_singleline(&mut e.variable);ui.text_edit_singleline(&mut e.pointer);if ui.small_button("Remove").clicked(){remove=Some(i);}});}if let Some(i)=remove{request.extractors.remove(i);}
            if ui.button("Add extraction").clicked(){request.extractors.push(Default::default());}
        }
        RequestTab::Scripts => {
            ui.label("Synchronous pm.test / pm.expect, response and variable APIs. 250 ms / 16 MiB limit.");
            ui.label("Pre-request");ui.add(egui::TextEdit::multiline(&mut request.pre_script).code_editor().desired_width(f32::INFINITY));
            ui.label("Post-response");ui.add(egui::TextEdit::multiline(&mut request.post_script).code_editor().desired_width(f32::INFINITY));
        }
        RequestTab::Settings => {
            let s=&mut request.settings;
            ui.horizontal(|ui|{ui.label("Timeout (ms)");ui.add(egui::DragValue::new(&mut s.timeout_ms).range(1..=3_600_000));});
            ui.checkbox(&mut s.follow_redirects,"Follow redirects");ui.checkbox(&mut s.verify_tls,"Verify TLS certificates");
            if !s.verify_tls{ui.colored_label(egui::Color32::YELLOW,"TLS verification disabled for this request");}
            ui.label("Proxy URL (empty uses system configuration)");ui.text_edit_singleline(&mut s.proxy);
            ui.label("Custom CA PEM file");ui.text_edit_singleline(&mut s.ca_file);
            ui.label("Client certificate + private key PEM file");ui.text_edit_singleline(&mut s.identity_file);
            ui.horizontal(|ui|{ui.label("Maximum response bytes");ui.add(egui::DragValue::new(&mut s.max_response_bytes).range(1..=256*1024*1024));});
        }
    });
}

/// Render key-value editor
pub fn render_key_value_editor(ui: &mut Ui, items: &mut Vec<KeyValue>) {
    let mut to_delete: Option<usize> = None;

    // Header row
    ui.horizontal(|ui| {
        ui.add_space(24.0);
        ui.label(egui::RichText::new("Key").strong());
        ui.add_space(150.0);
        ui.label(egui::RichText::new("Value").strong());
    });

    ui.add_space(4.0);

    for (idx, item) in items.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut item.enabled, "");

            let key_edit = egui::TextEdit::singleline(&mut item.key)
                .hint_text("Key")
                .desired_width(180.0);
            ui.add(key_edit);

            let value_edit = egui::TextEdit::singleline(&mut item.value)
                .hint_text("Value")
                .desired_width(ui.available_width() - 40.0);
            ui.add(value_edit);

            if ui.small_button("🗑").clicked() {
                to_delete = Some(idx);
            }
        });
    }

    // Delete if needed
    if let Some(idx) = to_delete {
        items.remove(idx);
    }

    ui.add_space(8.0);

    if ui.button("➕ Add").clicked() {
        items.push(KeyValue::default());
    }
}

/// Render body editor
fn render_body_editor(ui: &mut Ui, request: &mut ApiRequest) {
    // Body type selector
    ui.horizontal(|ui| {
        for body_type in BodyType::all() {
            if ui
                .selectable_label(request.body_type == *body_type, body_type.as_str())
                .clicked()
            {
                request.body_type = body_type.clone();
            }
        }
    });

    ui.add_space(8.0);

    match request.body_type {
        BodyType::None => {
            ui.label("This request does not have a body.");
        }
        BodyType::Json | BodyType::Text => {
            let text_edit = egui::TextEdit::multiline(&mut request.body)
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(10);
            ui.add(text_edit);
        }
        BodyType::FormData | BodyType::UrlEncoded => {
            render_key_value_editor(ui, &mut request.form_data);
            if request.body_type == BodyType::FormData {
                let mut additions = vec![];
                for (index, f) in request.form_data.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut f.file, format!("{} is a file", f.key));
                        if f.file && ui.button("Choose files").clicked() {
                            if let Some(paths) = rfd::FileDialog::new().pick_files() {
                                if let Some(first) = paths.first() {
                                    f.value = first.to_string_lossy().into();
                                    let extra: Vec<_> = paths
                                        .into_iter()
                                        .skip(1)
                                        .map(|path| {
                                            let mut part = f.clone();
                                            part.value = path.to_string_lossy().into();
                                            part
                                        })
                                        .collect();
                                    additions.push((index, extra));
                                }
                            }
                        }
                        ui.label("Part MIME type");
                        ui.text_edit_singleline(&mut f.content_type);
                    });
                }
                for (index, parts) in additions.into_iter().rev() {
                    request.form_data.splice(index + 1..index + 1, parts);
                }
            }
        }
        BodyType::Binary => {
            ui.label("Binary body - select a file to upload.");
            ui.text_edit_singleline(&mut request.binary_file);
            if ui.button("📁 Select File").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    request.binary_file = path.to_string_lossy().into();
                }
            }
        }
    }
}

/// Render the URL bar
pub fn render_url_bar(
    ui: &mut Ui,
    request: &mut ApiRequest,
    is_loading: bool,
    on_send: &mut bool,
    on_generate_code: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.add_space(16.0);

        // Method selector
        egui::ComboBox::from_id_salt("method_selector")
            .width(100.0)
            .selected_text(
                egui::RichText::new(request.method.as_str())
                    .color(request.method.color())
                    .strong(),
            )
            .show_ui(ui, |ui| {
                for method in HttpMethod::all() {
                    ui.selectable_value(
                        &mut request.method,
                        *method,
                        egui::RichText::new(method.as_str()).color(method.color()),
                    );
                }
            });

        // URL input
        let url_edit = egui::TextEdit::singleline(&mut request.url)
            .hint_text("Enter URL...")
            .font(egui::TextStyle::Monospace)
            .desired_width(ui.available_width() - 220.0);

        let url_response = ui.add(url_edit);

        // Send on Enter
        if !is_loading && url_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
        {
            *on_send = true;
        }

        // Code gen button
        if ui.button("< >").on_hover_text("Generate Code").clicked() {
            *on_generate_code = true;
        }

        // Send button
        let send_btn = egui::Button::new(if is_loading {
            egui::RichText::new("⏳ Sending...")
        } else {
            egui::RichText::new("▶ Send").strong()
        });

        if ui.add_enabled(!is_loading, send_btn).clicked() {
            *on_send = true;
        }

        ui.add_space(16.0);
    });
}
