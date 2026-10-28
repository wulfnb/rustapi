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
}

/// Render the request editor
pub fn render_request_editor(
    ui: &mut Ui,
    request: &mut ApiRequest,
    current_tab: &mut RequestTab,
    on_save: &mut bool,
    has_save_target: bool,
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

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if has_save_target && ui.button("💾 Save").clicked() {
                *on_save = true;
            }
        });
    });

    ui.separator();
    ui.add_space(8.0);

    // Tab content
    egui::ScrollArea::vertical().show(ui, |ui| match current_tab {
        RequestTab::Params => {
            render_key_value_editor(ui, &mut request.query_params);
        }
        RequestTab::Headers => {
            render_key_value_editor(ui, &mut request.headers);
        }
        RequestTab::Body => {
            render_body_editor(ui, request);
        }
        RequestTab::Auth => {
            render_auth_editor(ui, &mut request.auth);
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
        }
        BodyType::Binary => {
            ui.label("Binary body - select a file to upload.");
            if ui.button("📁 Select File").clicked() {
                // TODO: Implement file selection
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
        if url_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
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
