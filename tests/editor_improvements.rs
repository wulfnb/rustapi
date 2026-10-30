mod support;
use rustapi::{http::*, models::*};

#[test]
fn custom_http_methods_serialize_and_execute() {
    let server = support::Server::new(1, |_, request| {
        assert!(
            request.head.starts_with("PURGE /cache/item HTTP/1.1"),
            "Wire request: {}",
            request.head
        );
        support::Reply::json("{\"purged\":true}")
    });

    let custom = HttpMethod::from("PURGE");
    assert_eq!(custom.as_str(), "PURGE");
    assert_eq!(custom.to_string(), "PURGE");
    assert_ne!(custom.color(), egui::Color32::TRANSPARENT);

    // Test JSON serialization & deserialization roundtrip
    let serialized = serde_json::to_string(&custom).unwrap();
    assert_eq!(serialized, "\"PURGE\"");
    let deserialized: HttpMethod = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, HttpMethod::Custom("PURGE".into()));

    // Test standard methods deserialize to enum variants
    let get: HttpMethod = serde_json::from_str("\"GET\"").unwrap();
    assert_eq!(get, HttpMethod::GET);

    let request = ApiRequest {
        method: HttpMethod::Custom("PURGE".into()),
        url: format!("{}/cache/item", server.url),
        ..Default::default()
    };

    let response = HttpService::default().execute(&request, &Cancellation::default());
    assert!(response.error.is_none());
    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"purged\":true"));
    server.finish();

    // Verify code generation with custom method
    let curl = generate_code(&request, CodeGenLanguage::Curl, None);
    assert!(curl.contains("curl -X PURGE"));
    let js = generate_code(&request, CodeGenLanguage::JavaScript, None);
    assert!(js.contains("\"PURGE\""));
}

#[test]
fn bidirectional_url_query_synchronization() {
    let mut query_params = vec![
        KeyValue::new("search", "initial"),
        KeyValue {
            key: "disabled".into(),
            value: "stay".into(),
            enabled: false,
            ..Default::default()
        },
    ];

    // 1. Sync URL into query_params
    let url = "https://api.example.com/items?search=rust&limit=20#frag";
    sync_url_to_query_params(url, &mut query_params);

    assert_eq!(query_params.len(), 3);
    assert_eq!(query_params[0].key, "search");
    assert_eq!(query_params[0].value, "rust");
    assert!(query_params[0].enabled);

    assert_eq!(query_params[1].key, "limit");
    assert_eq!(query_params[1].value, "20");
    assert!(query_params[1].enabled);

    // Disabled parameter was preserved
    assert_eq!(query_params[2].key, "disabled");
    assert!(!query_params[2].enabled);

    // 2. Sync query_params back into URL
    let mut current_url = "https://api.example.com/items?old=value#frag".to_string();
    query_params[0].value = "web%20assembly".into();
    sync_query_params_to_url(&mut current_url, &query_params);

    assert!(current_url.starts_with("https://api.example.com/items?"));
    assert!(current_url.contains("search=web%2520assembly"));
    assert!(current_url.contains("limit=20"));
    assert!(current_url.ends_with("#frag"));

    // 3. Template variables preserved without escaping brackets
    let mut template_url = "https://api.example.com/items".to_string();
    let template_params = vec![KeyValue::new("user", "{{userId}}")];
    sync_query_params_to_url(&mut template_url, &template_params);
    assert_eq!(
        template_url,
        "https://api.example.com/items?user={{userId}}"
    );
}

#[test]
fn xml_and_html_body_modes_and_formatting() {
    let xml_kind = BodyType::Xml;
    assert_eq!(xml_kind.as_str(), "XML");
    assert_eq!(xml_kind.content_type(), Some("application/xml"));

    let html_kind = BodyType::Html;
    assert_eq!(html_kind.as_str(), "HTML");
    assert_eq!(html_kind.content_type(), Some("text/html"));

    let raw_xml = "<root><item id=\"1\">test</item><self/><!-- comment --></root>";
    let formatted = format_xml(raw_xml);
    assert!(formatted.contains("<root>"));
    assert!(formatted.contains("<item id=\"1\">"));
    assert!(formatted.contains("test"));
    assert!(formatted.contains("</item>"));
    assert!(formatted.contains("<self/>"));
    assert!(formatted.contains("</root>"));

    // Verify invalid / non-xml input is safe
    assert_eq!(format_xml("not xml"), "not xml");

    // Wire execution with XML body sets application/xml Content-Type
    let server = support::Server::new(1, |_, request| {
        assert!(request
            .head
            .to_ascii_lowercase()
            .contains("content-type: application/xml"));
        support::Reply {
            status: 200,
            headers: vec![("Content-Type".into(), "application/xml".into())],
            body: "<response><ok/></response>".as_bytes().to_vec(),
            delay_ms: 0,
        }
    });

    let request = ApiRequest {
        method: HttpMethod::POST,
        url: server.url.clone(),
        body_type: BodyType::Xml,
        body: "<request><id>123</id></request>".into(),
        ..Default::default()
    };

    let response = HttpService::default().execute(&request, &Cancellation::default());
    assert!(response.error.is_none());
    assert_eq!(response.status, 200);
    server.finish();
}
