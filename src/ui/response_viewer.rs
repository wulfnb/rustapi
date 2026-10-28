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
}

/// Render the response viewer
pub fn render_response_viewer(
    ui: &mut Ui,
    response: Option<&ApiResponse>,
    is_loading: bool,
    current_tab: &mut ResponseTab,
    format_json_enabled: &mut bool,
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(format_json_enabled, "Format JSON");

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

                        ui.add(
                            egui::TextEdit::multiline(&mut body.as_str())
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });
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
