//! Bounded, loopback-only HTTP listener for local integration tools.
use anyhow::{ensure, Context, Result};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
impl Response {
    pub fn json(status: u16, value: serde_json::Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: value.to_string().into_bytes(),
        }
    }
}
pub struct Server {
    pub address: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}
impl Server {
    pub fn start(
        port: u16,
        handler: impl Fn(Request) -> Response + Send + Sync + 'static,
    ) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let handler = Arc::new(handler);
        let active = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            while !signal.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if active.load(Ordering::SeqCst) >= 16 {
                            continue;
                        }
                        active.fetch_add(1, Ordering::SeqCst);
                        let active = active.clone();
                        let handler = handler.clone();
                        let signal = signal.clone();
                        std::thread::spawn(move || {
                            let response=read(&mut stream,&signal).map(|request| {
                                let host=request.headers.get("host").map(String::as_str).unwrap_or_default();
                                if host!=format!("127.0.0.1:{}",address.port())&&host!=format!("localhost:{}",address.port()) {Response::json(403,serde_json::json!({"error":"Invalid local Host"}))}else{handler(request)}
                            }).unwrap_or_else(|_|Response::json(400,serde_json::json!({"error":"Invalid or oversized local request"})));
                            let _ = write(&mut stream, response);
                            active.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self { address, stop })
    }
}
fn read(stream: &mut TcpStream, stop: &AtomicBool) -> Result<Request> {
    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = vec![];
    let mut buffer = [0; 4096];
    let end = loop {
        ensure!(
            Instant::now() < deadline && !stop.load(Ordering::SeqCst),
            "Local request deadline"
        );
        if let Some(end) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
            break end;
        }
        ensure!(bytes.len() <= 16384, "Headers exceed 16 KiB");
        match stream.read(&mut buffer) {
            Ok(0) => anyhow::bail!("Incomplete request"),
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e.into()),
        }
    };
    ensure!(end <= 16384, "Headers exceed 16 KiB");
    let mut lines = std::str::from_utf8(&bytes[..end])?.split("\r\n");
    let first = lines
        .next()
        .context("Missing method")?
        .split_whitespace()
        .collect::<Vec<_>>();
    ensure!(
        first.len() == 3
            && first[2] == "HTTP/1.1"
            && first[1].starts_with('/')
            && !first[1].starts_with("//"),
        "Invalid request line"
    );
    let method = first[0].to_owned();
    let target = first[1].to_owned();
    let mut headers = BTreeMap::new();
    for line in lines {
        let (key, value) = line.split_once(':').context("Invalid header")?;
        ensure!(
            !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Invalid header name"
        );
        ensure!(
            headers
                .insert(key.to_ascii_lowercase(), value.trim().to_owned())
                .is_none(),
            "Duplicate local header"
        );
    }
    ensure!(
        !headers.contains_key("transfer-encoding"),
        "Chunked local requests unsupported"
    );
    let length = headers
        .get("content-length")
        .map(|n| n.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    ensure!(length <= 1024 * 1024, "Local body exceeds 1 MiB");
    let body_start = end + 4;
    while bytes.len() < body_start + length {
        ensure!(
            Instant::now() < deadline && !stop.load(Ordering::SeqCst),
            "Local request deadline"
        );
        match stream.read(&mut buffer) {
            Ok(0) => anyhow::bail!("Incomplete local body"),
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(Request {
        method,
        target,
        headers,
        body: bytes[body_start..body_start + length].to_vec(),
    })
}
fn write(stream: &mut TcpStream, response: Response) -> Result<()> {
    if response.status == 0 {
        return Ok(());
    }
    let mut head=format!("HTTP/1.1 {} Local\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n",response.status,response.body.len());
    for (key, value) in response.headers {
        if key.eq_ignore_ascii_case("content-length")
            || key.eq_ignore_ascii_case("transfer-encoding")
            || key.eq_ignore_ascii_case("connection")
        {
            continue;
        }
        ensure!(
            !key.contains(['\r', '\n']) && !value.contains(['\r', '\n']),
            "Invalid response header"
        );
        head.push_str(&format!("{key}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    Ok(())
}
