mod support;
use rustapi::{
    http::{self, Cancellation, HttpService},
    models::*,
};
use sha2::{Digest, Sha256};
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn jwt_matches_independent_python_hmac_vector_and_rejects_unsafe_inputs() {
    let mut config = JwtConfig {
        key: "0123456789abcdef0123456789abcdef".into(), // gitleaks:allow -- public synthetic HMAC test key
        claims: r#"{"sub":"1234567890"}"#.into(),
        ..Default::default()
    };
    // Python stdlib: base64.urlsafe_b64encode + hmac.new(key, signing_input, hashlib.sha256).
    assert_eq!(http::auth::jwt(&config).unwrap(),"eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.FK8ZdXllxDGdkVa7jh8WxCWvVextKAFJidtCTPk4EBs"); // gitleaks:allow -- public synthetic JWT test vector
    for invalid in ["none", "HS1", "RS1"] {
        config.algorithm = invalid.into();
        assert!(http::auth::jwt(&config).is_err());
    }
    config.algorithm = "HS256".into();
    config.claims = "[]".into();
    assert!(http::auth::jwt(&config).is_err());
    config.claims = "{}".into();
    config.key = "short".into();
    assert!(http::auth::jwt(&config).is_err());
}

#[test]
fn asymmetric_jwt_signatures_verify_with_independent_openssl() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let run = |arguments: &[&str]| {
        let result = Command::new("openssl")
            .current_dir(root)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    for (algorithm, key_arguments, digest) in [
        (
            "RS256",
            vec![
                "genpkey",
                "-algorithm",
                "RSA",
                "-pkeyopt",
                "rsa_keygen_bits:2048",
                "-out",
                "private.pem",
            ],
            "-sha256",
        ),
        (
            "PS256",
            vec![
                "genpkey",
                "-algorithm",
                "RSA",
                "-pkeyopt",
                "rsa_keygen_bits:2048",
                "-out",
                "private.pem",
            ],
            "-sha256",
        ),
        (
            "ES256",
            vec![
                "genpkey",
                "-algorithm",
                "EC",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-out",
                "private.pem",
            ],
            "-sha256",
        ),
        (
            "ES384",
            vec![
                "genpkey",
                "-algorithm",
                "EC",
                "-pkeyopt",
                "ec_paramgen_curve:P-384",
                "-out",
                "private.pem",
            ],
            "-sha384",
        ),
        (
            "EdDSA",
            vec!["genpkey", "-algorithm", "ED25519", "-out", "private.pem"],
            "",
        ),
    ] {
        run(&key_arguments);
        run(&[
            "pkey",
            "-in",
            "private.pem",
            "-pubout",
            "-out",
            "public.pem",
        ]);
        let config = JwtConfig {
            algorithm: algorithm.into(),
            key: std::fs::read_to_string(root.join("private.pem")).unwrap(),
            claims: r#"{"sub":"openssl-validation"}"#.into(),
            ..Default::default()
        };
        let token = http::auth::jwt(&config).unwrap();
        let parts = token.split('.').collect::<Vec<_>>();
        let mut signature = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        if algorithm.starts_with("ES") {
            // JOSE encodes R||S; OpenSSL expects a DER SEQUENCE of positive INTEGERs.
            let integer = |bytes: &[u8]| {
                let start = bytes
                    .iter()
                    .position(|b| *b != 0)
                    .unwrap_or(bytes.len() - 1);
                let bytes = &bytes[start..];
                let pad = usize::from(bytes[0] & 0x80 != 0);
                let mut out = vec![2, (bytes.len() + pad) as u8];
                if pad == 1 {
                    out.push(0);
                }
                out.extend_from_slice(bytes);
                out
            };
            let mut body = integer(&signature[..signature.len() / 2]);
            body.extend(integer(&signature[signature.len() / 2..]));
            signature = vec![0x30, body.len() as u8];
            signature.extend(body);
        }
        std::fs::write(root.join("signature.bin"), signature).unwrap();
        std::fs::write(root.join("input.bin"), format!("{}.{}", parts[0], parts[1])).unwrap();
        if algorithm == "EdDSA" {
            run(&[
                "pkeyutl",
                "-verify",
                "-pubin",
                "-inkey",
                "public.pem",
                "-sigfile",
                "signature.bin",
                "-rawin",
                "-in",
                "input.bin",
            ]);
        } else {
            let mut args = vec![
                "dgst",
                digest,
                "-verify",
                "public.pem",
                "-signature",
                "signature.bin",
            ];
            if algorithm == "PS256" {
                args.extend([
                    "-sigopt",
                    "rsa_padding_mode:pss",
                    "-sigopt",
                    "rsa_pss_saltlen:-1",
                ]);
            }
            args.push("input.bin");
            run(&args);
        }
    }
}

