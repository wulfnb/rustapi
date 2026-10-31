//! Authentication editor UI

use crate::models::{ApiKeyLocation, AuthConfig, AuthType};
use egui::Ui;

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
        AuthType::Jwt => {
            ui.label("JWT algorithm");
            egui::ComboBox::from_id_salt("jwt_algorithm")
                .selected_text(&auth.jwt.algorithm)
                .show_ui(ui, |ui| {
                    for algorithm in [
                        "HS256", "HS384", "HS512", "RS256", "RS384", "RS512", "PS256", "PS384",
                        "PS512", "ES256", "ES384", "EdDSA",
                    ] {
                        ui.selectable_value(&mut auth.jwt.algorithm, algorithm.into(), algorithm);
                    }
                });
            ui.label("Signing secret or private PEM key ({{vault:name}} supported)");
            ui.add(
                egui::TextEdit::multiline(&mut auth.jwt.key)
                    .password(true)
                    .desired_rows(2),
            );
            ui.label("Key ID (optional)");
            ui.text_edit_singleline(&mut auth.jwt.key_id);
            ui.label("Claims JSON (variables supported)");
            ui.add(
                egui::TextEdit::multiline(&mut auth.jwt.claims)
                    .code_editor()
                    .desired_rows(6),
            );
            ui.small("Signed when sending. Set exp/iat explicitly in claims; no unsigned algorithm is supported.");
        }
        AuthType::AwsSigV4 => {
            for (label, field, secret) in [
                ("Access key", &mut auth.aws.access_key, true),
                ("Secret key", &mut auth.aws.secret_key, true),
                (
                    "Session token (optional)",
                    &mut auth.aws.session_token,
                    true,
                ),
                ("Region", &mut auth.aws.region, false),
                (
                    "Service (e.g. s3, execute-api)",
                    &mut auth.aws.service,
                    false,
                ),
            ] {
                ui.label(label);
                ui.add(egui::TextEdit::singleline(field).password(secret));
            }
            ui.small("Vault references are supported. Final encoded bytes are signed; cross-origin redirects are not followed.");
        }
        AuthType::Unsupported => {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                "Choose a supported authentication type before sending.",
            );
        }
        AuthType::Inherit => {
            ui.label("Use the nearest folder or collection authentication.");
        }
        AuthType::OAuth2 => {
            let c = &mut auth.oauth;
            ui.checkbox(
                &mut c.persist_tokens,
                "Keep tokens in the encrypted local vault",
            );
            if c.persist_tokens {
                ui.small("Unlock Local encrypted secrets before sending. Tokens can be inspected and forgotten there.");
            }
            egui::ComboBox::from_id_salt("oauth_grant")
                .selected_text(if c.grant == "authorization_code" {
                    "Authorization code + PKCE"
                } else {
                    "Client credentials"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut c.grant,
                        "client_credentials".into(),
                        "Client credentials",
                    );
                    ui.selectable_value(
                        &mut c.grant,
                        "authorization_code".into(),
                        "Authorization code + PKCE",
                    );
                });
            for (label, field) in [
                ("Authorization URL", &mut c.authorization_url),
                ("Token URL", &mut c.token_url),
                ("Client ID", &mut c.client_id),
                ("Scope", &mut c.scope),
            ] {
                ui.label(label);
                ui.text_edit_singleline(field);
            }
            egui::ComboBox::from_id_salt("oauth_client_auth")
                .selected_text(match c.client_auth {
                    crate::models::OAuthClientAuth::Body => "Client credentials in body",
                    crate::models::OAuthClientAuth::Basic => "HTTP Basic client authentication",
                    crate::models::OAuthClientAuth::Public => "Public client (no secret)",
                })
                .show_ui(ui, |ui| {
                    for (method, label) in [
                        (
                            crate::models::OAuthClientAuth::Body,
                            "Client credentials in body",
                        ),
                        (
                            crate::models::OAuthClientAuth::Basic,
                            "HTTP Basic client authentication",
                        ),
                        (
                            crate::models::OAuthClientAuth::Public,
                            "Public client (no secret)",
                        ),
                    ] {
                        ui.selectable_value(&mut c.client_auth, method, label);
                    }
                });
            ui.label("Refresh URL (empty uses Token URL)");
            ui.text_edit_singleline(&mut c.refresh_url);
            ui.label("Refresh scope (empty uses original grant scope)");
            ui.text_edit_singleline(&mut c.refresh_scope);
            egui::CollapsingHeader::new("OAuth provider parameters").show(ui,|ui|{
                ui.small("Managed state, PKCE and grant/client fields cannot be overridden. Duplicate resource parameters are retained.");
                for (id,label,rows) in [("authorization","Authorization parameters",&mut c.authorization_params),("token","Token parameters",&mut c.token_params),("refresh","Refresh parameters",&mut c.refresh_params)]{ui.label(label);ui.push_id(id,|ui|super::render_key_value_editor(ui,rows));}
            });
            ui.label("Client secret (or {{vault:name}} reference)");
            ui.add(egui::TextEdit::singleline(&mut c.client_secret).password(true));
            if c.grant == "authorization_code" {
                ui.label("Register a loopback redirect URI with your OAuth provider. The chosen URI is shown after authorization starts.");
                ui.text_edit_singleline(&mut c.redirect_uri);
                if ui.button("Authorize in browser (PKCE)").clicked() {
                    c.authorization_requested = true;
                }
            }
        }
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
                        .desired_width(ui.available_width() - 60.0),
                );
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("The token will be sent as: Authorization: Bearer <token>")
                    .small()
                    .weak(),
            );
        }
        AuthType::BasicAuth | AuthType::Digest => {
            ui.horizontal(|ui| {
                ui.label("Username:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.basic_username)
                        .hint_text("Username")
                        .desired_width(200.0),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Password:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.basic_password)
                        .hint_text("Password")
                        .password(true)
                        .desired_width(200.0),
                );
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new(if auth.auth_type == AuthType::Digest {
                "Digest negotiates the server challenge. MD5, SHA-256, SHA-512-256, sess and auth-int are supported."
            } else { "Credentials will be base64 encoded and sent as: Authorization: Basic <encoded>" }).small().weak());
        }
        AuthType::ApiKey => {
            ui.horizontal(|ui| {
                ui.label("Key Name:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.api_key_name)
                        .hint_text("e.g., X-API-Key")
                        .desired_width(200.0),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Value:");
                ui.add(
                    egui::TextEdit::singleline(&mut auth.api_key_value)
                        .hint_text("API key value")
                        .password(true)
                        .desired_width(200.0),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Add to:");
                egui::ComboBox::from_id_salt("api_key_location")
                    .selected_text(auth.api_key_location.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut auth.api_key_location,
                            ApiKeyLocation::Header,
                            "Header",
                        );
                        ui.selectable_value(
                            &mut auth.api_key_location,
                            ApiKeyLocation::QueryParam,
                            "Query Param",
                        );
                    });
            });
        }
    }
}
