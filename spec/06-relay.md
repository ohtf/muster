# 06 Relay

## Purpose

An optional, blind forwarder. It bridges VLANs, Wi-Fi client isolation and remote staff, and gives the admin a whole-practice presence view. It never holds group keys and cannot read alarms. Losing it removes one delivery path, not the system.

## Deployment shapes

- **Practice box:** a small Linux device on the practice network (a mini PC or Raspberry Pi 5 class device), reachable by internal clients. It can also run a headless agent.
- **PCN or federation host:** one relay serving several practices (multi-tenant).
- **OHTF-hosted:** a multi-tenant relay run by the foundation for practices that want it.

Clients can be configured with several relay URLs and connect to all of them.

## Configuration (`relay.toml`)

```toml
listen = "0.0.0.0:8443"          # plain HTTP if behind a TLS-terminating reverse proxy
tls_cert = ""                    # optional built-in TLS (rustls); leave empty when proxied
tls_key  = ""
public_source_url = "https://github.com/<org>/panic-button/tree/v{version}"
data_dir = "/var/lib/panic-button-relay"
persist = true                   # SQLite ring buffer survives restarts

[limits]
max_connections_per_tenant = 500
max_frame_bytes = 65536
msgs_per_sec_per_conn = 50
burst_per_conn = 200
buffer_hours = 24
buffer_max_envelopes = 10000

[[tenant]]
practice_id = "hex-16-bytes"
root_pub    = "hex-32-bytes"
name        = "Riverside Surgery"   # operator-facing label only
```

The tenant CLI: `panic-button-relay tenant add --practice-id .. --root-pub .. --name ..`, `tenant list`, `tenant remove`. The admin's desktop app shows the practice id and root pub with a "Copy relay registration details" button for sending to the relay operator.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| GET | `/v1/ws` | WebSocket (binary frames) |
| POST | `/v1/lp/auth` | Long-poll: start auth, returns challenge |
| POST | `/v1/lp/auth/complete` | Long-poll: submit RelayAuth, returns session token |
| POST | `/v1/lp/send` | Long-poll: body is a CBOR array of Envelopes |
| GET | `/v1/lp/recv?cursor=N&timeout=25` | Long-poll: returns envelopes after the cursor, waiting up to 25 s |
| GET | `/healthz` | `200 ok` |
| GET | `/metrics` | Prometheus metrics (counts only, no content); bind to localhost or protect |
| GET | `/source` | 302 to `public_source_url` (AGPL section 13) |

## Handshake

1. The server sends `Challenge = {0: nonce bstr(32), 1: server_time uint, 2: relay_version tstr}`.
2. The client sends a RelayAuth open envelope, signed by the device:
   `{0: practice_id, 1: chain [Cert] (device cert up to, but not including, root), 2: nonce (echo), 3: client_time, 4: resume_cursor uint (optional)}`.
