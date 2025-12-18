//! Code generation for various languages

use crate::models::{ApiRequest, BodyType, Environment, AuthType};

/// Code generation target language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CodeGenLanguage {
    #[default]
    Curl,
    Python,
    JavaScript,
    Rust,
}

impl CodeGenLanguage {
    pub fn all() -> &'static [CodeGenLanguage] {
        &[
            CodeGenLanguage::Curl,
            CodeGenLanguage::Python,
            CodeGenLanguage::JavaScript,
            CodeGenLanguage::Rust,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CodeGenLanguage::Curl => "cURL",
            CodeGenLanguage::Python => "Python",
            CodeGenLanguage::JavaScript => "JavaScript",
            CodeGenLanguage::Rust => "Rust",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            CodeGenLanguage::Curl => "sh",
            CodeGenLanguage::Python => "py",
            CodeGenLanguage::JavaScript => "js",
            CodeGenLanguage::Rust => "rs",
        }
    }
}

/// Generate code for a request
pub fn generate_code(
    request: &ApiRequest,
    language: CodeGenLanguage,
    environment: Option<&Environment>,
) -> String {
    match language {
        CodeGenLanguage::Curl => generate_curl(request, environment),
        CodeGenLanguage::Python => generate_python(request, environment),
        CodeGenLanguage::JavaScript => generate_javascript(request, environment),
        CodeGenLanguage::Rust => generate_rust(request, environment),
    }
}

/// Generate cURL command
fn generate_curl(request: &ApiRequest, environment: Option<&Environment>) -> String {
    let mut parts = vec!["curl".to_string()];
    
    // Method
    if request.method != crate::models::HttpMethod::GET {
        parts.push(format!("-X {}", request.method.as_str()));
    }
    
    // URL
    let url = substitute(environment, &request.url);
    
    // Query params
    let mut full_url = url.clone();
    let enabled_params: Vec<_> = request.query_params.iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .collect();
    
    if !enabled_params.is_empty() {
        let query = enabled_params.iter()
            .map(|p| format!("{}={}", 
                urlencoding::encode(&p.key), 
                urlencoding::encode(&substitute(environment, &p.value))))
            .collect::<Vec<_>>()
            .join("&");
        
        if full_url.contains('?') {
            full_url = format!("{}&{}", full_url, query);
        } else {
            full_url = format!("{}?{}", full_url, query);
        }
    }
    
    // Auth headers
    match request.auth.auth_type {
        AuthType::BearerToken if !request.auth.bearer_token.is_empty() => {
            let token = substitute(environment, &request.auth.bearer_token);
            parts.push(format!("-H \"Authorization: Bearer {}\"", token));
        }
        AuthType::BasicAuth => {
            let user = substitute(environment, &request.auth.basic_username);
            let pass = substitute(environment, &request.auth.basic_password);
            parts.push(format!("-u \"{}:{}\"", user, pass));
        }
        AuthType::ApiKey if request.auth.api_key_location == crate::models::ApiKeyLocation::Header => {
            let key = substitute(environment, &request.auth.api_key_name);
            let value = substitute(environment, &request.auth.api_key_value);
            parts.push(format!("-H \"{}: {}\"", key, value));
        }
        _ => {}
    }
    
    // Headers
    for header in &request.headers {
        if header.enabled && !header.key.is_empty() {
            let value = substitute(environment, &header.value);
            parts.push(format!("-H \"{}: {}\"", header.key, value));
        }
    }
    
    // Body
    match request.body_type {
        BodyType::Json => {
            parts.push("-H \"Content-Type: application/json\"".to_string());
            let body = substitute(environment, &request.body);
            parts.push(format!("-d '{}'", body.replace('\'', "'\\''")));
        }
        BodyType::Text => {
            parts.push("-H \"Content-Type: text/plain\"".to_string());
            let body = substitute(environment, &request.body);
            parts.push(format!("-d '{}'", body.replace('\'', "'\\''")));
        }
        BodyType::UrlEncoded => {
            for kv in &request.form_data {
                if kv.enabled && !kv.key.is_empty() {
                    let value = substitute(environment, &kv.value);
                    parts.push(format!("-d \"{}={}\"", kv.key, value));
                }
            }
        }
        BodyType::FormData => {
            for kv in &request.form_data {
                if kv.enabled && !kv.key.is_empty() {
                    let value = substitute(environment, &kv.value);
                    parts.push(format!("-F \"{}={}\"", kv.key, value));
                }
            }
        }
        _ => {}
    }
    
    parts.push(format!("\"{}\"", full_url));
    
    parts.join(" \\\n  ")
}

