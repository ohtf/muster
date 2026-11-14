// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! The local control API: JSON-RPC 2.0, one JSON object per line, over a
//! Unix domain socket with mode 0600 (`spec/02-architecture.md`, Runtime).
//! The CLI and the integration tests use it. Windows named-pipe transport
//! is tracked in `spec/queries.md`.

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

#[cfg(unix)]
pub async fn serve(path: &Path, tx: mpsc::Sender<Request>) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
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
        let tx = tx.clone();
        tokio::spawn(async move {
            let (r, mut w) = stream.into_split();
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
        });
    }
}

#[cfg(not(unix))]
pub async fn serve(_path: &Path, _tx: mpsc::Sender<Request>) -> std::io::Result<()> {
    tracing::warn!("the control API is not yet available on this platform");
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

/// A minimal client, used by the CLI and tests.
#[cfg(unix)]
pub async fn call(path: &Path, method: &str, params: Value) -> std::io::Result<Value> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let s = UnixStream::connect(path).await?;
    let (r, mut w) = s.into_split();
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
