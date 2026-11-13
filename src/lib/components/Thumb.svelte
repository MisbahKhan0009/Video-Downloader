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
    return h;
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
    width: 100%;
    height: 100%;
    overflow: hidden;
    background:
      radial-gradient(120% 90% at 20% 0%, hsla(var(--h), 30%, 40%, 0.28), transparent 60%),
      radial-gradient(90% 90% at 100% 100%, hsla(calc(var(--h) + 40), 25%, 30%, 0.22), transparent 60%),
      #15171b;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
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
