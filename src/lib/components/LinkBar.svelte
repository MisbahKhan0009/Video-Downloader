<script lang="ts">
  import Link from "@lucide/svelte/icons/link-2";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import X from "@lucide/svelte/icons/x";
  import ClipboardPaste from "@lucide/svelte/icons/clipboard-paste";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import CornerDownLeft from "@lucide/svelte/icons/corner-down-left";
  import { app } from "$lib/store.svelte";
  import { guessKind, host } from "$lib/format";

  let { large = false }: { large?: boolean } = $props();
  let focused = $state(false);
  let el: HTMLInputElement;

  const kind = $derived(guessKind(app.input));
  const kindLabel = $derived(
    kind === "video" ? "Video link" : kind === "stream" ? "Stream" : kind === "page" ? "Web page" : null,
  );

  function submit(e: SubmitEvent) {
    e.preventDefault();
    app.grab();
    el?.blur();
  }

  function onPaste(e: ClipboardEvent) {
    const t = e.clipboardData?.getData("text")?.trim();
    if (t && app.settings.autoGrab && guessKind(t)) {
      e.preventDefault();
      app.input = t;
      app.clipboardSuggestion = null;
      app.grab();
    }
  }

  function short(u: string) {
    try {
      const x = new URL(u);
      const p = x.pathname.length > 28 ? x.pathname.slice(0, 26) + "…" : x.pathname;
      return host(u) + (p === "/" ? "" : p);
    } catch {
      return u.slice(0, 40);
    }
  }
</script>

<div class="wrap" class:large>
  <form class="bar glass" class:focused onsubmit={submit}>
    <span class="lead"><Link size={large ? 20 : 18} strokeWidth={1.8} /></span>
    <input
      bind:this={el}
      bind:value={app.input}
      onfocus={() => (focused = true)}
      onblur={() => (focused = false)}
      onpaste={onPaste}
      placeholder="Paste a video or page link"
      spellcheck="false"
      autocomplete="off"
      autocapitalize="off"
      inputmode="url"
      enterkeyhint="go"
      aria-label="Video or page link"
    />
    {#if kindLabel}
      <span class="kind" data-kind={kind}>
        <i></i>{kindLabel}
      </span>
    {/if}
    {#if app.input}
      <button type="button" class="icon-btn clear" title="Clear" onclick={() => { app.input = ""; el?.focus(); }}>
        <X size={16} />
      </button>
    {:else}
      <button type="button" class="paste" onclick={() => app.pasteFromClipboard()}>
        <ClipboardPaste size={16} strokeWidth={1.8} /><span>Paste</span>
      </button>
    {/if}
    <button type="submit" class="go" disabled={!app.input.trim() || app.analyzing} title="Grab">
      {#if app.analyzing}
        <LoaderCircle size={20} class="spin" />
      {:else}
        <ArrowRight size={20} strokeWidth={2.2} />
      {/if}
    </button>
  </form>

  {#if app.clipboardSuggestion && !app.analyzing}
    <button class="suggest" onclick={() => app.useSuggestion()}>
      <span class="eyebrow">Copied</span>
      <span class="s-url mono">{short(app.clipboardSuggestion)}</span>
      <CornerDownLeft size={14} />
    </button>
  {/if}
</div>

<style>
  .wrap {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 10px;
    justify-items: start;
  }
  .bar {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 58px;
    padding: 0 7px 0 18px;
    border-radius: 20px;
    background: rgba(20, 22, 26, 0.7);
    transition:
      border-color 0.2s var(--ease),
      box-shadow 0.2s var(--ease);
  }
  .large .bar {
    height: 68px;
    border-radius: 24px;
    padding-left: 22px;
  }
  .bar.focused {
    border-color: rgba(212, 255, 63, 0.35);
    box-shadow:
      var(--edge),
      0 0 0 4px rgba(212, 255, 63, 0.07),
      var(--lift);
  }
  .lead {
    color: var(--muted);
    display: grid;
    flex: none;
  }
  .focused .lead {
    color: var(--accent);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    outline: 0;
    background: transparent;
    font-size: 15.5px;
    padding: 0 6px;
    letter-spacing: -0.005em;
  }
  .large input {
    font-size: 17px;
  }
  input::placeholder {
    color: var(--faint);
  }
  .kind {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    padding: 0 11px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
    background: var(--glass-2);
    border: 1px solid var(--line);
    animation: rise 0.25s var(--ease);
  }
  .kind i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--muted);
  }
  .kind[data-kind="video"] i,
  .kind[data-kind="stream"] i {
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }
  .kind[data-kind="page"] i {
    background: #9db7ff;
    box-shadow: 0 0 8px #9db7ff;
  }
  .paste {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 36px;
    padding: 0 12px;
    border-radius: 12px;
    color: var(--muted);
    font-size: 13.5px;
    font-weight: 500;
  }
  .paste:hover {
    background: var(--glass-2);
    color: var(--text);
  }
  .go {
    flex: none;
    width: 46px;
    height: 46px;
    border-radius: 15px;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow: 0 8px 24px -8px rgba(212, 255, 63, 0.6);
    transition:
      transform 0.18s var(--ease),
      opacity 0.18s,
      background 0.18s;
  }
  .large .go {
    width: 54px;
    height: 54px;
    border-radius: 18px;
  }
  .go:hover:not(:disabled) {
    background: #e0ff6e;
  }
  .go:active:not(:disabled) {
    transform: scale(0.94);
  }
  .go:disabled {
    background: var(--glass-3);
    color: var(--faint);
    box-shadow: none;
  }
  .suggest {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    max-width: 100%;
    height: 34px;
    padding: 0 14px;
    border-radius: 999px;
    background: var(--accent-soft);
    border: 1px solid rgba(212, 255, 63, 0.22);
    color: var(--accent);
    font-size: 12.5px;
    animation: rise 0.3s var(--ease);
  }
  .suggest .eyebrow {
    color: var(--accent);
    opacity: 0.75;
  }
  .s-url {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 760px) {
    .bar,
    .large .bar {
      height: 58px;
      border-radius: 20px;
      padding-left: 16px;
    }
    .large .go {
      width: 46px;
      height: 46px;
      border-radius: 15px;
    }
    .large input {
      font-size: 16px;
    }
    .kind {
      display: none;
    }
    .paste span {
      display: none;
    }
  }
</style>
