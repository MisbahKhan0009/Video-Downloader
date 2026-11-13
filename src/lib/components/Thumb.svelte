<script lang="ts">
  import Film from "@lucide/svelte/icons/film";

  let { src, seed = "", label = "", compact = false }: { src: string | null; seed?: string; label?: string; compact?: boolean } = $props();

  let failed = $state(false);
  let loaded = $state(false);

  $effect(() => {
    src;
    failed = false;
    loaded = false;
  });

  // A stable, quiet tint per video so placeholders don't all look identical.
  const hue = $derived.by(() => {
    let h = 0;
    for (const c of seed) h = (h * 31 + c.charCodeAt(0)) % 360;
    return 200 + (h % 70);
  });
</script>

<div class="thumb" class:compact style="--h:{hue}">
  {#if src && !failed}
    <img
      {src}
      alt=""
      loading="lazy"
      decoding="async"
      referrerpolicy="no-referrer"
      class:loaded
      onload={() => (loaded = true)}
      onerror={() => (failed = true)}
    />
  {/if}
  {#if !src || failed || !loaded}
    <div class="ph">
      <Film size={compact ? 16 : 22} strokeWidth={1.5} />
      {#if label && !compact}<span>{label}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .thumb {
    position: relative;
    isolation: isolate;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: linear-gradient(
      160deg,
      hsl(var(--h), 18%, 13%) 0%,
      hsl(calc(var(--h) + 25), 32%, 19%) 100%
    );
  }
  .thumb::after {
    content: "";
    position: absolute;
    inset: 0;
    opacity: 0.18;
    mix-blend-mode: overlay;
    z-index: 0;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix type='saturate' values='0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
    pointer-events: none;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    z-index: 1;
    opacity: 0;
    transform: scale(1.02);
    transition:
      opacity 0.4s var(--ease),
      transform 0.6s var(--ease);
  }
  img.loaded {
    opacity: 1;
    transform: none;
  }
  .ph {
    position: absolute;
    z-index: 2;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 6px;
    color: rgba(255, 255, 255, 0.28);
  }
  .ph span {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.14em;
  }
</style>
