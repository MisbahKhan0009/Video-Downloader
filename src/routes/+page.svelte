<script lang="ts">
  import { onMount } from "svelte";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import ArrowDownToLine from "@lucide/svelte/icons/arrow-down-to-line";
  import Nav from "$lib/components/Nav.svelte";
  import Logo from "$lib/components/Logo.svelte";
  import LinkBar from "$lib/components/LinkBar.svelte";
  import SingleVideo from "$lib/components/SingleVideo.svelte";
  import PageResults from "$lib/components/PageResults.svelte";
  import DownloadsView from "$lib/components/DownloadsView.svelte";
  import SettingsView from "$lib/components/SettingsView.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { app } from "$lib/store.svelte";
  import { speed } from "$lib/format";

  onMount(() => {
    app.init();
    const onFocus = () => {
      app.checkShared();
      app.checkClipboard();
    };
    const onVisible = () => document.visibilityState === "visible" && onFocus();
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisible);
    };
  });

  const hero = $derived(app.view === "grab" && app.mode === "empty" && !app.analyzing && !app.error);
</script>

<Nav />

<main class:hero>
  <header class="top">
    <div class="brand">
      <span class="brand-logo"><Logo size={28} /></span>
      <span class="word">Vidra</span>
    </div>
    {#if app.active.length}
      <button class="activity glass" onclick={() => (app.view = "downloads")}>
        <span class="dot"></span>
        <ArrowDownToLine size={14} />
        <span class="mono">{app.active.length}</span>
        <span class="mono sp">{speed(app.totalSpeed)}</span>
      </button>
    {/if}
  </header>

  {#if app.view === "grab"}
    {#if hero}
      <section class="intro">
        <div class="eyebrow">Video grabber · Desktop & Android</div>
        <h1>Paste a link.<br /><span>Vidra finds the video.</span></h1>
        <p class="lede">
          Direct files download right away. Pages are read for every video inside, and paginated lists can be followed page after page.
        </p>
        <LinkBar large />
        <ol class="how">
          <li><span class="n mono">01</span><div><b>Video link</b><span>Recognised instantly and ready to save.</span></div></li>
          <li><span class="n mono">02</span><div><b>Page link</b><span>Scanned for video tags, players and linked cards.</span></div></li>
          <li><span class="n mono">03</span><div><b>Paginated site</b><span>Jump between pages or collect several at once.</span></div></li>
        </ol>
      </section>
    {:else}
      <div class="stack">
        <LinkBar />

        {#if app.analyzing}
          <div class="skeleton">
            <div class="sk-head"><i></i><i></i></div>
            <div class="sk-grid">
              {#each Array(8) as _, i (i)}<div class="sk-card" style="animation-delay:{i * 60}ms"></div>{/each}
            </div>
          </div>
        {:else if app.error}
          <div class="error glass">
            <CircleAlert size={20} />
            <div>
              <b>Couldn't read that link</b>
              <p>{app.error}</p>
            </div>
            <button class="btn sm" onclick={() => app.grab()}><RotateCw size={14} />Try again</button>
          </div>
        {:else if app.mode === "video" && app.videos[0]}
          <SingleVideo video={app.videos[0]} />
        {:else if app.mode === "page" && app.page}
          <PageResults />
        {/if}
      </div>
    {/if}
  {:else if app.view === "downloads"}
    <div class="stack"><DownloadsView /></div>
  {:else}
    <div class="stack"><SettingsView /></div>
  {/if}
</main>

<Toasts />

<style>
  main {
    position: relative;
    z-index: 1;
    margin-left: 96px;
    padding: 22px 40px 64px 24px;
    max-width: 1280px;
    min-height: 100vh;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    margin-bottom: 22px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .brand-logo {
    display: none;
  }
  .word {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .activity {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border-radius: 999px;
    font-size: 12.5px;
    color: var(--text);
  }
  .activity .sp {
    color: var(--accent);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 10px var(--accent);
  }

  .stack {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 22px;
    animation: rise 0.35s var(--ease);
  }

  /* ---------- hero ---------- */
  .intro {
    max-width: 760px;
    margin: 9vh auto 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 18px;
    animation: rise 0.6s var(--ease);
  }
  h1 {
    margin: 0;
    font-size: clamp(36px, 5.4vw, 60px);
    line-height: 1.02;
    letter-spacing: -0.045em;
    font-weight: 600;
  }
  h1 span {
    color: var(--muted);
    font-weight: 500;
  }
  .lede {
    margin: 0 0 10px;
    max-width: 560px;
    color: var(--muted);
    font-size: 16px;
    line-height: 1.55;
  }
  .how {
    list-style: none;
    margin: 22px 0 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }
  .how li {
    display: flex;
    gap: 12px;
    padding: 16px;
    border-radius: 18px;
    border: 1px solid var(--line);
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.025), transparent);
  }
  .how .n {
    font-size: 11px;
    color: var(--accent);
    padding-top: 2px;
  }
  .how div {
    display: grid;
    gap: 3px;
  }
  .how b {
    font-size: 14px;
    font-weight: 500;
  }
  .how div span {
    font-size: 12.5px;
    color: var(--muted);
    line-height: 1.45;
  }

  /* ---------- states ---------- */
  .error {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px 18px;
    border-radius: var(--r-lg);
    color: var(--danger);
    border-color: rgba(255, 107, 91, 0.25);
    background: rgba(255, 107, 91, 0.05);
  }
  .error div {
    flex: 1;
    min-width: 0;
    color: var(--text);
  }
  .error b {
    font-weight: 600;
    font-size: 14.5px;
  }
  .error p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 13.5px;
  }

  .skeleton {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 22px;
  }
  .sk-head {
    display: grid;
    gap: 10px;
  }
  .sk-head i {
    display: block;
    height: 14px;
    width: 160px;
    border-radius: 6px;
    background: var(--glass-2);
  }
  .sk-head i + i {
    height: 30px;
    width: 420px;
    max-width: 80%;
    border-radius: 10px;
  }
  .sk-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(232px, 1fr));
    gap: 16px;
  }
  .sk-card {
    aspect-ratio: 4 / 3.3;
    border-radius: var(--r-lg);
    border: 1px solid var(--line);
    background: linear-gradient(100deg, rgba(255, 255, 255, 0.02) 30%, rgba(255, 255, 255, 0.06) 50%, rgba(255, 255, 255, 0.02) 70%);
    background-size: 300% 100%;
    animation: shimmer 1.6s linear infinite;
  }
  @keyframes shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -200% 0;
    }
  }

  @media (max-width: 760px) {
    main {
      margin-left: 0;
      padding: calc(14px + var(--safe-top)) 16px calc(110px + var(--safe-bottom));
    }
    .top {
      margin-bottom: 16px;
    }
    .brand-logo {
      display: grid;
    }
    .intro {
      margin-top: 4vh;
      gap: 14px;
    }
    .lede {
      font-size: 15px;
    }
    .how {
      grid-template-columns: 1fr;
      gap: 8px;
      margin-top: 12px;
    }
    .how li {
      padding: 12px 14px;
    }
    .sk-grid {
      grid-template-columns: repeat(auto-fill, minmax(158px, 1fr));
      gap: 10px;
    }
  }
</style>
