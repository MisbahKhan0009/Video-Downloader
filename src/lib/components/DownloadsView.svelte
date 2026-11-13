<script lang="ts">
  import ArrowDownToLine from "@lucide/svelte/icons/arrow-down-to-line";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import DownloadRow from "./DownloadRow.svelte";
  import { app } from "$lib/store.svelte";
  import { api } from "$lib/api";
  import { speed } from "$lib/format";

  let tab = $state<"all" | "active" | "done">("all");

  const list = $derived(
    app.downloads.filter((d) =>
      tab === "all" ? true : tab === "done" ? d.status === "done" : d.status !== "done",
    ),
  );
  const counts = $derived({
    all: app.downloads.length,
    active: app.downloads.filter((d) => d.status !== "done").length,
    done: app.downloads.filter((d) => d.status === "done").length,
  });
  const hasPaused = $derived(app.downloads.some((d) => d.status === "paused" || d.status === "error"));
</script>

<header class="vhead">
  <div>
    <div class="eyebrow">Library</div>
    <h1>Downloads</h1>
    <div class="live mono">
      {#if app.active.length}
        <span class="pulse"></span>{app.active.length} active · {speed(app.totalSpeed) || "starting"}
      {:else}
        {counts.done} saved
      {/if}
    </div>
  </div>
  <div class="vacts">
    {#if app.active.length}
      <button class="btn sm" onclick={() => app.pauseAll()}>Pause all</button>
    {/if}
    {#if hasPaused}
      <button class="btn sm" onclick={() => app.resumeAll()}>Resume all</button>
    {/if}
    {#if counts.done}
      <button class="btn sm quiet" onclick={() => app.clearFinished()}>Clear finished</button>
    {/if}
    {#if !app.mobile && app.settings.dir}
      <button class="btn sm quiet" onclick={() => api.revealFile(app.settings.dir)}><FolderOpen size={14} />Folder</button>
    {/if}
  </div>
</header>

<div class="tabs" role="tablist">
  {#each [["all", "All"], ["active", "In progress"], ["done", "Finished"]] as [id, name] (id)}
    <button role="tab" aria-selected={tab === id} class:on={tab === id} onclick={() => (tab = id as typeof tab)}>
      {name}<span class="mono">{counts[id as keyof typeof counts]}</span>
    </button>
  {/each}
</div>

{#if list.length}
  <div class="list">
    {#each list as d (d.id)}
      <DownloadRow {d} />
    {/each}
  </div>
{:else}
  <div class="empty">
    <div class="e-icon"><ArrowDownToLine size={22} /></div>
    <h3>{tab === "done" ? "Nothing finished yet" : "No downloads yet"}</h3>
    <p>Grab a link and pick the videos you want. They'll show up here with live progress.</p>
    <button class="btn" onclick={() => (app.view = "grab")}>Grab a link</button>
  </div>
{/if}

<style>
  .vhead {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  h1 {
    margin: 6px 0 6px;
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.035em;
    line-height: 1.05;
  }
  .live {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 0 rgba(212, 255, 63, 0.6);
    animation: pulse 1.6s infinite;
  }
  @keyframes pulse {
    70% {
      box-shadow: 0 0 0 8px rgba(212, 255, 63, 0);
    }
    100% {
      box-shadow: 0 0 0 0 rgba(212, 255, 63, 0);
    }
  }
  .vacts {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: 16px;
    background: var(--glass);
    border: 1px solid var(--line);
    width: fit-content;
    max-width: 100%;
  }
  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border-radius: 12px;
    font-size: 13.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .tabs button span {
    font-size: 11px;
    color: var(--faint);
  }
  .tabs button.on {
    background: var(--glass-3);
    color: var(--text);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
  }
  .tabs button.on span {
    color: var(--accent);
  }
  .list {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 10px;
  }
  .empty {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 10px;
    padding: 60px 20px;
  }
  .e-icon {
    width: 52px;
    height: 52px;
    border-radius: 18px;
    display: grid;
    place-items: center;
    background: var(--glass-2);
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .empty h3 {
    margin: 4px 0 0;
    font-size: 17px;
    font-weight: 600;
  }
  .empty p {
    margin: 0 0 8px;
    max-width: 360px;
    color: var(--muted);
    font-size: 14px;
  }
  @media (max-width: 760px) {
    h1 {
      font-size: 28px;
    }
    .tabs {
      width: 100%;
    }
    .tabs button {
      flex: 1;
      justify-content: center;
      padding: 0 8px;
    }
  }
</style>
