<script lang="ts">
  import Download from "@lucide/svelte/icons/arrow-down-to-line";
  import Copy from "@lucide/svelte/icons/copy";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Thumb from "./Thumb.svelte";
  import { app } from "$lib/store.svelte";
  import { api, type VideoItem } from "$lib/api";
  import { bytes, eta, host, label, qualityLabel, speed, titleOf } from "$lib/format";

  let { video }: { video: VideoItem } = $props();
  let name = $state("");
  $effect(() => {
    name = titleOf(video);
  });

  const dl = $derived(app.statusOf(video.url));
  const pct = $derived.by(() => {
    if (!dl) return 0;
    if (dl.status === "done") return 100;
    if (dl.fraction != null) return dl.fraction * 100;
    return dl.total ? (dl.received / dl.total) * 100 : 0;
  });
</script>

<section class="single glass">
  <div class="media">
    <Thumb src={video.thumbnail} seed={video.url} label={label(video)} />
    <span class="badge" class:hls={video.kind === "hls"}>{label(video)}</span>
  </div>
  <div class="info">
    <div class="eyebrow">Direct {video.kind === "hls" ? "stream" : "video"} · {host(video.url)}</div>
    <label class="name">
      <span class="sr">File name</span>
      <input bind:value={name} spellcheck="false" disabled={!!dl && dl.status !== "error"} />
    </label>
    <dl class="facts">
      <div><dt>Format</dt><dd class="mono">{video.kind === "hls" ? "HLS stream" : label(video)}</dd></div>
      {#if video.quality}<div><dt>Quality</dt><dd class="mono">{qualityLabel(video.quality)}</dd></div>{/if}
      <div><dt>Size</dt><dd class="mono">{bytes(video.size ?? dl?.total ?? null) || "Unknown"}</dd></div>
      <div><dt>Save to</dt><dd class="mono path" title={app.settings.dir}>{app.settings.dir}</dd></div>
    </dl>

    {#if dl}
      <div class="prog">
        <div class="track"><i style="width:{pct}%" class:done={dl.status === "done"}></i></div>
        <div class="ptext mono">
          {#if dl.status === "done"}
            <span class="ok">Saved · {dl.fileName}</span>
          {:else if dl.status === "error"}
            <span class="err">{dl.error}</span>
          {:else if dl.note}
            <span class="warn">{dl.note}</span>
          {:else}
            <span>{bytes(dl.received)}{dl.total ? " / " + bytes(dl.total) : ""}</span>
            <span>{dl.status === "downloading" ? speed(dl.speed) : dl.status}</span>
            <span>{dl.status === "downloading" ? eta(dl.received, dl.total, dl.speed, dl.fraction) : ""}</span>
          {/if}
        </div>
      </div>
    {/if}

    <div class="row">
      {#if !dl || dl.status === "error"}
        <button class="btn primary" onclick={() => app.download(video, name)}>
          <Download size={17} strokeWidth={2.2} />Download
        </button>
      {:else if dl.status === "downloading" || dl.status === "queued"}
        <button class="btn" onclick={() => app.pause(dl.id)}><Pause size={16} />Pause</button>
      {:else if dl.status === "paused"}
        <button class="btn primary" onclick={() => app.resume(dl.id)}><Play size={16} />Resume</button>
      {:else if dl.status === "done" && dl.path && !app.mobile}
        <button class="btn primary" onclick={() => api.openFile(dl.path!)}><Play size={16} />Play</button>
        <button class="btn" onclick={() => api.revealFile(dl.path!)}><FolderOpen size={16} />Show in folder</button>
      {/if}
      <button class="btn quiet" onclick={() => app.copy(video.url)}><Copy size={15} />Copy link</button>
    </div>
  </div>
</section>

<style>
  .single {
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    gap: 28px;
    padding: 14px;
    border-radius: var(--r-xl);
    background: rgba(18, 20, 24, 0.55);
    animation: rise 0.45s var(--ease);
  }
  .media {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 18px;
    overflow: hidden;
  }
  .media .badge {
    position: absolute;
    top: 12px;
    right: 12px;
  }
  .info {
    padding: 10px 10px 10px 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .name input {
    width: 100%;
    border: 0;
    outline: 0;
    background: transparent;
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.025em;
    line-height: 1.2;
    padding: 4px 0;
    border-bottom: 1px dashed var(--line-2);
  }
  .name input:focus {
    border-bottom-color: var(--accent);
  }
  .name input:disabled {
    border-bottom-color: transparent;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  .facts {
    margin: 0;
    display: grid;
    gap: 8px;
  }
  .facts div {
    display: grid;
    grid-template-columns: 76px 1fr;
    gap: 12px;
    font-size: 13px;
  }
  dt {
    color: var(--faint);
  }
  dd {
    margin: 0;
    min-width: 0;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
  }
  .prog {
    display: grid;
    gap: 8px;
  }
  .track {
    height: 6px;
    border-radius: 6px;
    background: var(--glass-3);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    border-radius: 6px;
    background: var(--accent);
    box-shadow: 0 0 14px rgba(var(--accent-rgb), 0.6);
    transition: width 0.3s linear;
  }
  .track i.done {
    background: var(--ok);
    box-shadow: none;
  }
  .ptext {
    display: flex;
    gap: 14px;
    font-size: 12px;
    color: var(--muted);
  }
  .ok {
    color: var(--ok);
  }
  .err {
    color: var(--danger);
  }
  .warn {
    color: var(--warn);
  }
  .row {
    margin-top: auto;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  @media (max-width: 860px) {
    .single {
      grid-template-columns: 1fr;
      gap: 6px;
      padding: 10px;
    }
    .info {
      padding: 10px 6px 6px;
    }
    .name input {
      font-size: 20px;
    }
  }
</style>
