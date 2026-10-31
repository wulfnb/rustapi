//! Scope precedence: global < collection/folder < environment (with local overrides) < iteration < local < vault references.
use crate::models::*;
use anyhow::{bail, Result};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct Variables {
    pub global: BTreeMap<String, String>,
    pub collection: BTreeMap<String, String>,
    pub environment: BTreeMap<String, String>,
    pub local_overrides: BTreeMap<String, String>,
    pub data: BTreeMap<String, String>,
    pub local: BTreeMap<String, String>,
    pub secrets: BTreeMap<String, String>,
}
impl Variables {
    pub fn for_request(r: &ApiRequest, env: Option<&Environment>) -> Self {
        Self {
            collection: r.inherited_variables.clone().into_iter().collect(),
            environment: env
                .map(|e| {
                    e.variables
                        .iter()
                        .filter(|v| v.enabled)
                        .map(|v| (v.key.clone(), v.value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            ..Default::default()
        }
    }
    pub fn merged(&self) -> BTreeMap<String, String> {
        let mut out = self.global.clone();
        out.extend(self.collection.clone());
        out.extend(self.environment.clone());
        out.extend(self.local_overrides.clone());
        out.extend(self.data.clone());
        out.extend(self.local.clone());
        out.extend(
            self.secrets
                .iter()
                .map(|(k, v)| (format!("vault:{k}"), v.clone())),
        );
        out
    }
    pub fn resolve(&self, text: &str) -> Result<String> {
        let vars = self.merged();
        let mut current = text.to_string();
        for _ in 0..16 {
            let mut out = String::new();
            let mut rest = current.as_str();
            let mut found = false;
            while let Some(start) = rest.find("{{") {
                out.push_str(&rest[..start]);
                let after = &rest[start + 2..];
                let Some(end) = after.find("}}") else {
                    bail!("Unclosed variable reference");
                };
                let key = after[..end].trim();
                let value = if let Some(val) = vars.get(key) {
                    val.clone()
                } else if let Some(dynamic) = resolve_dynamic_variable(key) {
                    dynamic
                } else {
                    bail!("Unresolved variable: {key}");
                };
                out.push_str(&value);
                rest = &after[end + 2..];
                found = true;
            }
            out.push_str(rest);
            if !found {
                return Ok(out);
            }
            current = out;
        }
        bail!("Variable expansion is cyclic or exceeds 16 levels")
    }
    pub fn resolve_oauth(&self, config: &OAuthConfig) -> Result<OAuthConfig> {
        let request = ApiRequest {
            url: "http://localhost/".into(),
            auth: AuthConfig {
                auth_type: AuthType::OAuth2,
                oauth: config.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        Ok(self.resolve_request(&request)?.auth.oauth)
    }
    pub fn resolve_request(&self, r: &ApiRequest) -> Result<ApiRequest> {
        let mut r = r.clone();
        r.url = self.resolve(&r.url)?;
        for h in r
            .headers
            .iter_mut()
            .chain(&mut r.query_params)
            .chain(&mut r.path_params)
            .chain(&mut r.form_data)
        {
            if h.enabled {
                h.key = self.resolve(&h.key)?;
                h.value = self.resolve(&h.value)?;
                h.content_type = self.resolve(&h.content_type)?;
            }
        }
        r.url = resolve_path_variables(&r.url, &r.path_params)?;
        r.body = self.resolve(&r.body)?;
        if let Some(graphql) = &mut r.graphql {
            graphql.query = self.resolve(&graphql.query)?;
            graphql.variables = self.resolve(&graphql.variables)?;
            graphql.operation_name = self.resolve(&graphql.operation_name)?;
        }
        r.binary_file = self.resolve(&r.binary_file)?;
        for field in [
            &mut r.auth.bearer_token,
            &mut r.auth.basic_username,
            &mut r.auth.basic_password,
            &mut r.auth.api_key_name,
            &mut r.auth.api_key_value,
            &mut r.auth.jwt.algorithm,
            &mut r.auth.jwt.key,
            &mut r.auth.jwt.key_id,
            &mut r.auth.jwt.claims,
            &mut r.auth.aws.access_key,
            &mut r.auth.aws.secret_key,
            &mut r.auth.aws.session_token,
            &mut r.auth.aws.region,
            &mut r.auth.aws.service,
            &mut r.auth.oauth.authorization_url,
            &mut r.auth.oauth.refresh_url,
            &mut r.auth.oauth.refresh_scope,
            &mut r.auth.oauth.token_url,
            &mut r.auth.oauth.client_id,
            &mut r.auth.oauth.client_secret,
            &mut r.auth.oauth.scope,
            &mut r.auth.oauth.redirect_uri,
            &mut r.auth.oauth.code,
            &mut r.auth.oauth.verifier,
        ] {
            *field = self.resolve(field)?;
        }
        for row in r
            .auth
            .oauth
            .authorization_params
            .iter_mut()
            .chain(&mut r.auth.oauth.token_params)
            .chain(&mut r.auth.oauth.refresh_params)
            .filter(|row| row.enabled)
        {
            row.key = self.resolve(&row.key)?;
            row.value = self.resolve(&row.value)?;
        }
        for field in [
            &mut r.settings.proxy,
            &mut r.settings.ca_file,
            &mut r.settings.identity_file,
        ] {
            *field = self.resolve(field)?;
        }
        for field in [
            &mut r.binary_file,
            &mut r.settings.ca_file,
            &mut r.settings.identity_file,
        ] {
            if !field.is_empty()
                && !r.file_base.as_os_str().is_empty()
                && std::path::Path::new(field.as_str()).is_relative()
            {
                *field = r.file_base.join(field.as_str()).to_string_lossy().into();
            }
        }
        for f in &mut r.form_data {
            if f.file
                && !r.file_base.as_os_str().is_empty()
                && std::path::Path::new(&f.value).is_relative()
            {
                f.value = r.file_base.join(&f.value).to_string_lossy().into();
            }
        }
        Ok(r)
    }
}

pub fn resolve_path_variables(url: &str, params: &[KeyValue]) -> Result<String> {
    if params.is_empty() {
        return Ok(url.into());
    }
    let mut parsed = reqwest::Url::parse(url)?;
    let mut values = BTreeMap::new();
    for row in params.iter().filter(|row| row.enabled) {
        anyhow::ensure!(!row.key.is_empty(), "Empty path variable name");
        anyhow::ensure!(
            values
                .insert(row.key.as_str(), row.value.as_str())
                .is_none(),
            "Duplicate path variable"
        );
    }
    let path = parsed
        .path()
        .split('/')
        .map(|segment| -> Result<String> {
            if let Some(name) = segment.strip_prefix(':') {
                let value = values
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("Unresolved path variable"))?;
                anyhow::ensure!(
                    ![".", ".."].contains(value),
                    "Dot segments cannot be path variable values"
                );
                Ok(urlencoding::encode(value).into_owned())
            } else {
                Ok(segment.into())
            }
        })
        .collect::<Result<Vec<_>>>()?
        .join("/");
    parsed.set_path(&path);
    Ok(parsed.into())
}

pub fn resolve_dynamic_variable(key: &str) -> Option<String> {
    match key {
        "$guid" | "$uuid" => Some(uuid::Uuid::new_v4().to_string()),
        "$timestamp" => Some(chrono::Utc::now().timestamp().to_string()),
        "$isoTimestamp" => Some(chrono::Utc::now().to_rfc3339()),
        "$randomInt" => {
            let n = (uuid::Uuid::new_v4().as_u128() % 1000) + 1;
            Some(n.to_string())
        }
        "$randomEmail" => {
            let id = &uuid::Uuid::new_v4().simple().to_string()[..8];
            Some(format!("user-{id}@example.com"))
        }
        "$randomAlphaNumeric" => {
            let id = &uuid::Uuid::new_v4().simple().to_string()[..8];
            Some(id.to_string())
        }
        "$randomUserName" => {
            let id = &uuid::Uuid::new_v4().simple().to_string()[..6];
            Some(format!("user_{id}"))
        }
        "$randomBoolean" => {
            let b = uuid::Uuid::new_v4().as_u128().is_multiple_of(2);
            Some(b.to_string())
        }
        _ => None,
    }
}

/// Describe variable origins and resolved value for UI inspection and tooltips
pub fn describe_variable(
    key: &str,
    vars: &Variables,
    _environment_name: Option<&str>,
) -> Option<(&'static str, String, bool)> {
    if let Some(v) = vars.secrets.get(key.strip_prefix("vault:").unwrap_or(key)) {
        return Some(("Vault", v.clone(), true));
    }
    if let Some(v) = vars.local.get(key) {
        return Some(("Local", v.clone(), false));
    }
    if let Some(v) = vars.data.get(key) {
        return Some(("Iteration", v.clone(), false));
    }
    if let Some(v) = vars.local_overrides.get(key) {
        return Some(("Environment (local override)", v.clone(), false));
    }
    if let Some(v) = vars.environment.get(key) {
        return Some(("Environment", v.clone(), false));
    }
    if let Some(v) = vars.collection.get(key) {
        return Some(("Collection/Folder", v.clone(), false));
    }
    if let Some(v) = vars.global.get(key) {
        return Some(("Global", v.clone(), false));
    }
    if let Some(dynamic) = resolve_dynamic_variable(key) {
        return Some(("Dynamic", dynamic, false));
    }
    None
}
