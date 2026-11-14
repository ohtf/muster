<!--
SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Running a Muster relay

The relay is an optional, blind forwarder (`spec/06-relay.md`). It bridges VLANs, Wi-Fi client isolation and remote staff. It never holds group keys and cannot read alarms. Losing it removes one delivery path, not the system.

## Docker Compose (with Caddy for TLS)

1. Point a DNS name at the host and put it in `Caddyfile` in place of `relay.example.org.uk`.
2. Review `relay.toml`. If you change the code, point `public_source_url` at your source.
3. Start it: `docker compose up -d`. If you have no published image to pull, run `docker compose build` first.
4. Register each practice. The practice admin sends you the practice id and root public key (in the app: "Copy relay registration details"; or `muster practice init --format json`):

   ```sh
   docker compose exec relay /muster-relay --config /etc/muster-relay/relay.toml \
     tenant add --practice-id <32 hex> --root-pub <64 hex> --name "Riverside Surgery"
   docker compose restart relay
   ```

5. Give practices the URL `wss://<your host>/v1/ws` for `network.relay_urls`.

Check it with `curl https://<your host>/healthz` (expect `ok`) and `curl -I https://<your host>/source`.

## systemd

`muster-relay.service` runs the binary with `DynamicUser=yes`, state in `/var/lib/muster-relay` and config in `/etc/muster-relay/relay.toml`, with sandboxing. The relay speaks plain HTTP on 8443, so put a TLS-terminating reverse proxy in front of it. Long-poll requests wait up to 25 s, so the proxy's read timeout must be longer than that. Tenants: `muster-relay --config /etc/muster-relay/relay.toml tenant add ...`, then `systemctl restart muster-relay`.

## What the relay can see

It cannot read alarms, names or rooms. It does see:

- which devices of each tenant are connected, and when (device ids, not names);
- the IP addresses they connect from;
- when envelopes are sent and how large they are, which reveals that something is happening;
- revocations, which are not encrypted (a device id and a cert serial).

Logs record connection events with short device ids and the tenant name, never payloads. `/metrics` gives counts only; keep it off the internet (the Caddyfile blocks it).

## Obligations

The relay is AGPL-3.0-or-later. If you modify it and let others use it over a network, you must offer them your source. `GET /source` redirects to `public_source_url` for this.

## Behind a proxy that breaks WebSockets

Clients fall back to HTTPS long-poll for an hour when the WebSocket upgrade fails, then try again. Clients use the system proxy settings (or `HTTPS_PROXY`). NTLM and Kerberos proxy authentication are not supported; add a proxy bypass rule for the relay host.
