//! Standalone, script-free API documentation. All output is local unless published explicitly.
use crate::models::*;
use anyhow::{ensure, Result};
pub fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn fence(value: &str) -> String {
    let length = value
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
        .max(2)
        + 1;
    let marker = "`".repeat(length);
    format!("{marker}\n{value}\n{marker}\n")
}
pub struct Documentation {
    pub markdown: String,
    pub html: String,
}
pub fn generate(collection: &Collection, include_credentials: bool) -> Result<Documentation> {
    validate_collection(collection)?;
    let source = if include_credentials {
        collection.clone()
    } else {
        crate::secrets::redact_collection(collection)
    };
    let mut markdown = format!(
        "# {}\n\n{}\n\n",
        escape(&source.name),
        escape(&source.description)
    );
    let mut html=format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\"><title>{}</title><style>body{{font:16px system-ui;max-width:980px;margin:40px auto;padding:0 20px;color:#243047}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;background:#f1f4f8;padding:16px}}article{{border-top:1px solid #ddd;margin-top:32px}}nav a{{display:block;margin:6px 0}}.method{{color:#a33d00}}h1,h2{{overflow-wrap:anywhere}}</style><h1>{}</h1><pre>{}</pre><nav>",escape(&source.name),escape(&source.name),escape(&source.description));
    let requests = source.ordered_requests(None);
    for (index, request) in requests.iter().enumerate() {
        html.push_str(&format!(
            "<a href=\"#request-{index}\">{} {}</a>",
            escape(request.method.as_str()),
            escape(&request.name)
        ));
    }
    html.push_str("</nav><main>");
    for (index, request) in requests.iter().enumerate() {
        markdown.push_str(&format!(
            "## {} {}\n\n{}\n\n",
            escape(request.method.as_str()),
            escape(&request.name),
            escape(&request.description)
        ));
        markdown.push_str(&fence(&request.url));
        html.push_str(&format!("<article id=\"request-{index}\"><h2><span class=\"method\">{}</span> {}</h2><pre>{}</pre><h3>URL</h3><pre>{}</pre>",escape(request.method.as_str()),escape(&request.name),escape(&request.description),escape(&request.url)));
        for (label, rows) in [
            ("Headers", &request.headers),
            ("Query parameters", &request.query_params),
            ("Path parameters", &request.path_params),
            ("Form fields", &request.form_data),
        ] {
            if rows.is_empty() {
                continue;
            }
            let content = rows
                .iter()
                .map(|r| {
                    format!(
                        "{}{}: {}{}",
                        if r.enabled { "" } else { "[disabled] " },
                        r.key,
                        if r.file { "[local upload]" } else { &r.value },
                        if r.description.is_empty() {
                            String::new()
                        } else {
                            format!(" — {}", r.description)
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            markdown.push_str(&format!("\n### {label}\n\n{}", fence(&content)));
            html.push_str(&format!("<h3>{label}</h3><pre>{}</pre>", escape(&content)));
        }
        markdown.push_str(&format!(
            "\nAuthentication: {}\n",
            request.auth.auth_type.as_str()
        ));
        html.push_str(&format!(
            "<p>Authentication: {}</p>",
            escape(request.auth.auth_type.as_str())
        ));
        if !request.body.is_empty() {
            markdown.push_str(&format!("\n### Request body\n\n{}", fence(&request.body)));
            html.push_str(&format!(
                "<h3>Request body</h3><pre>{}</pre>",
                escape(&request.body)
            ));
        }
        for example in &request.examples {
            markdown.push_str(&format!(
                "\n### Example: {} ({})\n\n{}",
                escape(&example.name),
                example.response.status,
                fence(&example.response.body)
            ));
            html.push_str(&format!(
                "<h3>Example: {} ({})</h3><pre>{}</pre>",
                escape(&example.name),
                example.response.status,
                escape(&example.response.body)
            ));
        }
        html.push_str("</article>");
        ensure!(
            markdown.len() <= 32 * 1024 * 1024 && html.len() <= 64 * 1024 * 1024,
            "Documentation exceeds output limits"
        );
    }
    html.push_str("</main></html>");
    Ok(Documentation { markdown, html })
}
pub fn write(
    collection: &Collection,
    directory: &std::path::Path,
    include_credentials: bool,
) -> Result<()> {
    ensure!(
        !directory.exists(),
        "Documentation output directory already exists"
    );
    let docs = generate(collection, include_credentials)?;
    let parent = directory
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temporary = tempfile::tempdir_in(parent)?;
    crate::workspace::atomic_write(&temporary.path().join("index.html"), docs.html.as_bytes())?;
    crate::workspace::atomic_write(
        &temporary.path().join("README.md"),
        docs.markdown.as_bytes(),
    )?;
    std::fs::rename(temporary.path(), directory)?;
    Ok(())
}
