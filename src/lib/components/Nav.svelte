<script lang="ts">
  import Link from "@lucide/svelte/icons/link-2";
  import ArrowDownToLine from "@lucide/svelte/icons/arrow-down-to-line";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import Logo from "./Logo.svelte";
  import { app, type View } from "$lib/store.svelte";

  const items: { id: View; label: string; icon: typeof Link }[] = [
    { id: "grab", label: "Grab", icon: Link },
    { id: "downloads", label: "Downloads", icon: ArrowDownToLine },
    { id: "settings", label: "Settings", icon: SlidersHorizontal },
  ];
  const activeCount = $derived(app.active.length);
</script>

<nav class="rail glass" aria-label="Main">
  <div class="mark"><Logo size={34} /></div>
  <div class="items">
    {#each items as it (it.id)}
      {@const Icon = it.icon}
      <button
        class="item"
        class:on={app.view === it.id}
        onclick={() => (app.view = it.id)}
        aria-current={app.view === it.id ? "page" : undefined}
        title={it.label}
      >
        <span class="ico">
          <Icon size={20} strokeWidth={1.7} />
          {#if it.id === "downloads" && activeCount > 0}
            <span class="count mono">{activeCount}</span>
          {/if}
        </span>
        <span class="lbl">{it.label}</span>
      </button>
    {/each}
  </div>
  <div class="foot mono">v0.1</div>
</nav>

<style>
  .rail {
    position: fixed;
    z-index: 20;
    left: 14px;
    top: 14px;
    bottom: 14px;
    width: 68px;
    border-radius: var(--r-xl);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 14px 0;
    background: rgba(18, 20, 24, 0.62);
  }
  .mark {
    margin-bottom: 26px;
    filter: drop-shadow(0 6px 18px rgba(var(--accent-rgb), 0.25));
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .item {
    position: relative;
    width: 46px;
    height: 46px;
    border-radius: 15px;
    display: grid;
    place-items: center;
    color: var(--muted);
    transition:
      background 0.18s var(--ease),
      color 0.18s var(--ease);
  }
  .item:hover {
    color: var(--text);
    background: var(--glass-2);
  }
  .item.on {
    color: var(--text);
    background: var(--glass-3);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      0 0 0 1px var(--line);
  }
  .item.on::before {
    content: "";
    position: absolute;
    left: -11px;
    top: 50%;
    width: 4px;
    height: 18px;
    margin-top: -9px;
    border-radius: 0 4px 4px 0;
    background: var(--accent);
    box-shadow: 0 0 12px rgba(var(--accent-rgb), 0.6);
  }
  .ico {
    position: relative;
    display: grid;
    place-items: center;
  }
  .count {
    position: absolute;
    top: -8px;
    right: -11px;
    min-width: 17px;
    height: 17px;
    padding: 0 4px;
    border-radius: 9px;
    background: var(--accent);
    color: var(--accent-ink);
    font-size: 10px;
    font-weight: 700;
    display: grid;
    place-items: center;
    box-shadow: 0 0 0 2px #121418;
  }
  .lbl {
    display: none;
  }
  .foot {
    margin-top: auto;
    font-size: 10px;
    color: var(--faint);
  }

  /* Phones and narrow windows: floating bottom bar */
  @media (max-width: 760px) {
    .rail {
      top: auto;
      left: 50%;
      bottom: calc(12px + var(--safe-bottom));
      transform: translateX(-50%);
      width: auto;
      height: 64px;
      flex-direction: row;
      padding: 0 8px;
      border-radius: 22px;
      background: rgba(18, 20, 24, 0.78);
      box-shadow:
        var(--edge),
        0 20px 40px -10px rgba(0, 0, 0, 0.8);
    }
    .mark,
    .foot {
      display: none;
    }
    .items {
      flex-direction: row;
      gap: 4px;
    }
    .item {
      width: 84px;
      height: 50px;
      border-radius: 16px;
      grid-template-rows: auto auto;
      gap: 3px;
      align-content: center;
    }
    .item.on::before {
      left: 50%;
      top: auto;
      bottom: -7px;
      width: 18px;
      height: 3px;
      margin: 0 0 0 -9px;
      border-radius: 3px 3px 0 0;
    }
    .lbl {
      display: block;
      font-size: 11px;
      font-weight: 500;
      letter-spacing: 0.01em;
    }
  }
</style>
