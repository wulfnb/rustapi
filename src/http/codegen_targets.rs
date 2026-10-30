//! Standalone SDK targets. Shared body bytes preserve multipart ordering and MIME.
use super::code_gen::{multipart_part_headers, CodeGenLanguage};
use crate::models::*;
use anyhow::{ensure, Result};
use std::collections::BTreeMap;
enum Chunk {
    Text(String),
    File(String),
}
fn literal(value: &str, language: CodeGenLanguage) -> String {
    if language == CodeGenLanguage::Java {
        let mut output = String::from("\"");
        for character in value.chars() {
            match character {
                '\\' => output.push_str("\\\\"),
                '"' => output.push_str("\\\""),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                character if character.is_ascii_control() => {
                    output.push_str(&format!("\\{:03o}", character as u32))
                }
                character => output.push(character),
            }
        }
        output.push('"');
        output
    } else {
        let text = serde_json::to_string(value)
            .unwrap()
            .replace('\u{85}', "\\u0085")
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029");
        if language == CodeGenLanguage::Dart {
            text.replace('$', "\\$")
        } else {
            text
        }
    }
}
fn render(template: &str, values: &BTreeMap<&str, String>) -> Result<String> {
    let mut remaining = template;
    let mut output = String::new();
    while let Some(start) = remaining.find("@@") {
        output.push_str(&remaining[..start]);
        let rest = &remaining[start + 2..];
        let end = rest
            .find("@@")
            .ok_or_else(|| anyhow::anyhow!("Invalid snippet template"))?;
        output.push_str(
            values
                .get(&rest[..end])
                .ok_or_else(|| anyhow::anyhow!("Unknown snippet template field"))?,
        );
        remaining = &rest[end + 2..];
    }
    output.push_str(remaining);
    Ok(output)
}
pub(super) fn generate(
    r: &ApiRequest,
    language: CodeGenLanguage,
    url: &str,
    configured: &[(String, String)],
) -> Result<String> {
    ensure!(
        (1..=3_600_000).contains(&r.settings.timeout_ms)
            && (1..=256 * 1024 * 1024).contains(&r.settings.max_response_bytes),
        "Invalid request timeout or response limit"
    );
    let parsed = reqwest::Url::parse(url)?;
    ensure!(
        ["http", "https"].contains(&parsed.scheme()),
        "HTTP(S) URL required"
    );
    ensure!(
        parsed.username().is_empty() && parsed.password().is_none(),
        "Use request authentication fields instead of URL userinfo for this target"
    );
    if language == CodeGenLanguage::Java {
        ensure!(r.settings.verify_tls,"Java target keeps TLS verification enabled; choose another target for an explicit insecure override");
        ensure!(r.settings.identity_file.is_empty(),"Java target requires a configured key store for client certificates; direct PEM identity export is unsupported");
        ensure!(
            !configured.iter().any(|(key, _)| key == "host"),
            "Java HTTP client does not support an explicit Host override"
        );
    }
    if matches!(
        language,
        CodeGenLanguage::CSharp | CodeGenLanguage::Java | CodeGenLanguage::Dart
    ) {
        ensure!(configured.iter().all(|(_,value)|value.is_ascii()),"This SDK target requires ASCII header values; choose Go or Rust for opaque UTF-8 header bytes");
    }
    let mut seen = std::collections::HashSet::new();
    if language == CodeGenLanguage::CSharp {
        ensure!(
            configured.iter().all(|(key, _)| seen.insert(key)),
            "C# HTTP headers combine repeated fields; choose a target that preserves repetition"
        );
    }
    if !r.settings.proxy.is_empty() {
        let proxy = reqwest::Url::parse(&r.settings.proxy)?;
        ensure!(
            ["http", "https"].contains(&proxy.scheme()),
            "Only HTTP(S) proxies are supported by these snippets"
        );
        if matches!(language, CodeGenLanguage::Java | CodeGenLanguage::Dart) {
            ensure!(
                proxy.scheme() == "http",
                "This target supports HTTP proxies; HTTPS proxy transport is unsupported"
            );
        }
        if language == CodeGenLanguage::Java {
            ensure!(
                proxy.username().is_empty() && proxy.password().is_none(),
                "Java target does not export proxy credentials"
            );
        }
    }
    let mut headers = configured.to_vec();
    let mut chunks = vec![];
    match r.body_type {
        BodyType::None => (),
        BodyType::Json | BodyType::Text | BodyType::Xml | BodyType::Html => {
            chunks.push(Chunk::Text(r.body.clone()))
        }
        BodyType::Binary => chunks.push(if r.binary_file.is_empty() {
            Chunk::Text(r.body.clone())
        } else {
            Chunk::File(r.binary_file.clone())
        }),
        BodyType::UrlEncoded => {
            let rows: Vec<_> = r
                .form_data
                .iter()
                .filter(|row| row.enabled && !row.key.is_empty())
                .map(|row| (&row.key, &row.value))
                .collect();
            chunks.push(Chunk::Text(serde_urlencoded::to_string(rows)?));
            if !headers.iter().any(|(key, _)| key == "content-type") {
                headers.push((
                    "content-type".into(),
                    "application/x-www-form-urlencoded".into(),
                ));
            }
        }
        BodyType::FormData => {
            let boundary = format!("----rustapi-{}", uuid::Uuid::new_v4());
            for row in r
                .form_data
                .iter()
                .filter(|row| row.enabled && !row.key.is_empty())
            {
                chunks.push(Chunk::Text(format!(
                    "--{boundary}\r\n{}",
                    multipart_part_headers(row)
                )));
                chunks.push(if row.file {
                    Chunk::File(row.value.clone())
                } else {
                    Chunk::Text(row.value.clone())
                });
                chunks.push(Chunk::Text("\r\n".into()));
            }
            chunks.push(Chunk::Text(format!("--{boundary}--\r\n")));
            headers.push((
                "content-type".into(),
                format!("multipart/form-data; boundary={boundary}"),
            ));
        }
    }
    if language == CodeGenLanguage::Http {
        ensure!(
            !chunks.iter().any(|chunk| matches!(chunk, Chunk::File(_))),
            "Raw HTTP cannot inline upload files; choose an executable target"
        );
        ensure!(
            !headers.iter().any(|(key, _)| key == "transfer-encoding"),
            "Raw HTTP chunked framing is unsupported"
        );
        let body = chunks
            .iter()
            .map(|chunk| match chunk {
                Chunk::Text(text) => text.as_str(),
                _ => unreachable!(),
            })
            .collect::<String>();
        if let Some((_, length)) = headers.iter().find(|(key, _)| key == "content-length") {
            ensure!(
                length.parse::<usize>()? == body.len(),
                "Explicit Content-Length differs from the generated body"
            );
        } else if r.body_type != BodyType::None {
            headers.push(("content-length".into(), body.len().to_string()));
        }
        if !headers.iter().any(|(key, _)| key == "host") {
            let authority = parsed
                .host()
                .ok_or_else(|| anyhow::anyhow!("Missing host"))?
                .to_string();
            headers.insert(
                0,
                (
                    "host".into(),
                    format!(
                        "{authority}{}",
                        parsed
                            .port()
                            .map(|port| format!(":{port}"))
                            .unwrap_or_default()
                    ),
                ),
            );
        }
        let target = format!(
            "{}{}",
            parsed.path(),
            parsed
                .query()
                .map(|query| format!("?{query}"))
                .unwrap_or_default()
        );
        return Ok(format!(
            "{} {} HTTP/1.1\r\n{}\r\n{}",
            r.method.as_str(),
            target,
            headers
                .iter()
                .map(|(key, value)| format!("{key}: {value}\r\n"))
                .collect::<String>(),
            body
        ));
    }
    ensure!(!headers.iter().any(|(key,_)|["content-length","transfer-encoding","trailer","connection","expect","upgrade"].contains(&key.as_str())),"This target lets its transport manage framing headers; remove explicit framing/connection headers or choose raw HTTP");
    let q = |value: &str| literal(value, language);
    let mut body = String::new();
    for chunk in chunks {
        body.push_str(&match (language,chunk){
        (CodeGenLanguage::Go,Chunk::Text(text))=>format!("body.WriteString({});\n",q(&text)),
        (CodeGenLanguage::Go,Chunk::File(path))=>format!("{{ data, err := os.ReadFile({}); if err != nil {{ panic(err) }}; body.Write(data) }}\n",q(&path)),
        (CodeGenLanguage::CSharp,Chunk::Text(text))=>format!("await bodyBuilder.WriteAsync(Encoding.UTF8.GetBytes({}));\n",q(&text)),
        (CodeGenLanguage::CSharp,Chunk::File(path))=>format!("await bodyBuilder.WriteAsync(await File.ReadAllBytesAsync({}));\n",q(&path)),
        (CodeGenLanguage::Java,Chunk::Text(text))=>format!("bodyBuilder.write({}.getBytes(StandardCharsets.UTF_8));\n",q(&text)),
        (CodeGenLanguage::Java,Chunk::File(path))=>format!("bodyBuilder.write(Files.readAllBytes(Path.of({})));\n",q(&path)),
        (CodeGenLanguage::Dart,Chunk::Text(text))=>format!("bodyBuilder.add(utf8.encode({}));\n",q(&text)),
        (CodeGenLanguage::Dart,Chunk::File(path))=>format!("bodyBuilder.add(await File({}).readAsBytes());\n",q(&path)),_=>unreachable!(),
    });
    }
    let headers = headers
        .iter()
        .map(|(key, value)| match language {
            CodeGenLanguage::Go => format!("{{{}, {}}}", q(key), q(value)),
            CodeGenLanguage::CSharp => format!("new({}, {})", q(key), q(value)),
            CodeGenLanguage::Java => format!("new String[]{{{}, {}}}", q(key), q(value)),
            CodeGenLanguage::Dart => format!("[{}, {}]", q(key), q(value)),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let values = BTreeMap::from([
        ("BODY", body),
        ("HEADERS", headers),
        ("URL", q(parsed.as_str())),
        ("METHOD", q(r.method.as_str())),
        ("TIMEOUT", r.settings.timeout_ms.to_string()),
        ("LIMIT", r.settings.max_response_bytes.to_string()),
        ("FOLLOW", r.settings.follow_redirects.to_string()),
        ("INSECURE", (!r.settings.verify_tls).to_string()),
        ("COOKIES", r.settings.use_cookies.to_string()),
        ("PROXY", q(&r.settings.proxy)),
        ("CA", q(&r.settings.ca_file)),
        ("IDENTITY", q(&r.settings.identity_file)),
    ]);
    render(
        match language {
            CodeGenLanguage::Go => include_str!("templates/request.go.txt"),
            CodeGenLanguage::CSharp => include_str!("templates/request.cs.txt"),
            CodeGenLanguage::Java => include_str!("templates/RequestSample.java.txt"),
            CodeGenLanguage::Dart => include_str!("templates/request.dart.txt"),
            _ => unreachable!(),
        },
        &values,
    )
}
