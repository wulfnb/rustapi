//! Bounded synchronous collection scripting subset. No host I/O is exposed.
use crate::{models::*, variables::Variables};
use anyhow::{bail, Result};
use serde_json::json;
use std::time::{Duration, Instant};
const BOOTSTRAP: &str = include_str!("scripts/bootstrap.js");
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Workflow {
    pub skip_request: bool,
    pub next_set: bool,
    pub next_request: Option<String>,
}
#[derive(Default)]
pub struct ScriptResult {
    pub tests: Vec<TestResult>,
    pub workflow: Workflow,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct RequestView {
    method: HttpMethod,
    url: String,
    headers: Vec<KeyValue>,
    query_params: Vec<KeyValue>,
    body_type: BodyType,
    body: String,
}
pub fn execute(
    script: &str,
    vars: &mut Variables,
    response: Option<&ApiResponse>,
    cancel: &crate::http::Cancellation,
) -> Result<Vec<TestResult>> {
    Ok(execute_request(script, vars, None, response, cancel)?.tests)
}
pub fn execute_request(
    script: &str,
    vars: &mut Variables,
    mut request: Option<&mut ApiRequest>,
    response: Option<&ApiResponse>,
    cancel: &crate::http::Cancellation,
) -> Result<ScriptResult> {
    if script.trim().is_empty() {
        return Ok(ScriptResult::default());
    }
    anyhow::ensure!(script.len() <= 256 * 1024, "Script exceeds 256 KiB limit");
    anyhow::ensure!(
        response.is_none_or(|r| r.raw_body.len().max(r.body.len()) <= 1024 * 1024),
        "Response exceeds the 1 MiB script input limit; use native assertions/extractors"
    );
    let rt = rquickjs::Runtime::new()?;
    rt.set_memory_limit(16 * 1024 * 1024);
    rt.set_max_stack_size(256 * 1024);
    let start = Instant::now();
    let cancel = cancel.clone();
    rt.set_interrupt_handler(Some(Box::new(move || {
        start.elapsed() > Duration::from_millis(250) || cancel.is_cancelled()
    })));
    let context = rquickjs::Context::full(&rt)?;
    let request_view = request.as_ref().map(|request| RequestView {
        method: request.method,
        url: request.url.clone(),
        headers: request.headers.clone(),
        query_params: request.query_params.clone(),
        body_type: request.body_type.clone(),
        body: request.body.clone(),
    });
    let input = json!({"collection":vars.collection,"environment":vars.environment,"data":vars.data,"local":vars.local,"response":response.cloned().unwrap_or_default(),"request":request_view,"readonly":response.is_some()});
    let input = serde_json::to_string(&input.to_string())?;
    let program=format!("const __input=JSON.parse({input});\n{BOOTSTRAP}\ntry{{const result=(function(){{\n{script}\n}})();if(result instanceof __Promise)throw new Error('Asynchronous scripts unsupported');}}catch(error){{if(error!==__skip)throw error;}}\nJSON.stringify({{collection:__input.collection,environment:__input.environment,local:__input.local,tests:__tests,request:__input.request,workflow:__flow}})");
    let schema_instance = response
        .and_then(|response| serde_json::from_str::<serde_json::Value>(&response.body).ok());
    let output = context.with(|ctx| {
        ctx.globals().set(
            "__validateSchema",
            rquickjs::function::Func::from(move |schema: String| -> String {
                let result = schema_instance
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Response is not JSON"))
                    .and_then(|instance| crate::schema::validate(&schema, instance));
                serde_json::to_string(&match result {
                    Ok(errors) => errors,
                    Err(_) => vec!["Invalid or unsupported schema".into()],
                })
                .unwrap_or_else(|_| "[\"Schema failed\"]".into())
            }),
        )?;
        ctx.eval::<String, _>(program).map_err(|e| {
            let exception = ctx.catch();
            anyhow::anyhow!("Script failed or exceeded resource limit: {e}; {exception:?}")
        })
    })?;
    if rt.is_job_pending() {
        bail!("Asynchronous scripts are unsupported; use synchronous pm.* APIs");
    }
    let output: serde_json::Value = serde_json::from_str(&output)?;
    let collection = serde_json::from_value(output["collection"].clone())?;
    let environment = serde_json::from_value(output["environment"].clone())?;
    let local = serde_json::from_value(output["local"].clone())?;
    let tests = serde_json::from_value(output["tests"].clone())?;
    let workflow = serde_json::from_value(output["workflow"].clone())?;
    let view: Option<RequestView> = if response.is_none() && request.is_some() {
        Some(serde_json::from_value(output["request"].clone())?)
    } else {
        None
    };
    vars.collection = collection;
    vars.environment = environment;
    vars.local = local;
    if let (Some(request), Some(view)) = (&mut request, view) {
        request.method = view.method;
        request.url = view.url;
        request.headers = view.headers;
        request.query_params = view.query_params;
        request.body_type = view.body_type;
        request.body = view.body;
    }
    Ok(ScriptResult { tests, workflow })
}
