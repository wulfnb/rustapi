use anyhow::{ensure, Result};
use serde_json::{json, Value};
fn keys(value: &Value, allowed: &[&str]) -> Result<()> {
    ensure!(value.is_object(), "Expected a v3 object");
    ensure!(
        value
            .as_object()
            .unwrap()
            .keys()
            .all(|key| allowed.contains(&key.as_str())),
        "Unsupported v3 field; import stopped without writing"
    );
    Ok(())
}
fn array(value: &Value) -> Result<&[Value]> {
    if value.is_null() {
        Ok(&[])
    } else {
        value
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| anyhow::anyhow!("Expected a v3 array"))
    }
}
fn description(value: &Value) -> Value {
    if value.is_string() {
        value.clone()
    } else {
        value["content"].as_str().unwrap_or_default().into()
    }
}
fn field_rows(value: &Value, variables: bool) -> Result<Value> {
    let mut rows = vec![];
    for row in array(value)? {
        keys(
            row,
            &[
                "key",
                "value",
                "disabled",
                "description",
                "$type",
                "secret",
                "source",
            ],
        )?;
        ensure!(
            row["secret"] != true && row["source"].is_null(),
            "External secret sources are unsupported; configure local vault references"
        );
        let mut row = row.clone();
        row.as_object_mut().unwrap().remove("secret");
        if variables {
            row["type"] = "string".into();
        }
        rows.push(row);
    }
    Ok(Value::Array(rows))
}
fn auth_to_21(value: &Value) -> Result<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    keys(value, &["id", "name", "description", "type", "credentials"])?;
    let kind = value["type"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Auth type is required"))?;
    let mut auth = json!({"type":kind});
    if kind != "noauth" {
        auth[kind] = value["credentials"].clone();
    }
    Ok(auth)
}
fn events_to_21(value: &Value, scope: bool) -> Result<Value> {
    let mut events = vec![];
    for script in array(value)? {
        keys(
            script,
            &["type", "code", "language", "packages", "requests"],
        )?;
        ensure!(
            script["packages"].is_null() && script["requests"].is_null(),
            "Script packages and linked requests are unsupported"
        );
        ensure!(
            script["language"].is_null() || script["language"] == "text/javascript",
            "Unsupported script language"
        );
        let kind = script["type"].as_str().unwrap_or_default();
        let kind = if scope {
            kind.strip_prefix("http:").unwrap_or(kind)
        } else {
            kind
        };
        let listen = match kind {
            "beforeRequest" => "prerequest",
            "afterResponse" => "test",
            _ => anyhow::bail!("Unsupported script lifecycle event"),
        };
        ensure!(script["code"].is_string(), "Inline script code is required");
        events.push(json!({"listen":listen,"script":{"exec":script["code"]}}));
    }
    Ok(Value::Array(events))
}
fn body_to_21(value: &Value) -> Result<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    keys(value, &["type", "content", "$type"])?;
    Ok(match value["type"].as_str().unwrap_or("none") {
        "none" => Value::Null,
        "json" | "text" | "xml" | "html" | "javascript" => {
            ensure!(
                value["content"].is_string(),
                "Raw body content must be text"
            );
            json!({"mode":"raw","raw":value["content"],"options":{"raw":{"language":value["type"]}}})
        }
        "urlencoded" => {
            json!({"mode":"urlencoded","urlencoded":field_rows(&value["content"],false)?})
        }
        "formdata" => {
            for row in array(&value["content"])? {
                keys(
                    row,
                    &[
                        "type",
                        "key",
                        "value",
                        "src",
                        "contentType",
                        "description",
                        "disabled",
                    ],
                )?;
                ensure!(
                    row["type"] == "text" || row["type"] == "file",
                    "Unsupported multipart part type"
                );
            }
            json!({"mode":"formdata","formdata":value["content"]})
        }
        "file" => {
            keys(&value["content"], &["src"])?;
            json!({"mode":"file","file":value["content"]})
        }
        _ => anyhow::bail!("Unsupported v3 body type"),
    })
}
fn request_to_21(value: &Value) -> Result<Value> {
    let settings = &value["settings"];
    ensure!(
        settings.is_null() || settings.is_object(),
        "Expected request settings object"
    );
    let mut profile = settings.as_object().cloned().unwrap_or_default();
    if let Some(value) = profile.remove("disableCookies") {
        profile.insert("disableCookieJar".into(), value);
    }
    if profile.get("protocolVersion") == Some(&json!("auto")) {
        profile.remove("protocolVersion");
    }
    let max_size = profile.remove("maxResponseSize");
    let mut request = json!({"method":value["method"].as_str().unwrap_or("GET"),"url":{"raw":value["url"],"query":field_rows(&value["queryParams"],false)?,"variable":field_rows(&value["pathVariables"],false)?},"header":field_rows(&value["headers"],false)?,"description":value["description"],"body":body_to_21(&value["body"])?,"auth":auth_to_21(&value["auth"])?,"protocolProfileBehavior":profile});
    if let Some(size) = max_size {
        let size = size
            .as_u64()
            .filter(|size| (1..=256 * 1024 * 1024).contains(size))
            .ok_or_else(|| anyhow::anyhow!("Invalid response size limit"))?;
        let (mut settings, _) =
            crate::import::imported_profile_for_v3(&request["protocolProfileBehavior"])?;
        settings.max_response_bytes = size as usize;
        request["x-rustapi"] = json!({"version":1,"settings":settings});
    }
    Ok(request)
}
fn examples_to_21(value: &Value) -> Result<Value> {
    let mut output = vec![];
    for example in array(value)? {
        keys(
            example,
            &[
                "id",
                "name",
                "description",
                "auth",
                "$kind",
                "request",
                "response",
                "order",
            ],
        )?;
        ensure!(
            example["$kind"] == "http-example",
            "Unsupported example kind"
        );
        keys(
            &example["response"],
            &["statusCode", "statusText", "headers", "body"],
        )?;
        keys(&example["response"]["body"], &["type", "content", "$type"])?;
        let mut original = example["request"].clone();
        if original.is_null() {
            original = json!({});
        }
        keys(
            &original,
            &[
                "url",
                "method",
                "headers",
                "queryParams",
                "pathVariables",
                "body",
            ],
        )?;
        original["auth"] = example["auth"].clone();
        let mut snapshot = request_to_21(&original)?;
        if original["url"].is_null() {
            snapshot = Value::Null;
        }
        output.push(json!({"name":example["name"],"originalRequest":snapshot,"code":example["response"]["statusCode"],"status":example["response"]["statusText"],"header":field_rows(&example["response"]["headers"],false)?,"body":example["response"]["body"]["content"],"description":example["description"]}));
    }
    Ok(Value::Array(output))
}
pub fn to_21(value: &Value, root: bool) -> Result<Value> {
    match value["$kind"].as_str().unwrap_or_default() {
        "collection" => {
            keys(
                value,
                &[
                    "id",
                    "name",
                    "description",
                    "$kind",
                    "variables",
                    "scripts",
                    "auth",
                    "items",
                    "order",
                ],
            )?;
            let mut children = vec![];
            for child in array(&value["items"])? {
                children.push(to_21(child, false)?);
            }
            let mut result = json!({"name":value["name"],"description":value["description"],"variable":field_rows(&value["variables"],true)?,"auth":auth_to_21(&value["auth"])?,"event":events_to_21(&value["scripts"],true)?,"item":children});
            if root {
                result["info"] = json!({"name":value["name"],"description":value["description"],"schema":"urn:collection:v2.1.0"});
            }
            Ok(result)
        }
        "http-request" => {
            keys(
                value,
                &[
                    "id",
                    "name",
                    "description",
                    "url",
                    "method",
                    "headers",
                    "queryParams",
                    "pathVariables",
                    "body",
                    "auth",
                    "$kind",
                    "examples",
                    "settings",
                    "scripts",
                    "order",
                ],
            )?;
            Ok(
                json!({"name":value["name"],"request":request_to_21(value)?,"event":events_to_21(&value["scripts"],false)?,"response":examples_to_21(&value["examples"])?}),
            )
        }
        _ => anyhow::bail!("Only v3 HTTP requests and collection folders are supported"),
    }
}
fn auth_from_21(value: &Value) -> Value {
    let kind = value["type"].as_str().unwrap_or("inherit");
    if kind == "noauth" {
        json!({"type":kind})
    } else {
        json!({"type":kind,"credentials":value[kind].as_array().map(|rows|rows.iter().map(|row|json!({"key":row["key"],"value":row["value"]})).collect::<Vec<_>>()).unwrap_or_default()})
    }
}
fn rows_from_21(value: &Value, variables: bool) -> Value {
    Value::Array(value.as_array().into_iter().flatten().map(|row|{let mut output=json!({"key":row["key"],"value":row["value"],"disabled":row["disabled"].as_bool().unwrap_or(false),"description":description(&row["description"])});if variables{output["secret"]=false.into();}else if row["$type"].is_string(){output["$type"]=row["$type"].clone();}output}).collect())
}
fn scripts_from_21(value: &Value, scope: bool) -> Value {
    Value::Array(value.as_array().into_iter().flatten().map(|event|{let kind=if event["listen"]=="prerequest"{"beforeRequest"}else{"afterResponse"};json!({"type":format!("{}{kind}",if scope{"http:"}else{""}),"language":"text/javascript","code":event["script"]["exec"].as_array().map(|lines|lines.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n")).unwrap_or_default()})}).collect())
}
fn body_from_21(value: &Value) -> Value {
    match value["mode"].as_str().unwrap_or_default() {
        "raw" => {
            json!({"type":if value["options"]["raw"]["language"]=="json"{"json"}else{"text"},"content":value["raw"]})
        }
        "urlencoded" => {
            json!({"type":"urlencoded","content":rows_from_21(&value["urlencoded"],false)})
        }
        "formdata" => {
            json!({"type":"formdata","content":value["formdata"].as_array().into_iter().flatten().map(|row|{let mut out=json!({"type":row["type"],"key":row["key"],"disabled":row["disabled"],"description":description(&row["description"])});if row["type"]=="file"{out["src"]=row["src"].clone();}else{out["value"]=row["value"].clone();}
 if row["contentType"].is_string(){out["contentType"]=row["contentType"].clone();}out}).collect::<Vec<_>>()})
        }
        "file" => json!({"type":"file","content":value["file"]}),
        _ => json!({"type":"none"}),
    }
}
fn request_from_21(value: &Value) -> Value {
    json!({"method":value["method"],"url":value["url"]["raw"],"headers":rows_from_21(&value["header"],false),"queryParams":rows_from_21(&value["url"]["query"],false),"pathVariables":rows_from_21(&value["url"]["variable"],false),"body":body_from_21(&value["body"])})
}
pub fn from_21(value: &Value, root: bool) -> Value {
    if value["item"].is_array() {
        let info = if root { &value["info"] } else { value };
        let mut result = json!({"$kind":"collection","name":info["name"],"description":description(&info["description"]),"auth":auth_from_21(&value["auth"]),"scripts":scripts_from_21(&value["event"],true),"items":value["item"].as_array().unwrap().iter().map(|item|from_21(item,false)).collect::<Vec<_>>()});
        if root {
            result["variables"] = rows_from_21(&value["variable"], true);
            if result["auth"]["type"] == "inherit" {
                result["auth"] = json!({"type":"noauth"});
            }
        } else if result["auth"]["type"] == "inherit" {
            result.as_object_mut().unwrap().remove("auth");
        }
        result
    } else {
        let r = &value["request"];
        let mut out = request_from_21(r);
        out["$kind"] = "http-request".into();
        out["name"] = value["name"].clone();
        out["description"] = description(&r["description"]);
        out["auth"] = auth_from_21(&r["auth"]);
        out["scripts"] = scripts_from_21(&value["event"], false);
        let settings = &r["protocolProfileBehavior"];
        out["settings"] = json!({"strictSSL":settings["strictSSL"],"followRedirects":settings["followRedirects"],"disableCookies":settings["disableCookieJar"],"maxResponseSize":r["x-rustapi"]["settings"]["max_response_bytes"]});
        out["examples"]=Value::Array(value["response"].as_array().into_iter().flatten().map(|example|{
            let mut snapshot=request_from_21(&example["originalRequest"]);
            for field in ["headers","queryParams","pathVariables"] {for row in snapshot[field].as_array_mut().into_iter().flatten(){row.as_object_mut().unwrap().remove("$type");}}
            json!({"$kind":"http-example","name":example["name"],"auth":auth_from_21(&example["originalRequest"]["auth"]),"request":snapshot,"response":{"statusCode":example["code"],"statusText":example["status"],"headers":rows_from_21(&example["header"],false),"body":{"type":"text","content":example["body"]}}})
        }).collect());
        out
    }
}
