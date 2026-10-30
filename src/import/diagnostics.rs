//! Reports known omissions without copying imported names, values or response bodies.
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSeverity {
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportDisposition {
    Omitted,
    PartiallyPreserved,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportDiagnostic {
    /// JSON Pointer into the source document, using structural keys and indices only.
    pub path: String,
    pub feature: String,
    pub severity: ImportSeverity,
    pub disposition: ImportDisposition,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ImportReport<'a> {
    pub schema_version: u32,
    pub requires_confirmation: bool,
    pub diagnostics: &'a [ImportDiagnostic],
}

#[derive(Default)]
struct Scanner {
    diagnostics: Vec<ImportDiagnostic>,
}

#[derive(Clone, Copy)]
enum RowContext {
    Fields,
    Environment,
    ScopeVariables,
}

impl Scanner {
    fn add(&mut self, path: String, feature: &str, disposition: ImportDisposition, message: &str) {
        const MAX_DETAILS: usize = 256;
        if self.diagnostics.len() > MAX_DETAILS {
            return;
        }
        let (path, feature, disposition, message) = if self.diagnostics.len() == MAX_DETAILS {
            (
                String::new(),
                "report_limit",
                ImportDisposition::Unsupported,
                "Additional known omissions exist; the report is limited to 256 details.",
            )
        } else {
            (path, feature, disposition, message)
        };
        self.diagnostics.push(ImportDiagnostic {
            path,
            feature: feature.into(),
            severity: ImportSeverity::Warning,
            disposition,
            message: message.into(),
        });
    }
    fn omitted(&mut self, value: &Value, path: String, feature: &str, message: &str) {
        let present = match value {
            Value::Null => false,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
            _ => true,
        };
        if present {
            self.add(path, feature, ImportDisposition::Omitted, message);
        }
    }
    fn rows(&mut self, value: &Value, path: &str, context: RowContext) {
        if let Some(rows) = value.as_array() {
            for (i, row) in rows.iter().enumerate() {
                let base = format!("{path}/{i}");
                if !crate::models::description_supported(&row["description"]) {
                    self.omitted(
                        &row["description"],
                        format!("{base}/description"),
                        "row_description",
                        "Description content must be a string.",
                    );
                }
                if !matches!(context, RowContext::Fields) && !row["type"].is_string() {
                    self.omitted(
                        &row["type"],
                        format!("{base}/type"),
                        "variable_type",
                        "Variable type metadata must be a string.",
                    );
                }
            }
        }
    }
    fn settings_and_auth(&mut self, value: &Value, path: &str) {
        self.omitted(
            &value["protocolProfileBehavior"],
            format!("{path}/protocolProfileBehavior"),
            "request_settings",
            "Imported request behavior settings are not applied.",
        );
        if let Some(kind) = value["auth"]["type"].as_str() {
            if !["inherit", "noauth", "bearer", "basic", "apikey", "oauth2"].contains(&kind) {
                self.add(format!("{path}/auth"), "authentication", ImportDisposition::Unsupported,
                    "This authentication configuration is unsupported and must be configured locally.");
            }
        }
    }
    fn scope(&mut self, value: &Value, path: &str) {
        self.settings_and_auth(value, path);
        let variables = &value["variable"];
        self.rows(
            variables,
            &format!("{path}/variable"),
            RowContext::ScopeVariables,
        );
    }

    fn items(&mut self, items: &Value, path: &str) {
        if let Some(items) = items.as_array() {
            for (i, item) in items.iter().enumerate() {
                let base = format!("{path}/{i}");
                if !item["item"].is_array()
                    || !crate::models::description_supported(&item["description"])
                {
                    self.omitted(
                        &item["description"],
                        format!("{base}/description"),
                        "item_description",
                        "Description content must be a string; request-item descriptions must be stored inside request.",
                    );
                }
                if item["item"].is_array() {
                    self.scope(item, &base);
                    self.items(&item["item"], &format!("{base}/item"));
                    continue;
                }
                self.settings_and_auth(item, &base);
                self.omitted(
                    &item["response"],
                    format!("{base}/response"),
                    "saved_examples",
                    "Saved response examples are not retained.",
                );
                let request = &item["request"];
                let base = format!("{base}/request");
                self.settings_and_auth(request, &base);
                if !crate::models::description_supported(&request["description"]) {
                    self.omitted(
                        &request["description"],
                        format!("{base}/description"),
                        "request_description",
                        "Description content must be a string.",
                    );
                }
                self.rows(
                    &request["url"]["variable"],
                    &format!("{base}/url/variable"),
                    RowContext::Environment,
                );
                for (rows, suffix) in [
                    (&request["header"], "header"),
                    (&request["url"]["query"], "url/query"),
                    (&request["body"]["urlencoded"], "body/urlencoded"),
                    (&request["body"]["formdata"], "body/formdata"),
                ] {
                    self.rows(rows, &format!("{base}/{suffix}"), RowContext::Fields);
                }
                if let Some(parts) = request["body"]["formdata"].as_array() {
                    for (j, part) in parts.iter().enumerate() {
                        let base = format!("{base}/body/formdata/{j}");
                        self.omitted(
                            &part["contentType"],
                            format!("{base}/contentType"),
                            "part_content_type",
                            "Per-part content types are not retained.",
                        );
                        if part["type"] == "file"
                            && part["src"].as_array().is_some_and(|a| a.len() > 1)
                        {
                            self.add(
                                format!("{base}/src"),
                                "multiple_upload_files",
                                ImportDisposition::PartiallyPreserved,
                                "Only the first file in this upload field is retained.",
                            );
                        }
                    }
                }
            }
        }
    }
}

// Called only after the importer has validated collection structure and nesting.
pub(super) fn collection(value: &Value) -> Vec<ImportDiagnostic> {
    let mut scanner = Scanner::default();
    if !crate::models::description_supported(&value["info"]["description"]) {
        scanner.omitted(
            &value["info"]["description"],
            "/info/description".into(),
            "collection_description",
            "Description content must be a string.",
        );
    }
    scanner.scope(value, "");
    scanner.items(&value["item"], "/item");
    scanner.diagnostics
}

pub(super) fn environment(value: &Value) -> Vec<ImportDiagnostic> {
    let mut scanner = Scanner::default();
    scanner.rows(&value["values"], "/values", RowContext::Environment);
    scanner.diagnostics
}
