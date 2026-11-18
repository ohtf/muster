// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The local control API: JSON-RPC 2.0, one JSON object per line, over a
//! Unix domain socket with mode 0600 (`spec/02-architecture.md`, Runtime).
//! On Windows the transport is a named pipe (`spec/queries.md` Q7). The CLI
//! and the integration tests use it.

use std::path::Path;

use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

/// One request from a client, answered by the runtime.
#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub params: Value,
    pub reply: oneshot::Sender<Result<Value, RpcError>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub fn invalid_params(m: impl Into<String>) -> Self {
        RpcError {
            code: -32602,
            message: m.into(),
        }
    }
    pub fn not_found(m: &str) -> Self {
        RpcError {
            code: -32601,
            message: format!("unknown method {m}"),
        }
    }
    pub fn refused(m: impl Into<String>) -> Self {
        RpcError {
            code: -32000,
            message: m.into(),
        }
    }
}

/// Encodes a JSON-RPC response line.
pub fn response(id: &Value, r: Result<Value, RpcError>) -> String {
    let v = match r {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(e) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": e.code, "message": e.message}})
        }
    };
    let mut s = v.to_string();
    s.push('\n');
    s
}

/// Answers requests on one connection until it closes.
async fn serve_conn<S>(stream: S, tx: mpsc::Sender<Request>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (r, mut w) = tokio::io::split(stream);
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.len() > 64 * 1024 {
            break;
        }
        let reply = handle_line(&line, &tx).await;
        if w.write_all(reply.as_bytes()).await.is_err() {
            break;
        }
    }
}

/// Serves the control API on a Unix socket (mode 0600 in a 0700
/// directory).
#[cfg(unix)]
pub async fn serve(path: &Path, tx: mpsc::Sender<Request>) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    use tokio::net::UnixListener;

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(serve_conn(stream, tx.clone()));
    }
}

/// Serves the control API on a named pipe (`path` is the pipe name, for
/// example `\\.\pipe\panic-button-DOMAIN-user`). The pipe keeps Windows' default
/// security: full access for the owner, SYSTEM and Administrators, read
/// only for others, which cannot send requests. Remote clients are
/// refused, and the first instance must be ours, so another process cannot
/// squat the name.
#[cfg(windows)]
pub async fn serve(path: &Path, tx: mpsc::Sender<Request>) -> std::io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let name = path.as_os_str();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(name)?;
    loop {
        server.connect().await?;
        let conn = server;
        server = ServerOptions::new()
            .reject_remote_clients(true)
            .create(name)?;
        tokio::spawn(serve_conn(conn, tx.clone()));
    }
}

#[cfg(not(any(unix, windows)))]
pub async fn serve(_path: &Path, _tx: mpsc::Sender<Request>) -> std::io::Result<()> {
    tracing::warn!("the control API is not available on this platform");
    std::future::pending().await
}

async fn handle_line(line: &str, tx: &mpsc::Sender<Request>) -> String {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => {
            return response(
                &Value::Null,
                Err(RpcError {
                    code: -32700,
                    message: "parse error".into(),
                }),
            );
        }
    };
    let id = v.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = v.get("method").and_then(Value::as_str) else {
        return response(
            &id,
            Err(RpcError {
                code: -32600,
                message: "invalid request".into(),
            }),
        );
    };
    let params = v.get("params").cloned().unwrap_or(Value::Null);
    let (reply, rx) = oneshot::channel();
    if tx
        .send(Request {
            method: method.to_owned(),
            params,
            reply,
        })
        .await
        .is_err()
    {
        return response(&id, Err(RpcError::refused("agent is stopping")));
    }
    match rx.await {
        Ok(r) => response(&id, r),
        Err(_) => response(&id, Err(RpcError::refused("no reply"))),
    }
}

/// One request on an open connection.
async fn call_on<S>(stream: S, method: &str, params: Value) -> std::io::Result<Value>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (r, mut w) = tokio::io::split(stream);
    let mut req =
        json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    req.push('\n');
    w.write_all(req.as_bytes()).await?;
    let mut lines = BufReader::new(r).lines();
    let line = lines
        .next_line()
        .await?
        .ok_or_else(|| std::io::Error::other("connection closed"))?;
    let v: Value = serde_json::from_str(&line).map_err(std::io::Error::other)?;
    if let Some(e) = v.get("error") {
        return Err(std::io::Error::other(
            e.get("message")
                .and_then(Value::as_str)
                .unwrap_or("error")
                .to_owned(),
        ));
    }
    Ok(v.get("result").cloned().unwrap_or(Value::Null))
}

/// A minimal client, used by the CLI and tests.
#[cfg(unix)]
pub async fn call(path: &Path, method: &str, params: Value) -> std::io::Result<Value> {
    let s = tokio::net::UnixStream::connect(path).await?;
    call_on(s, method, params).await
}

/// A minimal client, used by the CLI and tests. Retries briefly while
/// every pipe instance is busy.
#[cfg(windows)]
pub async fn call(path: &Path, method: &str, params: Value) -> std::io::Result<Value> {
    use tokio::net::windows::named_pipe::ClientOptions;
    const ERROR_PIPE_BUSY: i32 = 231;
    let mut tries = 0;
    let client = loop {
        match ClientOptions::new().open(path.as_os_str()) {
            Ok(c) => break c,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && tries < 40 => {
                tries += 1;
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(e) => return Err(e),
        }
    };
    call_on(client, method, params).await
}

#[cfg(not(any(unix, windows)))]
pub async fn call(_path: &Path, _method: &str, _params: Value) -> std::io::Result<Value> {
    Err(std::io::Error::other(
        "the control API is not available on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn requests_round_trip() {
        let d = tempfile::tempdir().expect("tempdir");
        #[cfg(unix)]
        let path = d.path().join("ctl.sock");
        #[cfg(windows)]
        let path = std::path::PathBuf::from(format!(
            r"\\.\pipe\panic-button-test-{}",
            std::process::id()
        ));
        let _ = &d;
        let (tx, mut rx) = mpsc::channel::<Request>(4);
        let p = path.clone();
        tokio::spawn(async move { serve(&p, tx).await });
        tokio::spawn(async move {
            while let Some(r) = rx.recv().await {
                let out = if r.method == "echo" {
                    Ok(r.params)
                } else {
                    Err(RpcError::not_found(&r.method))
                };
                let _ = r.reply.send(out);
            }
        });
        let mut v = Err(std::io::Error::other("not started"));
        for _ in 0..50 {
            v = call(&path, "echo", json!({"x": 1})).await;
            if v.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(v.expect("echo"), json!({"x": 1}));
        let e = call(&path, "nope", Value::Null).await.expect_err("unknown");
        assert!(e.to_string().contains("unknown method"));
    }
}
