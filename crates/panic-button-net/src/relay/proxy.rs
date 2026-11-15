// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! A TCP stream to the relay, through an HTTP CONNECT proxy if the proxy
//! settings say so.

use hyper_util::client::proxy::matcher::{Intercept, Matcher};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::End;
use super::ws::Target;

/// The most a CONNECT response header may be.
const MAX_HEADER: usize = 8 * 1024;

fn unbracket(h: &str) -> &str {
    h.trim_start_matches('[').trim_end_matches(']')
}

pub(super) async fn tcp_to(proxy: &Matcher, t: &Target) -> Result<TcpStream, End> {
    let scheme = if t.tls { "https" } else { "http" };
    let dst: Option<http::Uri> = format!("{scheme}://{}:{}/", t.host, t.port).parse().ok();
    match dst.and_then(|d| proxy.intercept(&d)) {
        None => TcpStream::connect((unbracket(&t.host), t.port))
            .await
            .map_err(|e| End::lost(e.to_string())),
        Some(i) => connect_via(&i, t).await,
    }
}

async fn connect_via(i: &Intercept, t: &Target) -> Result<TcpStream, End> {
    let p = i.uri();
    if p.scheme_str() == Some("https") {
        // TLS to the proxy itself is not supported here; long-poll is.
        return Err(End::Upgrade("HTTPS proxy: using long-poll".into()));
    }
    let host = p.host().ok_or_else(|| End::lost("proxy URL has no host"))?;
    let port = p.port_u16().unwrap_or(80);
    let mut s = TcpStream::connect((unbracket(host), port))
        .await
        .map_err(|e| End::lost(format!("proxy {host}:{port}: {e}")))?;
    let authority = format!("{}:{}", t.host, t.port);
    let mut req = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n");
    if let Some(a) = i.basic_auth().and_then(|v| v.to_str().ok()) {
        req.push_str(&format!("Proxy-Authorization: {a}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes())
        .await
        .map_err(|e| End::lost(e.to_string()))?;
    // Read the response header a byte at a time so nothing after it (the
    // relay's first TLS bytes) is consumed.
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= MAX_HEADER {
            return Err(End::Upgrade("proxy response too long".into()));
        }
        let b = s
            .read_u8()
            .await
            .map_err(|e| End::lost(format!("proxy: {e}")))?;
        head.push(b);
    }
    match connect_status(&head) {
        Some(200..=299) => Ok(s),
        Some(407) => Err(End::Upgrade(
            "proxy authentication required (only Basic, from the proxy URL, is supported)".into(),
        )),
        Some(c) => Err(End::Upgrade(format!("proxy refused CONNECT (HTTP {c})"))),
        None => Err(End::Upgrade("proxy sent a malformed response".into())),
    }
}

fn connect_status(head: &[u8]) -> Option<u16> {
    let line = head.split(|b| *b == b'\n').next()?;
    let line = std::str::from_utf8(line).ok()?;
    let mut parts = line.split_whitespace();
    if !parts.next()?.starts_with("HTTP/1.") {
        return None;
    }
    parts.next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lines() {
        assert_eq!(
            connect_status(b"HTTP/1.1 200 Connection established\r\n\r\n"),
            Some(200)
        );
        assert_eq!(connect_status(b"HTTP/1.0 407 Auth\r\n\r\n"), Some(407));
        assert_eq!(connect_status(b"SSH-2.0\r\n\r\n"), None);
    }

    #[tokio::test]
    async fn connect_through_a_proxy() {
        use tokio::net::TcpListener;
        // A one-shot proxy that checks the CONNECT line and then echoes.
        let l = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = l.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (mut c, _) = l.accept().await.expect("accept");
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                head.push(c.read_u8().await.expect("read"));
            }
            let h = String::from_utf8(head).expect("utf8");
            assert!(
                h.starts_with("CONNECT relay.example:443 HTTP/1.1\r\n"),
                "{h}"
            );
            assert!(h.contains("Proxy-Authorization: Basic "), "{h}");
            c.write_all(b"HTTP/1.1 200 OK\r\n\r\nhi")
                .await
                .expect("write");
        });
        let m = Matcher::builder()
            .https(format!("http://user:pw@{addr}"))
            .build();
        let t = Target {
            tls: true,
            host: "relay.example".into(),
            port: 443,
        };
        let mut s = tcp_to(&m, &t).await.expect("tunnel");
        let mut b = [0u8; 2];
        s.read_exact(&mut b).await.expect("bytes after the header");
        assert_eq!(&b, b"hi");
        server.await.expect("server");
    }
}
