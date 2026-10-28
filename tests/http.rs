use rustapi::{http::*, models::*};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

// A bounded loopback server: no internet, fixed ports, proxies, or external service.
fn exchange(
    mut request: ApiRequest,
    env: Option<&Environment>,
    status: &str,
) -> (ApiResponse, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    request.url = request.url.replace(
        "$SERVER",
        &format!("http://{}", listener.local_addr().unwrap()),
    );
    let status = status.to_owned();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(5))
                }
                Err(e) => panic!("request never reached loopback server: {e}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut captured = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0, "client closed before completing its request");
            captured.extend_from_slice(&buffer[..count]);
            if let Some(end) = captured.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&captured[..end]).to_lowercase();
                let length = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length: "))
                    .map_or(0, |n| n.parse::<usize>().unwrap());
                if captured.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let is_head = captured.starts_with(b"HEAD ");
        let body = if is_head { "" } else { "{\"ok\":true}" };
        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nX-Test: loopback\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        String::from_utf8(captured).unwrap()
    });
    let response = execute_request(&request, env);
    let captured = server.join().unwrap();
    assert!(response.error.is_none(), "{:?}", response.error);
    (response, captured)
}

fn request() -> ApiRequest {
    ApiRequest {
        url: "$SERVER/resource".into(),
        ..Default::default()
    }
}

#[test]
fn body_encodings_work_with_and_without_environment_variables() {
    let mut env = Environment::new("test");
    env.variables.push(KeyValue::new("value", "resolved"));
    for environment in [None, Some(&env)] {
        for body_type in [
            BodyType::Json,
            BodyType::Text,
            BodyType::UrlEncoded,
            BodyType::FormData,
        ] {
            let mut request = request();
            request.method = HttpMethod::POST;
            request.body_type = body_type;
            request.body = "{{value}}".into();
            request.form_data.push(KeyValue::new("field", "{{value}}"));
            let (_, wire) = exchange(request, environment, "200 OK");
            if environment.is_some() {
                assert!(wire.contains("resolved"));
            } else {
                assert!(!wire.contains("resolved"));
            }
        }
    }
}

macro_rules! method_case {
    ($name:ident, $method:ident) => {
        #[test]
        fn $name() {
            let mut request = request();
            request.method = HttpMethod::$method;
            let (response, wire) = exchange(request, None, "200 OK");
            assert!(wire.starts_with(concat!(stringify!($method), " /resource HTTP/1.1\r\n")));
            assert_eq!(response.status, 200);
            assert!(response
                .headers
                .iter()
                .any(|(k, v)| k == "x-test" && v == "loopback"));
        }
    };
}
method_case!(get_request, GET);
method_case!(post_request, POST);
method_case!(put_request, PUT);
method_case!(delete_request, DELETE);
method_case!(patch_request, PATCH);
method_case!(head_request, HEAD);
method_case!(options_request, OPTIONS);

#[test]
fn response_preserves_http_errors_and_body() {
    let (response, _) = exchange(request(), None, "422 Unprocessable Entity");
    assert_eq!(response.status, 422);
    assert_eq!(response.body, "{\"ok\":true}");
    assert_eq!(response.size, response.body.len());
    assert!(!response.is_success());
}

#[test]
fn invalid_url_returns_transport_error() {
    let response = execute_request(
        &ApiRequest {
            url: "://invalid".into(),
            ..Default::default()
        },
        None,
    );
    assert!(response.error.unwrap().contains("Request failed"));
}

#[test]
fn query_params_precede_fragment_and_preserve_existing_query() {
    let mut request = request();
    request.url = "$SERVER/resource?existing=1#fragment".into();
    request.query_params = vec![
        KeyValue::new("search", "a b&c"),
        KeyValue::new("", "ignored"),
        KeyValue {
            enabled: false,
            ..KeyValue::new("disabled", "secret")
        },
    ];
    let (_, wire) = exchange(request, None, "200 OK");
    assert!(wire.starts_with("GET /resource?existing=1&search=a%20b%26c HTTP/1.1"));
    assert!(!wire.contains("disabled"));
}

