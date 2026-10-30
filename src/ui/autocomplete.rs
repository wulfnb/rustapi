//! Autocomplete and variable completion helpers
use rustapi::variables::Variables;

pub const STANDARD_HEADERS: &[&str] = &[
    "Accept",
    "Accept-Charset",
    "Accept-Encoding",
    "Accept-Language",
    "Authorization",
    "Cache-Control",
    "Connection",
    "Content-Disposition",
    "Content-Encoding",
    "Content-Length",
    "Content-Type",
    "Cookie",
    "Date",
    "ETag",
    "Expect",
    "Forwarded",
    "From",
    "Host",
    "If-Match",
    "If-Modified-Since",
    "If-None-Match",
    "If-Range",
    "If-Unmodified-Since",
    "Origin",
    "Pragma",
    "Proxy-Authorization",
    "Range",
    "Referer",
    "Server",
    "Set-Cookie",
    "TE",
    "Trailer",
    "Transfer-Encoding",
    "Upgrade",
    "User-Agent",
    "Via",
    "Warning",
    "X-API-Key",
    "X-Forwarded-For",
    "X-Forwarded-Host",
    "X-Forwarded-Proto",
    "X-Requested-With",
];

pub const DYNAMIC_VARIABLES: &[&str] = &[
    "$guid",
    "$uuid",
    "$timestamp",
    "$isoTimestamp",
    "$randomInt",
    "$randomEmail",
    "$randomAlphaNumeric",
    "$randomUserName",
    "$randomBoolean",
];

/// Returns header suggestions that match the given prefix
pub fn suggest_headers(prefix: &str) -> Vec<&'static str> {
    if prefix.trim().is_empty() {
        return STANDARD_HEADERS.iter().take(8).copied().collect();
    }
    let lower = prefix.trim().to_ascii_lowercase();
    STANDARD_HEADERS
        .iter()
        .filter(|h| h.to_ascii_lowercase().starts_with(&lower))
        .copied()
        .collect()
}

/// Suggest variables matching the given prefix or all available variables
pub fn suggest_variables(prefix: &str, vars: &Variables) -> Vec<(String, &'static str)> {
    let lower = prefix.trim().trim_start_matches('{').to_ascii_lowercase();
    let mut results = Vec::new();

    for key in vars.secrets.keys() {
        let name = format!("vault:{key}");
        if lower.is_empty() || name.to_ascii_lowercase().contains(&lower) {
            results.push((name, "Vault"));
        }
    }
    for key in vars.local.keys() {
        if lower.is_empty() || key.to_ascii_lowercase().contains(&lower) {
            results.push((key.clone(), "Local"));
        }
    }
    for key in vars.data.keys() {
        if lower.is_empty() || key.to_ascii_lowercase().contains(&lower) {
            results.push((key.clone(), "Iteration"));
        }
    }
    for key in vars.local_overrides.keys() {
        if lower.is_empty() || key.to_ascii_lowercase().contains(&lower) {
            results.push((key.clone(), "Environment (local override)"));
        }
    }
    for key in vars.environment.keys() {
        if lower.is_empty() || key.to_ascii_lowercase().contains(&lower) {
            results.push((key.clone(), "Environment"));
        }
    }
    for key in vars.collection.keys() {
        if lower.is_empty() || key.to_ascii_lowercase().contains(&lower) {
            results.push((key.clone(), "Collection/Folder"));
        }
    }
    for key in vars.global.keys() {
        if lower.is_empty() || key.to_ascii_lowercase().contains(&lower) {
            results.push((key.clone(), "Global"));
        }
    }
    for &key in DYNAMIC_VARIABLES {
        if lower.is_empty() || key.to_ascii_lowercase().contains(&lower) {
            results.push((key.to_string(), "Dynamic"));
        }
    }
    results
}

/// Find variables referenced in text as `{{name}}`
pub fn extract_variables(text: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("}}") {
            let key = after[..end].trim();
            if !key.is_empty() && !vars.iter().any(|v| v == key) {
                vars.push(key.to_string());
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    vars
}

/// Render a compact variable picker palette that shows available variables and dynamic generators
pub fn render_variable_palette(ui: &mut egui::Ui, vars: &Variables, target: &mut String) {
    egui::CollapsingHeader::new("Variable Palette ({{...}})").show(ui, |ui| {
        let suggestions = suggest_variables("", vars);
        ui.horizontal_wrapped(|ui| {
            for (name, scope) in suggestions.iter().take(20) {
                let label = egui::RichText::new(format!("{{{{{}}}}}", name))
                    .monospace()
                    .small();
                if ui
                    .button(label)
                    .on_hover_text(format!("Scope: {scope}"))
                    .clicked()
                {
                    target.push_str(&format!("{{{{{}}}}}", name));
                }
            }
        });
    });
}
