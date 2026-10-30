use rustapi::{models::*, variables::Variables};
use std::collections::{BTreeMap, BTreeSet};
pub fn render_variable_inspector(
    ui: &mut egui::Ui,
    variables: &Variables,
    mut origins: BTreeMap<String, (String, bool)>,
    environment: Option<&Environment>,
    reveal: bool,
) {
    if let Some(environment) = environment {
        for row in environment.variables.iter().filter(|row| row.enabled) {
            origins.insert(
                row.key.clone(),
                (
                    format!("Environment: {}", environment.name),
                    row.value_type.eq_ignore_ascii_case("secret"),
                ),
            );
        }
    }
    for (scope, values) in [("Iteration", &variables.data), ("Local", &variables.local)] {
        for key in values.keys() {
            origins.insert(key.clone(), (scope.into(), false));
        }
    }
    for key in variables.secrets.keys() {
        origins.insert(format!("vault:{key}"), ("Vault".into(), true));
    }
    let merged = variables.merged();
    let mut secret: BTreeSet<String> = origins
        .iter()
        .filter(|(key, (_, typed))| *typed || rustapi::secrets::sensitive(key))
        .map(|(key, _)| key.clone())
        .collect();
    secret.extend(
        merged
            .keys()
            .filter(|key| rustapi::secrets::sensitive(key))
            .cloned(),
    );
    for _ in 0..16 {
        let next: Vec<_> = merged
            .iter()
            .filter(|(_, value)| {
                let mut rest = value.as_str();
                while let Some(start) = rest.find("{{") {
                    let after = &rest[start + 2..];
                    let Some(end) = after.find("}}") else {
                        break;
                    };
                    if secret.contains(after[..end].trim()) {
                        return true;
                    }
                    rest = &after[end + 2..];
                }
                false
            })
            .map(|(key, _)| key.clone())
            .collect();
        let size = secret.len();
        secret.extend(next);
        if size == secret.len() {
            break;
        }
    }
    egui::Grid::new("variable_inspector")
        .striped(true)
        .show(ui, |ui| {
            for label in ["Variable", "Winning scope", "Raw value", "Resolved value"] {
                ui.strong(label);
            }
            ui.end_row();
            for (key, value) in merged {
                ui.monospace(&key);
                ui.label(
                    origins
                        .get(&key)
                        .map(|(origin, _)| origin.as_str())
                        .unwrap_or("Collection/folder context"),
                );
                let masked = !reveal && (secret.contains(&key) || value.contains("{{vault:"));
                ui.monospace(if masked { "[masked]" } else { &value });
                match variables.resolve(&value) {
                    Ok(resolved) => {
                        ui.monospace(if masked { "[masked]" } else { &resolved });
                    }
                    Err(error) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
                    }
                }
                ui.end_row();
            }
        });
}
