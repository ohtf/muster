#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
# SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception
#
# Real-network integration tests (spec/07-testing.md, section 4): real
# muster-agent --headless binaries in Linux network namespaces joined by
# bridges, with nftables and bridge port isolation shaping what gets through.
#
# Usage: tests/netns/run.sh [scenario...]   (default: all)
# Scenarios: s1 s2 s4 s7 (LAN), s5 s6 s12 s14 (relay and revocation)
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
RELAY=$BIN/muster-relay
PORT=47474
for b in "$AGENT" "$CLI" "$RELAY"; do
  [[ -x $b ]] || { echo "missing $b: cargo build --release -p muster-agent -p muster-cli -p muster-relay" >&2; exit 2; }
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

fail() {
  echo "FAIL: $*" >&2
  for sock in "$WORK"/*.sock; do
    [[ -S $sock ]] || continue
    echo "== peers of $(basename "$sock" .sock)" >&2
    "$CLI" --control-socket "$sock" ctl peers >&2 || true
  done
  for f in "$WORK"/*.log; do echo "== $f" >&2; tail -20 "$f" >&2; done
  exit 1
}
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
  "$CLI" --format json --data-dir "$WORK/$admin" practice init --name "Netns Surgery" \
    --recovery-kit "$WORK/kit" --passphrase-file "$WORK/pass" --label "$admin" >"$WORK/practice.json"
  PRACTICE_ID=$(sed -n 's/.*"practice_id": "\([0-9a-f]*\)".*/\1/p' "$WORK/practice.json")
  ROOT_PUB=$(sed -n 's/.*"root_pub": "\([0-9a-f]*\)".*/\1/p' "$WORK/practice.json")
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
  # shellcheck disable=SC2086 # AGENT_ENV is a list of NAME=value words.
  ip netns exec "$node" env MUSTER_LOG=info ${AGENT_ENV:-} "$AGENT" --headless \
    --config "$WORK/$node.toml" --policy /dev/null --data-dir "$WORK/$node" \
    --control-socket "$WORK/$node.sock" --label "$node" --allow-raise \
    >"$WORK/$node.log" 2>&1 &
  PIDS+=($!)
}

ctl() { local node=$1; shift; "$CLI" --control-socket "$WORK/$node.sock" "$@"; }

# relay_start NODE: run a relay in that namespace serving the practice
# (RELAY_PERSIST=true keeps its buffer in SQLite across restarts).
relay_start() {
  local node=$1
  mkdir -p "$WORK/relay"
  printf 'listen = "0.0.0.0:8443"\ndata_dir = "%s"\npersist = %s\n[[tenant]]\npractice_id = "%s"\nroot_pub = "%s"\nname = "netns"\n' \
    "$WORK/relay" "${RELAY_PERSIST:-false}" "$PRACTICE_ID" "$ROOT_PUB" > "$WORK/relay.toml"
  ip netns exec "$node" "$RELAY" --config "$WORK/relay.toml" serve >>"$WORK/relay.log" 2>&1 &
  RELAY_PID=$!
  PIDS+=("$RELAY_PID")
  sleep 0.5
}

relay_stop() { kill "$RELAY_PID" 2>/dev/null || true; wait "$RELAY_PID" 2>/dev/null || true; }

# view NODE ALARM_ID FIELD -> value of a JSON field from ctl alarms
coming_count() {
  ctl "$1" --format json ctl alarms | tr -d '\n ' | sed -n "s/.*\"alarm_id\":\"$2\"[^}]*\"coming\":\[\([^]]*\)\].*/\1/p" | tr ',' '\n' | grep -c . || true
}

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