#[test]
fn headers_and_json_body_substitute_environment() {
    let mut env = Environment::new("test");
    env.variables.push(KeyValue::new("name", "Ada"));
    let mut request = request();
    request.method = HttpMethod::POST;
    request.body_type = BodyType::Json;
    request.body = "{\"name\":\"{{name}}\"}".into();
    request.headers = vec![
        KeyValue::new("x-name", "{{name}}"),
        KeyValue {
            enabled: false,
            ..KeyValue::new("x-disabled", "secret")
        },
    ];
    let (_, wire) = exchange(request, Some(&env), "201 Created");
    assert!(wire.contains("x-name: Ada\r\n"));
    assert!(wire.contains("content-type: application/json\r\n"));
    assert!(!wire.contains("x-disabled"));
    assert!(wire.ends_with("{\"name\":\"Ada\"}"));
}

#[test]
fn explicit_content_type_is_preserved() {
    let mut request = request();
    request.body_type = BodyType::Text;
    request.body = "text".into();
    request
        .headers
        .push(KeyValue::new("content-type", "application/custom"));
    let (_, wire) = exchange(request, None, "200 OK");
    assert!(wire.contains("content-type: application/custom\r\n"));
    assert!(wire.ends_with("text"));
}

#[test]
fn basic_auth_substitutes_before_encoding() {
    let mut env = Environment::new("test");
    env.variables = vec![KeyValue::new("user", "a"), KeyValue::new("pass", "bc")];
    let mut request = request();
    request.auth = AuthConfig {
        auth_type: AuthType::BasicAuth,
        basic_username: "{{user}}".into(),
        basic_password: "{{pass}}".into(),
        ..Default::default()
    };
    let (_, wire) = exchange(request, Some(&env), "200 OK");
    assert!(wire.contains("authorization: Basic YTpiYw==\r\n"));
}

#[test]
fn bearer_auth_and_user_override() {
    let mut request = request();
    request.auth.auth_type = AuthType::BearerToken;
    request.auth.bearer_token = "token".into();
    let (_, wire) = exchange(request.clone(), None, "200 OK");
    assert!(wire.contains("authorization: Bearer token\r\n"));
    request
        .headers
        .push(KeyValue::new("authorization", "custom"));
    let (_, wire) = exchange(request, None, "200 OK");
    assert!(wire.contains("authorization: custom\r\n"));
    assert!(!wire.contains("Bearer token"));
}

#[test]
fn api_key_is_substituted_and_encoded_in_query() {
    let mut env = Environment::new("test");
    env.variables.push(KeyValue::new("token", "a&b"));
    let mut request = request();
    request.auth = AuthConfig {
        auth_type: AuthType::ApiKey,
        api_key_name: "api_key".into(),
        api_key_value: "{{token}}".into(),
        api_key_location: ApiKeyLocation::QueryParam,
        ..Default::default()
    };
    let (_, wire) = exchange(request, Some(&env), "200 OK");
    assert!(wire.starts_with("GET /resource?api_key=a%26b HTTP/1.1"));
}

#[test]
fn urlencoded_form_filters_disabled_fields() {
    let mut request = request();
    request.method = HttpMethod::POST;
    request.body_type = BodyType::UrlEncoded;
    request.form_data = vec![
        KeyValue::new("name", "Ada Lovelace"),
        KeyValue {
            enabled: false,
            ..KeyValue::new("secret", "hidden")
        },
    ];
    let (_, wire) = exchange(request, None, "200 OK");
    assert!(wire.contains("application/x-www-form-urlencoded"));
    assert!(wire.ends_with("name=Ada+Lovelace"));
    assert!(!wire.contains("hidden"));
}

#[test]
fn multipart_form_sends_fields_and_boundary() {
    let mut request = request();
    request.method = HttpMethod::POST;
    request.body_type = BodyType::FormData;
    request.form_data = vec![KeyValue::new("name", "Ada")];
    let (_, wire) = exchange(request, None, "200 OK");
    assert!(wire.contains("multipart/form-data; boundary="));
    assert!(wire.contains("name=\"name\"\r\n\r\nAda"));
}

#[test]
fn binary_body_is_sent_verbatim() {
    let mut request = request();
    request.method = HttpMethod::POST;
    request.body_type = BodyType::Binary;
    request.body = "a\0b\nc".into();
    let (_, wire) = exchange(request, None, "200 OK");
    assert!(wire.ends_with("a\0b\nc"));
}

#[test]
fn json_formatter_preserves_invalid_input() {
    assert_eq!(format_json("not json"), "not json");
    assert_eq!(format_json("{\"ok\":true}"), "{\n  \"ok\": true\n}");
}
