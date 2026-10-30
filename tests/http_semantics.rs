mod support;
use rustapi::{http::*, models::*};
use serde_json::json;

#[test]
fn structured_query_rows_override_raw_keys_and_repeated_headers_reach_the_server() {
    let server = support::Server::new(1, |_, request| {
        assert!(
            request
                .head
                .starts_with("GET /?keep=a%2Fb&q=one&q=two HTTP/1.1"),
            "{}",
            request.head
        );
        let headers = request.head.to_ascii_lowercase();
        assert_eq!(headers.matches("x-trace:").count(), 2);
        assert!(headers.contains("authorization: manual"));
        assert!(!headers.contains("generated"));
        support::Reply::json("{}")
    });
    let source = json!({"info":{"name":"Semantics","schema":"v2.1.0"},"item":[{"name":"Get","request":{
        "url":{"raw":format!("{}/?q=stale&keep=a%2Fb&disabled=omit#fragment",server.url),"query":[
            {"key":"q","value":"one"},{"key":"q","value":"two"},{"key":"disabled","value":"omit","disabled":true}]},
        "header":[{"key":"X-Trace","value":"one"},{"key":"x-trace","value":"two"},{"key":"Authorization","value":"manual"}],
        "auth":{"type":"bearer","bearer":[{"key":"token","value":"generated"}]}}}]});
    let imported = rustapi::import::json(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert!(!imported.requires_confirmation());
    let request = imported
        .collection
        .unwrap()
        .ordered_requests(None)
        .remove(0);
    assert!(HttpService::default()
        .execute(&request, &Cancellation::default())
        .error
        .is_none());
    server.finish();
    for language in [CodeGenLanguage::Curl, CodeGenLanguage::Rust] {
        let sample = generate_code(&request, language, None);
        assert_eq!(sample.matches("x-trace").count(), 2);
        assert!(sample.contains("keep=a%2Fb&q=one&q=two"));
    }
    for language in [CodeGenLanguage::Python, CodeGenLanguage::JavaScript] {
        assert!(generate_code(&request, language, None)
            .contains("cannot preserve repeated header fields"));
    }
}

#[test]
fn structured_url_components_work_without_a_raw_url() {
    let source = json!({"info":{"name":"URL","schema":"v2.1.0"},"item":[{"name":"Get","request":{
        "url":{"protocol":"https","host":["example","test"],"port":"8443","path":["items",":id"],"hash":"fragment",
        "variable":[{"key":"id","value":"a/b"}],"query":[{"key":"name","value":"a+b %"}]}}}]});
    let collection = rustapi::import::json(&serde_json::to_vec(&source).unwrap())
        .unwrap()
        .collection
        .unwrap();
    let request = rustapi::variables::Variables::default()
        .resolve_request(&collection.ordered_requests(None)[0])
        .unwrap();
    assert_eq!(
        build_url_with_params(&request.url, &request.query_params, None),
        "https://example.test:8443/items/a%2Fb?name=a%2Bb%20%25#fragment"
    );
    assert_eq!(
        build_url_with_params(
            "https://example.test?a+b=old&unknown=keep#x",
            &[KeyValue::new("a b", "new")],
            None
        ),
        "https://example.test?unknown=keep&a%20b=new#x"
    );
}

#[test]
fn single_valued_headers_and_header_control_characters_are_rejected() {
    for name in ["Authorization", "Host", "Content-Length", "Content-Type"] {
        let request = ApiRequest {
            headers: vec![
                KeyValue::new(name, "one"),
                KeyValue::new(name.to_ascii_lowercase(), "two"),
            ],
            ..Default::default()
        };
        assert!(effective_headers(&request)
            .unwrap_err()
            .to_string()
            .contains("Duplicate single-valued header"));
    }
    let request = ApiRequest {
        headers: vec![KeyValue::new("X-Test", "one\r\nInjected: two")],
        ..Default::default()
    };
    assert!(effective_headers(&request).is_err());
}
