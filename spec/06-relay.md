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
public_source_url = "https://github.com/<org>/muster/tree/v{version}"
data_dir = "/var/lib/muster-relay"
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

The tenant CLI: `muster-relay tenant add --practice-id .. --root-pub .. --name ..`, `tenant list`, `tenant remove`. The admin's desktop app shows the practice id and root pub with a "Copy relay registration details" button for sending to the relay operator.

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
- systemd unit with `DynamicUser=yes`, `StateDirectory=muster-relay` and hardening options (`ProtectSystem=strict`, `NoNewPrivileges=yes`, and so on).
- Logs: connection events with device short ids and tenant name. Never payloads.
- Operator guidance documents what metadata the relay can see (see `04-security.md`, Privacy).
