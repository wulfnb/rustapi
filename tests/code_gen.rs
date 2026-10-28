use rustapi::{http::*, models::*};

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
    ".post(\"https://api.test/users\")"
);

#[test]
fn curl_escapes_single_quotes_in_body() {
    let (mut request, _) = sample();
    request.body = "{\"name\":\"O'Reilly\"}".into();
    let code = generate_code(&request, CodeGenLanguage::Curl, None);
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
