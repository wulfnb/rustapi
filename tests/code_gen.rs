use rustapi::{http::*, models::*};

#[test]
fn every_generator_handles_auth_body_fields_and_query_parameters() {
    let mut env = Environment::new("test");
    env.variables.push(KeyValue::new("value", "resolved"));
    for language in CodeGenLanguage::all() {
        assert!(!language.file_extension().is_empty());
        assert!(!language.as_str().is_empty());
        for auth_type in AuthType::all() {
            for body_type in [
                BodyType::None,
                BodyType::Json,
                BodyType::Text,
                BodyType::FormData,
                BodyType::UrlEncoded,
                BodyType::Binary,
            ] {
                let request = ApiRequest {
                    url: "https://test/resource".into(),
                    method: HttpMethod::POST,
                    settings: HttpSettings {
                        follow_redirects: *auth_type != AuthType::AwsSigV4,
                        ..Default::default()
                    },
                    body_type,
                    body: "{\"name\":\"{{value}}\"}".into(),
                    headers: vec![KeyValue::new("x-value", "{{value}}"), KeyValue::default()],
                    query_params: vec![KeyValue::new("query", "{{value}}"), KeyValue::default()],
                    form_data: vec![KeyValue::new("field", "{{value}}"), KeyValue::default()],
                    auth: AuthConfig {
                        auth_type: auth_type.clone(),
                        bearer_token: "{{value}}".into(),
                        basic_username: "{{value}}".into(),
                        basic_password: "secret".into(),
                        api_key_name: "x-api-key".into(),
                        api_key_value: "{{value}}".into(),
                        jwt: JwtConfig {
                            key: "0123456789abcdef0123456789abcdef".into(), // gitleaks:allow -- public synthetic HMAC test key
                            ..Default::default()
                        },
                        aws: AwsConfig {
                            access_key: "test".into(),
                            secret_key: "secret".into(),
                            region: "us-east-1".into(),
                            service: "s3".into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let code = generate_code(&request, *language, Some(&env));
                if matches!(auth_type, AuthType::Inherit | AuthType::OAuth2) {
                    assert!(code.contains("Resolve inherited auth"));
                    continue;
                }
                if matches!(auth_type, AuthType::AwsSigV4 | AuthType::Digest)
                    && *language != CodeGenLanguage::Curl
                {
                    assert!(code.contains("require the cURL target"));
                    continue;
                }
                if *language == CodeGenLanguage::Http {
                    assert!(code.starts_with("POST /resource?"));
                    assert!(code.contains("host: test"));
                } else {
                    assert!(code.contains("https://test/resource"));
                }
                assert!(code.contains("resolved"));
                assert!(!code.contains("{{value}}"));
                let without_env = generate_code(&request, *language, None);
                assert!(without_env.contains("Unresolved variable: value"));
            }
        }
    }
}

fn sample() -> (ApiRequest, Environment) {
    let mut env = Environment::new("test");
    env.variables = vec![
        KeyValue::new("host", "https://api.test"),
        KeyValue::new("token", "abc"),
    ];
    let mut request = ApiRequest {
        url: "{{host}}/users".into(),
        method: HttpMethod::POST,
        body_type: BodyType::Json,
        body: "{\"name\":\"Ada\"}".into(),
        ..Default::default()
    };
    request.auth.auth_type = AuthType::BearerToken;
    request.auth.bearer_token = "{{token}}".into();
    request.headers = vec![KeyValue {
        enabled: false,
        ..KeyValue::new("x-disabled", "hidden")
    }];
    (request, env)
}

macro_rules! language_case {
    ($name:ident, $language:ident, $expected:expr) => {
        #[test]
        fn $name() {
            let (request, env) = sample();
            let code = generate_code(&request, CodeGenLanguage::$language, Some(&env));
            assert!(code.contains("https://api.test/users"));
            assert!(code.contains("abc"));
            assert!(!code.contains("{{host}}"));
            assert!(!code.contains("x-disabled"));
            assert!(code.contains($expected), "{code}");
        }
    };
}
language_case!(curl_post_with_environment, Curl, "-X POST");
language_case!(python_post_with_environment, Python, "requests.post(url");
language_case!(
    javascript_post_with_environment,
    JavaScript,
    "method: \"POST\""
);
language_case!(
    rust_post_with_environment,
    Rust,
    "reqwest::Method::POST, \"https://api.test/users\""
);

#[test]
fn curl_escapes_single_quotes_in_body() {
    let (mut request, env) = sample();
    request.body = "{\"name\":\"O'Reilly\"}".into();
    let code = generate_code(&request, CodeGenLanguage::Curl, Some(&env));
    assert!(code.contains("O'\\''Reilly"));
}

#[test]
fn curl_encodes_query_and_preserves_existing_parameters() {
    let mut request = ApiRequest {
        url: "https://api.test?first=1".into(),
        ..Default::default()
    };
    request.query_params = vec![KeyValue::new("name", "Ada Lovelace")];
    let code = generate_code(&request, CodeGenLanguage::Curl, None);
    assert!(code.contains("?first=1&name=Ada%20Lovelace"));
}

language_case!(go_post_with_environment, Go, "http.NewRequestWithContext");
language_case!(csharp_post_with_environment, CSharp, "HttpRequestMessage");
language_case!(java_post_with_environment, Java, "HttpRequest.newBuilder");
language_case!(dart_post_with_environment, Dart, "client.openUrl");

#[test]
fn raw_http_uses_wire_target_host_and_utf8_byte_length() {
    let request = ApiRequest {
        url: "https://api.test:8443/path?q=a%20b#ignored".into(),
        method: HttpMethod::POST,
        body_type: BodyType::Text,
        body: "🦀".into(),
        ..Default::default()
    };
    let code = generate_code(&request, CodeGenLanguage::Http, None);
    assert!(code.starts_with("POST /path?q=a%20b HTTP/1.1\r\nhost: api.test:8443\r\n"));
    assert!(code.contains("content-length: 4\r\n"));
    assert!(code.ends_with("\r\n\r\n🦀"));
    assert!(!code.contains("#ignored"));
}
#[test]
fn unsupported_sdk_options_fail_explicitly_and_template_data_is_not_reexpanded() {
    let mut request = ApiRequest {
        url: "https://example.test".into(),
        body_type: BodyType::Text,
        body: "@@URL@@ ${literal} \u{1} Unicode 🦀".into(),
        headers: vec![
            KeyValue::new("X-Test", "first"),
            KeyValue::new("X-Test", "second"),
        ],
        ..Default::default()
    };
    assert!(generate_code(&request, CodeGenLanguage::CSharp, None).contains("combine repeated"));
    for language in [
        CodeGenLanguage::Go,
        CodeGenLanguage::Java,
        CodeGenLanguage::Dart,
    ] {
        let code = generate_code(&request, language, None);
        assert!(code.contains("@@URL@@"));
        assert!(!code.starts_with("Cannot generate"));
    }
    request.headers.clear();
    request.settings.verify_tls = false;
    assert!(generate_code(&request, CodeGenLanguage::Java, None).contains("keeps TLS verification"));
    request.settings.verify_tls = true;
    request.settings.identity_file = "identity.pem".into();
    assert!(generate_code(&request, CodeGenLanguage::Java, None).contains("key store"));
    request.settings.identity_file.clear();
    request.body_type = BodyType::Binary;
    request.binary_file = "upload.bin".into();
    assert!(
        generate_code(&request, CodeGenLanguage::Http, None).contains("cannot inline upload files")
    );
}
