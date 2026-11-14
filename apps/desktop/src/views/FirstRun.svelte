<!--
SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

First run (`spec/05-desktop-agent.md`, First run): join with an enrolment
code or set up a new practice, choose the room, then a practice hold that
alerts nobody.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import HoldButton from "../lib/HoldButton.svelte";
  import type { AppInfo } from "../lib/types";

  let { info, ondone }: { info: AppInfo; ondone: () => void } = $props();

  type Step = "welcome" | "join" | "create" | "created" | "room" | "practice";
  let step = $state<Step>("welcome");
  let error = $state("");
  let busy = $state(false);

  let token = $state("");
  let name = $state("");
  let label = $state("");
  let pass = $state("");
  let pass2 = $state("");
  let kitPath = $state("");
  let created = $state<{ practice: string; practice_id: string; root_pub: string; recovery_kit: string } | null>(null);
  let room = $state("");
  let practiced = $state(false);
  let holdMs = $state(1500);

  onMount(() => {
    // Enrolled already (for example by a token in the config): skip to the room.
    if (info.enrolled || info.running) afterEnrol();
    kitPath = `${info.home_dir}/Muster recovery kit.muster-kit`;
    const off = api.onEvent((e) => {
      if (e.type === "hold_progress" && e.fraction >= 1 && step === "practice") practiced = true;
    });
    return () => off.then((f) => f());
  });

  async function afterEnrol() {
    try {
      const c = await api.config();
      room = c.room;
      holdMs = c.hold_ms;
    } catch {
      /* the agent may still be starting */
    }
    step = info.room_from_policy ? "practice" : "room";
  }

  async function join() {
    error = "";
    busy = true;
    try {
      await api.joinWithToken(token);
      await afterEnrol();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function create() {
    error = "";
    if (pass.length < 12) {
      error = "The passphrase needs at least 12 characters.";
      return;
    }
    if (pass !== pass2) {
      error = "The two passphrases are different.";
      return;
    }
    busy = true;
    try {
      created = await api.setupPractice(name, label || "Admin computer", pass, kitPath);
      pass = pass2 = "";
      step = "created";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function saveRoom() {
    error = "";
    try {
      if (room.trim()) await api.setRoom(room);
      step = "practice";
    } catch (e) {
      error = String(e);
    }
  }

  async function finish() {
    await api.showWindow("button");
    ondone();
  }
</script>

<section class="first-run">
  {#if step === "welcome"}
    <h1>Welcome to Muster</h1>
    <p class="lead">Muster lets you call colleagues for help from this computer.</p>
    <div class="choices">
      <button class="choice" onclick={() => (step = "join")}>
        <strong>Join an existing practice</strong>
        <span>You have an enrolment code from your Practice Admin or IT.</span>
      </button>
      <button class="choice" onclick={() => (step = "create")}>
        <strong>Set up a new practice</strong>
        <span>This computer becomes the first Practice Admin.</span>
      </button>
    </div>
  {:else if step === "join"}
    <h1>Join a practice</h1>
    <div class="field">
      <label for="token">Enrolment code</label>
      <textarea id="token" rows="4" bind:value={token} spellcheck="false" autocomplete="off"></textarea>
      <span class="hint">Paste the whole code. Treat it like a password.</span>
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="row">
      <button class="btn" onclick={() => (step = "welcome")}>Back</button>
      <button class="btn primary" disabled={busy || !token.trim()} onclick={join}>
        {busy ? "Joining..." : "Join"}
      </button>
    </div>
  {:else if step === "create"}
    <h1>Set up a new practice</h1>
    <div class="field">
      <label for="pname">Practice name</label>
      <input id="pname" bind:value={name} placeholder="Riverside Surgery" autocomplete="organization" />
    </div>
    <div class="field">
      <label for="plabel">Name for this computer</label>
      <input id="plabel" bind:value={label} placeholder="Practice Manager's PC" />
    </div>
    <div class="field">
      <label for="pass">Recovery passphrase</label>
      <input id="pass" type="password" bind:value={pass} autocomplete="new-password" />
      <span class="hint">At least 12 characters. With the recovery kit, it restores admin access if this computer is lost.</span>
    </div>
    <div class="field">
      <label for="pass2">Passphrase again</label>
      <input id="pass2" type="password" bind:value={pass2} autocomplete="new-password" />
    </div>
    <div class="field">
      <label for="kit">Save the recovery kit to</label>
      <input id="kit" bind:value={kitPath} spellcheck="false" />
      <span class="hint">Afterwards, copy it somewhere safe and offline, for example a USB stick in the safe.</span>
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="row">
      <button class="btn" onclick={() => (step = "welcome")} disabled={busy}>Back</button>
      <button class="btn primary" disabled={busy || !name.trim()} onclick={create}>
        {busy ? "Creating (this takes a few seconds)..." : "Create practice"}
      </button>
    </div>
  {:else if step === "created" && created}
    <h1>{created.practice} is ready</h1>
    <p>The recovery kit is saved at <code>{created.recovery_kit}</code>. Copy it somewhere safe and offline now.</p>
    <details>
      <summary>Relay registration details</summary>
      <p class="muted">Only needed if your practice uses a relay. Send these to the relay operator.</p>
      <dl>
        <dt>Practice id</dt>
        <dd><code>{created.practice_id}</code></dd>
        <dt>Root public key</dt>
        <dd><code>{created.root_pub}</code></dd>
      </dl>
    </details>
    <div class="row"><button class="btn primary" onclick={afterEnrol}>Next</button></div>
  {:else if step === "room"}
    <h1>Which room is this computer in?</h1>
    <div class="field">
      <label for="room">Room</label>
      <input id="room" bind:value={room} placeholder="Consulting Room 4" />
      <span class="hint">Shown to colleagues when you call for help, so they know where to go.</span>
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="row"><button class="btn primary" onclick={saveRoom}>Next</button></div>
  {:else if step === "practice"}
    <h1>Try it now</h1>
    <p>
      Press and hold the button for {(holdMs / 1000).toFixed(1)} seconds. This is a practice and will not alert
      anyone.
    </p>
    <div class="practice">
      <HoldButton size={96} practice label="Practice: press and hold" />
      <p aria-live="polite" class:ok={practiced}>
        {practiced ? "That's it. Holding for the full time is what sends a real call for help." : ""}
      </p>
    </div>
    <div class="row">
      <button class="btn" onclick={finish}>Skip</button>
      <button class="btn primary" disabled={!practiced} onclick={finish}>Finish</button>
    </div>
  {/if}
</section>

<style>
  .first-run {
    max-width: 560px;
    margin: 0 auto;
    padding: 32px 24px;
  }

  h1 {
    font-size: 26px;
    margin: 0 0 12px;
  }

  .lead {
    font-size: 17px;
    margin-bottom: 24px;
  }

  .choices {
    display: grid;
    gap: 12px;
  }

  .choice {
    display: grid;
    gap: 4px;
    text-align: left;
    padding: 16px 18px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--paper);
    cursor: pointer;
  }

  .choice:hover {
    border-color: var(--action-blue);
  }

  .choice strong {
    font-size: 17px;
    color: var(--action-blue);
  }

  .choice span {
    color: var(--slate);
  }

  .row {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 16px;
  }

  textarea {
    font-family: ui-monospace, monospace;
    font-size: 13px;
  }

  code {
    word-break: break-all;
    font-size: 13px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 12px;
  }

  dd {
    margin: 0;
  }

  .practice {
    display: grid;
    justify-items: center;
    gap: 12px;
    padding: 24px 0;
  }

  .practice p {
    min-height: 2.9em;
    text-align: center;
    max-width: 420px;
  }

  .ok {
    color: var(--status-green);
    font-weight: 600;
  }
</style>
