<script lang="ts">
  import { onMount } from "svelte";

  let {
    muted = false,
    count = 0,
    preview = false,
    onToggleMute,
    onAdd,
  }: {
    muted?: boolean;
    count?: number;
    preview?: boolean;
    onToggleMute?: () => void;
    onAdd?: () => void;
  } = $props();

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

<!--
  The rail carries only things that exist: the product, the size of the library,
  the time, and the three controls. The previous version invented an account
  ("Ash") and a friend count, which is the tell that a UI was decorated rather
  than designed.
-->
<header class="rail" data-tauri-drag-region>
  <span class="mark"><i class="sq"></i>PirateLink</span>

  <span class="slash">/</span>
  <span class="label">Library</span>
  <span class="count num">{String(count).padStart(2, "0")}</span>

  {#if preview}
    <span class="preview">Preview</span>
  {/if}

  <span class="spacer"></span>

  <span class="clock num">{clock}</span>

  {#if onAdd}
    <button class="add" onclick={onAdd} title="Add games from a folder">
      <svg viewBox="0 0 12 12" width="9" height="9" aria-hidden="true">
        <path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.4" fill="none" />
      </svg>
      Add games
    </button>
  {/if}

  <button
    class="icon-btn"
    aria-label={muted ? "Unmute UI sounds" : "Mute UI sounds"}
    title={muted ? "Unmute (M)" : "Mute (M)"}
    onclick={onToggleMute}
  >
    {#if muted}
      <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
        <path d="M4 9h3l4-4v14l-4-4H4z" fill="currentColor" />
        <path d="M16 9l5 6M21 9l-5 6" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" />
      </svg>
    {:else}
      <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
        <path d="M4 9h3l4-4v14l-4-4H4z" fill="currentColor" />
        <path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" />
      </svg>
    {/if}
  </button>

  {#if isTauri}
    <div class="win-controls">
      <button class="win" aria-label="Minimize" onclick={() => win("min")}>
        <svg viewBox="0 0 12 12" width="11" height="11"><path d="M2.5 6h7" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
      <button class="win" aria-label="Maximize" onclick={() => win("max")}>
        <svg viewBox="0 0 12 12" width="11" height="11"><rect x="2.5" y="2.5" width="7" height="7" stroke="currentColor" stroke-width="1.1" fill="none" /></svg>
      </button>
      <button class="win close" aria-label="Close" onclick={() => win("close")}>
        <svg viewBox="0 0 12 12" width="11" height="11"><path d="M3.2 3.2l5.6 5.6M8.8 3.2l-5.6 5.6" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </div>
  {/if}
</header>

<style>
  .rail {
    position: relative;
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px 0 var(--gutter);
    border-bottom: 1px solid var(--line);
  }

  .mark {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--ink);
  }

  .sq {
    width: 5px;
    height: 5px;
    background: var(--ink);
  }

  .slash {
    color: var(--ink-ghost);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .count {
    color: var(--ink);
    font-size: 11px;
  }

  .preview {
    margin-left: 6px;
    padding: 3px 7px;
    border: 1px solid var(--line-strong);
    border-radius: 2px;
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 8.5px;
    font-weight: 500;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }

  .spacer {
    flex: 1;
  }

  .clock {
    margin-right: 6px;
    color: var(--ink-faint);
    font-size: 11px;
  }

  .add {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 12px;
    border: 1px solid var(--line-strong);
    border-radius: 2px;
    background: transparent;
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    cursor: pointer;
    transition:
      background var(--dur-1) linear,
      border-color var(--dur-1) linear;
  }

  .add:hover {
    background: rgba(255, 255, 255, 0.07);
    border-color: rgba(255, 255, 255, 0.4);
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
    transition:
      background var(--dur-1) linear,
      color var(--dur-1) linear;
  }

  .icon-btn:hover {
    background: rgba(255, 255, 255, 0.07);
    color: var(--ink);
  }

  .win-controls {
    display: flex;
    align-items: center;
    gap: 1px;
    margin-left: 4px;
  }

  .win {
    display: grid;
    place-items: center;
    width: 30px;
    height: 24px;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
    transition:
      background var(--dur-1) linear,
      color var(--dur-1) linear;
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
