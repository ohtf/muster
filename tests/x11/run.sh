#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
# SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception
#
# Runs tests/x11/test.py against the release desktop app and CLI in an Arch
# Linux container with Xvfb: once with no window manager and once under
# openbox. Arch, because binaries built on an up-to-date Arch host need its
# newer C library. Needs Docker and the release binaries
# (target/release/panic-button-desktop and target/release/panic-button).

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BIN=${PANIC_BUTTON_BIN:-$ROOT/target/release}
IMAGE=archlinux:base@sha256:d4594bb2300cba11f41c2f27923eb56f63d9988909c402f9c95517d1e362c899
for b in "$BIN/panic-button-desktop" "$BIN/panic-button"; do
  [[ -x $b ]] || { echo "missing $b: run s/x11-test, which builds it" >&2; exit 2; }
done
command -v docker >/dev/null || { echo "tests/x11 needs Docker" >&2; exit 2; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp "$BIN/panic-button-desktop" "$BIN/panic-button" "$ROOT/tests/x11/test.py" "$work/"

exec docker run --rm -v "$work:/w:ro" "$IMAGE" bash -c '
  set -euo pipefail
  pacman -Syu --noconfirm --needed -q xorg-server-xvfb openbox python python-xlib \
    webkit2gtk-4.1 gtk3 alsa-lib libayatana-appindicator >/dev/null 2>&1
  Xvfb :99 -screen 0 1920x1080x24 >/dev/null 2>&1 &
  sleep 2
  echo "== no window manager"
  python3 /w/test.py || { tail -40 /tmp/app.log; exit 1; }
  rm -rf /tmp/d /tmp/kit /tmp/app.log
  DISPLAY=:99 openbox >/dev/null 2>&1 &
  sleep 2
  echo "== openbox"
  python3 /w/test.py || { tail -40 /tmp/app.log; exit 1; }
'
