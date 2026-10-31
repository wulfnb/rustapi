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
    Description,
}

/// Render the request editor
pub fn render_request_editor(
    ui: &mut Ui,
    request: &mut ApiRequest,
    current_tab: &mut RequestTab,
    on_save: &mut bool,
    _has_save_target: bool,
) {
    ui.push_id(request.id.clone(), |ui| {
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
            (RequestTab::Description, "Description"),
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
            let before_query = serde_json::to_vec(&request.query_params).unwrap_or_default();
            ui.push_id("query_parameters", |ui| render_key_value_editor(ui, &mut request.query_params));
            if serde_json::to_vec(&request.query_params).unwrap_or_default() != before_query {
                crate::http::sync_query_params_to_url(&mut request.url, &request.query_params);
            }
            ui.separator();
            let vars = rustapi::variables::Variables {
                collection: request.inherited_variables.clone().into_iter().collect(),
                ..Default::default()
            };
            super::autocomplete::render_variable_palette(ui, &vars, &mut request.url);
            ui.separator();
            ui.label("Path variables (:name segments)");
            ui.push_id("path_parameters", |ui| render_key_value_editor(ui, &mut request.path_params));
        }
        RequestTab::Headers => {
            super::render_header_presets(ui,&mut request.headers);
            render_key_value_editor(ui, &mut request.headers);
        }
        RequestTab::Body => {
            let mut graphql=request.graphql.is_some();
            if ui.checkbox(&mut graphql,"GraphQL document").changed(){request.graphql=graphql.then(Default::default);if graphql{request.method=HttpMethod::POST;}}
            if let Some(graphql)=&mut request.graphql {
                ui.label("Query / mutation");ui.add(egui::TextEdit::multiline(&mut graphql.query).code_editor().desired_rows(10).desired_width(f32::INFINITY));
                ui.label("Variables JSON");ui.add(egui::TextEdit::multiline(&mut graphql.variables).code_editor().desired_rows(5).desired_width(f32::INFINITY));
                ui.label("Operation name (required for multiple operations)");ui.text_edit_singleline(&mut graphql.operation_name);
                ui.horizontal(|ui|{if ui.button("Format GraphQL").clicked(){if let Err(error)=graphql.format(){ui.label(error.to_string());}}
if ui.button("Use introspection query").clicked(){graphql.query=rustapi::graphql::INTROSPECTION.into();graphql.operation_name="RustAPIIntrospection".into();graphql.variables="{}".into();}});
                if let Err(error)=graphql.body(){ui.colored_label(egui::Color32::LIGHT_RED,error.to_string());}
                egui::CollapsingHeader::new("GraphQL schema / field catalog").show(ui,|ui|{
                    ui.small("Paste SDL or an introspection response, then inspect its type fields.");
                    let catalog_id=ui.id().with("graphql_catalog");
                    if ui.add(egui::TextEdit::multiline(&mut graphql.schema).code_editor().desired_rows(4)).changed(){ui.data_mut(|data|data.remove::<Result<Vec<String>,String>>(catalog_id));}
                    if ui.button("Inspect schema fields").clicked(){let result=graphql.fields().map_err(|error|error.to_string());ui.data_mut(|data|data.insert_temp(catalog_id,result));}
                    if let Some(result)=ui.data(|data|data.get_temp::<Result<Vec<String>,String>>(catalog_id)){match result{Ok(fields)=>{for field in fields.into_iter().take(200){ui.label(field);}},Err(error)=>{ui.label(error);}}}
                });
            }else{render_body_editor(ui, request);}
        }
        RequestTab::Auth => { render_auth_editor(ui, &mut request.auth); }
        RequestTab::Tests => {
            ui.label("All checks must pass. JSON targets use JSON Pointer, e.g. /user/id. JsonSchema Expected is a schema document; an empty target validates the whole JSON response. Schema values are literal JSON, without variable substitution.");
            let mut remove=None;
            for (i,c) in request.checks.iter_mut().enumerate() { ui.push_id(i,|ui| {
                ui.text_edit_singleline(&mut c.name);
                egui::ComboBox::from_id_salt("kind").selected_text(format!("{:?}",c.kind)).show_ui(ui,|ui|{for k in [crate::models::AssertionKind::Status,crate::models::AssertionKind::Header,crate::models::AssertionKind::Json,crate::models::AssertionKind::BodyContains,crate::models::AssertionKind::MaxTime,crate::models::AssertionKind::JsonSchema,crate::models::AssertionKind::OpenApi]{let label=format!("{k:?}");ui.selectable_value(&mut c.kind,k,label);}});
                ui.horizontal(|ui|{ui.label("Target");ui.text_edit_singleline(&mut c.target);ui.label("Expected");if matches!(c.kind,crate::models::AssertionKind::JsonSchema|crate::models::AssertionKind::OpenApi){ui.add(egui::TextEdit::multiline(&mut c.expected).code_editor().desired_rows(5));}else{ui.text_edit_singleline(&mut c.expected);}
 if ui.small_button("Remove").clicked(){remove=Some(i);}});
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
        RequestTab::Description => { ui.label("Request description (plain text or Markdown)"); ui.add(egui::TextEdit::multiline(&mut request.description).desired_rows(8).desired_width(f32::INFINITY)); }
        RequestTab::Settings => {
            let s=&mut request.settings;
            ui.horizontal(|ui|{ui.label("Timeout (ms)");ui.add(egui::DragValue::new(&mut s.timeout_ms).range(1..=3_600_000));});
            ui.checkbox(&mut s.follow_redirects,"Follow redirects");ui.checkbox(&mut s.verify_tls,"Verify TLS certificates");
            ui.checkbox(&mut s.use_cookies,"Use workspace cookie jar");
            if !s.verify_tls{ui.colored_label(egui::Color32::YELLOW,"TLS verification disabled for this request");}
            ui.label("Proxy URL (empty uses system configuration)");ui.text_edit_singleline(&mut s.proxy);
            ui.label("Custom CA PEM file");ui.text_edit_singleline(&mut s.ca_file);
            ui.label("Client certificate + private key PEM file");ui.text_edit_singleline(&mut s.identity_file);
            ui.horizontal(|ui|{ui.label("Maximum response bytes");ui.add(egui::DragValue::new(&mut s.max_response_bytes).range(1..=256*1024*1024));});
        }
    });
    });
}

/// Render key-value editor
pub fn render_key_value_editor(ui: &mut Ui, items: &mut Vec<KeyValue>) {
    super::render_bulk_rows(ui, items);
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
        ui.push_id(idx, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut item.enabled, "");

                let key_edit = egui::TextEdit::singleline(&mut item.key)
                    .hint_text("Key")
                    .desired_width(180.0);
                let key_response = ui.add(key_edit);
                let suggestions = crate::ui::autocomplete::suggest_headers(&item.key);
                if !suggestions.is_empty()
                    && !item.key.is_empty()
                    && !suggestions.contains(&item.key.as_str())
                {
                    key_response.on_hover_ui(|ui| {
                        ui.label(egui::RichText::new("Header suggestions:").small().strong());
                        for s in suggestions.iter().take(5) {
                            ui.monospace(*s);
                        }
                    });
                }

                let value_edit = egui::TextEdit::singleline(&mut item.value)
                    .hint_text("Value")
                    .desired_width(ui.available_width() - 40.0);
                let value_response = ui.add(value_edit);
                let detected = crate::ui::autocomplete::extract_variables(&item.value);
                if !detected.is_empty() {
                    value_response.on_hover_ui(|ui| {
                        ui.label(
                            egui::RichText::new("Variables referenced:")
                                .small()
                                .strong(),
                        );
                        for var_name in &detected {
                            ui.monospace(format!("{{{{{var_name}}}}}"));
                        }
                    });
                }

                if ui.small_button("🗑").clicked() {
                    to_delete = Some(idx);
                }
            });
            egui::CollapsingHeader::new("Row description").show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut item.description)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
            });
        });
    }

    // Delete if needed
    if let Some(idx) = to_delete {
        items.remove(idx);
    }

    ui.add_space(8.0);

    if ui.button("➕ Add").clicked() {
        items.push(KeyValue::new("", ""));
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
        BodyType::Json | BodyType::Text | BodyType::Xml | BodyType::Html => {
            ui.horizontal(|ui| {
                if request.body_type == BodyType::Json && ui.button("✨ Format JSON").clicked() {
                    request.body = crate::http::format_json(&request.body);
                } else if request.body_type == BodyType::Xml && ui.button("✨ Format XML").clicked()
                {
                    request.body = crate::http::format_xml(&request.body);
                }
            });
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
                        method.clone(),
                        egui::RichText::new(method.as_str()).color(method.color()),
                    );
                }
                ui.separator();
                let is_custom = matches!(request.method, HttpMethod::Custom(_));
                if ui.selectable_label(is_custom, "Custom...").clicked() && !is_custom {
                    request.method = HttpMethod::Custom("CUSTOM".into());
                }
            });

        if let HttpMethod::Custom(ref mut custom_name) = request.method {
            ui.add(
                egui::TextEdit::singleline(custom_name)
                    .hint_text("METHOD")
                    .desired_width(70.0)
                    .font(egui::TextStyle::Monospace),
            );
        }

        // URL input
        let url_width = if matches!(request.method, HttpMethod::Custom(_)) {
            ui.available_width() - 300.0
        } else {
            ui.available_width() - 220.0
        };
        let url_edit = egui::TextEdit::singleline(&mut request.url)
            .hint_text("Enter URL...")
            .font(egui::TextStyle::Monospace)
            .desired_width(url_width);

        let url_response = ui.add(url_edit);
        if url_response.changed() {
            crate::http::sync_url_to_query_params(&request.url, &mut request.query_params);
        }

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
