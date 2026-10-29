<script lang="ts">
  import { fly } from "svelte/transition";
  import { onMount } from "svelte";

  let {
    title,
    body,
    onDone,
  }: { title: string; body: string; onDone: () => void } = $props();

  onMount(() => {
    const id = setTimeout(onDone, 3600);
    return () => clearTimeout(id);
  });
</script>

<div class="toast glass" transition:fly={{ y: -14, duration: 260 }} role="status">
  <div class="badge" aria-hidden="true">
    <svg viewBox="0 0 24 24" width="14" height="14">
      <circle cx="12" cy="8.5" r="3.4" fill="currentColor" />
      <path d="M4.5 20c0-4 3.4-6.2 7.5-6.2s7.5 2.2 7.5 6.2z" fill="currentColor" />
    </svg>
  </div>
  <div class="txt">
    <strong>{title}</strong>
    <span>{body}</span>
  </div>
</div>

<style>
  .toast {
    position: fixed;
    top: 78px;
    right: 22px;
    z-index: 60;
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 268px;
    max-width: 340px;
    padding: 11px 16px 11px 11px;
    border-radius: 14px;
    background: rgba(16, 18, 26, 0.78);
    border-color: var(--stroke-strong);
    box-shadow: 0 22px 60px -26px rgba(0, 0, 0, 0.95);
  }

  .badge {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: none;
    border-radius: 10px;
    background: var(--accent-soft, rgba(127, 176, 255, 0.2));
    color: var(--accent);
  }

  .txt {
    display: flex;
    flex-direction: column;
    line-height: 1.3;
  }

  .txt strong {
    font-size: 13px;
    font-weight: 650;
  }

  .txt span {
    color: var(--ink-faint);
    font-size: 12px;
    font-weight: 500;
  }
</style>
