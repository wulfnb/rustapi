//! Request authentication over the final encoded URL, headers and body.
use crate::models::{AuthConfig, AuthType, AwsConfig, JwtConfig};
use anyhow::{bail, Context, Result};
use reqwest::{header, Client, Request, Response};
use std::time::SystemTime;

pub fn jwt(config: &JwtConfig) -> Result<String> {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use std::str::FromStr;
    anyhow::ensure!(
        config.claims.len() <= 256 * 1024,
        "JWT claims exceed 256 KiB"
    );
    anyhow::ensure!(
        config.key.len() <= 64 * 1024,
        "JWT signing key exceeds 64 KiB"
    );
    let claims: serde_json::Value =
        serde_json::from_str(&config.claims).context("JWT claims must be valid JSON")?;
    anyhow::ensure!(claims.is_object(), "JWT claims must be an object");
    let algorithm = Algorithm::from_str(&config.algorithm).context("Unsupported JWT algorithm")?;
    let key = match algorithm {
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
            let minimum = match algorithm {
                Algorithm::HS256 => 32,
                Algorithm::HS384 => 48,
                _ => 64,
            };
            anyhow::ensure!(
                config.key.len() >= minimum,
                "JWT HMAC key is too short for the selected algorithm"
            );
            EncodingKey::from_secret(config.key.as_bytes())
        }
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::PS256
        | Algorithm::PS384
        | Algorithm::PS512 => EncodingKey::from_rsa_pem(config.key.as_bytes())
            .context("Invalid JWT RSA private key")?,
        Algorithm::ES256 | Algorithm::ES384 => {
            EncodingKey::from_ec_pem(config.key.as_bytes()).context("Invalid JWT EC private key")?
        }
        Algorithm::EdDSA => EncodingKey::from_ed_pem(config.key.as_bytes())
            .context("Invalid JWT Ed25519 private key")?,
        _ => bail!("Unsupported JWT algorithm"),
    };
    let mut header = Header::new(algorithm);
    if !config.key_id.is_empty() {
        header.kid = Some(config.key_id.clone());
    }
    jsonwebtoken::encode(&header, &claims, &key).context("JWT signing failed")
}

pub fn sign_aws(request: &mut Request, config: &AwsConfig, time: SystemTime) -> Result<()> {
    request.url_mut().set_fragment(None);
    use aws_sigv4::http_request::{
        sign, PayloadChecksumKind, PercentEncodingMode, SignableBody, SignableRequest,
        SigningSettings, UriPathNormalizationMode,
    };
    anyhow::ensure!(
        !config.access_key.is_empty() && !config.secret_key.is_empty(),
        "AWS access key and secret key are required"
    );
    anyhow::ensure!(
        !config.region.is_empty() && !config.service.is_empty(),
        "AWS region and service are required"
    );
    let identity = aws_credential_types::Credentials::new(
        &config.access_key,
        &config.secret_key,
        (!config.session_token.is_empty()).then(|| config.session_token.clone()),
        None,
        "RustAPI",
    )
    .into();
    let mut settings = SigningSettings::default();
    if config.service == "s3" {
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        settings.uri_path_normalization_mode = UriPathNormalizationMode::Disabled;
        settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;
    }
    let parameters = aws_sigv4::sign::v4::SigningParams::builder()
        .identity(&identity)
        .region(&config.region)
        .name(&config.service)
        .time(time)
        .settings(settings)
        .build()?
        .into();
    let body = request
        .body()
        .map(|body| {
            body.as_bytes()
                .context("AWS signing requires a buffered request body")
        })
        .transpose()?
        .unwrap_or_default();
    let signable = SignableRequest::new(
        request.method().as_str(),
        request.url().as_str(),
        request
            .headers()
            .iter()
            .map(|(name, value)| Ok((name.as_str(), std::str::from_utf8(value.as_bytes())?)))
            .collect::<Result<Vec<_>>>()?
            .into_iter(),
        SignableBody::Bytes(body),
    )?;
    let (instructions, _) = sign(signable, &parameters)?.into_parts();
    let headers = instructions
        .headers()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect::<Vec<_>>();
    for (name, value) in headers {
        let mut value = header::HeaderValue::from_str(&value)?;
        value.set_sensitive(true);
        request
            .headers_mut()
            .insert(header::HeaderName::from_bytes(name.as_bytes())?, value);
    }
    Ok(())
}

