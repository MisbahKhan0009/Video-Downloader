<script lang="ts">
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Code from "@lucide/svelte/icons/code-xml";
  import Toggle from "./Toggle.svelte";
  import Logo from "./Logo.svelte";
  import { app } from "$lib/store.svelte";
  import { api } from "$lib/api";

  async function pickFolder() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const dir = await open({ directory: true, defaultPath: app.settings.dir || undefined });
      if (typeof dir === "string") {
        app.settings.dir = dir;
        app.saveSettings();
      }
    } catch (e) {
      app.toast(String(e), "error");
    }
  }

  async function resetFolder() {
    app.settings.dir = await api.defaultDir();
    app.saveSettings();
  }
</script>

<header>
  <div class="eyebrow">Preferences</div>
  <h1>Settings</h1>
</header>

<section class="group glass">
  <div class="g-title">Save location</div>
  <div class="path mono">{app.settings.dir || "Not set"}</div>
  <div class="row-btns">
    {#if !app.mobile}
      <button class="btn sm" onclick={pickFolder}><FolderOpen size={14} />Change folder</button>
      <button class="btn sm quiet" onclick={() => api.revealFile(app.settings.dir)}>Open</button>
    {/if}
    <button class="btn sm quiet" onclick={resetFolder}><RotateCcw size={13} />Default</button>
  </div>
</section>

<section class="group glass">
  <div class="g-title">Behaviour</div>
  <div class="opt">
    <div>
      <div class="o-name">Grab on paste</div>
      <div class="o-desc">Start scanning as soon as a link is pasted or picked from the clipboard.</div>
    </div>
    <Toggle label="Grab on paste" bind:checked={app.settings.autoGrab} onchange={() => app.saveSettings()} />
  </div>
  <div class="opt">
    <div>
      <div class="o-name">Look inside linked pages</div>
      <div class="o-desc">When a listing page has no videos itself, open each card it links to and collect the videos there.</div>
    </div>
    <Toggle label="Look inside linked pages" bind:checked={app.settings.autoDeep} onchange={() => app.saveSettings()} />
  </div>
  <div class="opt">
    <div>
      <div class="o-name">Pages per collect</div>
      <div class="o-desc">How many "next" pages to follow each time you press Collect.</div>
    </div>
    <div class="seg">
      {#each [3, 5, 10, 20] as n (n)}
        <button class:on={app.settings.pagesPerBatch === n} onclick={() => { app.settings.pagesPerBatch = n; app.saveSettings(); }}>{n}</button>
      {/each}
    </div>
  </div>
</section>

<section class="group glass about">
  <Logo size={40} />
  <div>
    <div class="o-name">Vidra <span class="mono ver">0.1.0</span></div>
    <div class="o-desc">Downloads up to 3 videos at once. Partial downloads resume where they stopped, even after a restart. Only download videos you have the right to save.</div>
  </div>
  <button class="icon-btn" title="Source on GitHub" onclick={() => api.openUrl("https://github.com/MisbahKhan0009/Video-Downloader")}>
    <Code size={18} />
  </button>
</section>

<style>
  h1 {
    margin: 6px 0 0;
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.035em;
  }
  .group {
    border-radius: var(--r-lg);
    padding: 18px 20px;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
    max-width: 760px;
  }
  .g-title {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--muted);
  }
  .path {
    font-size: 13px;
    padding: 12px 14px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--line);
    word-break: break-all;
  }
  .row-btns {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .opt {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 6px 0;
  }
  .opt + .opt {
    border-top: 1px solid var(--line);
    padding-top: 14px;
  }
  .o-name {
    font-size: 14.5px;
    font-weight: 500;
  }
  .o-desc {
    margin-top: 2px;
    font-size: 13px;
    color: var(--muted);
    max-width: 480px;
  }
  .seg {
    display: flex;
    padding: 3px;
    border-radius: 12px;
    background: var(--glass);
    border: 1px solid var(--line);
    flex: none;
  }
  .seg button {
    min-width: 36px;
    height: 28px;
    border-radius: 9px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--glass-3);
    color: var(--text);
  }
  .about {
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 16px;
  }
  .ver {
    font-size: 11px;
    color: var(--faint);
    margin-left: 6px;
  }
  @media (max-width: 760px) {
    h1 {
      font-size: 28px;
    }
    .opt {
      align-items: flex-start;
    }
  }
</style>