3. The server checks: the tenant exists; the chain is valid to the tenant `root_pub` (rules as in `04-security.md`, using the relay's own revocation store for that tenant); the nonce matches; the signature is valid.
4. The server replies `{0: ok bool, 1: session_id bstr(16), 2: cursor uint, 3: presence [DeviceId]}`, or closes with an error code: 4401 auth failed, 4403 revoked, 4404 unknown tenant, 4429 rate limited.

Long-poll uses the same challenge and response via the two `auth` endpoints. It returns a bearer session token valid for 1 h, which the client renews by re-authenticating.

## Forwarding

- Every envelope received from an authenticated session is assigned a per-tenant monotonic cursor, appended to the tenant ring buffer, and forwarded to every other session of the same tenant.
- The relay does **not** parse sealed envelopes beyond the header. It does verify that the sealed `tag` is one it has seen from that tenant's authenticated clients (it learns tags; it does not need keys). This stops a client from using the relay to send junk to another tenant's sessions.
- Open envelopes of type Revocation are parsed and verified (admin chain to the tenant root). On a valid revocation the relay stores it, closes sessions belonging to the revoked device (and to cascaded descendants), and refuses them in future.
- On (re)connect with `resume_cursor`, the server replays buffered envelopes after the cursor, then live traffic. Without a cursor it replays the last `buffer_hours`. Clients dedupe as usual.
- Presence: on connect and disconnect, the relay sends all sessions of the tenant a small `Presence {0: connected [DeviceId]}` control frame (frame type 0x01; envelopes use frame type 0x00; WebSocket messages are prefixed with 1 byte of frame type).
- Ping and pong every `RELAY_PING`. Close sessions after 2 missed pongs.

## Client behaviour

- Connect to every configured relay URL. Prefer WebSocket. If the WebSocket upgrade fails (for example HTTP 400/403/501 from a proxy, or a timeout after the HTTP response), fall back to long-poll for that relay for 1 h, then retry WebSocket.
- Honour the system proxy (Windows WinHTTP/WinINet proxy settings and PAC where feasible, `HTTPS_PROXY` on Unix). Support HTTP CONNECT. NTLM/Kerberos proxy auth is S for v1. If it is missing, document the workaround (proxy bypass rule for the relay host).
- TLS: system root store, plus an optional `relay_ca_pem` in config for private CAs.
- Reconnect with `RELAY_BACKOFF`. On reconnect, send `resume_cursor`.

## Operations

- The Docker image is distroless or Alpine-based, runs as non-root, and exposes 8443. Example `docker-compose.yml` with Caddy for TLS in `packaging/relay/`.
- systemd unit with `DynamicUser=yes`, `StateDirectory=panic-button-relay` and hardening options (`ProtectSystem=strict`, `NoNewPrivileges=yes`, and so on).
- Logs: connection events with device short ids and tenant name. Never payloads.
- Operator guidance documents what metadata the relay can see (see `04-security.md`, Privacy).

## Clarifications (added during implementation)

These fill gaps the original text left open. They are normative for version 1.

- **Cursors survive restarts.** A tenant's first cursor in a relay process is `now_ms × 1000`, or one past the highest persisted cursor if that is larger. Cursors therefore keep increasing across restarts even without `persist`, and a client resuming from an older cursor misses nothing.
- **`persist = true`** keeps three things in `<data_dir>/relay.sqlite`: the ring buffer, the certs learned from auth chains, and verified revocations. A restarted relay still refuses revoked devices and still replays the buffer. Write errors are logged and the relay carries on from memory.
- **Revocations from a signer the relay does not know yet** are held (up to 64 per tenant) and checked again whenever an auth chain introduces new certs. The relay learns certs only from auth chains, so an admin who has never connected to it would otherwise be unable to revoke through it.
- **Long-poll responses.** `GET /v1/lp/recv` returns a CBOR array of envelopes, with the newest cursor in `X-Panic Button-Cursor` and the other connected devices (comma-separated hex device ids) in `X-Panic Button-Presence`. It returns early when envelopes arrive or presence changes. A long-poll session that neither polls nor sends for 60 s is ended, and presence is updated.
- **Long-poll clients** start from cursor 0 when they have no cursor, so they read the whole buffer, just as a fresh WebSocket session is replayed the last `buffer_hours`. On a 401 they authenticate again without reporting a disconnect.
- **When WebSocket counts as failed** (and the client switches to long-poll for an hour): the server answers the upgrade with any HTTP status other than 101; the handshake fails at the protocol level; the handshake does not finish within 10 s; no challenge arrives within 10 s of the upgrade; or a proxy refuses `CONNECT`. A plain TCP or TLS failure is not an upgrade failure, so the client backs off and tries WebSocket again.
- **Proxies.** Proxy settings come from `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY` and `NO_PROXY` (in either case), or from the system settings on Windows and macOS. WebSocket goes through HTTP `CONNECT` with Basic credentials from the proxy URL. A proxy reached over HTTPS, or one that needs NTLM or Kerberos, is used through long-poll only. If that fails too, add a proxy bypass rule for the relay host.
- **Diagnostics.** `PANIC_BUTTON_RELAY_TRANSPORT=long-poll` in the agent's environment skips WebSocket entirely.
