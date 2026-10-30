//! Fixtures for executable code-generation validation; not part of the product CLI.
use rustapi::{http::*, models::*};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let dir = std::path::Path::new(&args[1]);
    std::fs::create_dir_all(dir)?;
    let base = &args[2];
    let upload = dir.join("upload.bin");
    std::fs::write(&upload, [0, 255, 128, 1, 13, 10])?;
    let second = dir.join("second.bin");
    std::fs::write(&second, [7, 6, 5, 4])?;
    for case in ["json", "text", "form", "urlencoded", "binary", "options"] {
        let mut r = ApiRequest {
            url: format!("{base}/{case}#fragment"),
            method: if case == "options" {
                HttpMethod::OPTIONS
            } else {
                HttpMethod::POST
            },
            ..Default::default()
        };
        r.query_params.push(KeyValue::new("q", "a b&c"));
        r.auth = AuthConfig {
            auth_type: AuthType::ApiKey,
            api_key_name: "api_key".into(),
            api_key_value: "key with spaces".into(),
            api_key_location: ApiKeyLocation::QueryParam,
            ..Default::default()
        };
        r.headers
            .push(KeyValue::new("x-quote", "quote-\"-dollar-$(echo nope)"));
        match case {
            "json" => {
                r.body_type = BodyType::Json;
                r.body = r#"{"ready":true,"nil":null,"name":"O'Reilly ${value} \u0001"}"#.into();
            }
            "text" => {
                r.body_type = BodyType::Text;
                r.body="quotes '\" backtick ` interpolation ${never} newline\ncontrol \u{1} Unicode 🦀".into();
            }
            "form" => {
                r.body_type = BodyType::FormData;
                r.form_data.push(KeyValue::new("name", "Ada & friends"));
                r.form_data[0].content_type = "text/plain; charset=utf-8".into();
                r.form_data.push(KeyValue {
                    file: true,
                    content_type: "application/x-fixture".into(),
                    ..KeyValue::new("upload", upload.to_string_lossy())
                });
                r.form_data.push(KeyValue {
                    file: true,
                    content_type: "application/x-fixture".into(),
                    ..KeyValue::new("upload", second.to_string_lossy())
                });
            }
            "urlencoded" => {
                r.body_type = BodyType::UrlEncoded;
                r.form_data.push(KeyValue::new("name", "Ada & friends"));
            }
            "binary" => {
                r.body_type = BodyType::Binary;
                r.binary_file = upload.to_string_lossy().into();
            }
            _ => {}
        }
        for (lang, ext) in [
            (CodeGenLanguage::Curl, "sh"),
            (CodeGenLanguage::Python, "py"),
            (CodeGenLanguage::JavaScript, "mjs"),
            (CodeGenLanguage::Rust, "rs"),
        ] {
            std::fs::write(
                dir.join(format!("request-{case}.{ext}")),
                generate_code(&r, lang, None),
            )?;
        }
    }
    Ok(())
}
