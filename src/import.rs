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
                .map(|v| {
                    let (description, description_metadata) = decode_description(&v["description"]);
                    KeyValue {
                        enabled: v["enabled"].as_bool().unwrap_or(true)
                            && !v["disabled"].as_bool().unwrap_or(false),
                        description,
                        description_metadata,
                        value_type: v["type"].as_str().unwrap_or_default().into(),
                        content_type: v["contentType"].as_str().unwrap_or_default().into(),
                        extensions: v
                            .as_object()
                            .map(|object| {
                                object
                                    .iter()
                                    .filter(|(key, _)| {
                                        ![
                                            "key",
                                            "value",
                                            "enabled",
                                            "disabled",
                                            "description",
                                            "type",
                                            "file",
                                            "src",
                                            "contentType",
                                        ]
                                        .contains(&key.as_str())
                                    })
                                    .map(|(key, value)| (key.clone(), value.clone()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        ..KeyValue::new(text(&v["key"]), text(&v["value"]))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}
fn auth(v: &Value, warnings: &mut Vec<String>) -> Result<AuthConfig> {
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
        "basic" | "digest" => {
            a.auth_type = if kind == "digest" {
                AuthType::Digest
            } else {
                AuthType::BasicAuth
            };
            a.basic_username = get("username");
            a.basic_password = get("password");
        }
        "jwt" => {
            anyhow::ensure!(
                get("isSecretBase64Encoded") != "true",
                "Base64-encoded JWT secrets must be decoded before import"
            );
            anyhow::ensure!(
                ["", "Bearer"].contains(&get("headerPrefix").as_str())
                    && ["", "header"].contains(&get("addTokenTo").as_str()),
                "Only JWT Bearer authorization headers are supported"
            );
            a.auth_type = AuthType::Jwt;
            a.jwt.algorithm = if get("algorithm").is_empty() {
                "HS256".into()
            } else {
                get("algorithm")
            };
            a.jwt.key = if a.jwt.algorithm.starts_with("HS") || get("privateKey").is_empty() {
                get("secret")
            } else {
                get("privateKey")
            };
            a.jwt.key_id = get("keyId");
            a.jwt.claims = if get("payload").is_empty() {
                "{}".into()
            } else {
                get("payload")
            };
        }
        "awsv4" => {
            a.auth_type = AuthType::AwsSigV4;
            a.aws = AwsConfig {
                access_key: get("accessKey"),
                secret_key: get("secretKey"),
                session_token: get("sessionToken"),
                region: get("region"),
                service: get("service"),
            };
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
            a.oauth.refresh_url = get("refreshTokenUrl");
            a.oauth.refresh_scope = get("refreshTokenScope");
            a.oauth.persist_tokens = get("_rustapiPersistTokens") == "true";
            a.oauth.client_auth = match get("client_authentication").as_str() {
                "" | "body" | "client_secret_post" => OAuthClientAuth::Body,
                "header" | "client_secret_basic" => OAuthClientAuth::Basic,
                "none" => OAuthClientAuth::Public,
                _ => bail!("Unsupported OAuth client authentication option"),
            };
            let parameters = |name: &str| -> Result<Vec<KeyValue>> {
                let value = v[kind]
                    .as_array()
                    .and_then(|fields| fields.iter().find(|field| field["key"] == name))
                    .map(|field| field["value"].clone())
                    .unwrap_or(Value::Null);
                let value = if let Some(text) = value.as_str() {
                    serde_json::from_str(text).context("Invalid OAuth parameter array")?
                } else {
                    value
                };
                anyhow::ensure!(
                    value.is_null() || value.is_array(),
                    "OAuth provider parameters must be an array"
                );
                Ok(values(&value))
            };
            a.oauth.authorization_params = parameters("authRequestParams")?;
            a.oauth.token_params = parameters("tokenRequestParams")?;
            a.oauth.refresh_params = parameters("refreshRequestParams")?;
            warnings.push("OAuth configuration imported; authorize locally before sending".into());
        }
        other => {
            warnings.push(format!(
                "Unsupported auth type {other}; configure authentication before sending"
            ));
            a.auth_type = AuthType::Unsupported;
        }
    }
    Ok(a)
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
fn structured_url(value: &Value) -> Result<String> {
    if let Some(raw) = value["raw"].as_str().filter(|raw| !raw.is_empty()) {
        return Ok(raw.into());
    }
    fn component(value: &Value, separator: &str) -> Result<String> {
        if let Some(text) = value.as_str() {
            return Ok(text.into());
        }
        if let Some(values) = value.as_array() {
            anyhow::ensure!(
                values.iter().all(Value::is_string),
                "URL components must be strings"
            );
            return Ok(values
                .iter()
                .map(|v| v.as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(separator));
        }
        anyhow::ensure!(value.is_null(), "Invalid URL component");
        Ok(String::new())
    }
    let protocol = value["protocol"].as_str().unwrap_or("https");
    anyhow::ensure!(
        ["http", "https"].contains(&protocol),
        "Only HTTP URL protocols are supported"
    );
    let host = component(&value["host"], ".")?;
    anyhow::ensure!(!host.is_empty(), "Structured URL requires host or raw URL");
    let port = component(&value["port"], "")?;
    let path = component(&value["path"], "/")?;
    let fragment = component(&value["hash"], "")?;
    let url = format!(
        "{protocol}://{host}{}/{}{}",
        if port.is_empty() {
            String::new()
        } else {
            format!(":{port}")
        },
        path.trim_start_matches('/'),
        if fragment.is_empty() {
            String::new()
        } else {
            format!("#{fragment}")
        }
    );
    reqwest::Url::parse(&url).context("Invalid structured URL")?;
    Ok(url)
}
fn imported_profile(
    value: &Value,
    inherited: &HttpSettings,
) -> Result<(HttpSettings, std::collections::BTreeMap<String, Value>)> {
    anyhow::ensure!(
        value.is_null() || value.is_object(),
        "Request settings must be an object"
    );
    let raw: std::collections::BTreeMap<String, Value> = value
        .as_object()
        .map(|v| v.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let mut settings = inherited.clone();
    for (key, value) in &raw {
        match key.as_str() {
            "followRedirects" => {
                settings.follow_redirects =
                    value.as_bool().context("followRedirects must be boolean")?
            }
            "disableCookieJar" => {
                settings.use_cookies = !value
                    .as_bool()
                    .context("disableCookieJar must be boolean")?
            }
            "strictSSL" => {
                settings.verify_tls = value.as_bool().context("strictSSL must be boolean")?
            }
            "timeout" => {
                let timeout = value.as_u64().context("timeout must be milliseconds")?;
                anyhow::ensure!(
                    (1..=3_600_000).contains(&timeout),
                    "Timeout must be between 1 and 3600000 ms"
                );
                settings.timeout_ms = timeout;
            }
            _ => {}
        }
    }
    Ok((settings, raw))
}
fn request(
    item: &Value,
    warnings: &mut Vec<String>,
    inherited: &HttpSettings,
) -> Result<ApiRequest> {
    let v = &item["request"];
    if let Some(url) = v.as_str() {
        let mut normalized = item.clone();
        normalized["request"] = serde_json::json!({"url": url});
        return request(&normalized, warnings, inherited);
    }
    let mut r = ApiRequest::new(text(&item["name"]));
    let (settings, mut profile) = imported_profile(&item["protocolProfileBehavior"], inherited)?;
    let (settings, request_profile) = imported_profile(&v["protocolProfileBehavior"], &settings)?;
    profile.extend(request_profile);
    r.settings = settings;
    r.imported_settings = profile;
    (r.description, r.description_metadata) = decode_description(&v["description"]);
    r.method = serde_json::from_value(Value::String(v["method"].as_str().unwrap_or("GET").into()))
        .context("Unsupported HTTP method")?;
    r.url = if v["url"].is_object() {
        structured_url(&v["url"])?
    } else {
        text(&v["url"])
    };
    anyhow::ensure!(!r.url.is_empty(), "Request {} has no raw URL", r.name);
    r.query_params = values(&v["url"]["query"]);
    anyhow::ensure!(
        v["url"]["variable"].is_null() || v["url"]["variable"].is_array(),
        "Path variables must be an array"
    );
    r.path_params = values(&v["url"]["variable"]);
    r.headers = values(&v["header"]);
    r.auth = auth(&v["auth"], warnings)?;
    let body = &v["body"];
    match body["mode"].as_str().unwrap_or("") {
        "graphql" => {
            anyhow::ensure!(
                body["graphql"].is_object() && body["graphql"]["query"].is_string(),
                "GraphQL body requires a query document"
            );
            r.graphql = Some(crate::graphql::Graphql {
                query: text(&body["graphql"]["query"]),
                variables: body["graphql"]["variables"].as_str().unwrap_or("{}").into(),
                operation_name: text(&body["graphql"]["operationName"]),
                ..Default::default()
            });
            r.method = HttpMethod::POST;
        }
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
                let mut fields = vec![];
                for (mut field, source) in r.form_data.into_iter().zip(a) {
                    anyhow::ensure!(
                        source["contentType"].is_null() || source["contentType"].is_string(),
                        "Part content type must be a string"
                    );
                    if source["type"] == "file" {
                        field.file = true;
                        let paths = if let Some(paths) = source["src"].as_array() {
                            anyhow::ensure!(
                                paths.iter().all(Value::is_string),
                                "Upload paths must be strings"
                            );
                            paths.iter().map(text).collect::<Vec<_>>()
                        } else {
                            vec![text(&source["src"])]
                        };
                        let paths = if paths.is_empty() {
                            vec![String::new()]
                        } else {
                            paths
                        };
                        for path in paths {
                            let mut file = field.clone();
                            file.value = path;
                            fields.push(file);
                        }
                        warnings.push(format!("Check local upload path for {}", r.name));
                    } else {
                        fields.push(field);
                    }
                }
                r.form_data = fields;
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
    if !v["x-rustapi"].is_null() {
        let extension = &v["x-rustapi"];
        anyhow::ensure!(
            extension["version"] == 1,
            "Unsupported native portable extension version"
        );
        if !extension["settings"].is_null() {
            r.settings = serde_json::from_value(extension["settings"].clone())?;
        }
        if !extension["checks"].is_null() {
            r.checks = serde_json::from_value(extension["checks"].clone())?;
        }
        if !extension["extractors"].is_null() {
            r.extractors = serde_json::from_value(extension["extractors"].clone())?;
        }
    }
    (r.pre_script, r.post_script) = scripts(item);
    if let Some(value) = v["x-rustapi"]["pre_script"].as_str() {
        r.pre_script = value.into();
    }
    if let Some(value) = v["x-rustapi"]["post_script"].as_str() {
        r.post_script = value.into();
    }
    if let Some(examples) = item["response"].as_array() {
        anyhow::ensure!(examples.len() <= 100, "At most 100 examples per request");
        for source in examples {
            let snapshot = if source["originalRequest"].is_object() {
                request(
                    &serde_json::json!({"name": r.name, "request": source["originalRequest"]}),
                    warnings,
                    &r.settings,
                )?
            } else {
                r.clone()
            };
            let body = text(&source["body"]);
            let response = ApiResponse {
                status: source["code"]
                    .as_u64()
                    .filter(|code| (100..=599).contains(code))
                    .context("Example status must be 100..599")? as u16,
                status_text: text(&source["status"]),
                headers: values(&source["header"])
                    .into_iter()
                    .map(|row| (row.key, row.value))
                    .collect(),
                size: body.len(),
                body,
                ..Default::default()
            };
            let mut example = RequestExample::capture(text(&source["name"]), &snapshot, &response)?;
            if !source["x-rustapi"].is_null() {
                anyhow::ensure!(
                    source["x-rustapi"]["version"] == 1,
                    "Unsupported example extension version"
                );
                if let Some(bytes) = source["x-rustapi"]["body_base64"].as_str() {
                    example.body_base64 = bytes.into();
                    example.validate()?;
                }
            }
            if let Some(object) = source.as_object() {
                example.metadata = object
                    .iter()
                    .filter(|(key, _)| {
                        ![
                            "name",
                            "originalRequest",
                            "code",
                            "status",
                            "header",
                            "body",
                            "id",
                            "x-rustapi",
                        ]
                        .contains(&key.as_str())
                    })
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
            }
            r.examples.push(example);
        }
    } else {
        anyhow::ensure!(
            item["response"].is_null(),
            "Response examples must be an array"
        );
    }
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
    inherited: &HttpSettings,
) -> Result<()> {
    anyhow::ensure!(depth < 64, "Folder nesting exceeds 64 levels");
    for item in v.as_array().context("Collection items must be an array")? {
        if item["item"].is_array() {
            let mut f = Folder::new(text(&item["name"]));
            let (settings, profile) =
                imported_profile(&item["protocolProfileBehavior"], inherited)?;
            f.imported_settings = profile;
            (f.description, f.description_metadata) = decode_description(&item["description"]);
            f.auth = auth(&item["auth"], warnings)?;
            f.variable_metadata = values(&item["variable"]);
            f.variables = f
                .variable_metadata
                .clone()
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
                &settings,
            )?;
            order.push(f.id.clone());
            folders.push(f);
        } else {
            let r = request(item, warnings, inherited)?;
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
    if v["openapi"].is_string() {
        let spec = crate::openapi::Specification::parse(std::str::from_utf8(bytes)?)?;
        let (collection, issues) = spec.import()?;
        let diagnostics = issues
            .into_iter()
            .map(|issue| ImportDiagnostic {
                path: issue.path,
                feature: "openapi_review".into(),
                severity: ImportSeverity::Warning,
                disposition: ImportDisposition::Omitted,
                message: issue.message,
            })
            .collect();
        return Ok(ImportResult {
            collection: Some(collection),
            environment: None,
            warnings: vec![],
            diagnostics,
        });
    }
    if v["$kind"] == "collection" {
        return crate::v3::import_document(&v);
    }
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
        let (settings, profile) =
            imported_profile(&v["protocolProfileBehavior"], &HttpSettings::default())?;
        c.imported_settings = profile;
        (c.description, c.description_metadata) = decode_description(&v["info"]["description"]);
        c.auth = auth(&v["auth"], &mut warnings)?;
        if c.auth.auth_type == AuthType::Inherit {
            c.auth.auth_type = AuthType::None;
        }
        c.variable_metadata = values(&v["variable"]);
        c.variables = c
            .variable_metadata
            .clone()
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
            &settings,
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

pub(crate) fn imported_profile_for_v3(
    value: &Value,
) -> Result<(HttpSettings, std::collections::BTreeMap<String, Value>)> {
    imported_profile(value, &HttpSettings::default())
}

pub fn path(path: &std::path::Path) -> Result<ImportResult> {
    if path.is_dir() {
        return crate::v3::import_directory(path);
    }
    let metadata = std::fs::metadata(path)?;
    anyhow::ensure!(metadata.len() <= 64 * 1024 * 1024, "Import exceeds 64 MiB");
    let bytes = std::fs::read(path)?;
    if path
        .extension()
        .is_some_and(|extension| extension == "yaml" || extension == "yml")
    {
        let value = crate::v3::parse_yaml(std::str::from_utf8(&bytes)?)?;
        if value["openapi"].is_string() {
            json(&serde_json::to_vec(&value)?)
        } else {
            crate::v3::import_document(&value)
        }
    } else {
        json(&bytes)
    }
}
