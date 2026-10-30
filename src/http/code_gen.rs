//! Code samples use the same resolved variables, URL, auth and body as execution.
use crate::models::*;
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CodeGenLanguage {
    #[default]
    Curl,
    Python,
    JavaScript,
    Rust,
    Http,
    Go,
    CSharp,
    Java,
    Dart,
}
impl CodeGenLanguage {
    pub fn all() -> &'static [Self] {
        &[
            Self::Curl,
            Self::Python,
            Self::JavaScript,
            Self::Rust,
            Self::Http,
            Self::Go,
            Self::CSharp,
            Self::Java,
            Self::Dart,
        ]
    }
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Curl => "sh",
            Self::Python => "py",
            Self::JavaScript => "js",
            Self::Rust => "rs",
            Self::Http => "http",
            Self::Go => "go",
            Self::CSharp => "cs",
            Self::Java => "java",
            Self::Dart => "dart",
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Curl => "cURL",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::Rust => "Rust",
            Self::Http => "HTTP",
            Self::Go => "Go",
            Self::CSharp => "C#",
            Self::Java => "Java",
            Self::Dart => "Dart",
        }
    }
}
fn quoted(v: &str) -> String {
    serde_json::to_string(v).expect("String serialization cannot fail")
}
fn shell(v: &str) -> String {
    format!("'{}'", v.replace('\'', "'\\''"))
}
pub(super) fn multipart_part_headers(field: &KeyValue) -> String {
    fn escape(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
    }
    let mut head = format!(
        "Content-Disposition: form-data; name=\"{}\"",
        escape(&field.key)
    );
    if field.file {
        let name = std::path::Path::new(&field.value)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        head.push_str(&format!("; filename=\"{}\"", escape(&name)));
    }
    head.push_str("\r\n");
    if !field.content_type.is_empty() {
        head.push_str(&format!("Content-Type: {}\r\n", field.content_type));
    }
    head.push_str("\r\n");
    head
}
pub fn generate_code(
    request: &ApiRequest,
    language: CodeGenLanguage,
    env: Option<&Environment>,
) -> String {
    match crate::variables::Variables::for_request(request, env).resolve_request(request) {
        Ok(r) => generate(&r, language),
        Err(e) => format!("Cannot generate code: {e}"),
    }
}
fn generate(r: &ApiRequest, language: CodeGenLanguage) -> String {
    if matches!(
        r.auth.auth_type,
        AuthType::Inherit | AuthType::OAuth2 | AuthType::Unsupported
    ) {
        return "Resolve inherited auth or acquire an OAuth token and choose Bearer Token before exporting code.".into();
    }
    let mut params = r.query_params.clone();
    if let Some((k, v)) = r.auth.get_api_key_query_param() {
        params.push(KeyValue::new(k, v));
    }
    let url = super::client::build_url_with_params(&r.url, &params, None);
    let configured = match super::effective_headers(r) {
        Ok(headers) => headers,
        Err(error) => return format!("Cannot generate code: {error}"),
    };
    let mut seen = std::collections::HashSet::new();
    let repeated = configured.iter().any(|(key, _)| !seen.insert(key));
    if repeated
        && matches!(
            language,
            CodeGenLanguage::Python | CodeGenLanguage::JavaScript
        )
    {
        return "Cannot generate code: this target cannot preserve repeated header fields; choose cURL or Rust.".into();
    }
    let headers: BTreeMap<_, _> = configured.iter().cloned().collect();
    let form: Vec<_> = r
        .form_data
        .iter()
        .filter(|v| v.enabled && !v.key.is_empty())
        .collect();
    if r.body_type == BodyType::FormData
        && form.iter().any(|field| {
            !field.content_type.is_empty()
                && reqwest::multipart::Part::text("")
                    .mime_str(&field.content_type)
                    .is_err()
        })
    {
        return "Cannot generate code: Invalid multipart content type".into();
    }
    match language {
        CodeGenLanguage::Http
        | CodeGenLanguage::Go
        | CodeGenLanguage::CSharp
        | CodeGenLanguage::Java
        | CodeGenLanguage::Dart => {
            match super::codegen_targets::generate(r, language, &url, &configured) {
                Ok(code) => code,
                Err(error) => format!("Cannot generate code: {error}"),
            }
        }
        CodeGenLanguage::Curl => {
            let mut prelude = String::new();
            let mut p = vec![format!("curl -X {}", r.method.as_str())];
            for (k, v) in &configured {
                p.push(format!("-H {}", shell(&format!("{k}: {v}"))));
            }
            match r.body_type {
                BodyType::Json | BodyType::Text | BodyType::Xml | BodyType::Html => {
                    p.push(format!("--data-raw {}", shell(&r.body)))
                }
                BodyType::Binary => p.push(format!(
                    "--data-binary {}",
                    shell(&if r.binary_file.is_empty() {
                        r.body.clone()
                    } else {
                        format!("@{}", r.binary_file)
                    })
                )),
                BodyType::UrlEncoded => {
                    for f in &form {
                        p.push(format!(
                            "--data-urlencode {}",
                            shell(&format!("{}={}", f.key, f.value))
                        ));
                    }
                }
                BodyType::FormData => {
                    let boundary = format!("----rustapi-{}", uuid::Uuid::new_v4());
                    prelude.push_str("set -eu\nbody=$(mktemp)\ntrap 'rm -f \"$body\"' EXIT\n{\n");
                    for field in &form {
                        let head = format!("--{boundary}\r\n{}", multipart_part_headers(field));
                        prelude.push_str(&format!("  printf '%s' {}\n", shell(&head)));
                        if field.file {
                            prelude.push_str(&format!("  cat -- {}\n", shell(&field.value)));
                        } else {
                            prelude.push_str(&format!("  printf '%s' {}\n", shell(&field.value)));
                        }
                        prelude.push_str("  printf '\\r\\n'\n");
                    }
                    prelude.push_str(&format!(
                        "  printf '%s' {}\n}} > \"$body\"\n",
                        shell(&format!("--{boundary}--\r\n"))
                    ));
                    p.push(format!(
                        "-H {}",
                        shell(&format!(
                            "Content-Type: multipart/form-data; boundary={boundary}"
                        ))
                    ));
                    p.push("--data-binary \"@$body\"".into());
                }
                BodyType::None => {}
            }
            if !r.settings.verify_tls {
                p.push("--insecure".into());
            }
            if r.settings.follow_redirects {
                p.push("--location".into());
            }
            p.push(format!(
                "--max-time {}",
                r.settings.timeout_ms as f64 / 1000.
            ));
            if !r.settings.proxy.is_empty() {
                p.push(format!("--proxy {}", shell(&r.settings.proxy)));
            }
            if !r.settings.ca_file.is_empty() {
                p.push(format!("--cacert {}", shell(&r.settings.ca_file)));
            }
            if !r.settings.identity_file.is_empty() {
                p.push(format!("--cert {}", shell(&r.settings.identity_file)));
            }
            p.push(shell(&url));
            prelude + &p.join(" \\\n  ")
        }
        CodeGenLanguage::Python => {
            let mut p=format!("import json\nimport requests\nfrom contextlib import ExitStack\n\nurl = {}\nheaders = json.loads({})\nwith ExitStack() as stack:\n",quoted(&url),quoted(&serde_json::to_string(&headers).unwrap()));
            let mut args = String::new();
            match r.body_type {
                BodyType::Json | BodyType::Text | BodyType::Xml | BodyType::Html => {
                    args = format!(", data={}.encode('utf-8')", quoted(&r.body))
                }
                BodyType::Binary => {
                    args = if r.binary_file.is_empty() {
                        format!(", data={}.encode('utf-8')", quoted(&r.body))
                    } else {
                        format!(
                            ", data=stack.enter_context(open({}, 'rb'))",
                            quoted(&r.binary_file)
                        )
                    }
                }
                BodyType::UrlEncoded => {
                    args = format!(
                        ", data=[{}]",
                        form.iter()
                            .map(|f| format!("({}, {})", quoted(&f.key), quoted(&f.value)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
                BodyType::FormData => {
                    args = format!(
                        ", files=[{}]",
                        form.iter()
                            .map(|f| {
                                let name = if f.file {
                                    quoted(
                                        std::path::Path::new(&f.value)
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .as_ref(),
                                    )
                                } else {
                                    "None".into()
                                };
                                let value = if f.file {
                                    format!("stack.enter_context(open({}, 'rb'))", quoted(&f.value))
                                } else {
                                    quoted(&f.value)
                                };
                                let mime = if f.content_type.is_empty() {
                                    "None".into()
                                } else {
                                    quoted(&f.content_type)
                                };
                                format!("({}, ({}, {}, {}))", quoted(&f.key), name, value, mime)
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
                BodyType::None => {}
            }
            let verify = if !r.settings.verify_tls {
                "False".into()
            } else if !r.settings.ca_file.is_empty() {
                quoted(&r.settings.ca_file)
            } else {
                "True".into()
            };
            p.push_str(&format!("    response = requests.{}(url, headers=headers{}, timeout={}, allow_redirects={}, verify={}{}{})\n    print(response.status_code)\n    print(response.text)\n",r.method.as_str().to_lowercase(),args,r.settings.timeout_ms as f64/1000.,if r.settings.follow_redirects{"True"}else{"False"},verify,if r.settings.proxy.is_empty(){String::new()}else{format!(", proxies={{'http': {}, 'https': {}}}",quoted(&r.settings.proxy),quoted(&r.settings.proxy))},if r.settings.identity_file.is_empty(){String::new()}else{format!(", cert={}",quoted(&r.settings.identity_file))}));
            p
        }
        CodeGenLanguage::JavaScript => {
            let mut p=format!("// Fetch API. Browser CORS and forbidden-header rules apply.\n// Use Node 20+ for file imports. Custom TLS/proxy options require a runtime-specific dispatcher.\nconst url = {};\nconst headers = {};\n",quoted(&url),serde_json::to_string_pretty(&headers).unwrap());
            let body = match r.body_type {
                BodyType::Json | BodyType::Text | BodyType::Xml | BodyType::Html => quoted(&r.body),
                BodyType::Binary => {
                    if r.binary_file.is_empty() {
                        quoted(&r.body)
                    } else {
                        p.push_str(&format!("const {{readFile}} = await import('node:fs/promises');\nconst body = await readFile({});\n",quoted(&r.binary_file)));
                        "body".into()
                    }
                }
                BodyType::UrlEncoded => format!(
                    "new URLSearchParams({})",
                    serde_json::to_string(
                        &form.iter().map(|f| (&f.key, &f.value)).collect::<Vec<_>>()
                    )
                    .unwrap()
                ),
                BodyType::FormData => {
                    let boundary = format!("----rustapi-{}", uuid::Uuid::new_v4());
                    p.push_str("const parts = [];\n");
                    for field in &form {
                        p.push_str(&format!(
                            "parts.push({});\n",
                            quoted(&format!(
                                "--{boundary}\r\n{}",
                                multipart_part_headers(field)
                            ))
                        ));
                        if field.file {
                            p.push_str(&format!("parts.push(await (await import('node:fs/promises')).readFile({}));\n", quoted(&field.value)));
                        } else {
                            p.push_str(&format!("parts.push({});\n", quoted(&field.value)));
                        }
                        p.push_str("parts.push('\\r\\n');\n");
                    }
                    p.push_str(&format!("parts.push({});\nconst body = new Blob(parts);\nheaders['content-type'] = {};\n", quoted(&format!("--{boundary}--\r\n")), quoted(&format!("multipart/form-data; boundary={boundary}"))));
                    "body".into()
                }
                BodyType::None => String::new(),
            };
            let body = if body.is_empty() {
                String::new()
            } else {
                format!(", body: {body}")
            };
            p.push_str(&format!("const options = {{method: {}, headers, redirect: {}, signal: AbortSignal.timeout({}){}}};\nconst response = await fetch(url, options);\nconsole.log(response.status);\nconsole.log(await response.text());\n",quoted(r.method.as_str()),quoted(if r.settings.follow_redirects{"follow"}else{"manual"}),r.settings.timeout_ms,body));
            p
        }
        CodeGenLanguage::Rust => {
            fn quoted(v: &str) -> String {
                format!("{v:?}")
            }
            let mut p=String::from("// Dependencies: reqwest 0.12 with multipart,rustls-tls; tokio with full\nuse reqwest;\n#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
            p.push_str(&format!("    let mut config = reqwest::Client::builder().use_rustls_tls().timeout(std::time::Duration::from_millis({})).danger_accept_invalid_certs({}).redirect({});\n",r.settings.timeout_ms,!r.settings.verify_tls,if r.settings.follow_redirects{"reqwest::redirect::Policy::limited(10)"}else{"reqwest::redirect::Policy::none()"}));
            if !r.settings.proxy.is_empty() {
                p.push_str(&format!(
                    "    config = config.proxy(reqwest::Proxy::all({})?);\n",
                    quoted(&r.settings.proxy)
                ));
            }
            if !r.settings.ca_file.is_empty() {
                p.push_str(&format!("    for cert in reqwest::Certificate::from_pem_bundle(&std::fs::read({})?)? {{ config = config.add_root_certificate(cert); }}\n",quoted(&r.settings.ca_file)));
            }
            if !r.settings.identity_file.is_empty() {
                p.push_str(&format!("    config = config.identity(reqwest::Identity::from_pem(&std::fs::read({})?)?);\n",quoted(&r.settings.identity_file)));
            }
            let method_call = match &r.method {
                HttpMethod::GET => "reqwest::Method::GET".to_string(),
                HttpMethod::POST => "reqwest::Method::POST".to_string(),
                HttpMethod::PUT => "reqwest::Method::PUT".to_string(),
                HttpMethod::DELETE => "reqwest::Method::DELETE".to_string(),
                HttpMethod::PATCH => "reqwest::Method::PATCH".to_string(),
                HttpMethod::HEAD => "reqwest::Method::HEAD".to_string(),
                HttpMethod::OPTIONS => "reqwest::Method::OPTIONS".to_string(),
                HttpMethod::Custom(name) => format!("reqwest::Method::from_bytes(b{:?})?", name),
            };
            p.push_str(&format!("    let client = config.build()?;\n    let mut request = client.request({}, {});\n",method_call,quoted(&url)));
            for (k, v) in configured {
                p.push_str(&format!(
                    "    request = request.header({}, {});\n",
                    quoted(&k),
                    quoted(&v)
                ));
            }
            match r.body_type {
                BodyType::Json | BodyType::Text | BodyType::Xml | BodyType::Html => p.push_str(
                    &format!("    request = request.body({});\n", quoted(&r.body)),
                ),
                BodyType::Binary => p.push_str(&format!(
                    "    request = request.body({});\n",
                    if r.binary_file.is_empty() {
                        format!("{}.as_bytes().to_vec()", quoted(&r.body))
                    } else {
                        format!("std::fs::read({})?", quoted(&r.binary_file))
                    }
                )),
                BodyType::UrlEncoded => p.push_str(&format!(
                    "    request = request.form(&[{}]);\n",
                    form.iter()
                        .map(|f| format!("({}, {})", quoted(&f.key), quoted(&f.value)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
                BodyType::FormData => {
                    p.push_str("    let mut form = reqwest::multipart::Form::new();\n");
                    for f in form {
                        let part = if f.file {
                            format!(
                                "reqwest::multipart::Part::bytes(std::fs::read({})?).file_name({})",
                                quoted(&f.value),
                                quoted(
                                    std::path::Path::new(&f.value)
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .as_ref()
                                )
                            )
                        } else {
                            format!("reqwest::multipart::Part::text({})", quoted(&f.value))
                        };
                        let mime = if f.content_type.is_empty() {
                            String::new()
                        } else {
                            format!(".mime_str({})?", quoted(&f.content_type))
                        };
                        p.push_str(&format!(
                            "    form = form.part({}, {}{});\n",
                            quoted(&f.key),
                            part,
                            mime
                        ));
                    }
                    p.push_str("    request = request.multipart(form);\n");
                }
                BodyType::None => {}
            }
            p.push_str("    let response = request.send().await?;\n    println!(\"Status: {}\", response.status());\n    println!(\"Body: {}\", response.text().await?);\n    Ok(())\n}\n");
            p
        }
    }
}
