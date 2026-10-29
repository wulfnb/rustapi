//! Modal dialogs

use crate::models::{Collection, Environment, KeyValue};
use crate::storage;
use egui::Context;

/// What we're renaming
#[derive(Debug, Clone, Default)]
pub enum RenameTarget {
    #[default]
    None,
    Collection(usize),
    Request(usize, usize),
}

/// Render new collection modal
pub fn render_new_collection_modal(
    ctx: &Context,
    show: &mut bool,
    new_name: &mut String,
) -> Option<String> {
    let mut result: Option<String> = None;

    if *show {
        egui::Window::new("New Collection")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(new_name);
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !new_name.is_empty() {
                        result = Some(new_name.clone());
                        *show = false;
                        new_name.clear();
                    }
                    if ui.button("Cancel").clicked() {
                        *show = false;
                        new_name.clear();
                    }
                });
            });
    }

    result
}

/// Render rename modal
pub fn render_rename_modal(
    ctx: &Context,
    show: &mut bool,
    target: &RenameTarget,
    rename_value: &mut String,
    collections: &mut [Collection],
    current_request_name: Option<&mut String>,
) {
    if !*show {
        return;
    }

    let (title, col_idx, req_idx) = match target {
        RenameTarget::Collection(idx) => ("Rename Collection", *idx, None),
        RenameTarget::Request(col, req) => ("Rename Request", *col, Some(*req)),
        RenameTarget::None => return,
    };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(rename_value);
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("Save").clicked() && !rename_value.is_empty() {
                    if let Some(collection) = collections.get_mut(col_idx) {
                        if let Some(req_idx) = req_idx {
                            if let Some(request) = collection.requests.get_mut(req_idx) {
                                request.name = rename_value.clone();
                                if let Some(current_name) = current_request_name {
                                    *current_name = rename_value.clone();
                                }
                                let _ = storage::save_collection(collection, None);
                            }
                        } else {
                            collection.name = rename_value.clone();
                            let _ = storage::save_collection(collection, None);
                        }
                    }
                    *show = false;
                    rename_value.clear();
                }
                if ui.button("Cancel").clicked() {
                    *show = false;
                    rename_value.clear();
                }
            });
        });
}

/// Render environment editor modal
pub fn render_environment_editor(
    ctx: &Context,
    show: &mut bool,
    environments: &mut Vec<Environment>,
    active_environment_id: &mut Option<String>,
) {
    if !*show {
        return;
    }

    egui::Window::new("Manage Environments")
        .collapsible(false)
        .resizable(true)
        .default_size([550.0, 450.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("➕ New Environment").clicked() {
                    let env = Environment::new(format!("Environment {}", environments.len() + 1));
                    let _ = storage::save_environment(&env);
                    environments.push(env);
                }
            });

            ui.separator();

            let mut env_to_delete: Option<usize> = None;
            let mut save_env_indices: Vec<usize> = vec![];

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (env_idx, env) in environments.iter_mut().enumerate() {
                    // Use stable ID based on environment id
                    let header_id = egui::Id::new(&env.id);
                    egui::CollapsingHeader::new(&env.name)
                        .id_salt(header_id)
                        .default_open(false)
                        .show(ui, |ui| {
                            // Name edit
                            ui.horizontal(|ui| {
                                ui.label("Name:");
                                let response = ui.text_edit_singleline(&mut env.name);
                                if response.lost_focus() {
                                    save_env_indices.push(env_idx);
                                }
                            });

                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("Variables:").strong());

                            // Variables editor
                            let mut var_to_delete: Option<usize> = None;
                            for (var_idx, var) in env.variables.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut var.enabled, "").changed() {
                                        save_env_indices.push(env_idx);
                                    }
                                    let key_resp = ui.add(
                                        egui::TextEdit::singleline(&mut var.key)
                                            .hint_text("Variable name")
                                            .desired_width(150.0),
                                    );
                                    let val_resp = ui.add(
                                        egui::TextEdit::singleline(&mut var.value)
                                            .hint_text("Value")
                                            .desired_width(200.0),
                                    );
                                    if key_resp.lost_focus() || val_resp.lost_focus() {
                                        save_env_indices.push(env_idx);
                                    }
                                    if ui.small_button("🗑").clicked() {
                                        var_to_delete = Some(var_idx);
                                    }
                                });
                            }

                            if let Some(idx) = var_to_delete {
                                env.variables.remove(idx);
                                save_env_indices.push(env_idx);
                            }

                            ui.add_space(4.0);

                            if ui.button("➕ Add Variable").clicked() {
                                env.variables.push(KeyValue::default());
                            }

                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                if ui.button("💾 Save").clicked() {
                                    let _ = storage::save_environment(env);
                                }
                                if ui.button("🗑 Delete Environment").clicked() {
                                    env_to_delete = Some(env_idx);
                                }
                            });
                        });
                }
            });

            // Save environments that need saving
            for idx in save_env_indices
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
            {
                if let Some(env) = environments.get(idx) {
                    let _ = storage::save_environment(env);
                }
            }

            if let Some(idx) = env_to_delete {
                let env = environments.remove(idx);
                let _ = storage::delete_environment(&env);
                if active_environment_id.as_ref() == Some(&env.id) {
                    *active_environment_id = None;
                }
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("Save All").clicked() {
                    for env in environments.iter() {
                        let _ = storage::save_environment(env);
                    }
                }
                if ui.button("Close").clicked() {
                    // Save all before closing
                    for env in environments.iter() {
                        let _ = storage::save_environment(env);
                    }
                    *show = false;
                }
            });
        });
}
