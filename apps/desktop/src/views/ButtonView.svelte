<!--
SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

The floating button window: the button, the status shape, a badge with the
responder count while our alarm is open, and a warning if it could not be
delivered.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import HoldButton from "../lib/HoldButton.svelte";
  import StatusIcon from "../lib/StatusIcon.svelte";
  import type { Alarm, Level } from "../lib/types";

  let level = $state<Level | null>(null);
  let summary = $state("Starting");
  let running = $state(false);
  let own = $state<Alarm | null>(null);
  let undelivered = $state(false);
  let moveMode = $state(false);
  let moveTimer: ReturnType<typeof setTimeout> | undefined;

  const tooltip = $derived(
    running ? `Press and hold to call for help. ${summary}` : "Panic Button is starting or not set up yet",
  );

  async function refresh() {
    try {
      const s = await api.status();
      running = true;
      level = s.level;
      summary = s.summary ?? "Starting";
      const mine = (await api.alarms()).filter((a) => a.own && !a.drill && a.state === "open");
      own = mine.sort((a, b) => b.raised_at - a.raised_at)[0] ?? null;
    } catch {
      running = false;
    }
  }

  function track(a: Alarm) {
    if (!a.own || a.drill) return;
    own = a.state === "open" ? a : null;
    if (a.state !== "open") undelivered = false;
  }

  function stopMove() {
    moveMode = false;
    clearTimeout(moveTimer);
  }

  onMount(() => {
    refresh();
    const offs = [
      api.onStarted(refresh),
      api.onEvent((e) => {
        if (e.type === "status") {
          level = e.level;
          summary = e.summary;
          running = true;
        }
        if (e.type === "sender_panel") {
          track(e.alarm);
          undelivered = e.phase === "could_not_reach";
          if (e.phase === "delivered") undelivered = false;
        }
        if (e.type === "alarm_updated" || e.type === "alarm_closed") track(e.alarm);
      }),
      api.onMove(() => {
        moveMode = true;
        clearTimeout(moveTimer);
        moveTimer = setTimeout(stopMove, 30_000);
      }),
    ];
    return () => offs.forEach((o) => o.then((f) => f()));
  });
</script>

<main>
  <h1 class="visually-hidden">Panic Button</h1>
  <HoldButton disabled={!running} {moveMode} label={tooltip} onmoved={stopMove} />
  <span class="dot" aria-hidden="true"><StatusIcon {level} size={14} outline /></span>
  {#if own}
    <button class="badge" title="Show the help request" onclick={() => api.showWindow("panel")}>
      {#if undelivered}
        <span aria-label="Not delivered">!</span>
      {:else}
        <span aria-label="{own.coming.length} coming">{own.coming.length}</span>
      {/if}
    </button>
  {/if}
  <span class="visually-hidden" aria-live="polite">{summary}</span>
</main>

<style>
  main {
    position: relative;
    width: 76px;
    height: 76px;
    display: grid;
    place-items: center;
  }

  .dot {
    position: absolute;
    top: 9px;
    right: 9px;
    line-height: 0;
    pointer-events: none;
  }

  .badge {
    position: absolute;
    bottom: 4px;
    right: 4px;
    min-width: 22px;
    height: 22px;
    padding: 0 5px;
    border-radius: 11px;
    border: 2px solid var(--paper);
    background: var(--ink);
    color: var(--paper);
    font-size: 12px;
    font-weight: 700;
    line-height: 1;
    cursor: pointer;
  }
</style>