#[test]
fn aws_sdk_reference_signature_and_s3_payload_checksum() {
    // AWS SDK Rust test_sign_headers_utf8 vector, fixed credentials/time, no network.
    let mut config = AwsConfig {
        access_key: "ANOTREAL".into(),
        secret_key: "notrealrnrELgWzOk3IfjzDKtFBhDby".into(),
        region: "us-east-1".into(),
        service: "service".into(),
        ..Default::default()
    };
    let client = reqwest::Client::new();
    let mut request = client
        .get("https://some-endpoint.some-region.amazonaws.com")
        .header("some-header", "テスト")
        .build()
        .unwrap();
    let time = chrono::DateTime::parse_from_rfc3339("2015-08-30T12:36:00Z")
        .unwrap()
        .timestamp() as u64;
    http::auth::sign_aws(
        &mut request,
        &config,
        UNIX_EPOCH + Duration::from_secs(time),
    )
    .unwrap();
    assert!(request.headers()["authorization"]
        .to_str()
        .unwrap()
        .ends_with("Signature=55e16b31f9bde5fd04f9d3b780dd2b5e5f11a5219001f91a8ca9ec83eaf1618f"));
    config.service = "s3".into();
    config.session_token = "temporary-session".into();
    let mut request = client
        .put("https://s3.example.test/a%20b//object?x=2&x=1")
        .body(vec![0, 255, 13, 10])
        .build()
        .unwrap();
    http::auth::sign_aws(
        &mut request,
        &config,
        UNIX_EPOCH + Duration::from_secs(time),
    )
    .unwrap();
    assert_eq!(
        request.headers()["x-amz-content-sha256"],
        format!("{:x}", Sha256::digest([0, 255, 13, 10]))
    );
    assert_eq!(
        request.headers()["x-amz-security-token"],
        "temporary-session"
    );
    assert_eq!(request.url().path(), "/a%20b//object");
    config.secret_key.clear();
    assert!(http::auth::sign_aws(&mut request, &config, UNIX_EPOCH).is_err());
}

