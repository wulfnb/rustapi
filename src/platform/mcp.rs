//! Stdio MCP tools. Execution requires both an explicit launch flag and workspace policy.
use crate::http::{Cancellation, HttpService};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};
pub fn collections(root: &Path) -> Result<Vec<crate::models::Collection>> {
    let directory = crate::storage::owned_path(root, &["collections"])?;
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut collections = vec![];
    let mut total = 0;
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.path().extension().is_none_or(|e| e != "json") {
            continue;
        }
        ensure!(collections.len() < 1000, "Catalog exceeds 1000 collections");
        let name = entry
            .file_name()
            .to_str()
            .context("Invalid collection filename")?
            .to_owned();
        let path = crate::storage::owned_path(root, &["collections", &name])?;
        total += std::fs::metadata(&path)?.len();
        ensure!(
            total <= 64 * 1024 * 1024,
            "MCP catalog exceeds its 64 MiB budget"
        );
        ensure!(
            std::fs::metadata(&path)?.len() <= 64 * 1024 * 1024,
            "Catalog collection exceeds 64 MiB"
        );
        collections.push(crate::storage::load_collection(&path)?);
    }
    Ok(collections)
}
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
pub fn handle(
    root: &Path,
    service: &HttpService,
    secrets: &BTreeMap<String, String>,
    allow_execution: bool,
    request: Value,
) -> Option<Value> {
    handle_with_cancel(
        root,
        service,
        secrets,
        allow_execution,
        request,
        &Cancellation::default(),
    )
}
fn handle_with_cancel(
    root: &Path,
    service: &HttpService,
    secrets: &BTreeMap<String, String>,
    allow_execution: bool,
    request: Value,
    cancel: &Cancellation,
) -> Option<Value> {
    if !request.is_object() {
        return Some(error(Value::Null, -32600, "Invalid JSON-RPC request"));
    }
    request.get("id")?;
    let id = request["id"].clone();
    if request["jsonrpc"] != "2.0"
        || request["method"].as_str().is_none()
        || !(id.is_string() || id.is_i64() || id.is_u64())
    {
        return Some(error(Value::Null, -32600, "Invalid JSON-RPC request"));
    }
    let version = request["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str();
    if let Some(version) = version {
        if version != "2026-07-28" {
            return Some(
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32022,"message":"Unsupported protocol version","data":{"supported":["2026-07-28","2025-11-25"],"requested":version}}}),
            );
        }
    }
    let execution = allow_execution
        && service
            .policy()
            .is_ok_and(|policy| policy.allow_mcp_execution);
    let result = match request["method"].as_str().unwrap_or_default() {
        "server/discover" => Ok(
            json!({"resultType":"complete","supportedVersions":["2026-07-28","2025-11-25"],"capabilities":{"tools":{}},"_meta":{"io.modelcontextprotocol/serverInfo":{"name":"RustAPI","version":env!("CARGO_PKG_VERSION")}},"instructions":"Local catalog and saved-profile tools. Execution requires explicit launch permission.","ttlMs":60000,"cacheScope":"private"}),
        ),
        "initialize" => {
            let requested = request["params"]["protocolVersion"]
                .as_str()
                .unwrap_or_default();
            Ok(
                json!({"protocolVersion":if ["2025-03-26","2025-06-18","2025-11-25"].contains(&requested){requested}else{"2025-11-25"},"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"RustAPI","version":env!("CARGO_PKG_VERSION")},"instructions":"Read-only tools are enabled by default. Executing saved run profiles requires --allow-execution and workspace policy permission."}),
            )
        }
        "ping" => Ok(json!({})),
        "tools/list" => {
            let mut tools = vec![
                json!({"name":"catalog","description":"List local collections and request metadata without credentials","inputSchema":{"type":"object","properties":{},"additionalProperties":false}}),
                json!({"name":"describe_request","description":"Read one saved request with known credential fields redacted","inputSchema":{"type":"object","properties":{"collection":{"type":"string"},"request":{"type":"string"}},"required":["collection","request"],"additionalProperties":false}}),
            ];
            if execution {
                tools.push(json!({"name":"run_profile","description":"Execute one saved local run profile and return its test report","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false}}));
            }
            Ok(json!({"tools":tools}))
        }
        "tools/call" => {
            let work = (|| -> Result<Value> {
                let args = &request["params"]["arguments"];
                match request["params"]["name"].as_str().unwrap_or_default(){
                "catalog"=>Ok(json!(collections(root)?.iter().map(|c|json!({"id":c.id,"name":c.name,"version":c.version,"requests":c.ordered_requests(None).iter().map(|r|json!({"id":r.id,"name":r.name,"method":r.method.as_str()})).collect::<Vec<_>>()})).collect::<Vec<_>>())),
                "describe_request"=>{let collections=collections(root)?;let collection=collections.iter().find(|c|Some(c.id.as_str())==args["collection"].as_str()).context("Collection missing")?;let request=collection.request(args["request"].as_str().context("Request ID missing")?).context("Request missing")?;Ok(serde_json::to_value(crate::secrets::redact_request(request))?)},
                "run_profile"=>{ensure!(execution,"MCP execution is disabled");let profile=crate::automation::profiles(root)?.into_iter().find(|p|Some(p.id.as_str())==args["id"].as_str()).context("Run profile missing")?;Ok(serde_json::to_value(crate::automation::run(root,service,&profile,secrets.clone(),cancel)?)?)},
                _=>anyhow::bail!("Unknown MCP tool"),
            }
            })();
            let work = work.and_then(|value| {
                let text = serde_json::to_string(&value)?;
                ensure!(text.len() <= 1024 * 1024, "MCP output exceeds 1 MiB");
                Ok(text)
            });
            Ok(match work {
                Ok(text) => json!({"content":[{"type":"text","text":text}],"isError":false}),
                Err(_) => {
                    json!({"content":[{"type":"text","text":"Tool failed. Check the requested resource, execution permission, workspace policy and configured credentials."}],"isError":true})
                }
            })
        }
        _ => Err(()),
    };
    Some(match result {
        Ok(mut result) => {
            if version.is_some() {
                result["resultType"] = "complete".into();
                result["_meta"]["io.modelcontextprotocol/serverInfo"] =
                    json!({"name":"RustAPI","version":env!("CARGO_PKG_VERSION")});
                if request["method"] == "tools/list" {
                    result["ttlMs"] = 0.into();
                    result["cacheScope"] = "private".into();
                }
            }
            json!({"jsonrpc":"2.0","id":id,"result":result})
        }
        Err(()) => error(id, -32601, "Method not found"),
    })
}
pub fn serve(
    root: &Path,
    service: &HttpService,
    secrets: &BTreeMap<String, String>,
    allow_execution: bool,
) -> Result<()> {
    use std::{
        io::{BufRead, Write},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let output = Arc::new(Mutex::new(std::io::stdout()));
    let jobs = Arc::new(Mutex::new(BTreeMap::<String, Cancellation>::new()));
    std::thread::scope(|scope| {
        let result = (|| -> Result<()> {
            for _ in 0..10000 {
                let mut bytes = vec![];
                let mut limited = std::io::Read::take(&mut reader, 1024 * 1024 + 1);
                if limited.read_until(b'\n', &mut bytes)? == 0 {
                    break;
                }
                ensure!(bytes.len() <= 1024 * 1024, "MCP input exceeds 1 MiB");
                let request: Value = match serde_json::from_slice(&bytes) {
                    Ok(request) => request,
                    Err(_) => {
                        let mut output = output.lock().unwrap_or_else(|p| p.into_inner());
                        writeln!(output, "{}", error(Value::Null, -32700, "Parse error"))?;
                        output.flush()?;
                        continue;
                    }
                };
                if request["method"] == "notifications/cancelled" {
                    let key = request["params"]["requestId"].to_string();
                    if let Some(cancel) = jobs.lock().unwrap_or_else(|p| p.into_inner()).get(&key) {
                        cancel.cancel();
                    }
                    continue;
                }
                if request.is_object() && request.get("id").is_none() {
                    continue;
                }
                let key = request["id"].to_string();
                let cancel = Cancellation::default();
                {
                    let mut jobs = jobs.lock().unwrap_or_else(|p| p.into_inner());
                    if jobs.len() >= 8 || jobs.contains_key(&key) {
                        let mut output = output.lock().unwrap_or_else(|p| p.into_inner());
                        writeln!(
                            output,
                            "{}",
                            error(
                                request["id"].clone(),
                                -32603,
                                "MCP concurrency limit or duplicate active request ID"
                            )
                        )?;
                        output.flush()?;
                        continue;
                    }
                    jobs.insert(key.clone(), cancel.clone());
                }
                let jobs = jobs.clone();
                let output = output.clone();
                scope.spawn(move || {
                    let response = handle_with_cancel(
                        root,
                        service,
                        secrets,
                        allow_execution,
                        request,
                        &cancel,
                    );
                    if !cancel.is_cancelled() {
                        if let Some(response) = response {
                            let mut output = output.lock().unwrap_or_else(|p| p.into_inner());
                            let _ = writeln!(output, "{response}");
                            let _ = output.flush();
                        }
                    }
                    jobs.lock().unwrap_or_else(|p| p.into_inner()).remove(&key);
                });
            }
            Ok(())
        })();
        let deadline = Instant::now() + Duration::from_millis(200);
        while Instant::now() < deadline
            && !jobs.lock().unwrap_or_else(|p| p.into_inner()).is_empty()
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        for cancel in jobs.lock().unwrap_or_else(|p| p.into_inner()).values() {
            cancel.cancel();
        }
        result
    })
}
