use crate::models::{ApiRequest, BodyType};
use anyhow::{Context, Result};
use std::collections::HashSet;

/// Configured/auth/body headers. Transport-managed Host, length and cookies may follow.
pub fn effective_headers(request: &ApiRequest) -> Result<Vec<(String, String)>> {
    let mut headers: Vec<(String, String)> = vec![];
    for field in request
        .headers
        .iter()
        .filter(|field| field.enabled && !field.key.is_empty())
    {
        let name = reqwest::header::HeaderName::try_from(field.key.as_str())
            .context("Invalid header name")?;
        reqwest::header::HeaderValue::try_from(field.value.as_str())
            .context("Invalid header value")?;
        if request.body_type == BodyType::FormData && name == reqwest::header::CONTENT_TYPE {
            continue;
        }
        headers.push((name.as_str().into(), field.value.clone()));
    }
    let explicit: HashSet<_> = headers.iter().map(|(key, _)| key.clone()).collect();
    let generated_auth = if request.auth.auth_type == crate::models::AuthType::Jwt
        && !explicit.contains("authorization")
    {
        Some((
            "authorization".into(),
            format!("Bearer {}", super::auth::jwt(&request.auth.jwt)?),
        ))
    } else {
        request.auth.get_auth_header()
    };
    if let Some((name, value)) = generated_auth {
        let name = reqwest::header::HeaderName::try_from(name.as_str())
            .context("Invalid auth header name")?
            .as_str()
            .to_string();
        if !explicit.contains(&name) {
            reqwest::header::HeaderValue::try_from(value.as_str())
                .context("Invalid auth header value")?;
            headers.insert(0, (name, value));
        }
    }
    if matches!(
        request.body_type,
        BodyType::Json | BodyType::Text | BodyType::Xml | BodyType::Html | BodyType::Binary
    ) && !explicit.contains("content-type")
    {
        headers.push((
            "content-type".into(),
            request.body_type.content_type().unwrap_or_default().into(),
        ));
    }
    for singleton in [
        "authorization",
        "proxy-authorization",
        "host",
        "content-length",
        "content-type",
    ] {
        anyhow::ensure!(
            headers.iter().filter(|(key, _)| key == singleton).count() <= 1,
            "Duplicate single-valued header: {singleton}"
        );
    }
    Ok(headers)
}
