mod support;
use rustapi::{
    graphql::Graphql,
    http::{Cancellation, HttpService},
    models::*,
    protocols::{self, Protocol, Session},
    variables::Variables,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
#[test]
fn graphql_documents_variables_schema_catalog_and_error_responses() {
    let mut graphql = Graphql {
        query: "query Pet($id: ID!) { pet(id: $id) { id } }".into(),
        variables: r#"{"id":"1"}"#.into(),
        schema: "type Query { pet: Pet } type Pet { id: ID! }".into(),
        ..Default::default()
    };
    assert!(graphql.body().unwrap().contains("variables"));
    assert_eq!(graphql.fields().unwrap(), vec!["Query.pet", "Pet.id"]);
    graphql.format().unwrap();
    graphql.query = "query A { a } query B { b }".into();
    assert!(graphql.body().is_err());
    graphql.operation_name = "B".into();
    assert!(graphql.body().is_ok());
    graphql.operation_name = "missing".into();
    assert!(graphql.body().is_err());
    graphql.operation_name.clear();
    graphql.query = "subscription { changed }".into();
    assert!(graphql.body().is_err());
    graphql.query = format!("{}field{}", "{".repeat(100), "}".repeat(100));
    assert!(graphql.body().is_err());
    let server = support::Server::new(1, |_, request| {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["variables"]["id"], "7");
        support::Reply::json(r#"{"data":null,"errors":[{"message":"private-server-error"}]}"#)
    });
    let request = ApiRequest {
        url: server.url.clone(),
        graphql: Some(Graphql {
            variables: r#"{"id":"{{id}}"}"#.into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut vars = Variables::default();
    vars.local.insert("id".into(), "7".into());
    let response = rustapi::runner::execute(
        &HttpService::default(),
        &request,
        &mut vars,
        &Cancellation::default(),
    );
    assert_eq!(response.status, 200);
    assert!(!response.tests[0].passed);
    assert!(!response.tests[0].message.contains("private-server-error"));
    server.finish();
    let mut c = Collection::new("GraphQL");
    c.requests.push(request);
    let portable = rustapi::export::collection(&c, true).unwrap();
    let copy = rustapi::import::json(&serde_json::to_vec(&portable.document).unwrap())
        .unwrap()
        .collection
        .unwrap();
    assert!(copy.requests[0].graphql.is_some());
}
#[test]
fn sse_parser_preserves_split_utf8_crlf_ids_multiline_data_and_bounds() {
    let text="\u{feff}: comment\r\nid: 7\r\nevent: update\r\ndata: first 🦀\r\ndata: second\r\nretry: 200\r\n\r\ndata: next\n\n";
    let mut parser = protocols::SseParser::default();
    let mut events = vec![];
    for byte in text.as_bytes() {
        events.extend(parser.feed(&[*byte]).unwrap());
    }
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].data, "first 🦀\nsecond");
    assert_eq!(events[0].event, "update");
    assert_eq!(events[1].id, "7");
    assert_eq!(events[0].retry, Some(200));
    assert!(parser.feed(&vec![b'a'; 1024 * 1024 + 1]).is_err());
}
#[test]
fn sse_delivers_events_before_connection_end_and_event_cap_stops_reading() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 4096];
        assert!(stream.read(&mut buffer).unwrap() > 0);
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: first\n\n").unwrap();
        stream.flush().unwrap();
        std::thread::sleep(Duration::from_millis(700));
        let _ = stream.write_all(b"data: second\n\n");
    });
    let profile = Session {
        protocol: Protocol::Sse,
        request: ApiRequest {
            url: format!("http://{address}/events"),
            ..Default::default()
        },
        max_events: 1,
        duration_seconds: 3,
        ..Default::default()
    };
    let start = Instant::now();
    let mut events = vec![];
    let summary = protocols::run(
        &HttpService::default(),
        &profile,
        &Variables::default(),
        &Cancellation::default(),
        None,
        |event| events.push(event),
    )
    .unwrap();
    assert_eq!(summary.events, 1);
    assert!(start.elapsed() < Duration::from_millis(600));
    assert_eq!(events.last().unwrap().data["data"], "first");
    thread.join().unwrap();
}
#[test]
// Tungstenite's Callback trait fixes the unboxed HTTP response error type.
#[allow(clippy::result_large_err)]
fn websocket_handshake_auth_initial_message_text_binary_and_cookie_update() {
    use futures_util::{SinkExt, StreamExt};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async move{
        let listener=tokio::net::TcpListener::from_std(listener).unwrap();let(stream,_)=listener.accept().await.unwrap();
        let mut socket=tokio_tungstenite::accept_hdr_async(stream,|request:&tokio_tungstenite::tungstenite::handshake::server::Request,mut response:tokio_tungstenite::tungstenite::handshake::server::Response|{assert_eq!(request.headers()["authorization"],"Bearer validator-token");response.headers_mut().insert("set-cookie","ws=session; Path=/".parse().unwrap());Ok(response)}).await.unwrap();
        assert_eq!(socket.next().await.unwrap().unwrap().into_text().unwrap(),"hello 🦀");socket.send(tokio_tungstenite::tungstenite::Message::Text("echo 🦀".into())).await.unwrap();socket.send(tokio_tungstenite::tungstenite::Message::Binary(vec![0,255,1].into())).await.unwrap();
    })
    });
    let service = HttpService::default();
    let profile = Session {
        request: ApiRequest {
            url: format!("ws://{address}/socket"),
            auth: AuthConfig {
                auth_type: AuthType::BearerToken,
                bearer_token: "validator-token".into(),
                ..Default::default()
            },
            ..Default::default()
        },
        messages: vec!["hello 🦀".into()],
        max_events: 2,
        duration_seconds: 3,
        ..Default::default()
    };
    let mut events = vec![];
    let summary = protocols::run(
        &service,
        &profile,
        &Variables::default(),
        &Cancellation::default(),
        None,
        |event| events.push(event),
    )
    .unwrap();
    assert_eq!(summary.events, 2);
    assert_eq!(events[1].data["text"], "echo 🦀");
    assert_eq!(events[2].data["base64"], "AP8B");
    assert_eq!(service.cookies.entries()[0].name, "ws");
    thread.join().unwrap();
}
#[test]
fn v3_shorthand_auth_variables_and_graphql_directories_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join(".resources")).unwrap();
    std::fs::write(directory.path().join(".resources/definition.yaml"),"$kind: collection\nname: Shorthand\nvariables:\n  host: https://example.test\nauth:\n  - type: bearer\n    credentials:\n      token: '{{token}}'\n").unwrap();
    std::fs::write(directory.path().join("query.request.yaml"),"$kind: graphql-request\nname: Query\nurl: '{{host}}/graphql'\nquery: 'query { __typename }'\nvariables: '{}'\n").unwrap();
    let imported = rustapi::v3::import_directory(directory.path()).unwrap();
    let collection = imported.collection.unwrap();
    assert_eq!(collection.variables["host"], "https://example.test");
    assert_eq!(collection.auth.auth_type, AuthType::BearerToken);
    assert!(collection.requests[0].graphql.is_some());
    let out = directory.path().join("exported");
    rustapi::v3::export_directory(&collection, &out, true).unwrap();
    let copy = rustapi::v3::import_directory(&out).unwrap();
    assert!(!copy.requires_confirmation());
    assert!(copy.collection.unwrap().requests[0].graphql.is_some());
    let value =
        json!({"$kind":"collection","name":"Rules","auth":[{"type":"noauth"},{"type":"noauth"}]});
    assert!(rustapi::v3::import_document(&value).is_err());
}
