//! Portable v2.1 JSON export. Native-only fields use a versioned extension.
use crate::models::*;
use anyhow::{ensure, Result};
use serde_json::{json, Value};
#[derive(Debug)]
pub struct PortableExport {
    pub document: Value,
    pub warnings: Vec<String>,
}
pub fn rows(rows: &[KeyValue]) -> Value {
    Value::Array(rows.iter().map(|row| {
        let mut object:serde_json::Map<String,Value>=row.extensions.clone().into_iter().collect();
        object.extend(serde_json::from_value::<serde_json::Map<String,Value>>(json!({"key":row.key,"value":row.value,"disabled":!row.enabled,"description":encode_description(&row.description,&row.description_metadata)})).unwrap());
        if !row.value_type.is_empty(){object.insert("type".into(),row.value_type.clone().into());}
        if !row.content_type.is_empty(){object.insert("contentType".into(),row.content_type.clone().into());}
        Value::Object(object)
    }).collect())
}
fn auth(auth: &AuthConfig) -> Result<Value> {
    let (kind, fields) = match auth.auth_type {
        AuthType::None => return Ok(json!({"type":"noauth"})),
        AuthType::Inherit => return Ok(Value::Null),
        AuthType::Unsupported => {
            anyhow::bail!("Configure unsupported imported authentication before portable export")
        }
        AuthType::BearerToken => ("bearer", vec![("token", auth.bearer_token.clone())]),
        AuthType::BasicAuth => (
            "basic",
            vec![
                ("username", auth.basic_username.clone()),
                ("password", auth.basic_password.clone()),
            ],
        ),
        AuthType::ApiKey => (
            "apikey",
            vec![
                ("key", auth.api_key_name.clone()),
                ("value", auth.api_key_value.clone()),
                (
                    "in",
                    if auth.api_key_location == ApiKeyLocation::QueryParam {
                        "query"
                    } else {
                        "header"
                    }
                    .into(),
                ),
            ],
        ),
        AuthType::OAuth2 => (
            "oauth2",
            vec![
                ("accessTokenUrl", auth.oauth.token_url.clone()),
                ("authUrl", auth.oauth.authorization_url.clone()),
                ("clientId", auth.oauth.client_id.clone()),
                ("clientSecret", auth.oauth.client_secret.clone()),
                ("scope", auth.oauth.scope.clone()),
                ("redirect_uri", auth.oauth.redirect_uri.clone()),
                ("grant_type", auth.oauth.grant.clone()),
            ],
        ),
    };
    let mut object = json!({"type":kind});
    object[kind] = Value::Array(
        fields
            .into_iter()
            .map(|(key, value)| json!({"key":key,"value":value,"type":"string"}))
            .collect(),
    );
    if auth.auth_type == AuthType::OAuth2 {
        let credentials = object[kind].as_array_mut().unwrap();
        for (key, value) in [
            ("refreshTokenUrl", json!(auth.oauth.refresh_url)),
            ("refreshTokenScope", json!(auth.oauth.refresh_scope)),
            (
                "client_authentication",
                json!(match auth.oauth.client_auth {
                    OAuthClientAuth::Body => "body",
                    OAuthClientAuth::Basic => "header",
                    OAuthClientAuth::Public => "none",
                }),
            ),
            ("authRequestParams", rows(&auth.oauth.authorization_params)),
            ("tokenRequestParams", rows(&auth.oauth.token_params)),
            ("refreshRequestParams", rows(&auth.oauth.refresh_params)),
        ] {
            credentials.push(json!({"key":key,"value":value}));
        }
    }
    Ok(object)
}
fn events(pre: &str, post: &str) -> Value {
    Value::Array([("prerequest",pre),("test",post)].into_iter().filter(|(_,text)|!text.is_empty()).map(|(listen,text)|json!({"listen":listen,"script":{"type":"text/javascript","exec":text.split('\n').collect::<Vec<_>>()}})).collect())
}
fn request(r: &ApiRequest) -> Result<Value> {
    let mut body = match r.body_type {
        BodyType::None => Value::Null,
        BodyType::Json => json!({"mode":"raw","raw":r.body,"options":{"raw":{"language":"json"}}}),
        BodyType::Text => json!({"mode":"raw","raw":r.body,"options":{"raw":{"language":"text"}}}),
        BodyType::Xml => json!({"mode":"raw","raw":r.body,"options":{"raw":{"language":"xml"}}}),
        BodyType::Html => json!({"mode":"raw","raw":r.body,"options":{"raw":{"language":"html"}}}),
        BodyType::UrlEncoded => json!({"mode":"urlencoded","urlencoded":rows(&r.form_data)}),
        BodyType::FormData => json!({"mode":"formdata","formdata":rows(&r.form_data)}),
        BodyType::Binary => json!({"mode":"file","file":{"src":r.binary_file}}),
    };
    if r.body_type == BodyType::FormData {
        for (row, field) in r
            .form_data
            .iter()
            .zip(body["formdata"].as_array_mut().unwrap())
        {
            field["type"] = if row.file { "file" } else { "text" }.into();
            if row.file {
                field["src"] = row.value.clone().into();
                field.as_object_mut().unwrap().remove("value");
            }
        }
    }
    let mut profile = r.imported_settings.clone();
    profile.extend(std::collections::BTreeMap::from([
        ("followRedirects".into(), json!(r.settings.follow_redirects)),
        ("disableCookieJar".into(), json!(!r.settings.use_cookies)),
        ("strictSSL".into(), json!(r.settings.verify_tls)),
        ("timeout".into(), json!(r.settings.timeout_ms)),
    ]));
    Ok(
        json!({"method":r.method.as_str(),"url":{"raw":r.url,"query":rows(&r.query_params),"variable":rows(&r.path_params)},"header":rows(&r.headers),"body":body,"auth":auth(&r.auth)?,"description":encode_description(&r.description,&r.description_metadata),"protocolProfileBehavior":profile,
        "x-rustapi":{"version":1,"settings":r.settings,"checks":r.checks,"extractors":r.extractors,"pre_script":r.pre_script,"post_script":r.post_script}}),
    )
}
fn items(requests: &[ApiRequest], folders: &[Folder], order: &[String]) -> Result<Value> {
    let mut output = vec![];
    for id in child_order(requests, folders, order) {
        if let Some(r) = requests.iter().find(|r| r.id == id) {
            let mut examples = vec![];
            for example in &r.examples {
                let mut value: serde_json::Map<String, Value> =
                    example.metadata.clone().into_iter().collect();
                let bytes = example.response()?.raw_body;
                let binary = std::str::from_utf8(&bytes).is_err();
                value.extend(serde_json::from_value::<serde_json::Map<String,Value>>(json!({"name":example.name,"originalRequest":request(&example.request)?,"code":example.response.status,"status":example.response.status_text,"header":example.response.headers.iter().map(|(key,value)|json!({"key":key,"value":value})).collect::<Vec<_>>(),"body":if binary {example.response.body.clone()}else{String::from_utf8(bytes)?}}))?);
                if binary {
                    value.insert(
                        "x-rustapi".into(),
                        json!({"version":1,"body_base64":example.body_base64}),
                    );
                }
                examples.push(Value::Object(value));
            }
            output.push(json!({"name":r.name,"request":request(r)?,"event":events(&r.pre_script,&r.post_script),"response":examples}));
        } else if let Some(f) = folders.iter().find(|f| f.id == id) {
            output.push(json!({"name":f.name,"description":encode_description(&f.description,&f.description_metadata),"auth":auth(&f.auth)?,"variable":rows(&scope_variable_rows(&f.variables,&f.variable_metadata)),"event":events(&f.pre_script,&f.post_script),"protocolProfileBehavior":f.imported_settings,"item":items(&f.requests,&f.folders,&f.order)?}));
        }
    }
    Ok(Value::Array(output))
}
pub fn collection(collection: &Collection, include_credentials: bool) -> Result<PortableExport> {
    validate_collection(collection)?;
    let c = if include_credentials {
        collection.clone()
    } else {
        crate::secrets::redact_collection(collection)
    };
    let document = json!({"info":{"name":c.name,"schema":"urn:collection:v2.1.0","description":encode_description(&c.description,&c.description_metadata)},"auth":auth(&c.auth)?,"variable":rows(&scope_variable_rows(&c.variables,&c.variable_metadata)),"event":events(&c.pre_script,&c.post_script),"protocolProfileBehavior":c.imported_settings,"item":items(&c.requests,&c.folders,&c.order)?});
    ensure!(
        serde_json::to_vec(&document)?.len() <= 64 * 1024 * 1024,
        "Portable export exceeds 64 MiB"
    );
    Ok(PortableExport{document,warnings:vec!["Native checks, extraction and transport options use x-rustapi version 1; other tools may ignore this extension. Binary example bytes also require the extension. Review credential fields before sharing.".into()]})
}
pub fn environment(environment: &Environment, include_credentials: bool) -> Result<PortableExport> {
    let e = if include_credentials {
        environment.clone()
    } else {
        crate::secrets::redact_environment(environment)
    };
    let document = json!({"name":e.name,"values":rows(&e.variables)});
    ensure!(
        serde_json::to_vec(&document)?.len() <= 64 * 1024 * 1024,
        "Environment export exceeds 64 MiB"
    );
    Ok(PortableExport {
        document,
        warnings: vec![],
    })
}
