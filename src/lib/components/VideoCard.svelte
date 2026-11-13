<script lang="ts">
  import Download from "@lucide/svelte/icons/arrow-down-to-line";
  import Check from "@lucide/svelte/icons/check";
  import Copy from "@lucide/svelte/icons/copy";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Thumb from "./Thumb.svelte";
  import { app } from "$lib/store.svelte";
  import { api, type VideoItem } from "$lib/api";
  import { bytes, host, label, qualityLabel, titleOf } from "$lib/format";

  let { video, index = 0 }: { video: VideoItem; index?: number } = $props();

  const sel = $derived(app.selected.has(video.url));
  const dl = $derived(app.statusOf(video.url));
  const pct = $derived.by(() => {
    if (!dl) return 0;
    if (dl.status === "done") return 100;
    if (dl.fraction != null) return dl.fraction * 100;
    if (dl.total) return (dl.received / dl.total) * 100;
    return 0;
  });
  const size = $derived(bytes(video.size ?? dl?.total ?? null));

  function act() {
    if (!dl || dl.status === "error") app.download(video);
    else if (dl.status === "downloading" || dl.status === "queued") app.pause(dl.id);
    else if (dl.status === "paused") app.resume(dl.id);
    else if (dl.status === "done" && dl.path && !app.mobile) api.revealFile(dl.path).catch(() => {});
  }
</script>

<article class="card" class:sel style="animation-delay:{Math.min(index, 12) * 28}ms">
  <button class="media" onclick={() => app.toggle(video.url)} aria-pressed={sel} aria-label="Select {titleOf(video)}">
    <Thumb src={video.thumbnail} seed={video.url} label={label(video)} />
    <span class="shade"></span>
    <span class="check" aria-hidden="true"><Check size={13} strokeWidth={3} /></span>
    <span class="badge" class:hls={video.kind === "hls"}>{label(video)}{#if video.quality}<b class="q">{qualityLabel(video.quality)}</b>{/if}</span>
    {#if size}<span class="size mono">{size}</span>{/if}
    {#if dl && dl.status !== "canceled"}
      <span class="bar" class:done={dl.status === "done"} class:err={dl.status === "error"}><i style="width:{pct}%"></i></span>
    {/if}
  </button>
  <div class="body">
    <h3 title={titleOf(video)}>{titleOf(video)}</h3>
    <div class="meta">
      <span>{host(video.pageUrl ?? video.url)}</span>
      {#if dl?.note}
        <span class="wait">Waiting</span>
      {:else if dl?.status === "downloading"}
        <span class="live mono">{Math.round(pct)}%</span>
      {:else if dl?.status === "done"}
        <span class="ok">Saved</span>
      {:else if dl?.status === "paused"}
        <span>Paused</span>
      {:else if dl?.status === "queued"}
        <span>Queued</span>
      {/if}
    </div>
  </div>
  <div class="acts">
    <button class="icon-btn" title="Copy link" onclick={() => app.copy(video.url)}><Copy size={16} /></button>
    <button
      class="icon-btn"
      class:get={!dl || dl.status === "error"}
      title={!dl ? "Download" : dl.status === "downloading" ? "Pause" : dl.status === "paused" ? "Resume" : dl.status === "error" ? "Retry" : "Show file"}
      onclick={act}
    >
      {#if !dl}
        <Download size={17} strokeWidth={2} />
      {:else if dl.status === "downloading" || dl.status === "queued"}
        <Pause size={16} />
      {:else if dl.status === "paused"}
        <Play size={16} />
      {:else if dl.status === "error"}
        <RotateCw size={16} />
      {:else}
        <Check size={17} color="var(--ok)" />
      {/if}
    </button>
  </div>
</article>

<style>
  .card {
    position: relative;
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto 1fr;
    border-radius: var(--r-lg);
    background: rgba(20, 22, 26, 0.55);
    border: 1px solid var(--line);
    box-shadow: var(--edge);
    overflow: hidden;
    animation: rise 0.45s var(--ease) both;
    transition:
      border-color 0.2s var(--ease),
      transform 0.25s var(--ease),
      background 0.2s;
  }
  .card:hover {
    border-color: var(--line-2);
    background: rgba(24, 26, 31, 0.7);
  }
  .card.sel {
    border-color: rgba(var(--accent-rgb), 0.55);
    box-shadow:
      0 0 0 1px rgba(var(--accent-rgb), 0.25),
      0 18px 40px -22px rgba(var(--accent-rgb), 0.4);
  }
  .media {
    grid-column: 1 / -1;
    position: relative;
    aspect-ratio: 16 / 9;
    display: block;
    width: 100%;
    overflow: hidden;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.35) 0%, transparent 35%, transparent 65%, rgba(0, 0, 0, 0.45) 100%);
    pointer-events: none;
  }
  .check {
    position: absolute;
    top: 10px;
    left: 10px;
    width: 22px;
    height: 22px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    border: 1.5px solid rgba(255, 255, 255, 0.55);
    background: rgba(10, 11, 13, 0.35);
    color: transparent;
    backdrop-filter: blur(6px);
    opacity: 0;
    transform: scale(0.85);
    transition:
      opacity 0.18s,
      transform 0.18s var(--ease),
      background 0.18s;
  }
  .card:hover .check,
  .sel .check {
    opacity: 1;
    transform: none;
  }
  .sel .check {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
  }
  .badge {
    position: absolute;
    top: 10px;
    right: 10px;
  }
  .size {
    position: absolute;
    right: 10px;
    bottom: 10px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.85);
    text-shadow: 0 1px 6px rgba(0, 0, 0, 0.8);
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: rgba(255, 255, 255, 0.12);
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--accent);
    box-shadow: 0 0 10px var(--accent);
    transition: width 0.3s linear;
  }
  .bar.done i {
    background: var(--ok);
    box-shadow: none;
  }
  .bar.err i {
    background: var(--danger);
  }
  .body {
    padding: 12px 4px 14px 14px;
    min-width: 0;
  }
  h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 500;
    line-height: 1.35;
    letter-spacing: -0.01em;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
  .meta {
    margin-top: 5px;
    display: flex;
    gap: 8px;
    font-size: 12px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
  }
  .meta span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .live {
    color: var(--accent);
  }
  .wait {
    color: var(--warn);
  }
  .q {
    margin-left: 6px;
    padding-left: 6px;
    border-left: 1px solid var(--line-2);
    font-weight: 600;
    color: var(--accent);
  }
  .ok {
    color: var(--ok);
  }
  .get {
    color: var(--accent);
    background: var(--accent-soft);
  }
  .get:hover:not(:disabled) {
    color: var(--accent-ink);
    background: var(--accent);
  }
  .acts {
    display: flex;
    align-items: flex-start;
    gap: 2px;
    padding: 9px 9px 0 0;
  }
</style>