async fn buffer(request: &mut Request) -> Result<()> {
    use http_body_util::BodyExt;
    if let Some(body) = request.body_mut().take() {
        let bytes = http_body_util::Limited::new(body, 64 * 1024 * 1024)
            .collect()
            .await
            .map_err(|_| anyhow::anyhow!("Signed request body is unreadable or exceeds 64 MiB"))?
            .to_bytes();
        *request.body_mut() = Some(bytes.into());
    }
    Ok(())
}

/// Sign each redirect hop; never forward helper credentials to another origin.
/// `client` must have automatic redirects disabled.
pub async fn send(
    client: &Client,
    mut request: Request,
    auth: &AuthConfig,
    follow: bool,
) -> Result<Response> {
    buffer(&mut request).await?;
    anyhow::ensure!(
        request.url().username().is_empty() && request.url().password().is_none(),
        "Signed URLs cannot contain userinfo"
    );
    let origin = request.url().origin();
    for hop in 0..=10 {
        let mut outgoing = request
            .try_clone()
            .context("Request body cannot be replayed")?;
        if auth.auth_type == AuthType::AwsSigV4 {
            sign_aws(&mut outgoing, &auth.aws, SystemTime::now())?;
        }
        let mut response = client
            .execute(outgoing)
            .await
            .context("Send authenticated request")?;
        if auth.auth_type == AuthType::Digest
            && response.status() == reqwest::StatusCode::UNAUTHORIZED
        {
            let challenge = response
                .headers()
                .get_all(header::WWW_AUTHENTICATE)
                .iter()
                .filter_map(|value| value.to_str().ok())
                .find(|value| {
                    value
                        .get(..7)
                        .is_some_and(|p| p.eq_ignore_ascii_case("Digest "))
                })
                .context("Server did not offer Digest authentication")?;
            let mut prompt = digest_auth::parse(challenge).context("Invalid Digest challenge")?;
            let url = request.url();
            let uri = format!(
                "{}{}",
                url.path(),
                url.query()
                    .map(|query| format!("?{query}"))
                    .unwrap_or_default()
            );
            let body = request
                .body()
                .and_then(|body| body.as_bytes())
                .unwrap_or_default();
            let context = digest_auth::AuthContext::new_with_method(
                &auth.basic_username,
                &auth.basic_password,
                &uri,
                Some(body),
                request.method().as_str().into(),
            );
            let authorization = prompt
                .respond(&context)
                .context("Digest authentication failed")?
                .to_string();
            let mut retry = request
                .try_clone()
                .context("Digest request body cannot be replayed")?;
            let mut header = header::HeaderValue::from_str(&authorization)?;
            header.set_sensitive(true);
            retry.headers_mut().insert(header::AUTHORIZATION, header);
            drop(response);
            response = client
                .execute(retry)
                .await
                .context("Send Digest response")?;
        }
        let redirect = matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308);
        if !follow || !redirect {
            return Ok(response);
        }
        let Some(location) = response.headers().get(header::LOCATION) else {
            return Ok(response);
        };
        let next = request.url().join(location.to_str()?)?;
        if next.origin() != origin {
            return Ok(response);
        }
        anyhow::ensure!(hop < 10, "Too many authenticated redirects");
        let to_get = response.status().as_u16() == 303 && request.method() != reqwest::Method::HEAD
            || matches!(response.status().as_u16(), 301 | 302)
                && request.method() == reqwest::Method::POST;
        if to_get {
            *request.method_mut() = reqwest::Method::GET;
            *request.body_mut() = None;
            for name in [
                header::CONTENT_LENGTH,
                header::CONTENT_TYPE,
                header::TRANSFER_ENCODING,
            ] {
                request.headers_mut().remove(name);
            }
        }
        *request.url_mut() = next;
    }
    bail!("Too many authenticated redirects")
}
