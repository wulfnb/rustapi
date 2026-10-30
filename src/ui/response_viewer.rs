//! Response viewer UI component

use crate::http::format_json;
use crate::models::ApiResponse;
use egui::Ui;

/// Response viewer tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseTab {
    #[default]
    Body,
    Headers,
    Tree,
    Raw,
    Preview,
    Timing,
}

/// Render the response viewer
pub fn render_response_viewer(
    ui: &mut Ui,
    response: Option<&ApiResponse>,
    is_loading: bool,
    current_tab: &mut ResponseTab,
    format_json_enabled: &mut bool,
    search: &mut String,
) {
    ui.horizontal(|ui| {
        ui.strong("Response");

        if let Some(response) = response {
            ui.separator();

            // Status badge
            let status_text = format!("{} {}", response.status, response.status_text);
            ui.label(
                egui::RichText::new(status_text)
                    .color(response.status_color())
                    .strong(),
            );

            ui.separator();
            ui.label(format!("{}ms", response.time_ms));

            ui.separator();
            ui.label(format_size(response.size));
        }
    });

    ui.separator();

    if let Some(response) = response {
        if let Some(ref error) = response.error {
            ui.colored_label(egui::Color32::RED, error);
        } else {
            ui.horizontal(|ui| {
                ui.label("Search response");
                ui.text_edit_singleline(search);
                if !search.is_empty() {
                    ui.label(format!(
                        "{} matches",
                        response.body.matches(search.as_str()).count()
                    ));
                }
            });
            if response.size > response.body.len() {
                ui.label("Showing a bounded text preview. Download saves the complete response.");
            }
            for test in &response.tests {
                ui.colored_label(
                    if test.passed {
                        egui::Color32::LIGHT_GREEN
                    } else {
                        egui::Color32::LIGHT_RED
                    },
                    format!(
                        "{} {} {}",
                        if test.passed { "PASS" } else { "FAIL" },
                        test.name,
                        test.message
                    ),
                );
            }
            // Response tabs
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(*current_tab == ResponseTab::Body, "Body")
                    .clicked()
                {
                    *current_tab = ResponseTab::Body;
                }
                if ui
                    .selectable_label(*current_tab == ResponseTab::Headers, "Headers")
                    .clicked()
                {
                    *current_tab = ResponseTab::Headers;
                }

                for (tab, label) in [
                    (ResponseTab::Tree, "JSON tree"),
                    (ResponseTab::Raw, "Raw"),
                    (ResponseTab::Preview, "Preview"),
                    (ResponseTab::Timing, "Timing"),
                ] {
                    ui.selectable_value(current_tab, tab, label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(format_json_enabled, "Format JSON");
                    if ui.button("Download").clicked() {
                        if let Some(path) = rfd::FileDialog::new().save_file() {
                            let bytes = if response.raw_body.is_empty() {
                                response.body.as_bytes()
                            } else {
                                &response.raw_body
                            };
                            if let Err(e) = rustapi::workspace::atomic_write(&path, bytes) {
                                ui.label(format!("Save failed: {e}"));
                            }
                        }
                    }

                    if ui.button("📋 Copy").clicked() {
                        ui.output_mut(|o| o.copied_text = response.body.clone());
                    }
                });
            });

            ui.add_space(4.0);

            match current_tab {
                ResponseTab::Body => {
                    egui::ScrollArea::both().show(ui, |ui| {
                        let body = if *format_json_enabled {
                            format_json(&response.body)
                        } else {
                            response.body.clone()
                        };

                        let filtered = if search.is_empty() {
                            body
                        } else {
                            body.lines()
                                .filter(|l| l.contains(search.as_str()))
                                .collect::<Vec<_>>()
                                .join("\n")
                        };
                        super::render_response_syntax(
                            ui,
                            &filtered,
                            if serde_json::from_str::<serde_json::Value>(&response.body).is_ok() {
                                "json"
                            } else {
                                "txt"
                            },
                        );
                    });
                }
                ResponseTab::Tree => {
                    egui::ScrollArea::both()
                        .show(ui, |ui| super::render_response_tree(ui, response));
                }
                ResponseTab::Raw => {
                    egui::ScrollArea::both().show(ui,|ui|{
                    ui.small("Raw text is the bounded UTF-8 preview; hex shows up to 4096 original bytes. Download saves all bytes.");
                    ui.add(egui::TextEdit::multiline(&mut response.body.as_str()).code_editor().desired_width(f32::INFINITY));
                    let bytes=if response.raw_body.is_empty(){response.body.as_bytes()}else{&response.raw_body};
                    ui.monospace(super::response_hex(bytes));
                });
                }
                ResponseTab::Preview => {
                    egui::ScrollArea::both()
                        .show(ui, |ui| super::render_response_preview(ui, response));
                }
                ResponseTab::Timing => {
                    ui.label(format!("Response headers: {} ms", response.headers_time_ms));
                    ui.label(format!(
                        "Body transfer: {} ms",
                        response.time_ms.saturating_sub(response.headers_time_ms)
                    ));
                    ui.label(format!("Total: {} ms", response.time_ms));
                    ui.small("Header timing includes connection and server wait. DNS/connect/TLS breakdown is unavailable.");
                }
                ResponseTab::Headers => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("response_headers_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                for (key, value) in &response.headers {
                                    ui.label(egui::RichText::new(key).strong());
                                    ui.label(value);
                                    ui.end_row();
                                }
                            });
                    });
                }
            }
        }
    } else if is_loading {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
    } else {
        ui.centered_and_justified(|ui| {
            ui.label("Send a request to see the response");
        });
    }
}

/// Format file size
fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
