//! Explicit provider bindings resolved to in-memory {{vault:ext/alias}} values.
use crate::{
    http::{Cancellation, HttpService},
    models::*,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
#[derive(Clone, Serialize, Deserialize)]
pub enum ProviderKind {
    VaultKv2,
    AwsSecretsManager,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Provider {
    pub name: String,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub token_env: String,
    pub namespace: String,
    pub mount: String,
    pub region: String,
}
impl Default for Provider {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: ProviderKind::VaultKv2,
            endpoint: String::new(),
            token_env: "VAULT_TOKEN".into(),
            namespace: String::new(),
            mount: "secret".into(),
            region: String::new(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub alias: String,
    pub provider: String,
    pub secret_id: String,
    pub pointer: String,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub providers: Vec<Provider>,
    pub bindings: Vec<Binding>,
}
fn name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-/.:".contains(&b))
}
pub fn load(
    root: &Path,
    service: &HttpService,
    cancel: &Cancellation,
) -> Result<BTreeMap<String, String>> {
    let path = crate::storage::owned_path(root, &["secret-providers.json"])?;
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    ensure!(
        service.policy()?.allow_external_secrets,
        "Workspace policy disables external secret providers"
    );
    let config: Config = serde_json::from_str(&crate::tools_cli::read(&path, 1024 * 1024)?)?;
    ensure!(
        config.providers.len() <= 20 && config.bindings.len() <= 50,
        "External secret binding limit exceeded"
    );
    let mut names = BTreeSet::new();
    for provider in &config.providers {
        ensure!(
            name(&provider.name) && names.insert(provider.name.clone()),
            "Provider names must be valid and unique"
        );
    }
    let mut output = BTreeMap::new();
    for binding in &config.bindings {
        ensure!(!cancel.is_cancelled(), "Secret refresh cancelled");
        ensure!(
            name(&binding.alias)
                && binding.secret_id.len() <= 1024
                && !output.contains_key(&format!("ext/{}", binding.alias)),
            "Invalid or duplicate secret binding"
        );
        let provider = config
            .providers
            .iter()
            .find(|p| p.name == binding.provider)
            .context("Secret provider binding is missing")?;
        let mut request = ApiRequest::default();
        request.settings.timeout_ms = 5000;
        request.settings.follow_redirects = false;
        request.settings.use_cookies = false;
        request.settings.max_response_bytes = 1024 * 1024;
        let value = match provider.kind {
            ProviderKind::VaultKv2 => {
                ensure!(
                    !provider.token_env.is_empty()
                        && provider
                            .token_env
                            .bytes()
                            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
                    "Vault credentials require an environment variable name"
                );
                ensure!(
                    name(&provider.mount) && !provider.mount.contains('/'),
                    "Vault mount must be a single path component"
                );
                let mut url = endpoint(&provider.endpoint)?;
                {
                    let mut path = url
                        .path_segments_mut()
                        .map_err(|_| anyhow::anyhow!("Invalid Vault endpoint"))?;
                    path.pop_if_empty()
                        .push("v1")
                        .push(&provider.mount)
                        .push("data");
                    for segment in binding.secret_id.split('/') {
                        ensure!(
                            !segment.is_empty() && segment != "." && segment != "..",
                            "Invalid Vault secret path"
                        );
                        path.push(segment);
                    }
                }
                request.url = url.into();
                request.headers.push(KeyValue::new(
                    "X-Vault-Token",
                    std::env::var(&provider.token_env)
                        .context("Vault token environment variable is missing")?,
                ));
                if !provider.namespace.is_empty() {
                    request
                        .headers
                        .push(KeyValue::new("X-Vault-Namespace", &provider.namespace));
                }
                let response = service.execute(&request, cancel);
                ensure!(
                    response.error.is_none() && response.is_success(),
                    "Vault provider request failed"
                );
                let value: serde_json::Value = serde_json::from_slice(&response.raw_body)
                    .map_err(|_| anyhow::anyhow!("Invalid Vault provider response"))?;
                value
                    .pointer("/data/data")
                    .context("Vault KV v2 response has no secret data")?
                    .clone()
            }
            ProviderKind::AwsSecretsManager => {
                ensure!(
                    !provider.region.is_empty()
                        && provider
                            .region
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                    "AWS provider needs a region"
                );
                request.url = endpoint(&if provider.endpoint.is_empty() {
                    format!("https://secretsmanager.{}.amazonaws.com", provider.region)
                } else {
                    provider.endpoint.clone()
                })?
                .into();
                request.method = HttpMethod::POST;
                request.body_type = BodyType::Json;
                request.headers = vec![
                    KeyValue::new("Content-Type", "application/x-amz-json-1.1"),
                    KeyValue::new("X-Amz-Target", "secretsmanager.GetSecretValue"),
                ];
                request.body = serde_json::json!({"SecretId":binding.secret_id}).to_string();
                request.auth.auth_type = AuthType::AwsSigV4;
                request.auth.aws = AwsConfig {
                    access_key: std::env::var("AWS_ACCESS_KEY_ID")
                        .context("AWS_ACCESS_KEY_ID is missing")?,
                    secret_key: std::env::var("AWS_SECRET_ACCESS_KEY")
                        .context("AWS_SECRET_ACCESS_KEY is missing")?,
                    session_token: std::env::var("AWS_SESSION_TOKEN").unwrap_or_default(),
                    region: provider.region.clone(),
                    service: "secretsmanager".into(),
                };
                let response = service.execute(&request, cancel);
                ensure!(
                    response.error.is_none() && response.is_success(),
                    "AWS secret provider request failed"
                );
                let value: serde_json::Value = serde_json::from_slice(&response.raw_body)
                    .map_err(|_| anyhow::anyhow!("Invalid AWS secret response"))?;
                let text = if let Some(text) = value["SecretString"].as_str() {
                    text.to_owned()
                } else {
                    use base64::Engine;
                    String::from_utf8(
                        base64::engine::general_purpose::STANDARD.decode(
                            value["SecretBinary"]
                                .as_str()
                                .context("Missing AWS secret value")?,
                        )?,
                    )
                    .context("AWS binary secret must be UTF-8")?
                };
                if binding.pointer.is_empty() {
                    serde_json::Value::String(text)
                } else {
                    serde_json::from_str(&text).map_err(|_| {
                        anyhow::anyhow!("AWS secret must be JSON for pointer extraction")
                    })?
                }
            }
        };
        let value = value
            .pointer(&binding.pointer)
            .context("Secret JSON Pointer was not found")?;
        let text = value
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| value.to_string());
        ensure!(text.len() <= 256 * 1024, "Resolved secret exceeds 256 KiB");
        output.insert(format!("ext/{}", binding.alias), text);
    }
    Ok(output)
}
fn endpoint(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value)?;
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && (url.scheme() == "https"
                || url.scheme() == "http"
                    && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))),
        "Secret provider endpoint must use HTTPS or loopback HTTP without URL credentials"
    );
    Ok(url)
}
