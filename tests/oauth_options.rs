mod support;
use base64::{engine::general_purpose::STANDARD, Engine};
use rustapi::{http::*, models::*, variables::Variables};
use serde_json::json;
#[test]
fn basic_client_auth_refresh_endpoint_scope_and_provider_fields_match_wire() {
    let expected = STANDARD.encode("id%3A+space:s%3A%2F%2B%C3%A9");
    let server = support::Server::new(4, move |index, request| {
        if index % 2 == 0 {
            let authorization = request
                .head
                .lines()
                .find_map(|line| {
                    line.split_once(':')
                        .filter(|(key, _)| key.eq_ignore_ascii_case("authorization"))
                        .map(|(_, value)| value.trim())
                })
                .unwrap();
            assert_eq!(authorization, format!("Basic {expected}"));
            let fields: std::collections::HashMap<String, String> =
                serde_urlencoded::from_bytes(&request.body).unwrap();
            assert!(!fields.contains_key("client_id"));
            assert!(!fields.contains_key("client_secret"));
            if index == 0 {
                assert!(request.head.starts_with("POST /token "));
                assert_eq!(fields["audience"], "api");
                assert_eq!(fields["grant_type"], "client_credentials");
                support::Reply::json("{\"access_token\":\"first\",\"refresh_token\":\"refresh-private\",\"expires_in\":0}")
            } else {
                assert!(request.head.starts_with("POST /refresh "));
                assert_eq!(fields["resource"], "refresh-api");
                assert_eq!(fields["scope"], "read");
                assert_eq!(fields["refresh_token"], "refresh-private");
                support::Reply::json("{\"access_token\":\"second\",\"expires_in\":300}")
            }
        } else {
            assert!(request.head.starts_with("GET /api "));
            support::Reply::json("{}")
        }
    });
    let request = ApiRequest {
        url: format!("{}/api", server.url),
        auth: AuthConfig {
            auth_type: AuthType::OAuth2,
            oauth: OAuthConfig {
                token_url: format!("{}/token", server.url),
                refresh_url: format!("{}/refresh", server.url),
                refresh_scope: "read".into(),
                client_id: "id: space".into(),
                client_secret: "s:/+é".into(),
                client_auth: OAuthClientAuth::Basic,
                token_params: vec![KeyValue::new("audience", "api")],
                refresh_params: vec![KeyValue::new("resource", "refresh-api")],
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let service = HttpService::default();
    for _ in 0..2 {
        let response = service.execute(&request, &Cancellation::default());
        assert_eq!(response.status, 200, "{:?}", response.error);
    }
    let logs = serde_json::to_string(&service.debug.snapshot()).unwrap();
    assert!(!logs.contains("refresh-private"));
    assert!(!logs.contains("s:/+é"));
    server.finish();
}
#[test]
fn public_pkce_and_authorization_parameters_are_scoped_and_protected() {
    let server = support::Server::new(2, |index, request| {
        if index == 0 {
            let fields: std::collections::HashMap<String, String> =
                serde_urlencoded::from_bytes(&request.body).unwrap();
            assert_eq!(fields["client_id"], "public-id");
            assert_eq!(fields["code"], "code-private");
            assert_eq!(fields["audience"], "api");
            assert!(!fields.contains_key("client_secret"));
            assert!(!request.head.to_lowercase().contains("authorization:"));
            support::Reply::json("{\"access_token\":\"public-token\"}")
        } else {
            support::Reply::json("{}")
        }
    });
    let config = OAuthConfig {
        authorization_url: "https://auth.example.test/authorize?tenant=one".into(),
        token_url: format!("{}/token", server.url),
        client_id: "public-id".into(),
        client_secret: "unused-private".into(),
        client_auth: OAuthClientAuth::Public,
        grant: "authorization_code".into(),
        code: "code-private".into(),
        verifier: "x".repeat(43),
        redirect_uri: "http://127.0.0.1:3000/callback".into(),
        authorization_params: vec![KeyValue::new("prompt", "login")],
        token_params: vec![KeyValue::new("audience", "{{audience}}")],
        ..Default::default()
    };
    let authorization =
        oauth::authorization_url(&config, "state-value", "challenge-value").unwrap();
    let parsed = reqwest::Url::parse(&authorization).unwrap();
    let pairs: std::collections::HashMap<_, _> = parsed.query_pairs().collect();
    assert_eq!(pairs["prompt"], "login");
    assert_eq!(pairs["state"], "state-value");
    assert_eq!(pairs["code_challenge_method"], "S256");
    let mut vars = Variables::default();
    vars.local.insert("audience".into(), "api".into());
    let config = vars.resolve_oauth(&config).unwrap();
    let request = ApiRequest {
        url: format!("{}/api", server.url),
        auth: AuthConfig {
            auth_type: AuthType::OAuth2,
            oauth: config.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        HttpService::default()
            .execute(&request, &Cancellation::default())
            .status,
        200
    );
    server.finish();
    for reserved in [
        "state",
        "code_challenge",
        "client_id",
        "redirect_uri",
        "scope",
    ] {
        let mut modified = config.clone();
        modified
            .authorization_params
            .push(KeyValue::new(reserved, "override"));
        assert!(oauth::authorization_url(&modified, "state", "challenge").is_err());
    }
    let mut modified = config;
    modified.authorization_url = "https://auth.example.test/authorize?state=wrong".into();
    assert!(oauth::authorization_url(&modified, "state", "challenge").is_err());
}
#[test]
fn oauth_advanced_options_roundtrip_and_redact_provider_credentials() {
    let mut c = Collection::new("OAuth");
    let mut r = ApiRequest::default();
    r.auth.auth_type = AuthType::OAuth2;
    r.auth.oauth = OAuthConfig {
        client_auth: OAuthClientAuth::Basic,
        refresh_url: "https://auth.example.test/refresh".into(),
        refresh_scope: "read".into(),
        authorization_params: vec![KeyValue::new("audience", "api")],
        token_params: vec![KeyValue::new("client_assertion", "private-assertion")],
        refresh_params: vec![KeyValue {
            enabled: false,
            ..KeyValue::new("resource", "disabled")
        }],
        ..Default::default()
    };
    r.url = "https://example.test".into();
    c.requests.push(r);
    let portable = rustapi::export::collection(&c, true).unwrap();
    let copy = rustapi::import::json(&serde_json::to_vec(&portable.document).unwrap())
        .unwrap()
        .collection
        .unwrap();
    assert_eq!(
        serde_json::to_value(&copy.requests[0].auth.oauth).unwrap(),
        serde_json::to_value(&c.requests[0].auth.oauth).unwrap()
    );
    let redactor =
        rustapi::debug::LogRedactor::new(&Variables::default(), Some(&c.requests[0]), None);
    assert!(!serde_json::to_string(
        &redactor.logs(&json!([{"level":"log","args":["private-assertion"]}]))
    )
    .unwrap()
    .contains("private-assertion"));
    let redacted = rustapi::export::collection(&c, false).unwrap();
    assert!(!redacted.document.to_string().contains("private-assertion"));
    let mut old = json!(c.requests[0].auth.oauth);
    for field in [
        "client_auth",
        "refresh_url",
        "refresh_scope",
        "authorization_params",
        "token_params",
        "refresh_params",
    ] {
        old.as_object_mut().unwrap().remove(field);
    }
    let old: OAuthConfig = serde_json::from_value(old).unwrap();
    assert_eq!(old.client_auth, OAuthClientAuth::Body);
    assert!(old.token_params.is_empty());
}
#[test]
fn token_redirect_is_not_followed_and_protected_token_fields_fail_before_io() {
    let server = support::Server::new(1, |_, _| support::Reply {
        status: 307,
        headers: vec![("Location".into(), "http://127.0.0.1:1/leak".into())],
        body: vec![],
        delay_ms: 0,
    });
    let mut request = ApiRequest {
        url: "http://127.0.0.1:1/api-never".into(),
        auth: AuthConfig {
            auth_type: AuthType::OAuth2,
            oauth: OAuthConfig {
                token_url: format!("{}/token", server.url),
                client_id: "id".into(),
                client_secret: "secret".into(),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let service = HttpService::default();
    let response = service.execute(&request, &Cancellation::default());
    assert!(response.error.unwrap().contains("307"));
    server.finish();
    request
        .auth
        .oauth
        .token_params
        .push(KeyValue::new("client_secret", "override"));
    assert!(service
        .execute(&request, &Cancellation::default())
        .error
        .unwrap()
        .contains("managed OAuth"));
}

#[test]
fn callback_rejects_duplicate_and_wrong_path_and_reads_fragmented_headers() {
    use std::io::{Read, Write};
    let mut config = OAuthConfig {
        authorization_url: "https://auth.example.test/authorize".into(),
        client_id: "id".into(),
        ..Default::default()
    };
    let authorization = oauth::begin_authorization(&mut config).unwrap();
    let state = authorization.state.clone();
    let callback = config.redirect_uri.clone();
    let handle = std::thread::spawn(move || authorization.wait(&Cancellation::default()).unwrap());
    let client = reqwest::blocking::Client::new();
    assert_eq!(
        client
            .get(format!("{callback}?state={state}&state={state}&code=no"))
            .send()
            .unwrap()
            .status(),
        400
    );
    let uri = reqwest::Url::parse(&callback).unwrap();
    let authority = format!("127.0.0.1:{}", uri.port().unwrap());
    assert_eq!(
        client
            .get(format!("http://{authority}/wrong?state={state}&code=no"))
            .send()
            .unwrap()
            .status(),
        400
    );
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", uri.port().unwrap())).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    write!(
        stream,
        "GET /callback?state={state}&code=valid-fragmented HTTP/1.1\r\n"
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(25));
    stream.write_all(b"Host: localhost\r\n\r\n").unwrap();
    let mut result = String::new();
    stream.read_to_string(&mut result).unwrap();
    assert!(result.starts_with("HTTP/1.1 200"));
    assert_eq!(handle.join().unwrap(), "valid-fragmented");
}
#[test]
fn oauth_url_redaction_preserves_unrelated_encoded_pairs_and_templates() {
    let input = "{{host}}/token?api_key=private-url&audience={{audience}}&encoded=a%2Fb#note";
    let output = rustapi::secrets::redact_url(input);
    assert!(!output.contains("private-url"));
    assert!(output.contains("audience={{audience}}&encoded=a%2Fb#note"));
    let mut request = ApiRequest::default();
    request.auth.oauth.token_url = input.into();
    assert!(!rustapi::secrets::redact_request(&request)
        .auth
        .oauth
        .token_url
        .contains("private-url"));
    assert_eq!(request.auth.oauth.token_url, input);
    let mut history = RequestHistory::new(5);
    history.add(
        request,
        Some(ApiResponse::error(format!(
            "OAuth exchange failed at {input}"
        ))),
    );
    assert!(!serde_json::to_string(&history)
        .unwrap()
        .contains("private-url"));
}

#[test]
fn empty_or_invalid_acquired_bearer_tokens_fail_before_api_request() {
    for token in ["", "token with spaces"] {
        let body = json!({"access_token":token}).to_string();
        let server = support::Server::new(1, move |_, _| support::Reply::json(&body));
        let request = ApiRequest {
            url: "http://127.0.0.1:1/never".into(),
            auth: AuthConfig {
                auth_type: AuthType::OAuth2,
                oauth: OAuthConfig {
                    token_url: format!("{}/token", server.url),
                    client_id: "id".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let response = HttpService::default().execute(&request, &Cancellation::default());
        assert!(response
            .error
            .unwrap()
            .contains("Invalid OAuth Bearer token"));
        server.finish();
    }
}
