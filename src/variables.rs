//! Scope precedence: collection/folder < environment < iteration < local < vault references.
use crate::models::*;
use anyhow::{bail, Result};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct Variables {
    pub collection: BTreeMap<String, String>,
    pub environment: BTreeMap<String, String>,
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
        let mut out = self.collection.clone();
        out.extend(self.environment.clone());
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
                let Some(value) = vars.get(key) else {
                    bail!("Unresolved variable: {key}");
                };
                out.push_str(value);
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
    pub fn resolve_request(&self, r: &ApiRequest) -> Result<ApiRequest> {
        let mut r = r.clone();
        r.url = self.resolve(&r.url)?;
        for h in r
            .headers
            .iter_mut()
            .chain(&mut r.query_params)
            .chain(&mut r.form_data)
        {
            if h.enabled {
                h.key = self.resolve(&h.key)?;
                h.value = self.resolve(&h.value)?;
            }
        }
        r.body = self.resolve(&r.body)?;
        r.binary_file = self.resolve(&r.binary_file)?;
        for field in [
            &mut r.auth.bearer_token,
            &mut r.auth.basic_username,
            &mut r.auth.basic_password,
            &mut r.auth.api_key_name,
            &mut r.auth.api_key_value,
            &mut r.auth.oauth.authorization_url,
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
