# SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
# SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

# The muster-relay image: distroless, non-root, plain HTTP on 8443 behind a
# TLS-terminating proxy (see docker-compose.yml). Build from the repo root:
#   docker build -f packaging/relay/Dockerfile -t muster-relay .

FROM rust:1.100.0-slim-trixie@sha256:e9d95b74296f7126a83ae3d041aeaba1ff4c478f13abc5cc034fcf7c131c3d4a AS build
# The toolchain file asks for clippy and rustfmt, which a build does not need.
ENV RUSTUP_TOOLCHAIN=1.100.0
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
# A workspace member: cargo needs its manifest and sources to load the
# workspace, though only the relay is built.
COPY apps/desktop/src-tauri apps/desktop/src-tauri
RUN cargo build --release --locked -p muster-relay \
 && mkdir -p /out/var/lib/muster-relay /out/etc/muster-relay \
 && cp target/release/muster-relay /out/muster-relay

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2
COPY --from=build /out/muster-relay /muster-relay
COPY --from=build --chown=65532:65532 /out/var/lib/muster-relay /var/lib/muster-relay
COPY --from=build /out/etc/muster-relay /etc/muster-relay
COPY LICENSE LICENSE-EXCEPTIONS.md /usr/share/doc/muster-relay/
USER 65532:65532
EXPOSE 8443
VOLUME ["/var/lib/muster-relay"]
ENTRYPOINT ["/muster-relay", "--config", "/etc/muster-relay/relay.toml"]
CMD ["serve"]
