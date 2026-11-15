<!--
SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

The big red button. It only reports pointer and key events; the hold
timer, cue and trigger live in the core (HoldMachine), which drives the
ring through hold_progress events. Pointer movement never cancels a hold
(ADR 0009). Shift at pointerdown, or move mode, drags the window instead.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { api, startDragging } from "./api";
  import hand from "../assets/hand.svg";

  let {
    size = 56,
    practice = false,
    disabled = false,
    moveMode = false,
    label = "Press and hold to call for help",
    onmoved,
  }: {
    size?: number;
    practice?: boolean;
    disabled?: boolean;
    moveMode?: boolean;
    label?: string;
    onmoved?: () => void;
  } = $props();

  let fraction = $state(0);
  let holding = $state(false);
  let keyHeld = false;

  const ring = $derived(size / 2 + 6);
  const box = $derived(ring * 2 + 6);
  const circumference = $derived(2 * Math.PI * ring);

  onMount(() => {
    const off = api.onEvent((e) => {
      if (e.type === "hold_progress") fraction = e.fraction;
      if (e.type === "hold_ended") {
        fraction = 0;
        holding = false;
      }
    });
    const hidden = () => {
      if (document.visibilityState === "hidden") end();
    };
    document.addEventListener("visibilitychange", hidden);
    return () => {
      off.then((f) => f());
      document.removeEventListener("visibilitychange", hidden);
    };
  });

  function start() {
    if (holding || disabled) return;
    holding = true;
    api.holdStart(practice).catch(() => (holding = false));
  }

  function end() {
    if (!holding) return;
    holding = false;
    api.holdEnd().catch(() => {});
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    if (e.shiftKey || moveMode) {
      e.preventDefault();
      startDragging().then(() => onmoved?.());
      return;
    }
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    start();
  }

  function keydown(e: KeyboardEvent) {
    if ((e.key === " " || e.key === "Enter") && !e.repeat && !keyHeld) {
      e.preventDefault();
      keyHeld = true;
      start();
    }
  }

  function keyup(e: KeyboardEvent) {
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      keyHeld = false;
      end();
    }
  }
</script>

<button
  class="hold"
  class:holding
  class:move={moveMode}
  style:--size="{size}px"
  style:--box="{box}px"
  {disabled}
  aria-label={label}
  title={label}
  onpointerdown={down}
  onpointerup={end}
  onpointercancel={end}
  onlostpointercapture={end}
  onkeydown={keydown}
  onkeyup={keyup}
  onblur={() => {
    keyHeld = false;
    end();
  }}
  oncontextmenu={(e) => e.preventDefault()}
>
  <svg class="ring" width={box} height={box} viewBox="0 0 {box} {box}" aria-hidden="true">
    <circle cx={box / 2} cy={box / 2} r={ring} class="track" />
    <circle
      cx={box / 2}
      cy={box / 2}
      r={ring}
      class="progress"
      stroke-dasharray={circumference}
      stroke-dashoffset={circumference * (1 - fraction)}
      transform="rotate(-90 {box / 2} {box / 2})"
    />
  </svg>
  <span class="face">
    <img src={hand} alt="" width={size * 0.58} height={size * 0.58} draggable="false" />
  </span>
</button>

<style>
  .hold {
    position: relative;
    width: var(--box);
    height: var(--box);
    padding: 0;
    border: 0;
    background: transparent;
    border-radius: 50%;
    cursor: pointer;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }

  .face {
    position: absolute;
    inset: calc((var(--box) - var(--size)) / 2);
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: radial-gradient(circle at 50% 40%, #d8243f 0%, var(--alarm-red) 55%, var(--alarm-red-pressed) 100%);
    box-shadow:
      0 2px 0 var(--alarm-red-pressed),
      0 3px 8px rgb(0 0 0 / 0.35);
    transition: transform 80ms ease;
  }

  .holding .face {
    transform: translateY(2px) scale(0.97);
    background: var(--alarm-red-pressed);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.35);
  }

  .hold:disabled .face {
    background: #9ca3af;
    box-shadow: none;
  }

  .move .face {
    outline: 2px dashed var(--ink);
    outline-offset: 3px;
    cursor: move;
  }

  .ring {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .ring circle {
    fill: none;
    stroke-width: 5;
  }

  .track {
    stroke: transparent;
  }

  .holding .track {
    stroke: rgb(255 255 255 / 0.85);
  }

  .progress {
    stroke: var(--alarm-red-pressed);
    stroke-linecap: round;
  }

  img {
    pointer-events: none;
  }
</style>
