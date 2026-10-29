//! Bounded synchronous collection scripting subset. No host I/O is exposed.
use crate::{models::*, variables::Variables};
use anyhow::{bail, Result};
use serde_json::json;
use std::time::{Duration, Instant};
const BOOTSTRAP: &str = r#"
const __tests=[];const __Promise=Promise;
globalThis.Atomics=undefined;globalThis.SharedArrayBuffer=undefined;globalThis.Promise=undefined;
function __strict(obj,label){return new Proxy(obj,{get(t,k){if(k in t)return t[k];throw new Error('Unsupported API: '+label+'.'+String(k));}});}
function __scope(name){return __strict({get:k=>__input[name][k],set:(k,v)=>{__input[name][k]=String(v)},unset:k=>delete __input[name][k],has:k=>k in __input[name],toObject:()=>({...__input[name]}),replaceIn:s=>s.replace(/\{\{([^}]+)\}\}/g,(_,k)=>{const v=__merged()[k];if(v===undefined)throw new Error('Unresolved variable: '+k);return v;})},'pm.'+name);}
function __merged(){return {...__input.collection,...__input.environment,...__input.data,...__input.local};}
function __expect(value){
 const x={};for(const k of ['to','be','have','and','that','is'])x[k]=x;
 x.equal=x.eql=x.equals=(other)=>{if(JSON.stringify(value)!==JSON.stringify(other))throw new Error('Expected values to be equal');return x;};
 x.include=x.contain=(other)=>{if(!value.includes(other))throw new Error('Expected value to include target');return x;};
 x.property=function(key,other){if(value==null || !(key in value))throw new Error('Missing property '+key);if(arguments.length>1 && JSON.stringify(value[key])!==JSON.stringify(other))throw new Error('Property differs');return __expect(value[key]);};
 x.status=code=>{if(value.code!==code)throw new Error('Expected status '+code);return x;};
 Object.defineProperty(x,'true',{get(){if(value!==true)throw new Error('Expected true');return x;}});
 Object.defineProperty(x,'false',{get(){if(value!==false)throw new Error('Expected false');return x;}});
 return __strict(x,'pm.expect');
}
const __response=__strict({code:__input.response.status,status:__input.response.status_text,responseTime:__input.response.time_ms,
 json:()=>JSON.parse(__input.response.body),text:()=>__input.response.body,
 headers:__strict({get:k=>{const h=__input.response.headers.find(h=>h[0].toLowerCase()===k.toLowerCase());return h&&h[1];},has:k=>__input.response.headers.some(h=>h[0].toLowerCase()===k.toLowerCase())},'pm.response.headers')},'pm.response');
__response.to=__strict({have:__strict({status:code=>__expect(__response).status(code)},'pm.response.to.have')},'pm.response.to');
const pm=__strict({environment:__scope('environment'),collectionVariables:__scope('collection'),iterationData:__scope('data'),
 variables:__strict({get:k=>__merged()[k],set:(k,v)=>{__input.local[k]=String(v)},unset:k=>delete __input.local[k],has:k=>k in __merged(),toObject:__merged,replaceIn:s=>__scope('local').replaceIn(s)},'pm.variables'),
 response:__response,expect:__expect,test:(name,fn)=>{try{const result=fn();if(result instanceof __Promise)throw new Error('Async tests unsupported');__tests.push({name:String(name),passed:true,message:''});}catch(e){__tests.push({name:String(name),passed:false,message:String(e).startsWith('Error: Unsupported API:')?String(e):'Script assertion failed'});}}},'pm');
const console={log:()=>{},warn:()=>{},error:()=>{}};
function require(){throw new Error('Module imports unsupported');}
"#;
pub fn execute(
    script: &str,
    vars: &mut Variables,
    response: Option<&ApiResponse>,
    cancel: &crate::http::Cancellation,
) -> Result<Vec<TestResult>> {
    if script.trim().is_empty() {
        return Ok(vec![]);
    }
    anyhow::ensure!(script.len() <= 256 * 1024, "Script exceeds 256 KiB limit");
    anyhow::ensure!(
        response.is_none_or(|r| r.raw_body.len() <= 1024 * 1024),
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
    let input = json!({"collection":vars.collection,"environment":vars.environment,"data":vars.data,"local":vars.local,"response":response.cloned().unwrap_or_default()});
    let program=format!("const __input={input};\n{BOOTSTRAP}\n(function(){{\n{script}\n}})();\nJSON.stringify({{collection:__input.collection,environment:__input.environment,local:__input.local,tests:__tests}})");
    let output = context.with(|ctx| {
        ctx.eval::<String, _>(program).map_err(|e| {
            let exception = ctx.catch();
            anyhow::anyhow!("Script failed or exceeded resource limit: {e}; {exception:?}")
        })
    })?;
    if rt.is_job_pending() {
        bail!("Asynchronous scripts are unsupported; use synchronous pm.* APIs");
    }
    let output: serde_json::Value = serde_json::from_str(&output)?;
    vars.collection = serde_json::from_value(output["collection"].clone())?;
    vars.environment = serde_json::from_value(output["environment"].clone())?;
    vars.local = serde_json::from_value(output["local"].clone())?;
    Ok(serde_json::from_value(output["tests"].clone())?)
}
