mod support;
use rustapi::{
    http::*,
    models::*,
    runner::{self, RunOptions},
    scripts,
    variables::Variables,
};
#[test]
fn common_assertions_and_schema_api_are_real_checks() {
    let response = ApiResponse {
        status: 200,
        body: "{\"id\":7}".into(),
        ..Default::default()
    };
    let mut vars = Variables::default();
    let source = r#"
    pm.test('comparisons',()=>{pm.expect(7).to.be.a('number').and.above(2).and.within(3,9);pm.expect(7).to.not.equal(8);pm.expect('abc').to.match(/^a/).and.lengthOf(3);});
    pm.test('deep',()=>{pm.expect({b:2,a:1}).to.deep.equal({a:1,b:2});pm.expect([{id:1}, {id:2}]).to.deep.have.members([{id:2},{id:1}]);pm.expect([1,2,3]).to.include.members([2,1]);pm.expect({x:1,y:2}).to.include.keys('x');pm.expect({id:7}).to.have.property('id').that.is.a('number');});
    pm.test('attributes',()=>{pm.expect([]).to.be.empty;pm.expect(null).to.be.null;pm.expect(undefined).to.not.exist;pm.expect(true).to.be.ok;});
    pm.test('schema',()=>pm.response.to.have.jsonSchema({type:'object',required:['id'],properties:{id:{type:'integer'}}}));
    pm.test('fails',()=>pm.expect(7).to.not.equal(7));
    "#;
    let tests =
        scripts::execute(source, &mut vars, Some(&response), &Cancellation::default()).unwrap();
    assert_eq!(
        tests.iter().map(|test| test.passed).collect::<Vec<_>>(),
        vec![true, true, true, true, false]
    );
    assert!(scripts::execute(
        "pm.execution.runRequest('outside')",
        &mut vars,
        None,
        &Cancellation::default()
    )
    .is_err());
    assert!(scripts::execute(
        "pm.test('async',async()=>{await 1})",
        &mut vars,
        None,
        &Cancellation::default()
    )
    .is_err());
}
#[test]
fn pre_request_mutations_reach_http_and_post_request_is_read_only() {
    let server = support::Server::new(1, |_, request| {
        assert!(request.head.starts_with("POST /changed?q=a%20b "));
        assert!(request.head.to_lowercase().contains("x-added: ready"));
        assert!(!request.head.to_lowercase().contains("x-remove:"));
        assert_eq!(request.body, b"{\"id\":7}");
        support::Reply::json("{\"id\":7}")
    });
    let request=ApiRequest{url:format!("{}/original",server.url),headers:vec![KeyValue::new("X-Remove","gone")],pre_script:format!("pm.request.method='POST';pm.request.url.update('{}/changed');pm.request.url.query.upsert({{key:'q',value:'a b'}});pm.request.headers.remove('x-remove');pm.request.headers.add({{key:'X-Added',value:'{{{{marker}}}}'}});pm.variables.set('marker','ready');pm.request.body.update({{mode:'raw',raw:'{{\\\"id\\\":7}}',options:{{raw:{{language:'json'}}}}}});",server.url),post_script:"pm.test('sent',()=>pm.expect(pm.request.method).to.equal('POST'));pm.test('readonly',()=>{let failed=false;try{pm.request.method='DELETE'}catch(e){failed=true}pm.expect(failed).to.be.true;});".into(),..Default::default()};
    let response = runner::execute(
        &HttpService::default(),
        &request,
        &mut Variables::default(),
        &Cancellation::default(),
    );
    assert!(response.error.is_none(), "{:?}", response.error);
    assert!(response.tests.iter().all(|test| test.passed));
    assert_eq!(request.method, HttpMethod::GET);
    assert!(request.url.ends_with("original"));
    server.finish();
}
#[test]
fn skip_branch_stop_and_ambiguous_target_reporting_are_bounded() {
    let server = support::Server::new(1, |_, request| {
        assert!(request.head.starts_with("GET /last "));
        support::Reply::json("{}")
    });
    let first=ApiRequest{name:"Skip".into(),url:"http://127.0.0.1:1/never".into(),pre_script:"pm.execution.setNextRequest('Last');pm.execution.skipRequest();throw new Error('unreachable')".into(),..Default::default()};
    let skipped = ApiRequest {
        name: "Bypassed".into(),
        url: "http://127.0.0.1:1/never".into(),
        ..Default::default()
    };
    let last = ApiRequest {
        name: "Last".into(),
        url: format!("{}/last", server.url),
        post_script: "pm.execution.setNextRequest(null)".into(),
        ..Default::default()
    };
    let mut c = Collection::new("Flow");
    c.requests = vec![first, skipped, last];
    let report = runner::run(
        &HttpService::default(),
        &c,
        None,
        &RunOptions::default(),
        Default::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(report.passed(), "{report:?}");
    assert_eq!(report.entries.len(), 2);
    assert!(report.entries[0].skipped);
    assert!(report.junit().contains("<skipped/>"));
    server.finish();
    c.requests[0].pre_script =
        "pm.execution.setNextRequest('duplicate');pm.execution.skipRequest()".into();
    c.requests[1].name = "duplicate".into();
    c.requests[2].name = "duplicate".into();
    let report = runner::run(
        &HttpService::default(),
        &c,
        None,
        &RunOptions::default(),
        Default::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert!(!report.passed());
    assert!(report.entries[0]
        .error
        .as_ref()
        .unwrap()
        .contains("ambiguous"));
    c.requests.truncate(1);
    let id = c.requests[0].id.clone();
    c.requests[0].pre_script =
        format!("pm.execution.setNextRequest('{id}');pm.execution.skipRequest()");
    let report = runner::run(
        &HttpService::default(),
        &c,
        None,
        &RunOptions::default(),
        Default::default(),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(report.entries.len(), 1000);
    assert!(!report.passed());
    assert!(report
        .entries
        .last()
        .unwrap()
        .error
        .as_ref()
        .unwrap()
        .contains("limit"));
}
#[test]
fn variable_scope_keys_are_data_including_prototype_names() {
    let mut vars = Variables::default();
    vars.local
        .insert("__proto__".into(), "ordinary value".into());
    let tests=scripts::execute("pm.test('prototype',()=>{pm.expect(pm.variables.get('__proto__')).to.equal('ordinary value');pm.variables.set('__proto__','updated');pm.expect(pm.variables.has('toString')).to.be.false;});",&mut vars,None,&Cancellation::default()).unwrap();
    assert!(tests[0].passed);
    assert_eq!(vars.local["__proto__"], "updated");
}
