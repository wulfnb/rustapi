//! HTTP client for executing API requests

use crate::models::{ApiRequest, ApiResponse, BodyType, Environment, HttpMethod, KeyValue};
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::time::Instant;

/// Execute an HTTP request and return the response
pub fn execute_request(request: &ApiRequest, environment: Option<&Environment>) -> ApiResponse {
    let client = match Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => return ApiResponse::error(format!("Failed to create HTTP client: {}", e)),
    };

    // Substitute environment variables
    let url = if let Some(env) = environment {
        env.substitute(&request.url)
    } else {
        request.url.clone()
    };

    // Build query params including API key if needed
    let mut query_params = request.query_params.clone();
    if let Some((key, value)) = request.auth.get_api_key_query_param() {
        query_params.push(KeyValue::new(key, value));
    }

    // Build the URL with query parameters
    let url_with_params = build_url_with_params(&url, &query_params, environment);

    // Create the request builder
    let mut builder: RequestBuilder = match request.method {
        HttpMethod::GET => client.get(&url_with_params),
        HttpMethod::POST => client.post(&url_with_params),
        HttpMethod::PUT => client.put(&url_with_params),
        HttpMethod::DELETE => client.delete(&url_with_params),
        HttpMethod::PATCH => client.patch(&url_with_params),
        HttpMethod::HEAD => client.head(&url_with_params),
        HttpMethod::OPTIONS => client.request(reqwest::Method::OPTIONS, &url_with_params),
    };

    // Add headers
    let mut headers = HeaderMap::new();
    
    // Add auth header if applicable
    if let Some((key, value)) = request.auth.get_auth_header() {
        let key_sub = if let Some(env) = environment {
            env.substitute(&key)
        } else {
            key
        };
        let value_sub = if let Some(env) = environment {
            env.substitute(&value)
        } else {
            value
        };
        
        if let (Ok(name), Ok(val)) = (
            HeaderName::try_from(key_sub.as_str()),
            HeaderValue::try_from(&value_sub),
        ) {
            headers.insert(name, val);
        }
    }

    // Add user headers
    for header in &request.headers {
        if header.enabled && !header.key.is_empty() {
            let value = if let Some(env) = environment {
                env.substitute(&header.value)
            } else {
                header.value.clone()
            };

            if let (Ok(name), Ok(val)) = (
                HeaderName::try_from(header.key.as_str()),
                HeaderValue::try_from(&value),
            ) {
                headers.insert(name, val);
            }
        }
    }

    // Add body based on body type
    match request.body_type {
        BodyType::None => {}
        BodyType::Json => {
            let body = if let Some(env) = environment {
                env.substitute(&request.body)
            } else {
                request.body.clone()
            };

            if !headers.contains_key("content-type") {
                headers.insert(
                    HeaderName::from_static("content-type"),
                    HeaderValue::from_static("application/json"),
                );
            }
            builder = builder.body(body);
        }
        BodyType::Text => {
            let body = if let Some(env) = environment {
                env.substitute(&request.body)
            } else {
                request.body.clone()
            };

            if !headers.contains_key("content-type") {
                headers.insert(
                    HeaderName::from_static("content-type"),
                    HeaderValue::from_static("text/plain"),
                );
            }
            builder = builder.body(body);
        }
        BodyType::UrlEncoded => {
            let mut form_data: Vec<(String, String)> = vec![];
            for kv in &request.form_data {
                if kv.enabled && !kv.key.is_empty() {
                    let value = if let Some(env) = environment {
                        env.substitute(&kv.value)
                    } else {
                        kv.value.clone()
                    };
                    form_data.push((kv.key.clone(), value));
                }
            }
            builder = builder.form(&form_data);
        }
        BodyType::FormData => {
            let mut form = reqwest::blocking::multipart::Form::new();
            for kv in &request.form_data {
                if kv.enabled && !kv.key.is_empty() {
                    let value = if let Some(env) = environment {
                        env.substitute(&kv.value)
                    } else {
                        kv.value.clone()
                    };
                    form = form.text(kv.key.clone(), value);
                }
            }
            builder = builder.multipart(form);
        }
        BodyType::Binary => {
            builder = builder.body(request.body.clone());
        }
    }

    builder = builder.headers(headers);

    // Execute request and time it
    let start = Instant::now();
    let result = builder.send();
    let elapsed = start.elapsed().as_millis();

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let status_text = response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown")
                .to_string();

            // Collect headers
            let response_headers: Vec<(String, String)> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            // Get body
            let body = response.text().unwrap_or_default();
            let size = body.len();

            ApiResponse {
                status,
                status_text,
                headers: response_headers,
                body,
                size,
                time_ms: elapsed,
                error: None,
            }
        }
        Err(e) => ApiResponse {
            error: Some(format!("Request failed: {}", e)),
            time_ms: elapsed,
            ..Default::default()
        },
    }
}

/// Build URL with query parameters
fn build_url_with_params(
    base_url: &str,
    params: &[KeyValue],
    environment: Option<&Environment>,
) -> String {
    let enabled_params: Vec<_> = params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .collect();

    if enabled_params.is_empty() {
        return base_url.to_string();
    }

    let query_string: String = enabled_params
        .iter()
        .map(|p| {
            let value = if let Some(env) = environment {
                env.substitute(&p.value)
            } else {
                p.value.clone()
            };
            format!(
                "{}={}",
                urlencoding::encode(&p.key),
                urlencoding::encode(&value)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    if base_url.contains('?') {
        format!("{}&{}", base_url, query_string)
    } else {
        format!("{}?{}", base_url, query_string)
    }
}

/// Format JSON body for display (pretty print)
pub fn format_json(json_str: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_else(|_| json_str.to_string()),
        Err(_) => json_str.to_string(),
    }
}

