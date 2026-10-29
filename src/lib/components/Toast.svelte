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

<div class="toast panel" transition:fly={{ y: 10, duration: 200 }} role="status">
  <span class="bar" aria-hidden="true"></span>
  <div class="txt">
    <strong>{title}</strong>
    <span>{body}</span>
  </div>
</div>

<style>
  .toast {
    position: fixed;
    top: calc(var(--rail) + 12px);
    right: 18px;
    z-index: 60;
    display: flex;
    align-items: stretch;
    gap: 12px;
    min-width: 250px;
    max-width: 348px;
    padding: 10px 14px 10px 12px;
    box-shadow: 0 18px 50px -24px rgba(0, 0, 0, 0.9);
  }

  .bar {
    width: 2px;
    flex: none;
    background: var(--accent);
  }

  .txt {
    display: flex;
    flex-direction: column;
    gap: 4px;
    line-height: 1.4;
  }

  .txt strong {
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .txt span {
    color: var(--ink-faint);
    font-size: 12px;
    font-weight: 500;
  }
</style>
