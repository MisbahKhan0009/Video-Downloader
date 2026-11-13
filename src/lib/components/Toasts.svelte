<script lang="ts">
  import CircleCheck from "@lucide/svelte/icons/circle-check";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Info from "@lucide/svelte/icons/info";
  import { app } from "$lib/store.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast glass" data-tone={t.tone}>
      {#if t.tone === "ok"}<CircleCheck size={16} />{:else if t.tone === "error"}<CircleAlert size={16} />{:else}<Info size={16} />{/if}
      <span>{t.text}</span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    z-index: 50;
    right: 20px;
    bottom: 20px;
    display: grid;
    gap: 8px;
    justify-items: end;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 380px;
    padding: 11px 16px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.92);
    font-size: 13.5px;
    box-shadow:
      var(--edge),
      0 18px 40px -12px rgba(0, 0, 0, 0.8);
    animation: rise 0.3s var(--ease);
  }
  .toast span {
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  [data-tone="ok"] :global(svg) {
    color: var(--accent);
  }
  [data-tone="error"] :global(svg) {
    color: var(--danger);
  }
  [data-tone="info"] :global(svg) {
    color: var(--muted);
  }
  @media (max-width: 760px) {
    .toasts {
      left: 16px;
      right: 16px;
      bottom: calc(92px + var(--safe-bottom));
      justify-items: center;
    }
  }
</style>
