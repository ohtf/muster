<!--
SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

The receiver alert (`spec/05-desktop-agent.md`, Receiver alert window):
stacked cards, newest first. No default button: nothing is focused until
the person tabs or clicks (FR-RCV-002). A closed alarm shows its outcome
for 15 s; the window hides when no cards remain.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { api, clock, when } from "../lib/api";
  import type { Alarm } from "../lib/types";

  let cards = $state<Alarm[]>([]);
  let now = $state(Date.now());
  const timers = new Map<string, ReturnType<typeof setTimeout>>();

  function upsert(a: Alarm, add: boolean) {
    if (a.own || a.drill) return;
    const i = cards.findIndex((c) => c.alarm_id === a.alarm_id);
    if (i >= 0) cards[i] = a;
    else if (add) cards = [a, ...cards].sort((x, y) => y.raised_at - x.raised_at);
    else return;
    if (a.state !== "open" && !timers.has(a.alarm_id)) {
      timers.set(
        a.alarm_id,
        setTimeout(() => {
          cards = cards.filter((c) => c.alarm_id !== a.alarm_id);
          timers.delete(a.alarm_id);
          if (cards.length === 0) api.hideWindow("alert");
        }, 15_000),
      );
    }
  }

  async function act(a: Alarm, kind: "respond" | "seen") {
    if (kind === "respond") await api.respond(a.alarm_id);
    else await api.seen(a.alarm_id);
  }

  onMount(() => {
    api
      .alarms()
      .then((all) => all.filter((a) => a.state === "open").forEach((a) => upsert(a, true)))
      .catch(() => {});
    const off = api.onEvent((e) => {
      if (e.type === "alarm_shown") upsert(e.alarm, true);
      if (e.type === "alarm_updated" || e.type === "alarm_closed") upsert(e.alarm, false);
    });
    const tick = setInterval(() => (now = Date.now()), 15_000);
    return () => {
      off.then((f) => f());
      clearInterval(tick);
    };
  });
</script>

<main>
  {#each cards as a (a.alarm_id)}
    <article class:closed={a.state !== "open"} aria-labelledby="h-{a.alarm_id}">
      {#if a.state === "open"}
        <h1 id="h-{a.alarm_id}">HELP NEEDED</h1>
      {:else}
        <h1 id="h-{a.alarm_id}">
          {a.state === "resolved" ? "Resolved" : a.state === "cancelled" ? "Cancelled - sent in error" : "Ended"}
        </h1>
      {/if}
      <p class="room">{a.room || "Room not set"}</p>
      <p class="who">
        {a.user}{a.device_label && a.device_label !== a.user ? ` - ${a.device_label}` : ""} - {when(a.raised_at, now)}
      </p>
      {#if a.state === "resolved"}
        <p class="outcome">Resolved by {a.resolved_by ?? "someone"}{a.closed_at ? ` at ${clock(a.closed_at)}` : ""}</p>
      {/if}
      <p class="coming" aria-live="polite">
        {#if a.coming.length}<strong>Coming:</strong> {a.coming.join(", ")}{:else}Nobody has said they are coming yet.{/if}
      </p>
      {#if a.state === "open"}
        {#if a.local_ack === "responding"}
          <p class="ack">You're coming</p>
        {:else if a.local_ack === "seen"}
          <p class="ack">Seen</p>
        {:else}
          <div class="actions">
            <button class="act coming-btn" onclick={() => act(a, "respond")}>I'm coming</button>
            <button class="act" onclick={() => act(a, "seen")}>Seen - can't come</button>
          </div>
        {/if}
      {/if}
    </article>
  {:else}
    <p class="empty">No alarms.</p>
  {/each}
</main>

<style>
  main {
    min-height: 100vh;
    padding: 20px 24px;
    background: var(--alert-deep-red);
    color: var(--paper);
  }

  article {
    padding: 4px 0 20px;
  }

  article + article {
    border-top: 2px solid rgb(255 255 255 / 0.35);
    padding-top: 18px;
  }

  h1 {
    margin: 0 0 2px;
    font-size: 32px;
    font-weight: 800;
    letter-spacing: 0.02em;
  }

  .closed h1 {
    font-size: 26px;
  }

  p {
    margin: 0 0 6px;
  }

  .room {
    font-size: 28px;
    font-weight: 600;
  }

  .who,
  .coming,
  .outcome {
    font-size: 16px;
  }

  .coming {
    margin: 12px 0 16px;
  }

  .ack {
    font-size: 20px;
    font-weight: 700;
  }

  .actions {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
  }

  .act {
    min-height: 48px;
    padding: 0 1.4em;
    border-radius: 10px;
    border: 2px solid var(--paper);
    background: transparent;
    color: var(--paper);
    font-size: 17px;
    font-weight: 700;
    cursor: pointer;
  }

  .act:hover {
    background: rgb(255 255 255 / 0.12);
  }

  .coming-btn {
    background: var(--paper);
    color: var(--alert-deep-red);
  }

  .coming-btn:hover {
    background: #f5e6e9;
  }

  .act:focus-visible {
    outline: 3px solid #ffd166;
    outline-offset: 3px;
  }

  .empty {
    opacity: 0.8;
  }
</style>
