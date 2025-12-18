//! Authentication editor UI

use egui::Ui;
use crate::models::{AuthConfig, AuthType, ApiKeyLocation};

/// Render authentication editor
pub fn render_auth_editor(ui: &mut Ui, auth: &mut AuthConfig) {
    ui.horizontal(|ui| {
        ui.label("Auth Type:");
        egui::ComboBox::from_id_salt("auth_type_selector")
            .selected_text(auth.auth_type.as_str())
            .show_ui(ui, |ui| {
                for auth_type in AuthType::all() {
                    ui.selectable_value(&mut auth.auth_type, auth_type.clone(), auth_type.as_str());
                }
            });
    });

    ui.add_space(8.0);

    match auth.auth_type {
        AuthType::None => {
            ui.label("No authentication configured.");
        }
        AuthType::BearerToken => {
            ui.horizontal(|ui| {
                ui.label("Token:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.bearer_token)
                        .hint_text("Enter bearer token...")
                        .password(true)
                        .desired_width(ui.available_width() - 60.0)
                );
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("The token will be sent as: Authorization: Bearer <token>").small().weak());
        }
        AuthType::BasicAuth => {
            ui.horizontal(|ui| {
                ui.label("Username:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.basic_username)
                        .hint_text("Username")
                        .desired_width(200.0)
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Password:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.basic_password)
                        .hint_text("Password")
                        .password(true)
                        .desired_width(200.0)
                );
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Credentials will be base64 encoded and sent as: Authorization: Basic <encoded>").small().weak());
        }
        AuthType::ApiKey => {
            ui.horizontal(|ui| {
                ui.label("Key Name:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.api_key_name)
                        .hint_text("e.g., X-API-Key")
                        .desired_width(200.0)
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Value:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.api_key_value)
                        .hint_text("API key value")
                        .password(true)
                        .desired_width(200.0)
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Add to:");
                egui::ComboBox::from_id_salt("api_key_location")
                    .selected_text(auth.api_key_location.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut auth.api_key_location, ApiKeyLocation::Header, "Header");
                        ui.selectable_value(&mut auth.api_key_location, ApiKeyLocation::QueryParam, "Query Param");
                    });
            });
        }
    }
}

