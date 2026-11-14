#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
# SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception
#
# Real-network integration tests (spec/07-testing.md, section 4): real
# muster-agent --headless binaries in Linux network namespaces joined by
# bridges, with nftables and bridge port isolation shaping what gets through.
#
# Usage: tests/netns/run.sh [scenario...]   (default: all)
# Scenarios: s1 s2 s4 s7
#
# Runs as root (CI uses sudo), or re-executes itself in an unprivileged user
# and network namespace where the kernel allows it. Needs ip, nft and the
# release binaries (cargo build --release -p muster-agent -p muster-cli).

set -euo pipefail

if [[ $(id -u) -ne 0 ]]; then
  exec unshare --user --map-root-user --net --mount "$0" "$@"
fi

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BIN=${MUSTER_BIN:-$ROOT/target/release}
AGENT=$BIN/muster-agent
CLI=$BIN/muster
PORT=47474
for b in "$AGENT" "$CLI"; do
  [[ -x $b ]] || { echo "missing $b: cargo build --release -p muster-agent -p muster-cli" >&2; exit 2; }
done

# Private /run so netns names do not clash with the host.
mount -t tmpfs none /run 2>/dev/null || true
mkdir -p /run/netns
WORK=$(mktemp -d)
PIDS=()
cleanup() {
  for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done
  wait 2>/dev/null || true
  for ns in $(ip netns list | awk '{print $1}'); do ip netns del "$ns" 2>/dev/null || true; done
  for br in $(ip -o link show type bridge | awk -F': ' '{print $2}'); do ip link del "$br" 2>/dev/null || true; done
  if [[ ${KEEP:-} != 1 ]]; then rm -rf "$WORK"; else echo "kept $WORK"; fi
}
trap cleanup EXIT

