mod support;
use rustapi::{
    http::{Cancellation, HttpService},
    models::*,
    variables::Variables,
};
use serde_json::json;

#[test]
fn imported_multiple_files_and_mime_types_reach_the_server_without_losing_metadata() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("first.bin"), [0, 255, 128]).unwrap();
    std::fs::write(directory.path().join("second.bin"), [7, 6, 5, 4]).unwrap();
    let server = support::Server::new(1, |_, request| {
        let text = String::from_utf8_lossy(&request.body).to_ascii_lowercase();
        assert_eq!(text.matches("name=\"upload\"").count(), 2);
        assert_eq!(
            text.matches("content-type: application/x-fixture").count(),
            2
        );
        assert!(text.contains("content-type: application/json"));
        assert!(request.body.windows(3).any(|part| part == [0, 255, 128]));
        assert!(request.body.windows(4).any(|part| part == [7, 6, 5, 4]));
        assert!(!text.contains("missing.bin"));
        support::Reply::json("{}")
    });
    let source = json!({"info":{"name":"Uploads","schema":"v2.1.0"},"item":[{"name":"Send",
        "request":{"method":"POST","url":server.url,"header":[{"key":"Content-Type","value":"multipart/form-data"}],
        "body":{"mode":"formdata","formdata":[
            {"key":"upload","type":"file","src":["first.bin","second.bin"],"contentType":"{{mime}}","description":"Files"},
            {"key":"meta","value":"{}","contentType":"application/json"},
            {"key":"skip","type":"file","src":"missing.bin","disabled":true}]}}}]});
    let imported = rustapi::import::json(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert!(!imported.requires_confirmation());
    let mut collection = imported.collection.unwrap();
    collection.file_path = Some(
        directory
            .path()
            .join("source.json")
            .to_string_lossy()
            .into(),
    );
    let request = collection.ordered_requests(None).remove(0);
    assert_eq!(request.form_data.len(), 4);
    assert!(request.form_data[..2]
        .iter()
        .all(|field| field.file && field.description == "Files"));
    let mut vars = Variables::default();
    vars.local
        .insert("mime".into(), "application/x-fixture".into());
    let response = rustapi::runner::execute(
        &HttpService::default(),
        &request,
        &mut vars,
        &Cancellation::default(),
    );
    assert!(response.error.is_none(), "{:?}", response.error);
    server.finish();
    let copy = rustapi::import::json(&serde_json::to_vec(&collection).unwrap())
        .unwrap()
        .collection
        .unwrap();
    assert_eq!(copy.requests[0].form_data[1].value, "second.bin");
    assert_eq!(copy.requests[0].form_data[1].content_type, "{{mime}}");
}

#[test]
fn invalid_part_mime_types_fail_in_transport_and_code_generation() {
    let request = ApiRequest {
        url: "http://127.0.0.1:1".into(),
        body_type: BodyType::FormData,
        form_data: vec![KeyValue {
            content_type: "invalid mime".into(),
            ..KeyValue::new("name", "value")
        }],
        ..Default::default()
    };
    let response = HttpService::default().execute(&request, &Cancellation::default());
    assert!(response
        .error
        .unwrap()
        .contains("Invalid part content type"));
    for language in rustapi::http::CodeGenLanguage::all() {
        assert!(rustapi::http::generate_code(&request, *language, None)
            .contains("Invalid multipart content type"));
    }
}
