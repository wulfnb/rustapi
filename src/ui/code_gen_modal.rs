//! Code generation modal

use crate::http::{generate_code, CodeGenLanguage};
use crate::models::{ApiRequest, Environment};
use egui::Context;

/// Render code generation modal
pub fn render_code_gen_modal(
    ctx: &Context,
    show: &mut bool,
    request: &ApiRequest,
    environment: Option<&Environment>,
    selected_language: &mut CodeGenLanguage,
) {
    if !*show {
        return;
    }

    egui::Window::new("Generate Code")
        .collapsible(false)
        .resizable(true)
        .default_size([600.0, 500.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            // Language selector
            ui.horizontal(|ui| {
                ui.label("Language:");
                for lang in CodeGenLanguage::all() {
                    if ui
                        .selectable_label(*selected_language == *lang, lang.as_str())
                        .clicked()
                    {
                        *selected_language = *lang;
                    }
                }
            });

            ui.separator();
            ui.add_space(8.0);

            // Generated code
            let code = generate_code(request, *selected_language, environment);

            egui::ScrollArea::both().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut code.as_str())
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(20),
                );
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("📋 Copy to Clipboard").clicked() {
                    ui.output_mut(|o| o.copied_text = code.clone());
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        *show = false;
                    }
                });
            });
        });
}