/// Generate Python requests code
fn generate_python(request: &ApiRequest, environment: Option<&Environment>) -> String {
    let mut code = String::from("import requests\n\n");
    
    let url = substitute(environment, &request.url);
    code.push_str(&format!("url = \"{}\"\n\n", url));
    
    // Headers
    let mut headers = vec![];
    
    // Auth
    match request.auth.auth_type {
        AuthType::BearerToken if !request.auth.bearer_token.is_empty() => {
            let token = substitute(environment, &request.auth.bearer_token);
            headers.push(format!("    \"Authorization\": \"Bearer {}\"", token));
        }
        AuthType::ApiKey if request.auth.api_key_location == crate::models::ApiKeyLocation::Header => {
            let key = substitute(environment, &request.auth.api_key_name);
            let value = substitute(environment, &request.auth.api_key_value);
            headers.push(format!("    \"{}\": \"{}\"", key, value));
        }
        _ => {}
    }
    
    for header in &request.headers {
        if header.enabled && !header.key.is_empty() {
            let value = substitute(environment, &header.value);
            headers.push(format!("    \"{}\": \"{}\"", header.key, value));
        }
    }
    
    if !headers.is_empty() {
        code.push_str("headers = {\n");
        code.push_str(&headers.join(",\n"));
        code.push_str("\n}\n\n");
    }
    
    // Query params
    let enabled_params: Vec<_> = request.query_params.iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .collect();
    
    if !enabled_params.is_empty() {
        code.push_str("params = {\n");
        for p in &enabled_params {
            let value = substitute(environment, &p.value);
            code.push_str(&format!("    \"{}\": \"{}\",\n", p.key, value));
        }
        code.push_str("}\n\n");
    }
    
    // Body
    let has_body = matches!(request.body_type, BodyType::Json | BodyType::Text | BodyType::FormData | BodyType::UrlEncoded);
    if has_body {
        match request.body_type {
            BodyType::Json => {
                let body = substitute(environment, &request.body);
                code.push_str(&format!("data = {}\n\n", body));
            }
            BodyType::Text => {
                let body = substitute(environment, &request.body);
                code.push_str(&format!("data = \"\"\"{}\"\"\"\n\n", body));
            }
            BodyType::UrlEncoded | BodyType::FormData => {
                code.push_str("data = {\n");
                for kv in &request.form_data {
                    if kv.enabled && !kv.key.is_empty() {
                        let value = substitute(environment, &kv.value);
                        code.push_str(&format!("    \"{}\": \"{}\",\n", kv.key, value));
                    }
                }
                code.push_str("}\n\n");
            }
            _ => {}
        }
    }
    
    // Auth for basic
    let auth_str = if request.auth.auth_type == AuthType::BasicAuth {
        let user = substitute(environment, &request.auth.basic_username);
        let pass = substitute(environment, &request.auth.basic_password);
        format!(", auth=(\"{}\", \"{}\")", user, pass)
    } else {
        String::new()
    };
    
    // Request call
    let method = request.method.as_str().to_lowercase();
    let mut call_parts = vec![format!("response = requests.{}(url", method)];
    
    if !headers.is_empty() {
        call_parts.push(", headers=headers".to_string());
    }
    if !enabled_params.is_empty() {
        call_parts.push(", params=params".to_string());
    }
    if has_body {
        if request.body_type == BodyType::Json {
            call_parts.push(", json=data".to_string());
        } else {
            call_parts.push(", data=data".to_string());
        }
    }
    call_parts.push(auth_str);
    call_parts.push(")".to_string());
    
    code.push_str(&call_parts.join(""));
    code.push_str("\n\nprint(response.status_code)\nprint(response.text)\n");
    
    code
}

