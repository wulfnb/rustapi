use reqwest::cookie::CookieStore;
use rustapi::http::{BrowserCookie, CookieBridge, CookieJar};
use std::sync::Arc;
#[test]
fn connector_bundle_exports_offline_without_replacing_existing_files() {
    let root = tempfile::tempdir().unwrap();
    let first = rustapi::http::export_connector(root.path()).unwrap();
    let second = rustapi::http::export_connector(root.path()).unwrap();
    assert_ne!(first, second);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(first.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["manifest_version"], 3);
    for file in ["popup.js", "popup.html", "README.md"] {
        assert!(first.join(file).is_file());
    }
}
fn cookie() -> BrowserCookie {
    BrowserCookie {
        name: "session".into(),
        value: "private-cookie".into(),
        domain: "api.example.test".into(),
        path: "/v1".into(),
        secure: true,
        http_only: true,
        host_only: true,
        same_site: "strict".into(),
        expiration_date: None,
        partition_key: None,
    }
}
#[test]
fn cookie_attributes_persist_encrypted_and_removal_survives_restart() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cookies.json");
    let jar = CookieJar::default();
    let password = "long enough cookie password";
    jar.unlock(&path, password).unwrap();
    jar.import_browser("https://api.example.test", &[cookie()])
        .unwrap();
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("private-cookie"));
    jar.lock();
    assert!(jar.entries().is_empty());
    assert!(jar.unlock(&path, "wrong password long enough").is_err());
    jar.unlock(&path, password).unwrap();
    let entry = &jar.entries()[0];
    assert!(entry.host_only && entry.secure && entry.http_only);
    assert_eq!(entry.path, "/v1");
    assert_eq!(entry.same_site, "strict");
    assert!(jar
        .cookies(&reqwest::Url::parse("http://api.example.test/v1").unwrap())
        .is_none());
    assert!(jar
        .cookies(&reqwest::Url::parse("https://api.example.test/v2").unwrap())
        .is_none());
    assert_eq!(
        jar.cookies(&reqwest::Url::parse("https://api.example.test/v1/resource").unwrap())
            .unwrap(),
        "session=private-cookie"
    );
    jar.remove("api.example.test", "/v1", "session").unwrap();
    jar.lock();
    jar.unlock(&path, password).unwrap();
    assert!(jar.entries().is_empty());
}
#[test]
fn browser_import_is_atomic_and_scoped_and_expiration_deletes() {
    let jar = CookieJar::default();
    let valid = cookie();
    let mut bad = cookie();
    bad.name = "second".into();
    bad.domain = "other.test".into();
    assert!(jar
        .import_browser("https://api.example.test", &[valid.clone(), bad])
        .is_err());
    assert!(jar.entries().is_empty());
    bad = valid.clone();
    bad.partition_key = Some(serde_json::json!({"topLevelSite":"https://example.test"}));
    assert!(jar
        .import_browser("https://api.example.test", &[bad])
        .is_err());
    jar.import_browser("https://api.example.test", &[valid])
        .unwrap();
    assert!(jar.for_origin("https://evil.test").unwrap().is_empty());
    jar.set("https://api.example.test", "session=; Path=/v1; Max-Age=0")
        .unwrap();
    assert!(jar.entries().is_empty());
    assert!(jar.set("file:///tmp/test", "a=b").is_err());
    assert!(jar
        .set("https://api.example.test", "a=b\r\nX-Leak: yes")
        .is_err());
}
#[test]
fn paired_bridge_rejects_sites_wrong_token_host_and_stops() {
    let jar = Arc::new(CookieJar::default());
    let bridge = CookieBridge::start(jar.clone()).unwrap();
    let address = bridge.server.address;
    let token = bridge.pairing.split_once('#').unwrap().1;
    let url = format!("http://{address}/cookies");
    let client = reqwest::blocking::Client::new();
    let payload = serde_json::json!({"origin":"https://api.example.test","cookies":[cookie()]});
    assert_eq!(
        client
            .post(&url)
            .header("Origin", "https://evil.test")
            .bearer_auth(token)
            .json(&payload)
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .post(&url)
            .header("Origin", "chrome-extension://abc")
            .bearer_auth("wrong")
            .json(&payload)
            .send()
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .post(&url)
            .header("Origin", "chrome-extension://abc")
            .header("Host", "evil.test")
            .bearer_auth(token)
            .json(&payload)
            .send()
            .unwrap()
            .status(),
        403
    );
    let response = client
        .post(&url)
        .header("Origin", "chrome-extension://abc")
        .bearer_auth(token)
        .json(&payload)
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(jar.entries().len(), 1);
    let response = client
        .get(format!("{url}?origin=https%3A%2F%2Fapi.example.test"))
        .header("Origin", "chrome-extension://abc")
        .bearer_auth(token)
        .send()
        .unwrap();
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        "chrome-extension://abc"
    );
    assert_eq!(
        response.json::<serde_json::Value>().unwrap()["cookies"][0]["value"],
        "private-cookie"
    );
    drop(bridge);
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(client.get(url).send().is_err());
}
