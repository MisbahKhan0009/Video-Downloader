<script lang="ts">
  let { checked = $bindable(false), onchange, label }: { checked?: boolean; onchange?: () => void; label: string } = $props();
</script>

<button
  class="tg"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  onclick={() => {
    checked = !checked;
    onchange?.();
  }}
>
  <i></i>
</button>

<style>
  .tg {
    flex: none;
    position: relative;
    width: 46px;
    height: 28px;
    border-radius: 999px;
    background: var(--glass-3);
    border: 1px solid var(--line-2);
    transition: background 0.2s var(--ease), border-color 0.2s;
  }
  .tg i {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #c9cdd3;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.45);
    transition: transform 0.25s var(--ease), background 0.2s;
  }
  .tg.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .tg.on i {
    transform: translateX(18px);
    background: var(--accent-ink);
  }
</style>
