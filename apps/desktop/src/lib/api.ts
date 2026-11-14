// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

// The boundary to Rust. In Tauri this calls commands and listens to
// events; in a plain browser (vite dev, screenshots) it uses the mock agent
// in mock.ts so every view can be seen without a desktop session.

import type { AgentConfig, Alarm, AppInfo, MusterEvent, Peer, Status } from "./types";
import { mock } from "./mock";

export const inTauri = "__TAURI_INTERNALS__" in window;

type Unlisten = () => void;

async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!inTauri) return mock.invoke(cmd, args) as Promise<T>;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

async function listenTo<T>(name: string, cb: (payload: T) => void): Promise<Unlisten> {
  if (!inTauri) return mock.listen(name, cb as (p: unknown) => void);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(name, (e) => cb(e.payload));
}

export const call = <T>(method: string, params: Record<string, unknown> = {}) =>
  invoke<T>("agent_call", { method, params });

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  status: () => call<Status>("status"),
  alarms: () => call<Alarm[]>("alarms"),
  peers: () => call<Peer[]>("peers"),
  config: () => call<AgentConfig>("config"),
  holdStart: (practice = false, source = "button") => call("hold.start", { source, practice }),
  holdEnd: (source = "button") => call("hold.end", { source }),
  respond: (alarm_id: string) => call("respond", { alarm_id }),
  seen: (alarm_id: string) => call("seen", { alarm_id }),
  resolve: (alarm_id: string) => call("resolve", { alarm_id }),
  cancel: (alarm_id: string) => call("cancel", { alarm_id }),
  runDrill: () => call("drill.run"),
  revoke: (device: string, reason: string) => call("admin.revoke", { device, reason }),
  approveJoin: (device: string) => call("admin.approve_join", { device }),
  rejectJoin: (device: string) => call("admin.reject_join", { device }),
  setupPractice: (name: string, label: string, passphrase: string, kitPath: string) =>
    invoke<{ practice: string; practice_id: string; root_pub: string; recovery_kit: string }>(
      "setup_practice",
      { name, label, passphrase, kitPath },
    ),
  joinWithToken: (token: string) => invoke<void>("join_with_token", { token }),
  createToken: (days: number, label: string) =>
    invoke<{ token_id: string; expires_at: number; text: string }>("create_token", { days, label }),
  setRoom: (room: string) => invoke<void>("set_room", { room }),
  playTestSound: (pattern?: string) => invoke<void>("play_test_sound", { pattern }),
  showWindow: (label: string) => invoke<void>("show_window", { label }),
  hideWindow: (label: string) => invoke<void>("hide_window", { label }),
  quit: () => invoke<void>("quit"),
  onEvent: (cb: (e: MusterEvent) => void) => listenTo<MusterEvent>("muster", cb),
  onStarted: (cb: () => void) => listenTo<null>("muster-started", () => cb()),
  onNav: (cb: (tab: string) => void) => listenTo<string>("muster-nav", cb),
  onMove: (cb: () => void) => listenTo<null>("muster-move", () => cb()),
  onError: (cb: (msg: string) => void) => listenTo<string>("muster-error", cb),
};

/** Starts dragging this window (Shift+drag or "Move button"). */
export async function startDragging(): Promise<void> {
  if (!inTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startDragging();
}

/** "09:12 (just now)", "09:12 (4 min ago)". */
export function when(ms: number, now = Date.now()): string {
  const t = new Date(ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
  const mins = Math.floor((now - ms) / 60000);
  if (mins < 1) return `${t} (just now)`;
  if (mins < 60) return `${t} (${mins} min ago)`;
  return t;
}

export function clock(ms: number): string {
  return new Date(ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });
}
