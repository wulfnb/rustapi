mod support;
use rustapi::{
    http::{Cancellation, HttpService},
    models::*,
};
use serde_json::json;

#[test]
fn inherited_settings_are_applied_once_and_unknown_options_remain_visible() {
    let source = json!({"info":{"name":"Settings","schema":"v2.1.0"},
        "protocolProfileBehavior":{"followRedirects":false,"disableCookieJar":true,"timeout":400},
        "item":[{"name":"Folder","protocolProfileBehavior":{"timeout":800},"item":[
            {"name":"Request","protocolProfileBehavior":{"followRedirects":true,"unknown-private-setting":"private-value"},
             "request":{"url":"https://example.test"}}]}]});
    let imported = rustapi::import::json(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert!(imported.requires_confirmation());
    assert_eq!(
        imported.diagnostics[0].path,
        "/item/0/item/0/protocolProfileBehavior"
    );
    assert_eq!(
        imported.diagnostics[0].disposition,
        rustapi::import::ImportDisposition::Unsupported
    );
    let report = serde_json::to_string(&imported.report()).unwrap();
    assert!(!report.contains("private-value"));
    assert!(!report.contains("unknown-private-setting"));
    let mut collection = imported.collection.unwrap();
    let request = collection.ordered_requests(None).remove(0);
    assert!(request.settings.follow_redirects);
    assert!(!request.settings.use_cookies);
    assert_eq!(request.settings.timeout_ms, 800);
    collection.folders[0].requests[0].settings.follow_redirects = false;
    let copy = rustapi::import::json(&serde_json::to_vec(&collection).unwrap())
        .unwrap()
        .collection
        .unwrap();
    assert!(!copy.ordered_requests(None)[0].settings.follow_redirects);
    assert_eq!(
        copy.folders[0].requests[0].imported_settings["unknown-private-setting"],
        "private-value"
    );
    let mut insecure = source.clone();
    insecure["protocolProfileBehavior"]["strictSSL"] = false.into();
    let imported = rustapi::import::json(&serde_json::to_vec(&insecure).unwrap()).unwrap();
    assert!(imported
        .diagnostics
        .iter()
        .any(|d| d.feature == "tls_override"));
    assert!(
        !imported.collection.unwrap().ordered_requests(None)[0]
            .settings
            .verify_tls
    );
    for value in [json!("false"), json!(null), json!([])] {
        let mut bad = source.clone();
        bad["protocolProfileBehavior"]["followRedirects"] = value;
        assert!(rustapi::import::json(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn imported_redirect_and_cookie_policies_control_real_http_requests() {
    let server = support::Server::new(1, |_, request| {
        assert!(!request.head.to_ascii_lowercase().contains("cookie:"));
        let mut reply = support::Reply::json("{}");
        reply.status = 302;
        reply
            .headers
            .push(("Location".into(), "http://127.0.0.1:1".into()));
        reply
            .headers
            .push(("Set-Cookie".into(), "new=ignored; Path=/".into()));
        reply
    });
    let source = json!({"info":{"name":"Policy","schema":"v2.1.0"},"item":[{"name":"Get",
        "protocolProfileBehavior":{"followRedirects":false,"disableCookieJar":true},"request":{"url":server.url}}]});
    let imported = rustapi::import::json(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert!(!imported.requires_confirmation());
    let request = imported
        .collection
        .unwrap()
        .ordered_requests(None)
        .remove(0);
    let service = HttpService::default();
    service
        .cookies
        .set(&server.url, "old=retained; Path=/")
        .unwrap();
    let response = service.execute(&request, &Cancellation::default());
    assert!(response.error.is_none(), "{:?}", response.error);
    assert_eq!(response.status, 302);
    assert!(service
        .cookies
        .list()
        .iter()
        .all(|cookie| !cookie.contains("new=ignored")));
    server.finish();
    assert!(HttpSettings::default().use_cookies);
}