# raise_and_expect ORIGIN BOUND_SECONDS RECEIVER...  (sets ALARM_ID)
raise_and_expect() {
  local origin=$1 bound=$2; shift 2
  local waits=()
  for r in "$@"; do
    ctl "$r" ctl wait-alarm --timeout "${bound}s" >"$WORK/wait.$r" 2>&1 &
    waits+=("$!:$r")
  done
  sleep 0.3
  ALARM_ID=$(ctl "$origin" --format json ctl raise | sed -n 's/.*"alarm_id": "\([0-9a-f]*\)".*/\1/p') || fail "raise on $origin"
  [[ -n $ALARM_ID ]] || fail "raise on $origin returned no alarm"
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

RELAY_URL='relay_urls = ["ws://10.20.0.100:8443/v1/ws"]'

# S5: two unrouted VLANs, relay reachable by both.
s5() {
  bridge br0; bridge br1; bridge brr
  attach r brr 10.20.0.100/24
  attach n1 br0 10.10.0.1/24; attach n1 brr 10.20.0.1/24
  attach n2 br0 10.10.0.2/24; attach n2 brr 10.20.0.2/24
  attach n3 br1 10.11.0.3/24; attach n3 brr 10.20.0.3/24 isolated
  # The relay network carries only TCP to the relay: drop everything else.
  nft add table bridge muster
  nft add chain bridge muster filter_forward '{ type filter hook forward priority 0; }'
  nft add rule bridge muster filter_forward meta ibrname brr ip daddr != 10.20.0.100 ip saddr != 10.20.0.100 drop
  practice n1
  relay_start r
  for n in n1 n2 n3; do start "$n" "$RELAY_URL"; done
  wait_ready n1 n2 n3
  expect_level n1 green 25
  expect_level n3 amber 25
  raise_and_expect n1 5 n2 n3
  raise_and_expect n3 5 n1 n2
  pass "S5 two VLANs via relay${S5_LABEL:-}"
}

# S5 again with every device on HTTP long-poll, as behind a proxy that
# breaks WebSockets.
s5lp() {
  AGENT_ENV=MUSTER_RELAY_TRANSPORT=long-poll S5_LABEL=" (long-poll)" s5
}

# S6: Wi-Fi with client isolation and a wired segment; laptops use the relay.
s6() {
  bridge br0; bridge brw
  attach r br0 10.10.0.100/24
  attach n1 br0 10.10.0.1/24
  attach n2 br0 10.10.0.2/24
  attach l1 brw 10.12.0.11/24 isolated
  attach l2 brw 10.12.0.12/24 isolated
  attach r brw 10.12.0.100/24
  practice n1
  relay_start r
  for n in n1 n2; do start "$n" 'relay_urls = ["ws://10.10.0.100:8443/v1/ws"]'; done
  for n in l1 l2; do start "$n" 'relay_urls = ["ws://10.12.0.100:8443/v1/ws"]'; done
  wait_ready n1 n2 l1 l2
  expect_level l1 amber 25
  raise_and_expect n1 5 n2 l1 l2
  raise_and_expect l1 5 n1 n2 l2
  pass "S6 Wi-Fi isolation with relay"
}

# S12: the relay goes down mid-alarm; the LAN is unaffected, and the
# relay-only device gets the events after the relay returns.
s12() {
  bridge br0; bridge brw
  attach r br0 10.10.0.100/24
  attach n1 br0 10.10.0.1/24
  attach n2 br0 10.10.0.2/24
  attach l1 brw 10.12.0.11/24 isolated
  attach r brw 10.12.0.100/24
  practice n1
  RELAY_PERSIST=true relay_start r
  for n in n1 n2; do start "$n" 'relay_urls = ["ws://10.10.0.100:8443/v1/ws"]'; done
  start l1 'relay_urls = ["ws://10.12.0.100:8443/v1/ws"]'
  wait_ready n1 n2 l1
  expect_level l1 amber 25
  raise_and_expect n1 5 n2 l1
  relay_stop
  ctl n2 ctl respond "$ALARM_ID" >/dev/null
  sleep 1
  [[ $(coming_count n1 "$ALARM_ID") == 1 ]] || fail "LAN did not get the response"
  [[ $(coming_count l1 "$ALARM_ID") == 0 ]] || fail "relay-only device got it with the relay down?"
  RELAY_PERSIST=true relay_start r
  grep -q "persisting to" "$WORK/relay.log" || fail "relay did not open its store"
  for _ in $(seq 120); do
    [[ $(coming_count l1 "$ALARM_ID") == 1 ]] && break
    sleep 0.5
  done
  [[ $(coming_count l1 "$ALARM_ID") == 1 ]] || fail "relay-only device never caught up"
  pass "S12 relay down mid-alarm"
}

# S14: an admin revokes a relay-only laptop during an open alarm. The
# relay closes and refuses the laptop, the alarm carries on for everyone
# else, and the laptop's later events are rejected.
s14() {
  bridge br0; bridge brw
  attach r br0 10.10.0.100/24
  for i in 1 2 3; do attach "n$i" br0 "10.10.0.$i/24"; done
  attach l1 brw 10.12.0.11/24 isolated
  attach r brw 10.12.0.100/24
  practice n1
  relay_start r
  for n in n1 n2 n3; do start "$n" 'relay_urls = ["ws://10.10.0.100:8443/v1/ws"]'; done
  start l1 'relay_urls = ["ws://10.12.0.100:8443/v1/ws"]'
  wait_ready n1 n2 n3 l1
  for n in n1 n2 n3; do expect_level "$n" green 25; done
  expect_level l1 amber 25
  raise_and_expect n2 5 n1 n3 l1
  local laptop
  laptop=$(ctl l1 --format json ctl status | sed -n 's/.*"device": "\([0-9a-f]*\)".*/\1/p')
  ctl n1 ctl revoke "$laptop" --reason "Laptop lost" >/dev/null
  expect_level l1 red 10
  ctl l1 ctl respond "$ALARM_ID" >/dev/null || true
  ctl n3 ctl respond "$ALARM_ID" >/dev/null
  sleep 2
  for n in n1 n2 n3; do
    [[ $(coming_count "$n" "$ALARM_ID") == 1 ]] || fail "$n: coming should be only n3"
  done
  pass "S14 revoke a relay-only laptop during an open alarm"
}

# Admin commands (R27): tokens list the devices they enrolled, a drill
# leaves a report with its receipts, and revoking a token stops it without
# removing the devices it enrolled.
admin() {
  bridge br0
  for i in 1 2; do attach "n$i" br0 "10.10.0.$i/24"; done
  practice n1
  start n1; start n2
  wait_ready n1 n2
  for n in n1 n2; do expect_level "$n" green 25; done
  local tokens token_id
  tokens=$(ctl n1 --format json token list | tr -d '\n ')
  token_id=$(sed -n 's/.*"token_id":"\([0-9a-f]*\)".*/\1/p' <<<"$tokens")
  [[ -n $token_id ]] || fail "token list shows no token: $tokens"
  grep -q '"enrolled":\[{' <<<"$tokens" || fail "token list does not show n2 as enrolled: $tokens"
  ctl n1 ctl drill >/dev/null
  # The report is made when the drill closes itself, after 60 s.
  local report=
  for _ in $(seq 160); do
    report=$(ctl n1 --format json ctl drill-reports | tr -d '\n ')
    grep -q '"received":1' <<<"$report" && break
    sleep 0.5
  done
  grep -q '"received":1' <<<"$report" || fail "no drill report with n2's receipt: $report"
  local drill_id
  drill_id=$(sed -n 's/.*"alarm_id":"\([0-9a-f]*\)".*/\1/p' <<<"$report")
  ctl n1 ctl drill-report "$drill_id" --csv | grep -q "n2" || fail "drill report CSV lacks n2"
  ctl n1 token revoke "$token_id" >/dev/null
  sleep 1
  ctl n1 --format json token list | tr -d '\n ' | grep -q '"revoked":true' || fail "token not revoked"
  [[ $(ctl n1 --format json device list | grep -c '"device_short"') == 2 ]] || fail "n2 should stay a member"
  expect_level n2 green 10
  pass "admin: tokens, drill report, token revoke"
}

scenarios=("$@")
[[ ${#scenarios[@]} -eq 0 ]] && scenarios=(s1 s2 s4 s7 s5 s5lp s6 s12 s14 admin)
if [[ ${#scenarios[@]} -gt 1 ]]; then
  # Each scenario in a fresh set of namespaces.
  for s in "${scenarios[@]}"; do
    unshare --net --mount "$0" "$s"
  done
  echo "all netns scenarios passed"
  exit 0
fi
"${scenarios[0]}"
