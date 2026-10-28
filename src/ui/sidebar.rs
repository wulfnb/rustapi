//! Sidebar UI component with collections tree

use crate::models::Collection;
use egui::Ui;

/// Actions that can be triggered from the sidebar
#[derive(Debug, Clone)]
pub enum SidebarAction {
    SelectRequest(usize, usize),
    AddRequest(usize),
    DeleteRequest(usize, usize),
    DuplicateRequest(usize, usize),
    RenameRequest(usize, usize),
    DeleteCollection(usize),
    RenameCollection(usize),
    ExportCollection(usize),
    NewCollection,
}

/// Render the sidebar with collections
pub fn render_sidebar(
    ui: &mut Ui,
    collections: &mut [Collection],
    search_query: &mut String,
    current_collection_idx: Option<usize>,
    current_request_idx: Option<usize>,
) -> Option<SidebarAction> {
    let mut action: Option<SidebarAction> = None;

    ui.add_space(8.0);

    // Search box
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let search_edit = egui::TextEdit::singleline(search_query)
            .hint_text("🔍 Search requests...")
            .desired_width(ui.available_width() - 16.0);
        ui.add(search_edit);
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Collections header
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.strong("Collections");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            if ui
                .small_button("➕")
                .on_hover_text("New Collection")
                .clicked()
            {
                action = Some(SidebarAction::NewCollection);
            }
        });
    });

    ui.add_space(8.0);

    // Collections list
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (col_idx, collection) in collections.iter_mut().enumerate() {
            ui.add_space(4.0);

            // Collection header
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let icon = if collection.expanded { "▼" } else { "▶" };
                if ui.small_button(icon).clicked() {
                    collection.expanded = !collection.expanded;
                }

                let col_label = ui.selectable_label(false, &collection.name);
                if col_label.clicked() {
                    collection.expanded = !collection.expanded;
                }

                col_label.context_menu(|ui| {
                    if ui.button("➕ Add Request").clicked() {
                        action = Some(SidebarAction::AddRequest(col_idx));
                        ui.close_menu();
                    }
                    if ui.button("✏ Rename").clicked() {
                        action = Some(SidebarAction::RenameCollection(col_idx));
                        ui.close_menu();
                    }
                    if ui.button("📤 Export").clicked() {
                        action = Some(SidebarAction::ExportCollection(col_idx));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🗑 Delete").clicked() {
                        action = Some(SidebarAction::DeleteCollection(col_idx));
                        ui.close_menu();
                    }
                });
            });

            // Requests in collection
            if collection.expanded {
                for (req_idx, request) in collection.requests.iter().enumerate() {
                    // Filter by search query
                    if !search_query.is_empty()
                        && !request
                            .name
                            .to_lowercase()
                            .contains(&search_query.to_lowercase())
                        && !request
                            .url
                            .to_lowercase()
                            .contains(&search_query.to_lowercase())
                    {
                        continue;
                    }

                    let is_selected = current_collection_idx == Some(col_idx)
                        && current_request_idx == Some(req_idx);

                    ui.horizontal(|ui| {
                        ui.add_space(32.0);

                        // Method badge
                        let method_color = request.method.color();
                        ui.label(
                            egui::RichText::new(request.method.as_str())
                                .color(method_color)
                                .size(10.0)
                                .strong(),
                        );

                        let req_label = ui.selectable_label(is_selected, &request.name);
                        if req_label.clicked() {
                            action = Some(SidebarAction::SelectRequest(col_idx, req_idx));
                        }

                        req_label.context_menu(|ui| {
                            if ui.button("✏ Rename").clicked() {
                                action = Some(SidebarAction::RenameRequest(col_idx, req_idx));
                                ui.close_menu();
                            }
                            if ui.button("📋 Duplicate").clicked() {
                                action = Some(SidebarAction::DuplicateRequest(col_idx, req_idx));
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("🗑 Delete").clicked() {
                                action = Some(SidebarAction::DeleteRequest(col_idx, req_idx));
                                ui.close_menu();
                            }
                        });
                    });
                }
            }
        }
    });

    action
}
