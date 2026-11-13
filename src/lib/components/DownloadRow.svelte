<script lang="ts">
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import X from "@lucide/svelte/icons/x";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Thumb from "./Thumb.svelte";
  import { app, type Download } from "$lib/store.svelte";
  import { api } from "$lib/api";
  import { bytes, eta, host, speed } from "$lib/format";

  let { d }: { d: Download } = $props();

  const pct = $derived.by(() => {
    if (d.status === "done") return 100;
    if (d.fraction != null) return d.fraction * 100;
    return d.total ? (d.received / d.total) * 100 : 0;
  });
  const indeterminate = $derived(d.status === "downloading" && d.fraction == null && !d.total);
</script>

<div class="row" data-status={d.status}>
  <div class="mini">
    <Thumb src={d.thumbnail} seed={d.url} compact />
    {#if d.status === "done"}<span class="done-dot"></span>{/if}
  </div>
  <div class="info">
    <div class="t" title={d.fileName ?? d.title}>{d.fileName ?? d.title}</div>
    <div class="sub mono">
      {#if d.status === "downloading"}
        <span class="hot">{d.fraction != null ? Math.round(d.fraction * 100) + "%" : d.total ? Math.round(pct) + "%" : ""}</span>
        <span>{bytes(d.received)}{d.total ? " of " + bytes(d.total) : ""}</span>
        <span>{speed(d.speed)}</span>
        <span class="hide-sm">{eta(d.received, d.total, d.speed, d.fraction)}</span>
      {:else if d.status === "queued"}
        <span>Waiting for a free slot</span>
      {:else if d.status === "paused"}
        <span>Paused{d.fraction != null ? ` at ${Math.round(d.fraction * 100)}%` : d.total ? ` at ${Math.round(pct)}%` : ""} · {bytes(d.received)}{d.total ? " of " + bytes(d.total) : ""}</span>
      {:else if d.status === "done"}
        <span class="ok">{bytes(d.total ?? d.received)}</span><span>{host(d.referer ?? d.url)}</span>
      {:else if d.status === "error"}
        <span class="err">{d.error ?? "Failed"}</span>
      {/if}
    </div>
    {#if d.status !== "done"}
      <div class="track" class:ind={indeterminate}><i style="width:{indeterminate ? 30 : pct}%"></i></div>
    {/if}
  </div>
  <div class="acts">
    {#if d.status === "downloading" || d.status === "queued"}
      <button class="icon-btn" title="Pause" onclick={() => app.pause(d.id)}><Pause size={16} /></button>
    {:else if d.status === "paused"}
      <button class="icon-btn" title="Resume" onclick={() => app.resume(d.id)}><Play size={16} /></button>
    {:else if d.status === "error"}
      <button class="icon-btn" title="Retry" onclick={() => app.resume(d.id)}><RotateCw size={16} /></button>
    {:else if d.status === "done" && d.path && !app.mobile}
      <button class="icon-btn" title="Play" onclick={() => api.openFile(d.path!)}><Play size={16} /></button>
      <button class="icon-btn" title="Show in folder" onclick={() => api.revealFile(d.path!)}><FolderOpen size={16} /></button>
    {/if}
    <button
      class="icon-btn danger"
      title={d.status === "done" ? "Remove from list" : "Cancel and delete partial file"}
      onclick={() => app.remove(d.id)}
    >
      <X size={16} />
    </button>
  </div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 88px 1fr auto;
    align-items: center;
    gap: 16px;
    padding: 10px 10px 10px 10px;
    border-radius: 18px;
    background: rgba(20, 22, 26, 0.5);
    border: 1px solid var(--line);
    box-shadow: var(--edge);
    animation: rise 0.35s var(--ease) both;
  }
  .mini {
    position: relative;
    width: 88px;
    aspect-ratio: 16 / 10;
    border-radius: 11px;
    overflow: hidden;
  }
  .done-dot {
    position: absolute;
    right: 6px;
    bottom: 6px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.5);
  }
  .info {
    min-width: 0;
    display: grid;
    gap: 6px;
  }
  .t {
    font-size: 14px;
    font-weight: 500;
    letter-spacing: -0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    display: flex;
    gap: 12px;
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
  }
  .hot {
    color: var(--accent);
  }
  .ok {
    color: var(--ok);
  }
  .err {
    color: var(--danger);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track {
    height: 4px;
    border-radius: 4px;
    background: var(--glass-3);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    border-radius: 4px;
    background: var(--accent);
    box-shadow: 0 0 10px rgba(212, 255, 63, 0.55);
    transition: width 0.3s linear;
  }
  [data-status="paused"] .track i,
  [data-status="queued"] .track i {
    background: var(--muted);
    box-shadow: none;
  }
  [data-status="error"] .track i {
    background: var(--danger);
    box-shadow: none;
  }
  .track.ind i {
    animation: slide 1.3s var(--ease) infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .acts {
    display: flex;
    gap: 2px;
  }

  @media (max-width: 760px) {
    .row {
      grid-template-columns: 64px 1fr auto;
      gap: 12px;
      padding: 8px;
    }
    .mini {
      width: 64px;
    }
    .hide-sm {
      display: none;
    }
  }
</style>
