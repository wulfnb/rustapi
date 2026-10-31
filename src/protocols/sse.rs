use super::*;
use anyhow::Context;
#[derive(Default)]
pub struct SseParser {
    line: Vec<u8>,
    skip_lf: bool,
    started: bool,
    data: String,
    event: String,
    id: String,
    retry: Option<u64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
    pub id: String,
    pub retry: Option<u64>,
}
impl SseParser {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>> {
        let mut events = vec![];
        for byte in bytes {
            if self.skip_lf {
                self.skip_lf = false;
                if *byte == b'\n' {
                    continue;
                }
            }
            if *byte == b'\n' || *byte == b'\r' {
                self.skip_lf = *byte == b'\r';
                let text = String::from_utf8_lossy(&self.line).into_owned();
                self.line.clear();
                let text = if !self.started {
                    self.started = true;
                    text.trim_start_matches('\u{feff}')
                } else {
                    &text
                };
                if text.is_empty() {
                    if !self.data.is_empty() {
                        self.data.pop();
                        events.push(SseEvent {
                            event: if self.event.is_empty() {
                                "message".into()
                            } else {
                                self.event.clone()
                            },
                            data: std::mem::take(&mut self.data),
                            id: self.id.clone(),
                            retry: self.retry,
                        });
                    }
                    self.event.clear();
                    continue;
                }
                if text.starts_with(':') {
                    continue;
                }
                let (field, value) = text.split_once(':').unwrap_or((text, ""));
                let value = value.strip_prefix(' ').unwrap_or(value);
                match field {
                    "data" => {
                        ensure!(
                            self.data.len() + value.len() < 1024 * 1024,
                            "SSE event exceeds 1 MiB"
                        );
                        self.data.push_str(value);
                        self.data.push('\n');
                    }
                    "event" => self.event = value.into(),
                    "id" if !value.contains('\0') => self.id = value.into(),
                    "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                        self.retry = value.parse().ok()
                    }
                    _ => {}
                }
            } else {
                ensure!(self.line.len() < 1024 * 1024, "SSE line exceeds 1 MiB");
                self.line.push(*byte);
            }
        }
        Ok(events)
    }
}
pub(super) async fn run(
    service: &HttpService,
    profile: &Session,
    sink: &mut Sink<'_>,
) -> Result<()> {
    let mut request = profile.request.clone();
    super::auth(service, &mut request).await?;
    let mut settings = request.settings.clone();
    settings.timeout_ms = profile.duration_seconds * 1000;
    let helper = matches!(
        request.auth.auth_type,
        crate::models::AuthType::AwsSigV4 | crate::models::AuthType::Digest
    );
    let helper = helper
        && !request
            .headers
            .iter()
            .any(|h| h.enabled && h.key.eq_ignore_ascii_case("authorization"));
    if helper {
        settings.follow_redirects = false;
    }
    let client = service.client(&settings)?;
    let mut params = request.query_params.clone();
    if let Some((key, value)) = request.auth.get_api_key_query_param() {
        params.push(crate::models::KeyValue::new(key, value));
    }
    let url = crate::http::build_url_with_params(&request.url, &params, None);
    let mut builder = client
        .request(
            reqwest::Method::from_bytes(request.method.as_str().as_bytes())?,
            url,
        )
        .header("Accept", "text/event-stream");
    for (key, value) in crate::http::effective_headers(&request)? {
        builder = builder.header(key, value);
    }
    if !request.body.is_empty() {
        builder = builder.body(request.body.clone());
    }
    let mut response = if helper {
        crate::http::auth::send(
            &client,
            builder.build()?,
            &request.auth,
            profile.request.settings.follow_redirects,
        )
        .await?
    } else {
        builder.send().await?
    };
    ensure!(
        response.status().is_success(),
        "SSE server returned {}",
        response.status()
    );
    ensure!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("text/event-stream")),
        "SSE response must use text/event-stream"
    );
    sink.send("connected", json!({"status":response.status().as_u16()}))?;
    let mut parser = SseParser::default();
    while let Some(chunk) = response.chunk().await.context("Read SSE stream")? {
        for event in parser.feed(&chunk)? {
            if sink.send("event", serde_json::to_value(event)?)? {
                return Ok(());
            }
        }
    }
    Ok(())
}
