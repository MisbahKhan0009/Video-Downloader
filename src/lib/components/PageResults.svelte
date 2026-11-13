<script lang="ts">
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Layers from "@lucide/svelte/icons/layers";
  import Radar from "@lucide/svelte/icons/radar";
  import Download from "@lucide/svelte/icons/arrow-down-to-line";
  import Search from "@lucide/svelte/icons/search";
  import Square from "@lucide/svelte/icons/square";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Check from "@lucide/svelte/icons/check";
  import VideoCard from "./VideoCard.svelte";
  import { app } from "$lib/store.svelte";
  import { api } from "$lib/api";
  import { host, titleOf } from "$lib/format";

  const page = $derived(app.page!);
  let filter = $state("");

  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return app.videos;
    return app.videos.filter((v) => titleOf(v).toLowerCase().includes(q) || v.url.toLowerCase().includes(q));
  });

  /** 1 … 4 5 [6] 7 8 … 40 */
  const pageList = $derived.by(() => {
    const pages = page.pages;
    if (pages.length <= 9) return pages.map((p) => ({ ...p, gap: false }));
    const cur = page.currentPage ?? 1;
    const first = pages[0];
    const last = pages[pages.length - 1];
    const near = pages.filter((p) => Math.abs(p.number - cur) <= 2 && p !== first && p !== last);
    const out: { number: number; url: string; gap: boolean }[] = [{ ...first, gap: false }];
    if (near.length && near[0].number > first.number + 1) out.push({ number: -1, url: "", gap: true });
    near.forEach((p) => out.push({ ...p, gap: false }));
    if (near.length && near[near.length - 1].number < last.number - 1) out.push({ number: -2, url: "", gap: true });
    out.push({ ...last, gap: false });
    return out;
  });

  const eyebrow = $derived(
    [
      host(page.url),
      page.currentPage ? `Page ${page.currentPage}` : null,
      app.scannedPages.length > 1 ? `${app.scannedPages.length} pages collected` : null,
    ]
      .filter(Boolean)
      .join("  ·  "),
  );
  const allSelected = $derived(app.videos.length > 0 && app.selected.size === app.videos.length);
  const working = $derived(!!app.deep || !!app.batch || !!app.loadingPage);
</script>

