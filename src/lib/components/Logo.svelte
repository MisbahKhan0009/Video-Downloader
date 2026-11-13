<script lang="ts">
  let { size = 32 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
  <rect width="32" height="32" rx="10" fill="var(--accent)" />
  <path d="M10.2 10.4h11.6c.9 0 1.4 1 .9 1.7l-5.8 7.6c-.4.5-1.2.5-1.6 0l-5.8-7.6c-.5-.7 0-1.7.7-1.7Z" fill="var(--accent-ink)" />
  <rect x="10" y="21.6" width="12" height="2.6" rx="1.3" fill="var(--accent-ink)" />
</svg>
