//! HTTP client for executing API requests

use crate::models::{ApiRequest, ApiResponse, BodyType, Environment, HttpMethod, KeyValue};
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::time::Instant;

/// Execute an HTTP request and return the response
pub fn execute_request(request: &ApiRequest, environment: Option<&Environment>) -> ApiResponse {
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build();
    execute_with_client(request, environment, client)
}

fn execute_with_client(
    request: &ApiRequest,
    environment: Option<&Environment>,
    client: Result<Client, reqwest::Error>,
) -> ApiResponse {
    let client = match client {
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
    // Substitute credentials before Basic auth encodes them as Base64.
    let mut auth = request.auth.clone();
    if let Some(env) = environment {
        auth.basic_username = env.substitute(&auth.basic_username);
        auth.basic_password = env.substitute(&auth.basic_password);
    }
    if let Some((key, value)) = auth.get_auth_header() {
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

    let (base, fragment) = base_url.split_once('#').unwrap_or((base_url, ""));
    let separator = if base.ends_with('?') || (base.contains('?') && base.ends_with('&')) {
        ""
    } else if base.contains('?') {
        "&"
    } else {
        "?"
    };
    let suffix = if base_url.contains('#') {
        format!("#{fragment}")
    } else {
        String::new()
    };
    format!("{base}{separator}{query_string}{suffix}")
}

/// Format JSON body for display (pretty print)
pub fn format_json(json_str: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(value) => format!("{value:#}"),
        Err(_) => json_str.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_configuration_failure_is_reported() {
        let client = Client::builder().user_agent("invalid\nvalue").build();
        assert!(client.is_err());
        let response = execute_with_client(&ApiRequest::default(), None, client);
        assert!(response
            .error
            .unwrap()
            .contains("Failed to create HTTP client"));
    }

    macro_rules! query_case {
        ($name:ident, $base:expr, $expected:expr) => {
            #[test]
            fn $name() {
                assert_eq!(
                    build_url_with_params($base, &[KeyValue::new("key", "a b")], None),
                    $expected
                );
            }
        };
    }
    query_case!(
        query_on_plain_url,
        "https://test/path",
        "https://test/path?key=a%20b"
    );
    query_case!(
        query_before_fragment,
        "https://test/path#section",
        "https://test/path?key=a%20b#section"
    );
    query_case!(
        query_after_existing_param,
        "https://test/path?first=1",
        "https://test/path?first=1&key=a%20b"
    );
    query_case!(
        query_after_empty_query,
        "https://test/path?",
        "https://test/path?key=a%20b"
    );
    query_case!(
        query_after_trailing_separator,
        "https://test/path?first=1&",
        "https://test/path?first=1&key=a%20b"
    );
    query_case!(
        ampersand_in_path_is_not_a_query,
        "https://test/path&",
        "https://test/path&?key=a%20b"
    );

    #[test]
    fn no_enabled_params_preserve_url_exactly() {
        let base = "https://test/path?existing=1#fragment";
        assert_eq!(build_url_with_params(base, &[], None), base);
        assert_eq!(
            build_url_with_params(
                base,
                &[KeyValue {
                    enabled: false,
                    ..KeyValue::new("key", "value")
                }],
                None
            ),
            base
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/http.rs"]
mod request_tests;