fail() { echo "FAIL: $*" >&2; for f in "$WORK"/*.log; do echo "== $f" >&2; tail -20 "$f" >&2; done; exit 1; }
pass() { echo "ok: $*"; }

# bridge NAME
bridge() {
  ip link add "$1" type bridge
  ip link set "$1" type bridge mcast_snooping 0
  ip link set "$1" up
}

# attach NODE BRIDGE ADDR/PREFIX [isolated]
attach() {
  local node=$1 br=$2 addr=$3 iso=${4:-}
  ip netns list | grep -qw "$node" || { ip netns add "$node"; ip -n "$node" link set lo up; }
  local n; n=$(ip -n "$node" -o link | grep -c ' eth' || true)
  local dev="eth$n" host="v${node}${n}"
  ip link add "$host" type veth peer name "$dev"
  ip link set "$dev" netns "$node"
  ip link set "$host" master "$br" up
  [[ $iso == isolated ]] && ip link set dev "$host" type bridge_slave isolated on
  ip -n "$node" addr add "$addr" brd + dev "$dev"
  ip -n "$node" link set "$dev" up
  # Multicast needs a route out of the interface.
  ip -n "$node" route add 224.0.0.0/4 dev "$dev" 2>/dev/null || true
}

# A practice: node NAME is the admin; returns via files in $WORK.
practice() {
  local admin=$1
  printf 'correct horse battery staple\n' > "$WORK/pass"
  "$CLI" --data-dir "$WORK/$admin" practice init --name "Netns Surgery" \
    --recovery-kit "$WORK/kit" --passphrase-file "$WORK/pass" --label "$admin" >/dev/null
  "$CLI" --data-dir "$WORK/$admin" token create --days 1 --out "$WORK/token" >/dev/null
}

# start NODE [extra config lines]
start() {
  local node=$1; shift
  {
    printf '[network]\nport = %s\nmdns = false\n' "$PORT"
    [[ $# -gt 0 ]] && printf '%s\n' "$@"
    [[ -d $WORK/$node ]] || printf '[enrolment]\ntoken_file = "%s"\n' "$WORK/token.$node"
  } > "$WORK/$node.toml"
  [[ -d $WORK/$node ]] || cp "$WORK/token" "$WORK/token.$node"
  ip netns exec "$node" env MUSTER_LOG=info "$AGENT" --headless \
    --config "$WORK/$node.toml" --policy /dev/null --data-dir "$WORK/$node" \
    --control-socket "$WORK/$node.sock" --label "$node" --allow-raise \
    >"$WORK/$node.log" 2>&1 &
  PIDS+=($!)
}

ctl() { local node=$1; shift; "$CLI" --control-socket "$WORK/$node.sock" "$@"; }

wait_ready() {
  for node in "$@"; do
    for _ in $(seq 50); do [[ -S $WORK/$node.sock ]] && break; sleep 0.1; done
    [[ -S $WORK/$node.sock ]] || fail "$node did not start"
  done
}

# level NODE -> green|amber|red
level() { ctl "$1" --format json ctl status | sed -n 's/.*"level": "\([a-z]*\)".*/\1/p'; }

# expect_level NODE LEVEL WITHIN_SECONDS
expect_level() {
  local node=$1 want=$2 secs=$3 got=
  for _ in $(seq $((secs * 4))); do
    got=$(level "$node")
    [[ $got == "$want" ]] && return 0
    sleep 0.25
  done
  ctl "$node" ctl status >&2 || true
  fail "$node is $got, expected $want"
}

# raise_and_expect ORIGIN BOUND_SECONDS RECEIVER...
raise_and_expect() {
  local origin=$1 bound=$2; shift 2
  local waits=()
  for r in "$@"; do
    ctl "$r" ctl wait-alarm --timeout "${bound}s" >"$WORK/wait.$r" 2>&1 &
    waits+=("$!:$r")
  done
  sleep 0.3
  ctl "$origin" ctl raise >/dev/null || fail "raise on $origin"
  for w in "${waits[@]}"; do
    wait "${w%%:*}" || fail "${w#*:} did not alert within ${bound}s: $(cat "$WORK/wait.${w#*:}")"
  done
}

expect_no_alarm() {
  local node=$1 secs=$2
  if ctl "$node" ctl wait-alarm --timeout "${secs}s" >/dev/null 2>&1; then
    fail "$node alerted but should not have"
  fi
}

reset() {
  cleanup
  trap cleanup EXIT
  WORK=$(mktemp -d)
  PIDS=()
}

# S1: flat LAN, three devices.
s1() {
  bridge br0
  for i in 1 2 3; do attach "n$i" br0 "10.10.0.$i/24"; done
  practice n1
  start n1; start n2; start n3
  wait_ready n1 n2 n3
  for n in n1 n2 n3; do expect_level "$n" green 25; done
  raise_and_expect n2 3 n1 n3
  pass "S1 flat LAN"
}

# S2: multicast blocked on the bridge; broadcast still works.
s2() {
  bridge br0
  for i in 1 2 3; do attach "n$i" br0 "10.10.0.$i/24"; done
  nft add table bridge muster
  nft add chain bridge muster filter_forward '{ type filter hook forward priority 0; }'
  nft add rule bridge muster filter_forward ether daddr 01:00:5e:00:00:00/25 drop
  practice n1
  start n1; start n2; start n3
  wait_ready n1 n2 n3
  for n in n1 n2 n3; do expect_level "$n" green 25; done
  raise_and_expect n1 3 n2 n3
  pass "S2 multicast blocked"
}

# S4: two unrouted VLANs bridged by one dual-homed device.
s4() {
  bridge br0; bridge br1
  attach n1 br0 10.10.0.1/24
  attach n2 br0 10.10.0.2/24
  attach n3 br1 10.11.0.3/24
  attach n4 br1 10.11.0.4/24
  attach n5 br0 10.10.0.5/24
  attach n5 br1 10.11.0.5/24
  practice n5
  for n in n5 n1 n2 n3 n4; do start "$n"; done
  wait_ready n1 n2 n3 n4 n5
  sleep 20
  raise_and_expect n1 3 n2 n3 n4 n5
  raise_and_expect n4 3 n1 n2 n3
  pass "S4 dual-homed bridge"
}

# S7: Wi-Fi client isolation and no relay: laptops are red and their alarms
# cannot be delivered, and they say so.
s7() {
  bridge br0; bridge br1
  attach n1 br0 10.10.0.1/24
  attach n2 br0 10.10.0.2/24
  attach n3 br1 10.11.0.3/24 isolated
  attach n4 br1 10.11.0.4/24 isolated
  practice n1
  for n in n1 n2 n3 n4; do start "$n"; done
  wait_ready n1 n2 n3 n4
  expect_level n1 green 25
  expect_level n3 red 25
  expect_level n4 red 25
  ctl n3 ctl raise >/dev/null
  expect_no_alarm n1 3
  pass "S7 client isolation, no relay: red"
}

scenarios=("$@")
[[ ${#scenarios[@]} -eq 0 ]] && scenarios=(s1 s2 s4 s7)
first=1
for s in "${scenarios[@]}"; do
  [[ $first == 1 ]] || reset
  first=0
  "$s"
done
echo "all netns scenarios passed"