#[test]
fn digest_auth_int_replays_exact_binary_and_matches_independent_hash() {
    let challenge = r#"Digest realm="test",nonce="server-nonce",algorithm=SHA-256,qop="auth-int""#;
    let server = support::Server::new(2, move |index, request| {
        assert_eq!(request.body, vec![0, 255, 13, 10]);
        if index == 0 {
            return support::Reply {
                status: 401,
                headers: vec![("WWW-Authenticate".into(), challenge.into())],
                body: vec![],
                delay_ms: 0,
            };
        }
        let header = request
            .head
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(k, _)| k.eq_ignore_ascii_case("authorization"))
                    .map(|(_, v)| v.trim())
            })
            .unwrap();
        let auth: digest_auth::AuthorizationHeader = header.parse().unwrap();
        let hash = |data: &[u8]| format!("{:x}", Sha256::digest(data));
        let ha1 = hash(b"user:test:private-password");
        let ha2 = hash(format!("POST:/path?x=1:{}", hash(&request.body)).as_bytes());
        let expected = hash(
            format!(
                "{ha1}:server-nonce:00000001:{}:auth-int:{ha2}",
                auth.cnonce.as_ref().unwrap()
            )
            .as_bytes(),
        );
        assert_eq!(auth.response, expected);
        support::Reply::json("{}")
    });
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), [0, 255, 13, 10]).unwrap();
    let request = ApiRequest {
        url: format!("{}/path?x=1", server.url),
        method: HttpMethod::POST,
        body_type: BodyType::Binary,
        binary_file: file.path().display().to_string(),
        auth: AuthConfig {
            auth_type: AuthType::Digest,
            basic_username: "user".into(),
            basic_password: "private-password".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let service = HttpService::default();
    let response = service.execute(&request, &Cancellation::default());
    assert_eq!(response.status, 200, "{:?}", response.error);
    server.finish();
    assert!(!serde_json::to_string(&service.debug.snapshot())
        .unwrap()
        .contains("private-password"));
}

#[test]
fn helper_redirects_stay_on_origin_and_explicit_headers_override_helpers() {
    let server = support::Server::new(2, |index, request| {
        assert!(request.head.contains("AWS4-HMAC-SHA256"));
        support::Reply {
            status: 307,
            headers: vec![(
                "Location".into(),
                if index == 0 {
                    "/next".into()
                } else {
                    "http://127.0.0.1:1/leak".into()
                },
            )],
            body: vec![],
            delay_ms: 0,
        }
    });
    let mut request = ApiRequest {
        url: server.url.clone(),
        auth: AuthConfig {
            auth_type: AuthType::AwsSigV4,
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
    assert_eq!(
        HttpService::default()
            .execute(&request, &Cancellation::default())
            .status,
        307
    );
    let wire = server.finish();
    assert!(wire[1].head.starts_with("GET /next "));
    let override_server = support::Server::new(1, |_, request| {
        assert!(request.head.contains("explicit-token"));
        support::Reply::json("{}")
    });
    request.url = override_server.url.clone();
    request.auth.auth_type = AuthType::Jwt;
    request
        .headers
        .push(KeyValue::new("Authorization", "Bearer explicit-token"));
    assert_eq!(
        HttpService::default()
            .execute(&request, &Cancellation::default())
            .status,
        200
    );
    override_server.finish();
    assert!(
        http::generate_code(&request, http::CodeGenLanguage::Python, None)
            .contains("explicit-token")
    );
}

#[test]
fn encrypted_oauth_survives_restart_refreshes_and_can_be_forgotten() {
    // Several real Argon2 unlock/write operations run between requests, including
    // an incorrect-password attempt; allow that work on busy CI hosts.
    let server = support::Server::with_timeout(4, Duration::from_secs(30), |index, request| {
        if index % 2 == 0 {
            if index == 2 {
                assert!(String::from_utf8_lossy(&request.body)
                    .contains("refresh_token=private-refresh"));
            }
            support::Reply::json(
                r#"{"access_token":"private-access","refresh_token":"private-refresh","expires_in":0}"#,
            )
        } else {
            assert!(request.head.contains("Bearer private-access"));
            support::Reply::json("{}")
        }
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tokens.json");
    let password = "test password at least twelve";
    let request = ApiRequest {
        url: format!("{}/api", server.url),
        auth: AuthConfig {
            auth_type: AuthType::OAuth2,
            oauth: OAuthConfig {
                persist_tokens: true,
                token_url: format!("{}/token", server.url),
                client_id: "client".into(),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let service = HttpService::default();
    assert!(service
        .execute(&request, &Cancellation::default())
        .error
        .unwrap()
        .contains("Unlock the vault"));
    service.unlock_tokens(&path, password).unwrap();
    assert_eq!(
        service.execute(&request, &Cancellation::default()).status,
        200
    );
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(!raw.contains("private-access") && !raw.contains("private-refresh"));
    service.lock_tokens();
    assert!(service.token_summaries().is_empty());
    let service = HttpService::default();
    assert!(service
        .unlock_tokens(&path, "wrong password long enough")
        .is_err());
    service.unlock_tokens(&path, password).unwrap();
    assert_eq!(
        service.execute(&request, &Cancellation::default()).status,
        200
    );
    server.finish();
    let summary = service.token_summaries();
    assert_eq!(summary.len(), 1);
    assert!(summary[0].refreshable);
    assert!(!serde_json::to_string(&summary)
        .unwrap()
        .contains("private-refresh"));
    service.forget_token(&summary[0].id).unwrap();
    service.lock_tokens();
    service.unlock_tokens(&path, password).unwrap();
    assert!(service.token_summaries().is_empty());
}

#[test]
fn helper_configuration_roundtrips_and_redacts() {
    for kind in [AuthType::Jwt, AuthType::AwsSigV4, AuthType::Digest] {
        let mut c = Collection::new("Auth");
        let mut request = ApiRequest::new("Signed");
        request.url = "https://example.test".into();
        request.auth.auth_type = kind;
        request.auth.jwt.key = "private-signing-key".into();
        request.auth.aws.secret_key = "private-aws-secret".into();
        request.auth.basic_password = "private-password".into();
        c.requests.push(request);
        let exported = rustapi::export::collection(&c, true).unwrap();
        let imported = rustapi::import::json(&serde_json::to_vec(&exported.document).unwrap())
            .unwrap()
            .collection
            .unwrap();
        assert_eq!(
            imported.requests[0].auth.auth_type,
            c.requests[0].auth.auth_type
        );
        let masked = rustapi::export::collection(&c, false)
            .unwrap()
            .document
            .to_string();
        for secret in [
            "private-signing-key",
            "private-aws-secret",
            "private-password",
        ] {
            assert!(!masked.contains(secret));
        }
    }
}

#[test]
fn digest_challenge_and_retry_share_one_deadline() {
    let server = support::Server::new(2, |index, _| support::Reply {
        status: if index == 0 { 401 } else { 200 },
        headers: if index == 0 {
            vec![(
                "WWW-Authenticate".into(),
                "Digest realm=\"test\",nonce=\"nonce\",algorithm=SHA-256,qop=\"auth\"".into(),
            )]
        } else {
            vec![]
        },
        body: vec![],
        delay_ms: 180,
    });
    let request = ApiRequest {
        url: server.url.clone(),
        settings: HttpSettings {
            timeout_ms: 300,
            follow_redirects: false,
            ..Default::default()
        },
        auth: AuthConfig {
            auth_type: AuthType::Digest,
            ..Default::default()
        },
        ..Default::default()
    };
    let service = HttpService::default();
    service.client(&request.settings).unwrap();
    let response = service.execute(&request, &Cancellation::default());
    assert!(response.error.is_some());
    assert!(response.time_ms < 500);
    server.finish();
}