<section class="head">
  <div class="title-block">
    <div class="eyebrow">{eyebrow}</div>
    <h2>{page.title ?? host(page.url)}</h2>
    <div class="stats">
      <span><b class="mono">{app.videos.length}</b> {app.videos.length === 1 ? "video" : "videos"}</span>
      {#if page.detailLinks.length}<span><b class="mono">{page.detailLinks.length}</b> linked pages</span>{/if}
      <button class="src" onclick={() => api.openUrl(page.url)}>Open page <ExternalLink size={12} /></button>
    </div>
  </div>
  <div class="head-acts">
    {#if page.detailLinks.length && !app.deep}
      <button class="btn" onclick={() => app.scanLinkedPages()} title="Open each linked page and look for videos inside">
        <Radar size={16} />Scan linked pages
      </button>
    {/if}
    <button class="btn primary" disabled={!app.videos.length} onclick={() => app.downloadAll()}>
      <Download size={16} strokeWidth={2.2} />Download all
    </button>
  </div>
</section>

{#if working}
  <div class="working glass">
    <LoaderCircle size={16} class="spin" />
    <div class="w-text">
      {#if app.loadingPage}
        Loading page…
      {:else if app.deep}
        Opening linked pages <b class="mono">{app.deep.done}/{app.deep.total}</b>
        <span class="dim">· {app.deep.found} found</span>
      {:else if app.batch}
        Collecting pages <b class="mono">{app.batch.done}/{app.batch.target}</b>
      {/if}
    </div>
    {#if app.deep}
      <div class="w-track"><i style="width:{app.deep.total ? (app.deep.done / app.deep.total) * 100 : 0}%"></i></div>
    {/if}
    {#if app.deep || app.batch}
      <button class="btn sm quiet" onclick={() => app.stopAll()}><Square size={12} fill="currentColor" />Stop</button>
    {/if}
  </div>
{/if}

{#if page.prevPage || page.nextPage || page.pages.length > 1}
  <nav class="pager" aria-label="Pagination">
    <div class="nums">
      <button class="icon-btn" disabled={!page.prevPage || working} onclick={() => app.goToPage(page.prevPage!)} title="Previous page">
        <ChevronLeft size={18} />
      </button>
      {#each pageList as p (p.number)}
        {#if p.gap}
          <span class="gap">…</span>
        {:else}
          <button
            class="num mono"
            class:cur={p.number === page.currentPage}
            disabled={working}
            onclick={() => p.number !== page.currentPage && app.goToPage(p.url)}
          >{p.number}</button>
        {/if}
      {/each}
      <button class="icon-btn" disabled={!page.nextPage || working} onclick={() => app.goToPage(page.nextPage!)} title="Next page">
        <ChevronRight size={18} />
      </button>
    </div>
    {#if page.nextPage}
      <div class="more">
        <span class="more-label">Collect next</span>
        <div class="seg">
          {#each [3, 5, 10] as n (n)}
            <button class:on={app.settings.pagesPerBatch === n} onclick={() => { app.settings.pagesPerBatch = n; app.saveSettings(); }}>{n}</button>
          {/each}
        </div>
        <button class="btn sm" disabled={working} onclick={() => app.scanMore()}>
          <Layers size={14} />Collect
        </button>
      </div>
    {/if}
  </nav>
{/if}

{#if app.videos.length}
  <div class="toolbar">
    <button class="selall" onclick={() => app.selectAll()} aria-pressed={allSelected}>
      <span class="box" class:on={allSelected} class:part={app.selected.size > 0 && !allSelected}>
        {#if allSelected}<Check size={12} strokeWidth={3} />{/if}
      </span>
      {app.selected.size ? `${app.selected.size} selected` : "Select all"}
    </button>
    <label class="filter">
      <Search size={15} />
      <input bind:value={filter} placeholder="Filter" spellcheck="false" />
    </label>
    {#if app.selected.size}
      <button class="btn sm primary" onclick={() => app.downloadSelected()}>
        <Download size={14} strokeWidth={2.4} />Download {app.selected.size}
      </button>
    {/if}
  </div>

  <div class="grid">
    {#each shown as v, i (v.url)}
      <VideoCard video={v} index={i} />
    {/each}
  </div>
{:else if !working}
  <div class="empty glass">
    <div class="e-icon"><Radar size={22} /></div>
    <h3>No videos on this page yet</h3>
    <p>
      {#if page.detailLinks.length}
        It links to {page.detailLinks.length} pages that may hold videos. Scan them to look inside each one.
      {:else}
        Nothing playable was found in the page source. The site may load videos with scripts after the page opens, or protect them.
      {/if}
    </p>
    {#if page.detailLinks.length}
      <button class="btn primary" onclick={() => app.scanLinkedPages()}><Radar size={16} />Scan {page.detailLinks.length} linked pages</button>
    {/if}
  </div>
{/if}

{#if app.selected.size}
  <div class="dock">
    <button class="btn primary" onclick={() => app.downloadSelected()}>
      <Download size={16} strokeWidth={2.4} />Download {app.selected.size} selected
    </button>
  </div>
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 20px;
    animation: rise 0.4s var(--ease);
  }
  .title-block {
    min-width: 0;
  }
  h2 {
    margin: 6px 0 8px;
    font-size: 28px;
    font-weight: 600;
    letter-spacing: -0.03em;
    line-height: 1.15;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .stats {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 16px;
    font-size: 13px;
    color: var(--muted);
  }
  .stats b {
    color: var(--text);
    font-weight: 500;
  }
  .src {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--faint);
    font-size: 12.5px;
  }
  .src:hover {
    color: var(--text);
  }
  .head-acts {
    display: flex;
    gap: 8px;
    flex: none;
  }

  .working {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 10px 10px 16px;
    border-radius: 16px;
    font-size: 13.5px;
    color: var(--accent);
    animation: rise 0.3s var(--ease);
  }
  .w-text {
    color: var(--text);
    white-space: nowrap;
  }
  .w-text .dim {
    color: var(--muted);
  }
  .w-track {
    flex: 1;
    height: 4px;
    border-radius: 4px;
    background: var(--glass-3);
    overflow: hidden;
  }
  .w-track i {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.3s var(--ease);
  }
  .working .btn {
    margin-left: auto;
  }

  .pager {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px;
  }
  .nums {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px;
    border-radius: 16px;
    background: var(--glass);
    border: 1px solid var(--line);
    overflow-x: auto;
    max-width: 100%;
  }
  .num {
    min-width: 36px;
    height: 36px;
    padding: 0 8px;
    border-radius: 12px;
    font-size: 13px;
    color: var(--muted);
    transition:
      background 0.16s,
      color 0.16s;
  }
  .num:hover:not(:disabled) {
    background: var(--glass-2);
    color: var(--text);
  }
  .num.cur {
    background: var(--text);
    color: var(--bg);
    font-weight: 600;
  }
  .num:disabled:not(.cur) {
    opacity: 0.5;
  }
  .gap {
    color: var(--faint);
    padding: 0 4px;
  }
  .more {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .more-label {
    font-size: 13px;
    color: var(--muted);
  }
  .seg {
    display: flex;
    padding: 3px;
    border-radius: 12px;
    background: var(--glass);
    border: 1px solid var(--line);
  }
  .seg button {
    min-width: 32px;
    height: 26px;
    border-radius: 9px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--glass-3);
    color: var(--text);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    position: sticky;
    top: 10px;
    z-index: 5;
    padding: 6px 6px 6px 14px;
    border-radius: 18px;
    background: rgba(16, 18, 21, 0.82);
    border: 1px solid var(--line);
    box-shadow: var(--edge), 0 16px 32px -20px rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
  }
  .selall {
    white-space: nowrap;
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: 13.5px;
    color: var(--muted);
    height: 36px;
  }
  .selall:hover {
    color: var(--text);
  }
  .box {
    width: 18px;
    height: 18px;
    border-radius: 6px;
    border: 1.5px solid var(--line-2);
    display: grid;
    place-items: center;
  }
  .box.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
  }
  .box.part {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 3px var(--bg), inset 0 0 0 10px var(--accent);
  }
  .filter {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--line);
    color: var(--faint);
    width: 220px;
  }
  .filter:focus-within {
    border-color: var(--line-2);
    color: var(--muted);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    font-size: 13.5px;
  }
  .filter input::placeholder {
    color: var(--faint);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(232px, 1fr));
    gap: 16px;
  }

  .empty {
    border-radius: var(--r-xl);
    padding: 44px 24px;
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 10px;
    animation: rise 0.4s var(--ease);
  }
  .e-icon {
    width: 48px;
    height: 48px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    background: var(--glass-2);
    color: var(--muted);
    margin-bottom: 4px;
  }
  .empty h3 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: -0.015em;
  }
  .empty p {
    margin: 0 0 8px;
    max-width: 440px;
    color: var(--muted);
    font-size: 14px;
  }

  .dock {
    position: fixed;
    left: 16px;
    right: 16px;
    bottom: calc(88px + var(--safe-bottom));
    z-index: 15;
    display: none;
    animation: rise 0.25s var(--ease);
  }
  .dock .btn {
    height: 48px;
    border-radius: 16px;
  }

  @media (max-width: 760px) {
    .head {
      flex-direction: column;
      align-items: stretch;
    }
    h2 {
      font-size: 22px;
    }
    .head-acts {
      display: grid;
      grid-template-columns: 1fr 1fr;
    }
    .head-acts .btn:only-child {
      grid-column: 1 / -1;
    }
    .grid {
      grid-template-columns: repeat(auto-fill, minmax(158px, 1fr));
      gap: 10px;
    }
    .filter {
      width: auto;
      flex: 1;
    }
    .toolbar .btn {
      display: none;
    }
    .dock {
      display: grid;
    }
    .more-label {
      display: none;
    }
    .pager {
      gap: 8px;
    }
  }
  @media (max-width: 400px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
