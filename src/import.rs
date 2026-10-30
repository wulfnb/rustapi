//! Import adapters validate the whole input before the caller persists it.
use crate::models::*;
use anyhow::{bail, Context, Result};
use serde_json::Value;
mod diagnostics;
pub use diagnostics::{ImportDiagnostic, ImportDisposition, ImportReport, ImportSeverity};
#[derive(Debug)]
pub struct ImportResult {
    pub collection: Option<Collection>,
    pub environment: Option<Environment>,
    pub warnings: Vec<String>,
    pub diagnostics: Vec<ImportDiagnostic>,
}
impl ImportResult {
    pub fn requires_confirmation(&self) -> bool {
        !self.diagnostics.is_empty()
    }
    pub fn report(&self) -> ImportReport<'_> {
        ImportReport {
            schema_version: 1,
            requires_confirmation: self.requires_confirmation(),
            diagnostics: &self.diagnostics,
        }
    }
}
fn text(v: &Value) -> String {
    v.as_str().map(String::from).unwrap_or_else(|| {
        if v.is_null() {
            String::new()
        } else {
            v.to_string()
        }
    })
}
fn values(v: &Value) -> Vec<KeyValue> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|v| KeyValue {
                    enabled: v["enabled"].as_bool().unwrap_or(true)
                        && !v["disabled"].as_bool().unwrap_or(false),
                    description: v["description"].as_str().unwrap_or_default().into(),
                    ..KeyValue::new(text(&v["key"]), text(&v["value"]))
                })
                .collect()
        })
        .unwrap_or_default()
}
fn auth(v: &Value, warnings: &mut Vec<String>) -> AuthConfig {
    let kind = v["type"].as_str().unwrap_or("inherit");
    let fields = values(&v[kind]);
    let get = |name: &str| {
        fields
            .iter()
            .find(|f| f.key == name)
            .map(|f| f.value.clone())
            .unwrap_or_default()
    };
    let mut a = AuthConfig::default();
    match kind {
        "inherit" => a.auth_type = AuthType::Inherit,
        "noauth" => {}
        "bearer" => {
            a.auth_type = AuthType::BearerToken;
            a.bearer_token = get("token");
        }
        "basic" => {
            a.auth_type = AuthType::BasicAuth;
            a.basic_username = get("username");
            a.basic_password = get("password");
        }
        "apikey" => {
            a.auth_type = AuthType::ApiKey;
            a.api_key_name = get("key");
            a.api_key_value = get("value");
            a.api_key_location = if get("in") == "query" {
                ApiKeyLocation::QueryParam
            } else {
                ApiKeyLocation::Header
            };
        }
        "oauth2" => {
            a.auth_type = AuthType::OAuth2;
            a.oauth.token_url = get("accessTokenUrl");
            a.oauth.authorization_url = get("authUrl");
            a.oauth.client_id = get("clientId");
            a.oauth.client_secret = get("clientSecret");
            a.oauth.scope = get("scope");
            a.oauth.redirect_uri = get("redirect_uri");
            a.oauth.grant = get("grant_type");
            warnings.push("OAuth configuration imported; authorize locally before sending".into());
        }
        other => {
            warnings.push(format!(
                "Unsupported auth type {other}; configure authentication before sending"
            ));
            a.auth_type = AuthType::Unsupported;
        }
    }
    a
}
fn scripts(v: &Value) -> (String, String) {
    let mut pre = vec![];
    let mut post = vec![];
    if let Some(events) = v["event"].as_array() {
        for e in events {
            let code = match &e["script"]["exec"] {
                Value::Array(a) => a.iter().map(text).collect::<Vec<_>>().join("\n"),
                v => text(v),
            };
            match e["listen"].as_str() {
                Some("prerequest") => pre.push(code),
                Some("test") => post.push(code),
                _ => {}
            }
        }
    }
    (pre.join("\n"), post.join("\n"))
}
fn request(item: &Value, warnings: &mut Vec<String>) -> Result<ApiRequest> {
    let v = &item["request"];
    let mut r = ApiRequest::new(text(&item["name"]));
    r.description = v["description"].as_str().unwrap_or_default().into();
    if let Some(url) = v.as_str() {
        r.url = url.into();
        return Ok(r);
    }
    r.method = serde_json::from_value(Value::String(v["method"].as_str().unwrap_or("GET").into()))
        .context("Unsupported HTTP method")?;
    r.url = if v["url"].is_object() {
        text(&v["url"]["raw"])
    } else {
        text(&v["url"])
    };
    anyhow::ensure!(!r.url.is_empty(), "Request {} has no raw URL", r.name);
    r.query_params = values(&v["url"]["query"]);
    if !r.query_params.is_empty() {
        let (before, fragment) = r.url.split_once('#').unwrap_or((&r.url, ""));
        let base = before.split('?').next().unwrap_or(before);
        r.url = if fragment.is_empty() {
            base.into()
        } else {
            format!("{base}#{fragment}")
        };
    }
    r.headers = values(&v["header"]);
    r.auth = auth(&v["auth"], warnings);
    let body = &v["body"];
    match body["mode"].as_str().unwrap_or("") {
        "raw" => {
            r.body = text(&body["raw"]);
            r.body_type = if body["options"]["raw"]["language"] == "json"
                || r.headers
                    .iter()
                    .any(|h| h.key.eq_ignore_ascii_case("content-type") && h.value.contains("json"))
            {
                BodyType::Json
            } else {
                BodyType::Text
            };
        }
        "urlencoded" => {
            r.body_type = BodyType::UrlEncoded;
            r.form_data = values(&body["urlencoded"]);
        }
        "formdata" => {
            r.body_type = BodyType::FormData;
            r.form_data = values(&body["formdata"]);
            if let Some(a) = body["formdata"].as_array() {
                for (field, source) in r.form_data.iter_mut().zip(a) {
                    if source["type"] == "file" {
                        field.file = true;
                        field.value = source["src"]
                            .as_str()
                            .map(String::from)
                            .or_else(|| source["src"].as_array().and_then(|a| a.first()).map(text))
                            .unwrap_or_default();
                        warnings.push(format!("Check local upload path for {}", r.name));
                    }
                }
            }
        }
        "file" => {
            r.body_type = BodyType::Binary;
            r.binary_file = text(&body["file"]["src"]);
            warnings.push(format!("Check local binary upload path for {}", r.name));
        }
        "" => {}
        other => bail!(
            "Unsupported body mode {other} in {}; import stopped without writing",
            r.name
        ),
    }
    (r.pre_script, r.post_script) = scripts(item);
    if !r.pre_script.is_empty() || !r.post_script.is_empty() {
        warnings.push(format!(
            "{}: scripts preserved; only the documented synchronous pm.* subset is supported",
            r.name
        ));
    }
    Ok(r)
}
fn items(
    v: &Value,
    requests: &mut Vec<ApiRequest>,
    folders: &mut Vec<Folder>,
    order: &mut Vec<String>,
    warnings: &mut Vec<String>,
    depth: usize,
) -> Result<()> {
    anyhow::ensure!(depth < 64, "Folder nesting exceeds 64 levels");
    for item in v.as_array().context("Collection items must be an array")? {
        if item["item"].is_array() {
            let mut f = Folder::new(text(&item["name"]));
            f.description = item["description"].as_str().unwrap_or_default().into();
            f.auth = auth(&item["auth"], warnings);
            f.variables = values(&item["variable"])
                .into_iter()
                .filter(|v| v.enabled)
                .map(|v| (v.key, v.value))
                .collect();
            (f.pre_script, f.post_script) = scripts(item);
            items(
                &item["item"],
                &mut f.requests,
                &mut f.folders,
                &mut f.order,
                warnings,
                depth + 1,
            )?;
            order.push(f.id.clone());
            folders.push(f);
        } else {
            let r = request(item, warnings)?;
            order.push(r.id.clone());
            requests.push(r);
        }
    }
    Ok(())
}
pub fn json(bytes: &[u8]) -> Result<ImportResult> {
    anyhow::ensure!(
        bytes.len() <= 64 * 1024 * 1024,
        "Import exceeds 64 MiB limit"
    );
    let v: Value = serde_json::from_slice(bytes)?;
    let mut warnings = vec![];
    if v["info"]["schema"].is_string() {
        anyhow::ensure!(
            v["info"]["schema"]
                .as_str()
                .unwrap_or_default()
                .contains("v2.1.0"),
            "Only collection schema v2.1 is supported"
        );
        let mut c = Collection::new(text(&v["info"]["name"]));
        c.description = text(&v["info"]["description"]);
        c.auth = auth(&v["auth"], &mut warnings);
        if c.auth.auth_type == AuthType::Inherit {
            c.auth.auth_type = AuthType::None;
        }
        c.variables = values(&v["variable"])
            .into_iter()
            .filter(|v| v.enabled)
            .map(|v| (v.key, v.value))
            .collect();
        (c.pre_script, c.post_script) = scripts(&v);
        items(
            &v["item"],
            &mut c.requests,
            &mut c.folders,
            &mut c.order,
            &mut warnings,
            0,
        )?;
        validate_collection(&c)?;
        Ok(ImportResult {
            collection: Some(c),
            environment: None,
            warnings,
            diagnostics: diagnostics::collection(&v),
        })
    } else if v["values"].is_array() {
        let mut e = Environment::new(text(&v["name"]));
        e.variables = values(&v["values"]);
        Ok(ImportResult {
            collection: None,
            environment: Some(e),
            warnings,
            diagnostics: diagnostics::environment(&v),
        })
    } else {
        let mut c: Collection = serde_json::from_value(v)?;
        anyhow::ensure!(c.schema_version == 1, "Unsupported collection schema");
        // Copy import gets a new collection identity; request IDs remain stable inside it.
        c.id = uuid::Uuid::new_v4().to_string();
        c.file_path = None;
        validate_collection(&c)?;
        Ok(ImportResult {
            collection: Some(c),
            environment: None,
            warnings,
            diagnostics: vec![],
        })
    }
}
pub fn curl(command: &str) -> Result<ImportResult> {
    let tokens = shlex::split(command).context("Unclosed quote in cURL command")?;
    anyhow::ensure!(
        tokens.first().is_some_and(|s| s == "curl"),
        "Expected a curl command"
    );
    let mut r = ApiRequest::new("Imported cURL");
    let mut i = 1;
    let mut explicit = false;
    let mut has_body = false;
    let mut get = false;
    while i < tokens.len() {
        let t = &tokens[i];
        let mut value = || -> Result<String> {
            i += 1;
            tokens.get(i).cloned().context("Missing cURL option value")
        };
        match t.as_str() {
            "-X" | "--request" => {
                r.method = serde_json::from_value(Value::String(value()?.to_uppercase()))?;
                explicit = true;
            }
            "-H" | "--header" => {
                let h = value()?;
                let (k, v) = h.split_once(':').context("Header requires name: value")?;
                r.headers.push(KeyValue::new(k.trim(), v.trim()));
            }
            "-u" | "--user" => {
                let v = value()?;
                let (u, p) = v.split_once(':').unwrap_or((&v, ""));
                r.auth.auth_type = AuthType::BasicAuth;
                r.auth.basic_username = u.into();
                r.auth.basic_password = p.into();
            }
            "--url" => r.url = value()?,
            "-G" | "--get" => get = true,
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-urlencode" => {
                let v = value()?;
                has_body = true;
                if t == "--data-binary" && v.starts_with('@') {
                    r.body_type = BodyType::Binary;
                    r.binary_file = v[1..].into();
                } else if t == "--data-urlencode" {
                    let (k, v) = v
                        .split_once('=')
                        .context("data-urlencode requires key=value")?;
                    r.body_type = BodyType::UrlEncoded;
                    r.form_data.push(KeyValue::new(k, v));
                } else {
                    anyhow::ensure!(
                        !v.starts_with('@') || t == "--data-raw",
                        "File bodies require --data-binary @path"
                    );
                    if !r.body.is_empty() {
                        r.body.push('&');
                    }
                    r.body.push_str(&v);
                    r.body_type = BodyType::Text;
                }
            }
            "-F" | "--form" | "--form-string" => {
                let v = value()?;
                let (k, v) = v.split_once('=').context("Form requires key=value")?;
                let mut f = KeyValue::new(k, v);
                if v.starts_with('@') && t != "--form-string" {
                    f.file = true;
                    f.value = v[1..].into();
                }
                r.form_data.push(f);
                r.body_type = BodyType::FormData;
                has_body = true;
            }
            "-k" | "--insecure" => r.settings.verify_tls = false,
            "-L" | "--location" => r.settings.follow_redirects = true,
            "--max-time" => r.settings.timeout_ms = (value()?.parse::<f64>()? * 1000.) as u64,
            "--proxy" | "-x" => r.settings.proxy = value()?,
            "--compressed" | "--silent" | "-s" | "--show-error" | "-S" => {}
            v if !v.starts_with('-') => {
                anyhow::ensure!(r.url.is_empty(), "Only one cURL URL per import");
                r.url = v.into();
            }
            other => bail!("Unsupported cURL option {other}; no request was imported"),
        }
        i += 1;
    }
    anyhow::ensure!(!r.url.is_empty(), "cURL command has no URL");
    if !explicit && has_body {
        r.method = HttpMethod::POST;
    }
    if get {
        r.method = HttpMethod::GET;
        if !r.body.is_empty() {
            for (k, v) in reqwest::Url::parse(&format!("http://local/?{}", r.body))?.query_pairs() {
                r.query_params.push(KeyValue::new(k, v));
            }
            r.body.clear();
            r.body_type = BodyType::None;
        }
    }
    if r.body_type == BodyType::Text
        && r.headers
            .iter()
            .any(|h| h.key.eq_ignore_ascii_case("content-type") && h.value.contains("json"))
    {
        r.body_type = BodyType::Json;
    }
    let mut c = Collection::new("cURL import");
    c.requests.push(r);
    Ok(ImportResult {
        collection: Some(c),
        environment: None,
        warnings: vec![],
        diagnostics: vec![],
    })
}
