mod support;
use rustapi::{
    http::{Cancellation, HttpService},
    models::*,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use support::*;
#[test]
fn cookie_login_reuse_edit_and_clear() {
    let server = Server::new(3, |i, r| {
        if i == 0 {
            let mut reply = Reply::json("{}");
            reply
                .headers
                .push(("Set-Cookie".into(), "session=abc; Path=/; HttpOnly".into()));
            reply
        } else {
            if i == 1 {
                assert!(r.head.to_lowercase().contains("cookie: session=edited"));
            } else {
                assert!(!r.head.to_lowercase().contains("cookie:"));
            }
            Reply::json("{}")
        }
    });
    let service = HttpService::default();
    let r = ApiRequest {
        url: server.url.clone(),
        ..Default::default()
    };
    assert!(service
        .execute(&r, &Cancellation::default())
        .error
        .is_none());
    assert!(!service.cookies.list().is_empty());
    service
        .cookies
        .set(&server.url, "session=edited; Path=/")
        .unwrap();
    assert!(service
        .execute(&r, &Cancellation::default())
        .error
        .is_none());
    service.cookies.clear();
    service.execute(&r, &Cancellation::default());
    server.finish();
}
#[test]
fn cancellation_interrupts_slow_response() {
    let (ready, received) = std::sync::mpsc::sync_channel(1);
    let server = Server::new(1, move |_, _| {
        ready.send(()).unwrap();
        let mut reply = Reply::json("{}");
        reply.delay_ms = 600;
        reply
    });
    let cancel = Cancellation::default();
    let c = cancel.clone();
    let cancelling = std::thread::spawn(move || {
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        let start = Instant::now();
        c.cancel();
        start
    });
    let r = HttpService::default().execute(
        &ApiRequest {
            url: server.url.clone(),
            ..Default::default()
        },
        &cancel,
    );
    assert!(r.error.unwrap().contains("cancelled"));
    // Measure cancellation latency after connection, not variable client-startup time.
    assert!(cancelling.join().unwrap().elapsed() < Duration::from_millis(500));
    server.finish();
}
#[test]
fn timeout_and_response_limit_are_transport_errors() {
    let server = Server::new(1, |_, _| {
        let mut r = Reply::json("{}");
        r.delay_ms = 400;
        r
    });
    let r = ApiRequest {
        url: server.url.clone(),
        settings: HttpSettings {
            timeout_ms: 100,
            ..Default::default()
        },
        ..Default::default()
    };
    let service = HttpService::default();
    // Isolate the response deadline from platform trust-store/client initialization.
    service.client(&r.settings).unwrap();
    assert!(service
        .execute(&r, &Cancellation::default())
        .error
        .is_some());
    server.finish();
    let server = Server::new(1, |_, _| Reply::json("1234567890"));
    let r = ApiRequest {
        url: server.url.clone(),
        settings: HttpSettings {
            max_response_bytes: 3,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(HttpService::default()
        .execute(&r, &Cancellation::default())
        .error
        .unwrap()
        .contains("limit"));
    server.finish();
}
#[test]
fn arbitrary_binary_and_multipart_file_bytes_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file.bin");
    let bytes = [0, 255, 128, 13, 10, 1];
    std::fs::write(&path, bytes).unwrap();
    let server = Server::new(2, move |i, r| {
        if i == 0 {
            assert_eq!(r.body, bytes);
        } else {
            assert!(r.body.windows(bytes.len()).any(|w| w == bytes));
            assert!(r
                .head
                .to_lowercase()
                .contains("multipart/form-data; boundary="));
        }
        Reply::json("{}")
    });
    let service = HttpService::default();
    let mut r = ApiRequest {
        url: server.url.clone(),
        body_type: BodyType::Binary,
        binary_file: path.to_string_lossy().into(),
        ..Default::default()
    };
    assert!(service
        .execute(&r, &Cancellation::default())
        .error
        .is_none());
    r.body_type = BodyType::FormData;
    r.form_data.push(KeyValue {
        file: true,
        ..KeyValue::new("upload", path.to_string_lossy())
    });
    assert!(service
        .execute(&r, &Cancellation::default())
        .error
        .is_none());
    server.finish();
}
#[test]
fn raw_download_bytes_and_total_body_time() {
    let server = Server::new(1, |_, _| Reply {
        status: 200,
        headers: vec![],
        body: vec![0, 255, 1],
        delay_ms: 40,
    });
    let r = HttpService::default().execute(
        &ApiRequest {
            url: server.url.clone(),
            ..Default::default()
        },
        &Cancellation::default(),
    );
    assert_eq!(r.raw_body, [0, 255, 1]);
    assert_eq!(r.size, 3);
    assert!(r.time_ms >= 35);
    assert!(r.time_ms >= r.headers_time_ms);
    server.finish();
}
#[test]
fn malformed_headers_fail_before_connecting() {
    let r = ApiRequest {
        url: "http://127.0.0.1:1".into(),
        headers: vec![KeyValue::new("bad header", "x")],
        ..Default::default()
    };
    assert!(HttpService::default()
        .execute(&r, &Cancellation::default())
        .error
        .unwrap()
        .contains("Invalid header"));
}
#[test]
fn truncated_http_body_is_an_error() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let t = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut b = [0; 4096];
        let _read = s.read(&mut b).unwrap();
        s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 99\r\nConnection: close\r\n\r\nabc")
            .unwrap();
    });
    let r = HttpService::default().execute(
        &ApiRequest {
            url,
            ..Default::default()
        },
        &Cancellation::default(),
    );
    assert!(r.error.is_some());
    t.join().unwrap();
}
#[test]
fn redirects_can_be_disabled() {
    let server = Server::new(1, |_, _| Reply {
        status: 302,
        headers: vec![("Location".into(), "/next".into())],
        body: vec![],
        delay_ms: 0,
    });
    let r = ApiRequest {
        url: server.url.clone(),
        settings: HttpSettings {
            follow_redirects: false,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        HttpService::default()
            .execute(&r, &Cancellation::default())
            .status,
        302
    );
    server.finish();
}
#[test]
fn proxy_configuration_reaches_the_proxy() {
    let server = Server::new(1, |_, r| {
        assert!(r.head.starts_with("GET http://api.example.test/path"));
        Reply::json("{}")
    });
    let r = ApiRequest {
        url: "http://api.example.test/path".into(),
        settings: HttpSettings {
            proxy: server.url.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        HttpService::default()
            .execute(&r, &Cancellation::default())
            .status,
        200
    );
    server.finish();
}
#[test]
fn oauth_client_credentials_then_refresh() {
    let server = Server::new(4, |i, r| match i {
        0 => {
            assert!(String::from_utf8_lossy(&r.body).contains("grant_type=client_credentials"));
            Reply::json(r#"{"access_token":"first","refresh_token":"refresh-me","expires_in":0}"#)
        }
        1 => {
            assert!(r
                .head
                .to_lowercase()
                .contains("authorization: bearer first"));
            Reply::json("{}")
        }
        2 => {
            let b = String::from_utf8_lossy(&r.body);
            assert!(
                b.contains("grant_type=refresh_token") && b.contains("refresh_token=refresh-me")
            );
            Reply::json(r#"{"access_token":"second","expires_in":300}"#)
        }
        _ => {
            assert!(r
                .head
                .to_lowercase()
                .contains("authorization: bearer second"));
            Reply::json("{}")
        }
    });
    let service = HttpService::default();
    let r = ApiRequest {
        url: format!("{}/api", server.url),
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
    for _ in 0..2 {
        let response = service.execute(&r, &Cancellation::default());
        assert_eq!(response.status, 200, "{:?}", response.error);
    }
    server.finish();
}
#[test]
fn pkce_callback_validates_state_and_returns_code() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use sha2::{Digest, Sha256};
    let (v, c) = rustapi::http::oauth::pkce();
    assert_eq!(c, URL_SAFE_NO_PAD.encode(Sha256::digest(v)));
    let mut config = OAuthConfig {
        authorization_url: "https://auth.example.test/authorize".into(),
        client_id: "id".into(),
        ..Default::default()
    };
    let auth = rustapi::http::oauth::begin_authorization(&mut config).unwrap();
    let state = auth.state.clone();
    let callback = config.redirect_uri.clone();
    let t = std::thread::spawn(move || auth.wait(&Cancellation::default()).unwrap());
    let client = reqwest::blocking::Client::new();
    assert_eq!(
        client
            .get(format!("{callback}?state=wrong&code=no"))
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .get(format!("{callback}?state={state}&code=valid"))
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(t.join().unwrap(), "valid");
}
#[test]
fn tls_is_verified_by_default_and_custom_ca_and_override_work() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let cert =
        rustls::pki_types::CertificateDer::from(include_bytes!("fixtures/tls/leaf.der").to_vec());
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(
        include_bytes!("fixtures/tls/leaf-key.der").to_vec(),
    );
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key.into())
        .unwrap();
    let config = Arc::new(config);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("https://{}", listener.local_addr().unwrap());
    let t = std::thread::spawn(move || {
        for _ in 0..3 {
            let (s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut stream =
                rustls::StreamOwned::new(rustls::ServerConnection::new(config.clone()).unwrap(), s);
            let mut buf = [0; 4096];
            if stream.read(&mut buf).is_ok() {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                );
            }
        }
    });
    let service = HttpService::default();
    let mut r = ApiRequest {
        url,
        ..Default::default()
    };
    assert!(service
        .execute(&r, &Cancellation::default())
        .error
        .is_some());
    r.settings.ca_file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tls/ca.pem").into();
    assert_eq!(service.execute(&r, &Cancellation::default()).status, 200);
    r.settings.ca_file.clear();
    r.settings.verify_tls = false;
    assert_eq!(service.execute(&r, &Cancellation::default()).status, 200);
    t.join().unwrap();
}
