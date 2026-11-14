// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

// A stand-in agent for running the views in a plain browser: `?mock=` picks
// a scenario (idle, alert, sender, firstrun, amber, red). Never used inside
// the app itself.

import type { Alarm, MusterEvent, Peer, Status } from "./types";

type Listener = (p: unknown) => void;

const scenario = new URLSearchParams(location.search).get("mock") ?? "idle";
const listeners = new Map<string, Set<Listener>>();
const now = Date.now();

function emit(name: string, payload: unknown) {
  for (const l of listeners.get(name) ?? []) l(payload);
}
const event = (e: MusterEvent) => emit("muster", e);

const status: Status = {
  device: "a".repeat(64),
  device_short: "aaaaaaaa",
  admin: true,
  enrolled: scenario !== "firstrun",
  practice: "Riverside Surgery",
  level: scenario === "red" ? "red" : scenario === "amber" ? "amber" : "green",
  summary:
    scenario === "red"
      ? "Not connected to any devices"
      : scenario === "amber"
        ? "Connected to 3 devices (1 direct, 2 via relay)"
        : "Connected to 14 devices (12 direct, 2 via relay)",
  reasons:
    scenario === "red"
      ? ["No other devices reachable"]
      : scenario === "amber"
        ? ["Only reachable via relay for some devices"]
        : [],
  direct: 12,
  via_relay: 2,
};

function alarm(p: Partial<Alarm>): Alarm {
  return {
    alarm_id: "0123456789abcdef0123456789abcdef",
    origin: "b".repeat(64),
    own: false,
    drill: false,
    room: "Consulting Room 4",
    user: "Dr A Example",
    device_label: "ROOM4-PC",
    raised_at: now - 20_000,
    state: "open",
    resolved_by: null,
    closed_at: null,
    delivered: 13,
    coming: [],
    seen: [],
    local_ack: null,
    ...p,
  };
}

const alarms: Alarm[] = [];
if (scenario === "alert") {
  alarms.push(alarm({ coming: ["Jane Smith (Reception)"] }));
  alarms.push(
    alarm({
      alarm_id: "fedcba9876543210fedcba9876543210",
      room: "Treatment Room 1",
      user: "Nurse B Example",
      device_label: "TREAT1",
      raised_at: now - 4 * 60_000,
      local_ack: "responding",
      coming: ["You", "Dr Ali (Room 2)"],
    }),
  );
}
if (scenario === "sender") {
  alarms.push(
    alarm({
      own: true,
      room: "Reception",
      user: "Jane Smith",
      device_label: "RECEPTION-1",
      raised_at: now - 6000,
      delivered: 12,
      coming: ["Dr Ali (Room 2)", "Nurse B Example (Treatment Room 1)"],
    }),
  );
}

const peers: Peer[] = [
  ["Reception", "Jane Smith", "lan"],
  ["Room 2", "Dr Ali", "lan"],
  ["Treatment Room 1", "Nurse B Example", "relay"],
  ["Admin office", "Practice Manager", "lan"],
].map(([room, user, path], i) => ({
  device: String(i).repeat(64),
  device_short: String(i).repeat(8),
  member: true,
  label: `PC-${i + 1}`,
  room,
  user,
  path: path as Peer["path"],
  last_heard_ms: 1200 + i * 700,
  app_version: "0.1.0",
  audio_ok: true,
}));

let hold: ReturnType<typeof setInterval> | undefined;
let holdStart = 0;
let practice = false;

function endHold(triggered: boolean) {
  clearInterval(hold);
  hold = undefined;
  event({ type: "hold_ended" });
  if (triggered && !practice) {
    const own = alarm({ own: true, room: "Reception", user: "Jane Smith", raised_at: Date.now(), delivered: 0 });
    alarms.unshift(own);
    event({ type: "sender_panel", alarm: own, phase: "requested" });
    setTimeout(() => {
      own.delivered = 13;
      event({ type: "sender_panel", alarm: { ...own }, phase: "delivered" });
    }, 600);
    setTimeout(() => {
      own.coming = ["Dr Ali (Room 2)"];
      event({ type: "alarm_updated", alarm: { ...own } });
    }, 2500);
  }
}

export const mock = {
  listen(name: string, cb: Listener): () => void {
    if (!listeners.has(name)) listeners.set(name, new Set());
    listeners.get(name)!.add(cb);
    return () => listeners.get(name)?.delete(cb);
  },
  async invoke(cmd: string, args: Record<string, unknown>): Promise<unknown> {
    switch (cmd) {
      case "app_info":
        return {
          enrolled: scenario !== "firstrun",
          running: scenario !== "firstrun",
          version: "0.1.0",
          data_dir: "~/.local/share/muster",
          allow_quit: true,
          audio_ok: true,
          wayland: scenario === "amber",
          room_from_policy: false,
          token_configured: false,
          home_dir: "/home/user",
          sounds_dir: "~/.local/share/muster/sounds",
        };
      case "setup_practice":
        await new Promise((r) => setTimeout(r, 1500));
        status.enrolled = true;
        return {
          practice: args.name,
          practice_id: "00112233445566778899aabbccddeeff",
          root_pub: "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
          recovery_kit: args.kitPath,
        };
      case "join_with_token":
        status.enrolled = true;
        return null;
      case "create_token":
        return { token_id: "1234", expires_at: now + 86_400_000, text: "MUSTER1-EXAMPLE-TOKEN-TEXT" };
      case "agent_call":
        return mock.call(args.method as string, (args.params ?? {}) as Record<string, unknown>);
      default:
        return null;
    }
  },
  call(method: string, p: Record<string, unknown>): unknown {
    const find = () => alarms.find((a) => a.alarm_id === p.alarm_id);
    switch (method) {
      case "status":
        return { ...status };
      case "alarms":
        return alarms.map((a) => ({ ...a }));
      case "peers":
        return peers;
      case "config":
        return {
          label: "RECEPTION-1",
          room: "Reception",
          hold_ms: 1500,
          cue_mode: "soft",
          sound_pattern: "A",
          volume_pct: 80,
          relay_urls: [],
        };
      case "hold.start":
        if (hold) return { ok: true };
        practice = Boolean(p.practice);
        holdStart = Date.now();
        hold = setInterval(() => {
          const f = Math.min(1, (Date.now() - holdStart) / 1500);
          event({ type: "hold_progress", fraction: f });
          if (f >= 1) endHold(true);
        }, 33);
        return { ok: true };
      case "hold.end":
        if (hold) endHold(false);
        return { ok: true };
      case "respond":
      case "seen": {
        const a = find();
        if (a) {
          a.local_ack = method === "respond" ? "responding" : "seen";
          if (method === "respond") a.coming = ["You", ...a.coming];
          event({ type: "alarm_updated", alarm: { ...a } });
        }
        return { ok: true };
      }
      case "resolve":
      case "cancel": {
        const a = find();
        if (a) {
          a.state = method === "resolve" ? "resolved" : "cancelled";
          a.resolved_by = method === "resolve" ? "Jane Smith" : null;
          a.closed_at = Date.now();
          event({ type: "alarm_closed", alarm: { ...a } });
        }
        return { ok: true };
      }
      default:
        return { ok: true };
    }
  },
};
