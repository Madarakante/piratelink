<script lang="ts">
  import { onMount } from "svelte";

  let {
    muted = false,
    onToggleMute,
    onAdd,
  }: { muted?: boolean; onToggleMute?: () => void; onAdd?: () => void } = $props();

  let clock = $state("");
  let isTauri = $state(false);

  function tick() {
    clock = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
  }

  onMount(() => {
    tick();
    const id = setInterval(tick, 15_000);
    isTauri = "__TAURI_INTERNALS__" in window;
    return () => clearInterval(id);
  });

  async function win(action: "min" | "max" | "close") {
    if (!isTauri) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const w = getCurrentWindow();
      if (action === "min") await w.minimize();
      else if (action === "max") await w.toggleMaximize();
      else await w.close();
    } catch (err) {
      console.warn("[window]", err);
    }
  }
</script>

<header class="topbar" data-tauri-drag-region>
  <div class="identity">
    <div class="avatar" aria-hidden="true">A</div>
    <div class="who">
      <span class="name">Ash</span>
      <span class="status"><i class="dot"></i> Online</span>
    </div>
  </div>

  <div class="spacer"></div>

  <div class="right">
    <span class="clock">{clock}</span>

    <div class="chip friends"><span class="pill-dot"></span> 2 friends online</div>

    {#if onAdd}
      <button class="add" onclick={onAdd} title="Add games from a folder">
        <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
          <path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2" stroke-linecap="round" fill="none" />
        </svg>
        Add games
      </button>
    {/if}

    <button class="icon-btn" aria-label={muted ? "Unmute UI sounds" : "Mute UI sounds"} onclick={onToggleMute}>
      {#if muted}
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
          <path d="M4 9h3l4-4v14l-4-4H4z" fill="currentColor" />
          <path d="M16 9l5 6M21 9l-5 6" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" />
        </svg>
      {:else}
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
          <path d="M4 9h3l4-4v14l-4-4H4z" fill="currentColor" />
          <path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" />
        </svg>
      {/if}
    </button>

    {#if isTauri}
      <div class="win-controls">
        <button class="win" aria-label="Minimize" onclick={() => win("min")}>
          <svg viewBox="0 0 12 12" width="12" height="12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.2" /></svg>
        </button>
        <button class="win" aria-label="Maximize" onclick={() => win("max")}>
          <svg viewBox="0 0 12 12" width="12" height="12"><rect x="2.5" y="2.5" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.2" fill="none" /></svg>
        </button>
        <button class="win close" aria-label="Close" onclick={() => win("close")}>
          <svg viewBox="0 0 12 12" width="12" height="12"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" /></svg>
        </button>
      </div>
    {/if}
  </div>
</header>

<style>
  .topbar {
    position: relative;
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 64px;
    padding: 0 18px 0 26px;
  }

  .identity {
    display: flex;
    align-items: center;
    gap: 11px;
  }

  .avatar {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: linear-gradient(145deg, #93b6ff, #7a5cff);
    color: #08090f;
    font-size: 14px;
    font-weight: 750;
    box-shadow: 0 6px 18px -8px rgba(122, 92, 255, 0.9);
  }

  .who {
    display: flex;
    flex-direction: column;
    line-height: 1.15;
  }

  .name {
    font-size: 13.5px;
    font-weight: 650;
    letter-spacing: 0.01em;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--ink-faint);
    font-size: 11.5px;
    font-weight: 500;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #4ade80;
    box-shadow: 0 0 10px #4ade80;
  }

  .pill-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 9px var(--accent);
  }

  .spacer {
    flex: 1;
  }

  .right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .clock {
    color: var(--ink-dim);
    font-size: 12.5px;
    font-weight: 550;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.05);
    color: var(--ink-dim);
    cursor: pointer;
    transition: background var(--dur-1) linear, color var(--dur-1) linear;
  }

  .add {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 13px 0 11px;
    border-radius: 999px;
    border: 1px solid var(--stroke-strong);
    background: rgba(255, 255, 255, 0.07);
    color: var(--ink);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background var(--dur-1) linear, border-color var(--dur-1) linear;
  }

  .add:hover {
    background: rgba(255, 255, 255, 0.13);
    border-color: rgba(255, 255, 255, 0.28);
  }

  .icon-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--ink);
  }

  .win-controls {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: 6px;
  }

  .win {
    display: grid;
    place-items: center;
    width: 34px;
    height: 30px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
    transition: background var(--dur-1) linear, color var(--dur-1) linear;
  }

  .win:hover {
    background: rgba(255, 255, 255, 0.09);
    color: var(--ink);
  }

  .win.close:hover {
    background: #d92d20;
    color: #fff;
  }
</style>
