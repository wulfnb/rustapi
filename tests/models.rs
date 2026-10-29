use rustapi::models::*;
use rustapi::utils::{format_size, truncate_str};

#[test]
fn model_labels_colors_and_content_types_cover_the_public_choices() {
    for method in HttpMethod::all() {
        assert_eq!(method.to_string(), method.as_str());
        assert_ne!(method.color(), egui::Color32::TRANSPARENT);
    }
    for kind in [
        BodyType::None,
        BodyType::Json,
        BodyType::Text,
        BodyType::FormData,
        BodyType::UrlEncoded,
        BodyType::Binary,
    ] {
        assert!(!kind.as_str().is_empty());
        assert_eq!(kind.content_type().is_none(), kind == BodyType::None);
    }
    for kind in AuthType::all() {
        assert!(!kind.as_str().is_empty());
    }
    for location in [ApiKeyLocation::Header, ApiKeyLocation::QueryParam] {
        assert!(!location.as_str().is_empty());
    }
    for status in [0, 200, 302, 404, 500] {
        assert_ne!(
            ApiResponse {
                status,
                ..Default::default()
            }
            .status_color(),
            egui::Color32::TRANSPARENT
        );
    }
    let entry = HistoryEntry::new(
        ApiRequest {
            url: "http://localhost".into(),
            ..Default::default()
        },
        Some(ApiResponse {
            status: 201,
            ..Default::default()
        }),
    );
    assert!(entry.display_string().ends_with("201"));
}

#[test]
fn duplicate_request_preserves_payload_and_has_independent_identity() {
    let mut original = ApiRequest::new("Create user");
    original.method = HttpMethod::POST;
    original.url = "https://example.test/users".into();
    original.body_type = BodyType::Json;
    original.body = "{\"name\":\"Ada\"}".into();
    original.headers.push(KeyValue::new("x-user", "Ada"));
    original.auth.bearer_token = "secret".into();
    let mut copy = original.duplicate();
    assert_ne!(original.id, copy.id);
    assert_eq!(copy.name, "Create user (copy)");
    assert_eq!(copy.method, original.method);
    assert_eq!(copy.body, original.body);
    assert_eq!(copy.url, original.url);
    assert_eq!(copy.auth.bearer_token, "secret");
    copy.headers[0].value = "Grace".into();
    assert_eq!(original.headers[0].value, "Ada");
}

#[test]
fn collection_json_preserves_nested_requests_but_omits_local_path() {
    let mut collection = Collection::new("Unicode 🦀");
    collection.file_path = Some("/private/local.json".into());
    let mut folder = Folder::new("Nested");
    folder.requests.push(ApiRequest::new("A request"));
    collection.folders.push(folder);
    collection
        .variables
        .insert("base".into(), "localhost".into());
    let json = serde_json::to_string(&collection).unwrap();
    assert!(!json.contains("file_path"));
    let loaded: Collection = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.name, collection.name);
    assert_eq!(loaded.folders[0].requests[0].name, "A request");
    assert_eq!(loaded.variables["base"], "localhost");
    assert!(loaded.file_path.is_none());
}

#[test]
fn environment_replaces_repeated_variables_and_preserves_unknowns() {
    let mut env = Environment::new("test");
    env.variables.push(KeyValue::new("host", "api.test"));
    assert_eq!(
        env.substitute("{{host}}/{{host}}/{{unknown}}"),
        "api.test/api.test/{{unknown}}"
    );
    assert_eq!(env.get("host"), Some("api.test"));
    assert_eq!(env.get("missing"), None);
}

#[test]
fn disabled_environment_variables_are_ignored() {
    let mut env = Environment::new("test");
    let mut disabled = KeyValue::new("token", "secret");
    disabled.enabled = false;
    env.variables.push(disabled);
    assert_eq!(env.substitute("{{token}}"), "{{token}}");
    assert_eq!(env.get("token"), None);
}

#[test]
fn history_is_newest_first_and_bounded() {
    let mut history = RequestHistory::new(2);
    for name in ["first", "second", "third"] {
        history.add(ApiRequest::new(name), None);
    }
    assert_eq!(history.entries.len(), 2);
    assert_eq!(history.recent(1)[0].request.name, "third");
    assert_eq!(history.recent(99).len(), 2);
    assert!(history.recent(0).is_empty());
    history.clear();
    assert!(history.entries.is_empty());
}

