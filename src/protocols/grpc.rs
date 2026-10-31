use super::*;
use anyhow::{bail, Context};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub(super) async fn run(
    service: &HttpService,
    profile: &Session,
    sink: &mut Sink<'_>,
) -> Result<()> {
    let mut configured = profile.clone();
    super::auth(service, &mut configured.request).await?;
    let profile = &configured;
    ensure!(
        !matches!(
            profile.request.auth.auth_type,
            crate::models::AuthType::AwsSigV4 | crate::models::AuthType::Digest
        ),
        "gRPC requires explicit token/header authentication for signing helpers"
    );
    ensure!(
        profile.request.settings.proxy.is_empty(),
        "gRPC proxy overrides are unsupported"
    );
    let url = reqwest::Url::parse(&profile.request.url)?;
    ensure!(
        ["http", "https", "grpc", "grpcs"].contains(&url.scheme()),
        "gRPC URL must use grpc(s):// or HTTP(S)"
    );
    let host = url.host_str().context("gRPC hostname missing")?;
    let port = url
        .port()
        .unwrap_or(if ["https", "grpcs"].contains(&url.scheme()) {
            443
        } else {
            80
        });
    let target = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let mut command = tokio::process::Command::new(
        std::env::var_os("RUSTAPI_GRPCURL").unwrap_or_else(|| "grpcurl".into()),
    );
    command.kill_on_drop(true);
    if ["http", "grpc"].contains(&url.scheme()) {
        command.arg("-plaintext");
    } else if !profile.request.settings.verify_tls {
        command.arg("-insecure");
    }
    if !profile.request.settings.ca_file.is_empty() {
        command
            .arg("-cacert")
            .arg(&profile.request.settings.ca_file);
    }
    if !profile.request.settings.identity_file.is_empty() {
        command
            .arg("-cert")
            .arg(&profile.request.settings.identity_file)
            .arg("-key")
            .arg(&profile.request.settings.identity_file);
    }
    if !profile.grpc_descriptor.is_empty() {
        let path = std::path::Path::new(&profile.grpc_descriptor);
        if path
            .extension()
            .is_some_and(|extension| extension == "proto")
        {
            let path = path
                .canonicalize()
                .context("Read local protobuf definition")?;
            command
                .arg("-import-path")
                .arg(path.parent().unwrap())
                .arg("-proto")
                .arg(path.file_name().unwrap());
        } else {
            command.arg("-protoset").arg(path);
        }
    }
    command.arg("-expand-headers");
    for (index, (key, value)) in crate::http::effective_headers(&profile.request)?
        .into_iter()
        .enumerate()
    {
        let name = format!("RUSTAPI_GRPC_HEADER_{index}");
        command
            .env(&name, value)
            .arg("-H")
            .arg(format!("{key}: ${{{name}}}"));
    }
    let mut input = String::new();
    match profile.grpc_action.as_str() {
        "invoke" => {
            ensure!(
                !profile.grpc_method.is_empty(),
                "Choose a gRPC service/method"
            );
            input = profile.request.body.clone();
            ensure!(input.len() <= 1024 * 1024, "gRPC payload exceeds 1 MiB");
            command
                .arg("-d")
                .arg("@")
                .arg("--")
                .arg(target)
                .arg(&profile.grpc_method);
        }
        "list" => {
            command.arg("--").arg(target).arg("list");
            if !profile.grpc_method.is_empty() {
                command.arg(&profile.grpc_method);
            }
        }
        "describe" => {
            command
                .arg("--")
                .arg(target)
                .arg("describe")
                .arg(&profile.grpc_method);
        }
        _ => bail!("gRPC action must be list, describe or invoke"),
    }
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .context("Install grpcurl or set RUSTAPI_GRPCURL to its executable")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).await?;
        stdin.shutdown().await?;
    }
    let stdout = child.stdout.take().context("gRPC stdout missing")?;
    let stderr = child.stderr.take().context("gRPC stderr missing")?;
    let read = async {
        let mut connected = false;
        let mut reader = stdout;
        let mut buffer = [0; 4096];
        let mut pending = vec![];
        loop {
            let count = reader.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            if !connected {
                sink.send(
                    "connected",
                    json!({"transport":"grpcurl","action":profile.grpc_action}),
                )?;
                connected = true;
            }
            pending.extend_from_slice(&buffer[..count]);
            ensure!(pending.len() <= 1024 * 1024, "gRPC message exceeds 1 MiB");
            if profile.grpc_action == "invoke" {
                loop {
                    let mut stream =
                        serde_json::Deserializer::from_slice(&pending).into_iter::<Value>();
                    match stream.next() {
                        Some(Ok(value)) => {
                            let consumed = stream.byte_offset();
                            if sink.send("message", value)? {
                                return Ok::<_, anyhow::Error>(true);
                            }
                            pending.drain(..consumed);
                        }
                        Some(Err(error)) if error.is_eof() => break,
                        Some(Err(_)) => bail!("Invalid JSON from gRPC helper"),
                        None => {
                            pending.clear();
                            break;
                        }
                    }
                }
            }
        }
        if profile.grpc_action != "invoke" {
            sink.send("result", json!({"text":String::from_utf8_lossy(&pending)}))?;
        } else {
            ensure!(
                pending.iter().all(u8::is_ascii_whitespace),
                "Incomplete gRPC JSON response"
            );
        }
        Ok(false)
    };
    let errors = async {
        let mut errors = vec![];
        stderr.take(16385).read_to_end(&mut errors).await?;
        ensure!(errors.len() <= 16384, "gRPC error output exceeds limit");
        Ok::<_, anyhow::Error>(())
    };
    tokio::pin!(read);
    tokio::pin!(errors);
    let (limit, errors_done) = tokio::select! {result=&mut read=>(result?,false),result=&mut errors=>{result?;((&mut read).await?,true)}};
    if limit {
        child.kill().await?;
        return Ok(());
    }
    if !errors_done {
        errors.await?;
    }
    let status = child.wait().await?;
    ensure!(status.success(),"gRPC helper failed; inspect the service/method, transport, metadata and descriptor settings");
    Ok(())
}
