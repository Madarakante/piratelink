<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { credits, type Game } from "./lib/games";
  import { fetchArtwork, launchGame, loadLibrary, removeGame, type LibrarySource } from "./lib/library";
  import { audio } from "./lib/audio";
  import { bindInput, gamepadConnected, type Action } from "./lib/input";
  import Backdrop from "./lib/components/Backdrop.svelte";
  import TopBar from "./lib/components/TopBar.svelte";
  import Carousel from "./lib/components/Carousel.svelte";
  import DetailOverlay from "./lib/components/DetailOverlay.svelte";
  import HintBar from "./lib/components/HintBar.svelte";
  import Toast from "./lib/components/Toast.svelte";
  import EmptyState from "./lib/components/EmptyState.svelte";
  import ImportSheet from "./lib/components/ImportSheet.svelte";

  let index = $state(0);
  let overlay = $state(false);
  let muted = $state(false);
  let booting = $state(true);
  let pad = $state(false);
  let stats = $state(false);
  let heapMB = $state<number | null>(null);
  let toast = $state<{ id: number; title: string; body: string } | null>(null);

  let games = $state<Game[]>([]);
  let source = $state<LibrarySource>("db");
  let canFetchArt = $state(false);
  let loading = $state(true);
  let loadError = $state("");
  let importOpen = $state(false);

  const current = $derived(games[index] ?? null);
  const accent = $derived(current?.accent ?? "#7fb0ff");

  /** #rrggbb -> rgba() so the accent can also drive translucent glows. */
  function accentSoft(hex: string, alpha: number): string {
    const n = parseInt(hex.slice(1), 16);
    if (Number.isNaN(n)) return `rgba(127, 176, 255, ${alpha})`;
    return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
  }

  function pushToast(title: string, body: string) {
    toast = { id: Date.now(), title, body };
  }

  async function reload() {
    loading = true;
    try {
      const res = await loadLibrary();
      games = res.games;
      source = res.source;
      canFetchArt = res.canFetchArt;
      if (index >= games.length) index = 0;
      loadError = "";
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  }

  function move(delta: number) {
    if (games.length === 0) return;
    const n = games.length;
    index = (index + delta + n) % n;
    audio.play("tick", 0.85);
  }

  function select(i: number) {
    if (i === index) return;
    index = i;
    audio.play("tick", 0.85);
  }

  function openDetails() {
    if (overlay || !current) return;
    overlay = true;
    audio.play("open");
  }

  function closeDetails() {
    if (!overlay) return;
    overlay = false;
    audio.play("back");
  }

  async function launch() {
    const game = current;
    if (!game) return;
    audio.play("select", 0.95);

    if (game.dbId == null) {
      pushToast("Demo entry", `${game.title} is sample data — add a folder to build a real library.`);
      return;
    }

    try {
      await launchGame(game.dbId);
      pushToast("Launching", game.title);
      overlay = false;
    } catch (err) {
      pushToast("Launch failed", String(err));
    }
  }

  async function refreshArtwork(game: Game) {
    if (game.dbId == null) return;
    try {
      await fetchArtwork(game.dbId);
      pushToast("Artwork updated", game.title);
      await reload();
    } catch (err) {
      pushToast("Artwork unchanged", String(err));
    }
  }

  async function removeEntry(game: Game) {
    if (game.dbId == null) return;
    try {
      await removeGame(game.dbId);
      overlay = false;
      pushToast("Removed from library", game.title);
      await reload();
    } catch (err) {
      pushToast("Could not remove", String(err));
    }
  }

  function demoFriend() {
    audio.play("chime", 1);
    pushToast("Kestrel is online", "Playing Sable Tide · joinable");
  }

  function handle(action: Action) {
    void audio.unlock();

    // The import sheet owns input while it is up.
    if (importOpen) {
      if (action === "back") importOpen = false;
      if (action === "mute") muted = audio.toggleMuted();
      return;
    }

    switch (action) {
      case "prev":
        if (!overlay) move(-1);
        break;
      case "next":
        if (!overlay) move(1);
        break;
      case "open":
        if (overlay) void launch();
        else openDetails();
        break;
      case "play":
        void launch();
        break;
      case "back":
        closeDetails();
        break;
      case "mute":
        muted = audio.toggleMuted();
        break;
      case "notify":
        demoFriend();
        break;
      case "stats":
        stats = !stats;
        break;
    }
  }

  onMount(() => {
    const stop = bindInput(handle);
    void audio.load();
    void reload();

    const unlock = () => void audio.unlock();
    window.addEventListener("pointerdown", unlock, { once: true });

    const bootTimer = setTimeout(() => (booting = false), 620);
    const padTimer = setInterval(() => (pad = gamepadConnected()), 1500);

    return () => {
      stop();
      window.removeEventListener("pointerdown", unlock);
      clearTimeout(bootTimer);
      clearInterval(padTimer);
    };
  });

  $effect(() => {
    if (!stats) return;
    const id = setInterval(() => {
      const m = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory;
      heapMB = m ? m.usedJSHeapSize / 1048576 : null;
    }, 400);
    return () => clearInterval(id);
  });
</script>

<div
  class="app"
  style="--accent: {accent}; --accent-soft: {accentSoft(accent, 0.22)};"
>
  {#if current}
    <Backdrop game={current} />
  {/if}

  <TopBar {muted} onToggleMute={() => (muted = audio.toggleMuted())} onAdd={() => (importOpen = true)} />

  <main class="stage" class:dim={overlay}>
    {#if loading}
      <div class="center"><span class="spinner" aria-hidden="true"></span></div>
    {:else if loadError}
      <div class="center message">
        <h2>Could not read the library</h2>
        <p>{loadError}</p>
      </div>
    {:else if games.length === 0}
      <EmptyState onOpen={() => (importOpen = true)} />
    {:else if current}
      <div class="hero">
        {#key current.id}
          <div in:fade={{ duration: 260 }}>
            {#if current.genre}<p class="eyebrow">{current.genre}</p>{/if}
            <h1>{current.title}</h1>
            {#if current.subtitle && current.subtitle.toLowerCase() !== current.title.toLowerCase()}
              <p class="sub">{current.subtitle}</p>
            {/if}
          </div>
        {/key}

        <div class="meta">
          {#if current.playtimeMinutes > 0}
            <span class="chip">{credits(current.playtimeMinutes)} played</span>
          {/if}
          {#if current.achievements.total > 0}
            <span class="chip">
              {current.achievements.unlocked}/{current.achievements.total} achievements
            </span>
          {/if}
          <span class="chip">{current.lastPlayed}</span>
          {#if source === "demo"}<span class="chip demo">preview data</span>{/if}
        </div>
      </div>

      <Carousel {games} {index} onSelect={select} onOpen={openDetails} />
    {/if}
  </main>

  <HintBar gamepad={pad} />

  {#if current}
    <DetailOverlay
      game={current}
      open={overlay}
      onClose={closeDetails}
      onPlay={launch}
      onRefreshArt={() => refreshArtwork(current)}
      onRemove={() => removeEntry(current)}
    />
  {/if}

  <ImportSheet
    open={importOpen}
    {canFetchArt}
    onClose={() => (importOpen = false)}
    onImported={reload}
  />

  {#if toast}
    {#key toast.id}
      <Toast title={toast.title} body={toast.body} onDone={() => (toast = null)} />
    {/key}
  {/if}

  {#if stats}
    <div class="stats glass">
      <span>renderer heap</span>
      <b>{heapMB === null ? "n/a" : `${heapMB.toFixed(1)} MB`}</b>
      <span class="sep">·</span>
      <span>{games.length} titles</span>
      <span class="sep">·</span>
      <span>{source}</span>
    </div>
  {/if}

  {#if booting}
    <div class="boot" transition:fade={{ duration: 420 }}>
      <span class="mark">PIRATELINK</span>
    </div>
  {/if}
</div>

<style>
  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    isolation: isolate;
  }

  .stage {
    position: relative;
    z-index: 10;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 6px;
    flex: 1;
    min-height: 0;
    padding: 0 24px 6px;
    transition: filter var(--dur-2) var(--ease-out), opacity var(--dur-2) linear;
  }

  .stage.dim {
    filter: blur(2px) brightness(0.62);
  }

  .center {
    display: grid;
    place-items: center;
    height: 100%;
  }

  .message {
    text-align: center;
    gap: 8px;
  }

  .message h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 650;
  }

  .message p {
    margin: 0;
    max-width: 60ch;
    color: var(--ink-dim);
    font-size: 13px;
  }

  .spinner {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.16);
    border-top-color: var(--accent);
    animation: spin 700ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .hero {
    position: relative;
    z-index: 12;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 10px;
    margin-bottom: 14px;
    pointer-events: none;
  }

  .eyebrow {
    margin: 0;
    color: var(--accent);
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    transition: color var(--dur-3) linear;
  }

  h1 {
    margin: 2px 0 0;
    font-family: var(--font-display);
    font-size: clamp(34px, 4.6vw, 62px);
    font-weight: 690;
    letter-spacing: -0.035em;
    line-height: 0.98;
    text-shadow: 0 18px 50px rgba(0, 0, 0, 0.65);
  }

  .sub {
    margin: 8px 0 0;
    color: var(--ink-dim);
    font-size: 14.5px;
    font-weight: 500;
    letter-spacing: 0.005em;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 4px;
  }

  .chip.demo {
    border-color: rgba(255, 196, 0, 0.35);
    color: #ffd479;
  }

  .stats {
    position: fixed;
    left: 26px;
    bottom: 66px;
    z-index: 55;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-radius: 12px;
    background: rgba(18, 20, 30, 0.7);
    border: 1px solid var(--stroke-subtle);
    color: var(--ink-faint);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  .stats b {
    color: var(--ink);
    font-weight: 650;
  }

  .sep {
    opacity: 0.4;
  }

  .boot {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    background: #04050a;
  }

  .mark {
    color: rgba(244, 246, 255, 0.9);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.42em;
    text-indent: 0.42em;
    animation: pulse 900ms var(--ease-out);
  }

  @keyframes pulse {
    from {
      opacity: 0;
      letter-spacing: 0.6em;
    }
  }
</style>
