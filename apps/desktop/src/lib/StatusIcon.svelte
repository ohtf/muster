<!--
SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

Status shape: green circle, amber triangle, red square. Shape carries the
meaning without colour (NFR-ACC-001).
-->
<script lang="ts">
  import type { Level } from "./types";

  let { level, size = 12, outline = false }: { level: Level | null; size?: number; outline?: boolean } = $props();
  const label = $derived(
    level === "green" ? "Connected" : level === "amber" ? "Limited" : level === "red" ? "Not connected" : "Starting",
  );
  const stroke = $derived(outline ? "#ffffff" : "none");
</script>

<svg width={size} height={size} viewBox="0 0 12 12" role="img" aria-label={label}>
  {#if level === "green"}
    <circle cx="6" cy="6" r="5" fill="var(--status-green)" {stroke} stroke-width="1.5" />
  {:else if level === "amber"}
    <path d="M6 1 L11.2 10.6 H0.8 Z" fill="var(--status-amber)" {stroke} stroke-width="1.3" stroke-linejoin="round" />
  {:else if level === "red"}
    <rect x="1.2" y="1.2" width="9.6" height="9.6" fill="var(--status-red)" {stroke} stroke-width="1.5" />
  {:else}
    <circle cx="6" cy="6" r="4.5" fill="none" stroke="var(--slate)" stroke-width="1.5" />
  {/if}
</svg>
