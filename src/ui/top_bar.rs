//! Top bar UI component

use egui::Ui;
use crate::models::Environment;

/// Render the top bar with environment selector
pub fn render_top_bar(
    ui: &mut Ui,
    environments: &[Environment],
    active_environment_id: &mut Option<String>,
    on_manage_environments: &mut bool,
    on_import: &mut bool,
    on_show_history: &mut bool,
) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        
        // Logo/Title
        ui.heading(egui::RichText::new("🦀 RustAPI").strong().size(20.0));
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(20.0);

        // Environment selector
        ui.label("Environment:");
        egui::ComboBox::from_id_salt("env_selector")
            .selected_text(
                active_environment_id
                    .as_ref()
                    .and_then(|id| environments.iter().find(|e| &e.id == id))
                    .map(|e| e.name.as_str())
                    .unwrap_or("No Environment")
            )
            .show_ui(ui, |ui| {
                if ui.selectable_label(active_environment_id.is_none(), "No Environment").clicked() {
                    *active_environment_id = None;
                }
                for env in environments {
                    if ui.selectable_label(
                        active_environment_id.as_ref() == Some(&env.id),
                        &env.name
                    ).clicked() {
                        *active_environment_id = Some(env.id.clone());
                    }
                }
            });

        if ui.button("⚙").on_hover_text("Manage Environments").clicked() {
            *on_manage_environments = true;
        }

        ui.add_space(10.0);
        
        if ui.button("📜 History").on_hover_text("Request History").clicked() {
            *on_show_history = true;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("📁 Import").clicked() {
                *on_import = true;
            }
        });
    });
    ui.add_space(8.0);
}

