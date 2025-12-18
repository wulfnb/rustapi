//! Request history panel

use egui::Context;
use crate::models::{RequestHistory, HistoryEntry, ApiRequest};

/// Action from history panel
#[derive(Debug, Clone)]
pub enum HistoryAction {
    LoadRequest(ApiRequest),
    ClearHistory,
}

/// Render history panel
pub fn render_history_panel(
    ctx: &Context,
    show: &mut bool,
    history: &RequestHistory,
) -> Option<HistoryAction> {
    if !*show {
        return None;
    }

    let mut action: Option<HistoryAction> = None;

    egui::Window::new("Request History")
        .collapsible(false)
        .resizable(true)
        .default_size([500.0, 400.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} requests in history", history.entries.len()));
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🗑 Clear All").clicked() {
                        action = Some(HistoryAction::ClearHistory);
                    }
                });
            });

            ui.separator();

            if history.entries.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("No requests in history");
                });
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in &history.entries {
                        ui.horizontal(|ui| {
                            // Method badge
                            let method_color = entry.request.method.color();
                            ui.label(egui::RichText::new(entry.request.method.as_str())
                                .color(method_color)
                                .strong()
                                .size(10.0));

                            // Status badge
                            if let Some(ref resp) = entry.response {
                                let status_color = resp.status_color();
                                ui.label(egui::RichText::new(format!("{}", resp.status))
                                    .color(status_color)
                                    .size(10.0));
                            } else {
                                ui.label(egui::RichText::new("ERR")
                                    .color(egui::Color32::RED)
                                    .size(10.0));
                            }

                            // URL (truncated)
                            let url_display = if entry.request.url.len() > 50 {
                                format!("{}...", &entry.request.url[..47])
                            } else {
                                entry.request.url.clone()
                            };
                            
                            if ui.link(&url_display).clicked() {
                                action = Some(HistoryAction::LoadRequest(entry.request.clone()));
                            }

                            // Timestamp
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(
                                    entry.timestamp.format("%H:%M:%S").to_string()
                                ).weak().size(10.0));
                            });
                        });
                        ui.add_space(2.0);
                    }
                });
            }

            ui.add_space(8.0);
            
            if ui.button("Close").clicked() {
                *show = false;
            }
        });

    action
}