/// Generate JavaScript fetch code
fn generate_javascript(request: &ApiRequest, environment: Option<&Environment>) -> String {
    let url = substitute(environment, &request.url);
    
    let mut code = String::new();
    
    // Build URL with params
    let enabled_params: Vec<_> = request.query_params.iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .collect();
    
    if !enabled_params.is_empty() {
        code.push_str(&format!("const url = new URL(\"{}\");\n", url));
        for p in &enabled_params {
            let value = substitute(environment, &p.value);
            code.push_str(&format!("url.searchParams.append(\"{}\", \"{}\");\n", p.key, value));
        }
        code.push('\n');
    } else {
        code.push_str(&format!("const url = \"{}\";\n\n", url));
    }
    
    // Options object
    code.push_str("const options = {\n");
    code.push_str(&format!("  method: \"{}\",\n", request.method.as_str()));
    
    // Headers
    let mut headers = vec![];
    
    match request.auth.auth_type {
        AuthType::BearerToken if !request.auth.bearer_token.is_empty() => {
            let token = substitute(environment, &request.auth.bearer_token);
            headers.push(format!("    \"Authorization\": \"Bearer {}\"", token));
        }
        AuthType::BasicAuth => {
            let user = substitute(environment, &request.auth.basic_username);
            let pass = substitute(environment, &request.auth.basic_password);
            headers.push(format!("    \"Authorization\": \"Basic \" + btoa(\"{}:{}\")", user, pass));
        }
        AuthType::ApiKey if request.auth.api_key_location == crate::models::ApiKeyLocation::Header => {
            let key = substitute(environment, &request.auth.api_key_name);
            let value = substitute(environment, &request.auth.api_key_value);
            headers.push(format!("    \"{}\": \"{}\"", key, value));
        }
        _ => {}
    }
    
    for header in &request.headers {
        if header.enabled && !header.key.is_empty() {
            let value = substitute(environment, &header.value);
            headers.push(format!("    \"{}\": \"{}\"", header.key, value));
        }
    }
    
    if request.body_type == BodyType::Json {
        headers.push("    \"Content-Type\": \"application/json\"".to_string());
    }
    
    if !headers.is_empty() {
        code.push_str("  headers: {\n");
        code.push_str(&headers.join(",\n"));
        code.push_str("\n  },\n");
    }
    
    // Body
    match request.body_type {
        BodyType::Json => {
            let body = substitute(environment, &request.body);
            code.push_str(&format!("  body: JSON.stringify({}),\n", body));
        }
        BodyType::Text => {
            let body = substitute(environment, &request.body);
            code.push_str(&format!("  body: `{}`,\n", body));
        }
        BodyType::UrlEncoded => {
            code.push_str("  body: new URLSearchParams({\n");
            for kv in &request.form_data {
                if kv.enabled && !kv.key.is_empty() {
                    let value = substitute(environment, &kv.value);
                    code.push_str(&format!("    \"{}\": \"{}\",\n", kv.key, value));
                }
            }
            code.push_str("  }),\n");
        }
        _ => {}
    }
    
    code.push_str("};\n\n");
    
    // Fetch call
    code.push_str("fetch(url, options)\n");
    code.push_str("  .then(response => response.json())\n");
    code.push_str("  .then(data => console.log(data))\n");
    code.push_str("  .catch(error => console.error('Error:', error));\n");
    
    code
}

/// Generate Rust reqwest code
fn generate_rust(request: &ApiRequest, environment: Option<&Environment>) -> String {
    let url = substitute(environment, &request.url);
    
    let mut code = String::from("use reqwest;\n\n");
    code.push_str("#[tokio::main]\n");
    code.push_str("async fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
    code.push_str("    let client = reqwest::Client::new();\n\n");
    
    let method = request.method.as_str().to_lowercase();
    code.push_str(&format!("    let response = client.{}(\"{}\")\n", method, url));
    
    // Auth
    match request.auth.auth_type {
        AuthType::BearerToken if !request.auth.bearer_token.is_empty() => {
            let token = substitute(environment, &request.auth.bearer_token);
            code.push_str(&format!("        .bearer_auth(\"{}\")\n", token));
        }
        AuthType::BasicAuth => {
            let user = substitute(environment, &request.auth.basic_username);
            let pass = substitute(environment, &request.auth.basic_password);
            code.push_str(&format!("        .basic_auth(\"{}\", Some(\"{}\"))\n", user, pass));
        }
        AuthType::ApiKey if request.auth.api_key_location == crate::models::ApiKeyLocation::Header => {
            let key = substitute(environment, &request.auth.api_key_name);
            let value = substitute(environment, &request.auth.api_key_value);
            code.push_str(&format!("        .header(\"{}\", \"{}\")\n", key, value));
        }
        _ => {}
    }
    
    // Headers
    for header in &request.headers {
        if header.enabled && !header.key.is_empty() {
            let value = substitute(environment, &header.value);
            code.push_str(&format!("        .header(\"{}\", \"{}\")\n", header.key, value));
        }
    }
    
    // Query params
    for p in &request.query_params {
        if p.enabled && !p.key.is_empty() {
            let value = substitute(environment, &p.value);
            code.push_str(&format!("        .query(&[(\"{}\", \"{}\")])\n", p.key, value));
        }
    }
    
    // Body
    match request.body_type {
        BodyType::Json => {
            let body = substitute(environment, &request.body);
            code.push_str(&format!("        .body(r#\"{}\"#)\n", body));
            code.push_str("        .header(\"Content-Type\", \"application/json\")\n");
        }
        BodyType::Text => {
            let body = substitute(environment, &request.body);
            code.push_str(&format!("        .body(\"{}\")\n", body));
        }
        BodyType::UrlEncoded => {
            code.push_str("        .form(&[\n");
            for kv in &request.form_data {
                if kv.enabled && !kv.key.is_empty() {
                    let value = substitute(environment, &kv.value);
                    code.push_str(&format!("            (\"{}\", \"{}\"),\n", kv.key, value));
                }
            }
            code.push_str("        ])\n");
        }
        _ => {}
    }
    
    code.push_str("        .send()\n");
    code.push_str("        .await?;\n\n");
    code.push_str("    println!(\"Status: {}\", response.status());\n");
    code.push_str("    println!(\"Body: {}\", response.text().await?);\n\n");
    code.push_str("    Ok(())\n");
    code.push_str("}\n");
    
    code
}

/// Helper to substitute environment variables
fn substitute(environment: Option<&Environment>, value: &str) -> String {
    if let Some(env) = environment {
        env.substitute(value)
    } else {
        value.to_string()
    }
}

