//! Offline OpenAPI 3.0/3.1 import, linting, response contracts and semantic drift.
use crate::models::*;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
const METHODS: &[&str] = &[
    "get", "post", "put", "patch", "delete", "head", "options", "trace",
];
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Issue {
    pub severity: String,
    pub path: String,
    pub message: String,
}
#[derive(Clone)]
pub struct Specification {
    pub value: Value,
}
fn pointer(text: &str) -> String {
    text.replace('~', "~0").replace('/', "~1")
}
fn message(issues: &mut Vec<Issue>, severity: &str, path: &str, message: &str) {
    if issues.len() < 256 {
        issues.push(Issue {
            severity: severity.into(),
            path: path.into(),
            message: message.into(),
        });
    }
}
fn sample(schema: &Value) -> Value {
    for key in ["example", "default", "const"] {
        if !schema[key].is_null() {
            return schema[key].clone();
        }
    }
    if let Some(value) = schema["enum"].as_array().and_then(|v| v.first()) {
        return value.clone();
    }
    match schema["type"].as_str().unwrap_or("string") {
        "integer" | "number" => json!(1),
        "boolean" => json!(true),
        "array" => json!([]),
        "object" => json!({}),
        _ => json!("value"),
    }
}
fn scalar(value: &Value) -> String {
    value
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| value.to_string())
}
impl Specification {
    pub fn parse(text: &str) -> Result<Self> {
        ensure!(text.len() <= 8 * 1024 * 1024, "Specification exceeds 8 MiB");
        let value: Value = if text.trim_start().starts_with('{') {
            serde_json::from_str(text)?
        } else {
            crate::v3::parse_yaml(text)?
        };
        ensure!(value.is_object(), "OpenAPI specification must be an object");
        ensure!(
            value["openapi"]
                .as_str()
                .is_some_and(|v| v.starts_with("3.0.") || v.starts_with("3.1.")),
            "Supported OpenAPI versions: 3.0 and 3.1"
        );
        Ok(Self { value })
    }
    /// Resolve only document-local references, with explicit recursion/expansion bounds.
    pub fn resolve(&self, value: &Value) -> Result<Value> {
        fn walk(
            root: &Value,
            value: &Value,
            depth: usize,
            left: &mut usize,
            stack: &mut Vec<String>,
        ) -> Result<Value> {
            ensure!(
                depth <= 40 && *left > 0,
                "OpenAPI reference expansion exceeds its budget"
            );
            *left -= 1;
            match value {
                Value::Object(map) => {
                    let mut output = serde_json::Map::new();
                    if let Some(reference) = map.get("$ref") {
                        let reference = reference.as_str().context("Invalid OpenAPI reference")?;
                        ensure!(
                            reference.starts_with("#/"),
                            "Only document-local OpenAPI references are supported"
                        );
                        ensure!(
                            !stack.iter().any(|r| r == reference),
                            "Recursive OpenAPI references need a bounded example/schema"
                        );
                        stack.push(reference.into());
                        let target = root
                            .pointer(&reference[1..])
                            .context("Missing OpenAPI reference")?;
                        let resolved = walk(root, target, depth + 1, left, stack)?;
                        stack.pop();
                        output = resolved
                            .as_object()
                            .context("Reference must resolve to an object")?
                            .clone();
                    }
                    for (key, value) in map {
                        if key != "$ref" {
                            output.insert(key.clone(), walk(root, value, depth + 1, left, stack)?);
                        }
                    }
                    if output.remove("nullable") == Some(json!(true)) {
                        let schema = Value::Object(output);
                        return Ok(json!({"anyOf":[schema,{"type":"null"}]}));
                    }
                    for (exclusive, bound) in [
                        ("exclusiveMinimum", "minimum"),
                        ("exclusiveMaximum", "maximum"),
                    ] {
                        if output.get(exclusive) == Some(&json!(true)) {
                            if let Some(value) = output.remove(bound) {
                                output.insert(exclusive.into(), value);
                            }
                        } else if output.get(exclusive) == Some(&json!(false)) {
                            output.remove(exclusive);
                        }
                    }
                    Ok(Value::Object(output))
                }
                Value::Array(values) => Ok(Value::Array(
                    values
                        .iter()
                        .map(|v| walk(root, v, depth + 1, left, stack))
                        .collect::<Result<_>>()?,
                )),
                _ => Ok(value.clone()),
            }
        }
        walk(&self.value, value, 0, &mut 100_000, &mut vec![])
    }
    pub fn lint(&self) -> Vec<Issue> {
        let mut issues = vec![];
        for key in ["title", "version"] {
            if self.value["info"][key].as_str().is_none_or(str::is_empty) {
                message(
                    &mut issues,
                    "error",
                    &format!("/info/{key}"),
                    "Required info field is missing",
                );
            }
        }
        let Some(paths) = self.value["paths"].as_object() else {
            message(&mut issues, "error", "/paths", "Paths must be an object");
            return issues;
        };
        let mut ids = BTreeSet::new();
        for (path, item) in paths {
            let location = format!("/paths/{}", pointer(path));
            if !path.starts_with('/') {
                message(&mut issues, "error", &location, "Path must start with /");
            }
            let item = match self.resolve(item) {
                Ok(value) => value,
                Err(_) => {
                    message(
                        &mut issues,
                        "error",
                        &location,
                        "Invalid, recursive or external reference",
                    );
                    continue;
                }
            };
            for method in METHODS {
                let operation = &item[method];
                if operation.is_null() {
                    continue;
                }
                let location = format!("{location}/{method}");
                if !operation.is_object() {
                    message(
                        &mut issues,
                        "error",
                        &location,
                        "Operation must be an object",
                    );
                    continue;
                }
                if let Some(id) = operation["operationId"].as_str() {
                    if !ids.insert(id.to_owned()) {
                        message(&mut issues, "error", &location, "Duplicate operationId");
                    }
                } else {
                    message(
                        &mut issues,
                        "warning",
                        &location,
                        "An operationId makes operation tracking more stable",
                    );
                }
                if operation["responses"]
                    .as_object()
                    .is_none_or(|r| r.is_empty())
                {
                    message(
                        &mut issues,
                        "error",
                        &location,
                        "Operation requires responses",
                    );
                }
                let params = parameters(&item, operation);
                for segment in path.split('/') {
                    if let Some(name) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}'))
                    {
                        if !params.iter().any(|p| {
                            p["in"] == "path" && p["name"] == name && p["required"] == true
                        }) {
                            message(
                                &mut issues,
                                "error",
                                &location,
                                "Template parameter must be declared as required in path",
                            );
                        }
                    }
                }
                for parameter in params {
                    if parameter["in"] == "cookie"
                        || parameter["schema"]["type"] == "array"
                        || parameter["schema"]["type"] == "object"
                        || parameter.get("content").is_some()
                    {
                        message(
                            &mut issues,
                            "warning",
                            &location,
                            "Import needs review for cookie or structured parameter serialization",
                        );
                    }
                }
                if operation.get("callbacks").is_some() {
                    message(
                        &mut issues,
                        "warning",
                        &location,
                        "Callbacks are documented but not imported as executable requests",
                    );
                }
            }
        }
        if self.value.get("webhooks").is_some() {
            message(
                &mut issues,
                "warning",
                "/webhooks",
                "Webhooks are not imported as executable requests",
            );
        }
        issues
    }
    pub fn import(&self) -> Result<(Collection, Vec<Issue>)> {
        let issues = self.lint();
        ensure!(
            !issues.iter().any(|i| i.severity == "error"),
            "Fix OpenAPI lint errors before importing"
        );
        let mut collection =
            Collection::new(self.value["info"]["title"].as_str().unwrap_or("OpenAPI"));
        collection.description = self.value["info"]["description"]
            .as_str()
            .unwrap_or_default()
            .into();
        collection.version = self.value["info"]["version"]
            .as_str()
            .unwrap_or("1.0.0")
            .into();
        let mut folders: BTreeMap<String, Folder> = BTreeMap::new();
        for (path, item) in self.value["paths"].as_object().context("Paths missing")? {
            let item = self.resolve(item)?;
            for method in METHODS {
                let operation = &item[method];
                if !operation.is_object() {
                    continue;
                }
                let mut request = ApiRequest::new(
                    operation["summary"]
                        .as_str()
                        .or(operation["operationId"].as_str())
                        .map(String::from)
                        .unwrap_or_else(|| format!("{} {path}", method.to_uppercase())),
                );
                request.method = HttpMethod::from(method.to_uppercase());
                request.description = operation["description"].as_str().unwrap_or_default().into();
                let servers = operation
                    .get("servers")
                    .or(item.get("servers"))
                    .unwrap_or(&self.value["servers"]);
                let server = servers.as_array().and_then(|s| s.first());
                let mut base = server
                    .and_then(|s| s["url"].as_str())
                    .unwrap_or("{{baseUrl}}")
                    .to_owned();
                if let Some(vars) = server.and_then(|s| s["variables"].as_object()) {
                    for (name, value) in vars {
                        base = base.replace(&format!("{{{name}}}"), &scalar(&value["default"]));
                    }
                }
                if base.starts_with('/') {
                    collection
                        .variables
                        .insert("baseUrl".into(), "http://localhost".into());
                    base = format!("{{{{baseUrl}}}}{base}");
                }
                if base == "{{baseUrl}}" {
                    collection
                        .variables
                        .insert("baseUrl".into(), "http://localhost".into());
                }
                let mut target = path.clone();
                for parameter in parameters(&item, operation) {
                    let Some(name) = parameter["name"].as_str() else {
                        continue;
                    };
                    let value = parameter
                        .get("example")
                        .cloned()
                        .unwrap_or_else(|| sample(&parameter["schema"]));
                    let mut row = KeyValue::new(name, scalar(&value));
                    row.description = parameter["description"].as_str().unwrap_or_default().into();
                    row.enabled = parameter["required"] == true
                        || parameter.get("example").is_some()
                        || parameter["schema"].get("default").is_some();
                    match parameter["in"].as_str() {
                        Some("path") => {
                            row.enabled = true;
                            target = target.replace(&format!("{{{name}}}"), &format!(":{name}"));
                            request.path_params.push(row);
                        }
                        Some("query") => request.query_params.push(row),
                        Some("header") => request.headers.push(row),
                        _ => {}
                    }
                }
                request.url = format!("{}{target}", base.trim_end_matches('/'));
                let body = &operation["requestBody"];
                if let Some(content) = body["content"].as_object() {
                    let selected = content
                        .get_key_value("application/json")
                        .or_else(|| content.iter().next());
                    if let Some((mime, media)) = selected {
                        request.headers.push(KeyValue::new("Content-Type", mime));
                        request.body_type = if mime.contains("json") {
                            BodyType::Json
                        } else if mime.contains("xml") {
                            BodyType::Xml
                        } else {
                            BodyType::Text
                        };
                        let example = media
                            .get("example")
                            .cloned()
                            .or_else(|| {
                                media["examples"]
                                    .as_object()
                                    .and_then(|v| v.values().next())
                                    .map(|v| v["value"].clone())
                            })
                            .unwrap_or_else(|| sample(&media["schema"]));
                        request.body = if request.body_type == BodyType::Json {
                            serde_json::to_string_pretty(&example)?
                        } else {
                            scalar(&example)
                        };
                    }
                }
                request.auth = auth(
                    &self.value,
                    operation.get("security").unwrap_or(&self.value["security"]),
                );
                let responses = operation["responses"].clone();
                ensure!(
                    serde_json::to_vec(&responses)?.len() <= 256 * 1024,
                    "Operation response contract exceeds 256 KiB"
                );
                request.checks.push(Assertion {
                    name: "OpenAPI response contract".into(),
                    kind: AssertionKind::OpenApi,
                    target: format!("{} {path}", method.to_uppercase()),
                    expected: responses.to_string(),
                });
                request.imported_settings.insert(
                    "openapi.operation".into(),
                    json!({"method":method,"path":path,"operationId":operation["operationId"]}),
                );
                let tag = operation["tags"]
                    .as_array()
                    .and_then(|t| t.first())
                    .and_then(Value::as_str);
                if let Some(tag) = tag {
                    folders
                        .entry(tag.into())
                        .or_insert_with(|| Folder::new(tag))
                        .requests
                        .push(request);
                } else {
                    collection.requests.push(request);
                }
            }
        }
        collection.folders = folders.into_values().collect();
        Ok((collection, issues))
    }
    pub fn drift(&self, newer: &Self) -> Result<Vec<Drift>> {
        let operations = |spec: &Self| -> Result<BTreeMap<String, Value>> {
            let mut out = BTreeMap::new();
            for (path, item) in spec.value["paths"].as_object().context("Paths missing")? {
                let item = spec.resolve(item)?;
                for method in METHODS {
                    if item[method].is_object() {
                        let mut operation = item[method].clone();
                        operation["x-rustapi-path-parameters"] = item["parameters"].clone();
                        operation["x-rustapi-path-servers"] = item["servers"].clone();
                        out.insert(format!("{} {path}", method.to_uppercase()), operation);
                    }
                }
            }
            Ok(out)
        };
        let before = operations(self)?;
        let after = operations(newer)?;
        let mut changes = vec![];
        for (operation, value) in &before {
            match after.get(operation) {
                None => changes.push(Drift {
                    operation: operation.clone(),
                    change: "removed".into(),
                    requires_review: true,
                }),
                Some(next) if value != next => changes.push(Drift {
                    operation: operation.clone(),
                    change: "changed".into(),
                    requires_review: true,
                }),
                _ => {}
            }
        }
        for operation in after.keys() {
            if !before.contains_key(operation) {
                changes.push(Drift {
                    operation: operation.clone(),
                    change: "added".into(),
                    requires_review: false,
                });
            }
        }
        if self.value["servers"] != newer.value["servers"]
            || self.value["security"] != newer.value["security"]
            || self.value["components"]["securitySchemes"]
                != newer.value["components"]["securitySchemes"]
        {
            changes.push(Drift {
                operation: "collection".into(),
                change: "servers or security changed".into(),
                requires_review: true,
            });
        }
        Ok(changes)
    }
}
fn parameters<'a>(item: &'a Value, operation: &'a Value) -> Vec<&'a Value> {
    let mut output = BTreeMap::new();
    for parameter in item["parameters"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(operation["parameters"].as_array().into_iter().flatten())
    {
        output.insert(
            (
                parameter["in"].as_str().unwrap_or_default(),
                parameter["name"].as_str().unwrap_or_default(),
            ),
            parameter,
        );
    }
    output.into_values().collect()
}
fn auth(spec: &Value, security: &Value) -> AuthConfig {
    let mut auth = AuthConfig::default();
    let Some(requirement) = security
        .as_array()
        .and_then(|s| s.first())
        .and_then(Value::as_object)
    else {
        return auth;
    };
    if requirement.is_empty() {
        return auth;
    }
    if requirement.len() != 1 {
        auth.auth_type = AuthType::Unsupported;
        return auth;
    }
    let (name, _) = requirement.iter().next().unwrap();
    let scheme = &spec["components"]["securitySchemes"][name];
    auth.auth_type = match (scheme["type"].as_str(), scheme["scheme"].as_str()) {
        (Some("http"), Some("basic")) => AuthType::BasicAuth,
        (Some("http"), Some("bearer")) => {
            auth.bearer_token = "{{vault:access_token}}".into();
            AuthType::BearerToken
        }
        (Some("apiKey"), _)
            if ["header", "query"].contains(&scheme["in"].as_str().unwrap_or_default()) =>
        {
            auth.api_key_name = scheme["name"].as_str().unwrap_or_default().into();
            auth.api_key_value = format!("{{{{vault:{name}}}}}");
            auth.api_key_location = if scheme["in"] == "query" {
                ApiKeyLocation::QueryParam
            } else {
                ApiKeyLocation::Header
            };
            AuthType::ApiKey
        }
        _ => AuthType::Unsupported,
    };
    auth
}
#[derive(Debug, Serialize)]
pub struct Drift {
    pub operation: String,
    pub change: String,
    pub requires_review: bool,
}
pub fn response_contract(responses: &str, response: &ApiResponse) -> Result<Vec<String>> {
    ensure!(
        responses.len() <= 256 * 1024,
        "Response contract exceeds 256 KiB"
    );
    let responses: Value = serde_json::from_str(responses)?;
    let status = response.status.to_string();
    let range = format!("{}XX", response.status / 100);
    let selected = responses
        .get(&status)
        .or(responses.get(&range))
        .or(responses.get("default"))
        .context("Response status is not declared in OpenAPI")?;
    let Some(content) = selected["content"].as_object() else {
        return Ok(vec![]);
    };
    let mime = response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| {
            v.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    let media = content
        .get(&mime)
        .or_else(|| {
            mime.split_once('/')
                .and_then(|(kind, _)| content.get(&format!("{kind}/*")))
        })
        .or(content.get("*/*"))
        .context("Response Content-Type is not declared in OpenAPI")?;
    if media.get("schema").is_none() {
        return Ok(vec![]);
    }
    if mime.contains("json") {
        let bytes = if response.raw_body.is_empty() {
            response.body.as_bytes()
        } else {
            &response.raw_body
        };
        let value: Value =
            serde_json::from_slice(bytes).context("OpenAPI response must be JSON")?;
        crate::schema::validate(&media["schema"].to_string(), &value)
    } else {
        Ok(vec!["Non-JSON body schema requires manual review".into()])
    }
}