#[test]
fn zero_capacity_history_keeps_no_credentials() {
    let mut history = RequestHistory::new(0);
    history.add(ApiRequest::new("request"), None);
    assert!(history.entries.is_empty());
}

#[test]
fn history_display_handles_unicode_and_missing_responses() {
    let request = ApiRequest {
        url: format!("https://example.test/{}", "🦀".repeat(50)),
        ..Default::default()
    };
    let entry = HistoryEntry::new(request, None);
    let text = entry.display_string();
    assert!(text.contains("..."));
    assert!(text.ends_with("ERR"));
}

#[test]
fn response_success_requires_a_2xx_status() {
    for (status, success) in [
        (0, false),
        (199, false),
        (200, true),
        (299, true),
        (300, false),
        (500, false),
    ] {
        assert_eq!(
            ApiResponse {
                status,
                ..Default::default()
            }
            .is_success(),
            success
        );
    }
    let error = ApiResponse::error("offline");
    assert_eq!(error.error.as_deref(), Some("offline"));
    assert!(!error.is_success());
}

macro_rules! basic_auth_case {
    ($name:ident, $user:expr, $pass:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let auth = AuthConfig {
                auth_type: AuthType::BasicAuth,
                basic_username: $user.into(),
                basic_password: $pass.into(),
                ..Default::default()
            };
            assert_eq!(auth.get_auth_header().unwrap().1, $expected);
        }
    };
}
basic_auth_case!(basic_auth_without_padding, "a", "b", "Basic YTpi");
basic_auth_case!(basic_auth_one_padding_byte, "ab", "cd", "Basic YWI6Y2Q=");
basic_auth_case!(basic_auth_two_padding_bytes, "a", "bc", "Basic YTpiYw==");
basic_auth_case!(basic_auth_unicode, "ü", "密", "Basic w7w65a+G");
basic_auth_case!(basic_auth_empty_password, "user", "", "Basic dXNlcjo=");

#[test]
fn empty_auth_does_not_emit_a_header() {
    for auth_type in AuthType::all() {
        let auth = AuthConfig {
            auth_type: auth_type.clone(),
            ..Default::default()
        };
        assert!(auth.get_auth_header().is_none());
        assert!(auth.get_api_key_query_param().is_none());
    }
}

#[test]
fn bearer_auth_emits_authorization() {
    let auth = AuthConfig {
        auth_type: AuthType::BearerToken,
        bearer_token: "abc".into(),
        ..Default::default()
    };
    assert_eq!(
        auth.get_auth_header(),
        Some(("Authorization".into(), "Bearer abc".into()))
    );
}

#[test]
fn api_key_locations_are_mutually_exclusive() {
    let mut auth = AuthConfig {
        auth_type: AuthType::ApiKey,
        api_key_name: "key".into(),
        api_key_value: "value".into(),
        ..Default::default()
    };
    assert!(auth.get_auth_header().is_some());
    assert!(auth.get_api_key_query_param().is_none());
    auth.api_key_location = ApiKeyLocation::QueryParam;
    assert!(auth.get_auth_header().is_none());
    assert_eq!(
        auth.get_api_key_query_param(),
        Some(("key".into(), "value".into()))
    );
}

#[test]
fn display_sizes_at_unit_boundaries() {
    for (size, expected) in [
        (0, "0 B"),
        (1023, "1023 B"),
        (1024, "1.0 KB"),
        (1536, "1.5 KB"),
        (1048576, "1.0 MB"),
    ] {
        assert_eq!(format_size(size), expected);
    }
}

#[test]
fn truncation_respects_unicode_boundaries_and_small_limits() {
    for (text, limit, expected) in [
        ("abc", 3, "abc"),
        ("abcdef", 5, "ab..."),
        ("🦀é中abc", 4, "🦀..."),
        ("🦀é中abc", 2, "🦀é"),
        ("🦀", 0, ""),
    ] {
        assert_eq!(truncate_str(text, limit), expected);
    }
}
