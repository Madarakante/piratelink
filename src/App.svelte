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
  const accent = $derived(current?.accent ?? "#eef0f4");

  /** #rrggbb -> rgba() so the accent can also drive a translucent wash. */
  function accentSoft(hex: string, alpha: number): string {
    const n = parseInt(hex.slice(1), 16);
    if (Number.isNaN(n)) return `rgba(238, 240, 244, ${alpha})`;
    return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
  }

  const pad2 = (n: number) => String(n).padStart(2, "0");

  /** "Played yesterday" reads as noise under a label already saying Played. */
  function lastPlayedLabel(value: string): string {
    if (!value) return "—";
    if (/^never/i.test(value)) return "Never";
    return value.replace(/^played\s*/i, "");
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

    const bootTimer = setTimeout(() => (booting = false), 260);
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
  style="--accent: {accent}; --accent-soft: {accentSoft(accent, 0.18)};"
>
  {#if current}
    <Backdrop game={current} />
  {/if}

  <TopBar
    {muted}
    count={games.length}
    preview={source === "demo"}
    onToggleMute={() => (muted = audio.toggleMuted())}
    onAdd={() => (importOpen = true)}
  />

  <main class="stage" class:dim={overlay}>
    {#if loading}
      <div class="span center"><span class="loader" aria-hidden="true"></span></div>
    {:else if loadError}
      <div class="span center message">
        <p class="label">Library</p>
        <h2>Could not read the library</h2>
        <p class="detail">{loadError}</p>
      </div>
    {:else if games.length === 0}
      <div class="span"><EmptyState onOpen={() => (importOpen = true)} /></div>
    {:else if current}
      <section class="hero">
        {#key current.id}
          <div class="hero-in" in:fade={{ duration: 220 }}>
            <div class="kicker">
              <span class="idx">{pad2(index + 1)}</span>
              <span class="sep">/</span>
              <span class="tot">{pad2(games.length)}</span>
              {#if current.genre}
                <span class="genre">{current.genre}</span>
              {/if}
            </div>

            <h1>{current.title}</h1>

            {#if current.subtitle && current.subtitle.toLowerCase() !== current.title.toLowerCase()}
              <p class="sub">{current.subtitle}</p>
            {/if}

            <div class="actions">
              <button class="btn" onclick={launch}>
                <svg viewBox="0 0 12 14" width="10" height="12" aria-hidden="true">
                  <path d="M1 1l10 6-10 6z" fill="currentColor" />
                </svg>
                Play
              </button>
              <button class="btn btn-ghost" onclick={openDetails}>Details</button>
            </div>
          </div>
        {/key}
      </section>

      <!-- Facts live in their own right-hand column. A centred row of chips under
           the title is the default shape of every generated dashboard; a small
           record table reads like the back of a sleeve instead. -->
      {#key current.id}
        <dl class="facts" in:fade={{ duration: 260 }}>
          <div>
            <dt>Playtime</dt>
            <dd>{current.playtimeMinutes > 0 ? credits(current.playtimeMinutes) : "—"}</dd>
          </div>
          {#if current.achievements.total > 0}
            <div>
              <dt>Achievements</dt>
              <dd>{current.achievements.unlocked} / {current.achievements.total}</dd>
            </div>
          {/if}
          <div>
            <dt>Last played</dt>
            <dd>{lastPlayedLabel(current.lastPlayed)}</dd>
          </div>
          <div>
            <dt>Index</dt>
            <dd>{pad2(index + 1)} of {pad2(games.length)}</dd>
          </div>
        </dl>
      {/key}

      <div class="strip">
        <Carousel {games} {index} onSelect={select} onOpen={openDetails} />
      </div>
    {/if}
  </main>

  <HintBar gamepad={pad} {index} total={games.length} />

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
    <div class="stats panel">
      <span class="label">Renderer heap</span>
      <b class="num">{heapMB === null ? "n/a" : `${heapMB.toFixed(1)} MB`}</b>
      <span class="divider"></span>
      <span class="num">{games.length} titles</span>
      <span class="divider"></span>
      <span class="num">{source}</span>
    </div>
  {/if}

  {#if booting}
    <div class="boot" transition:fade={{ duration: 200 }}>
      <span class="label">PirateLink</span>
    </div>
  {/if}
</div>

<style>
  .app {
    position: relative;
    display: grid;
    grid-template-rows: var(--rail) minmax(0, 1fr) var(--footer);
    height: 100%;
    isolation: isolate;
  }

  /* One left margin for the rail, the title and the first cover. That shared
     edge is what makes the screen read as a layout instead of a stack.
     Row 1 is the record (headline left, facts right); row 2 is the shelf. */
  .stage {
    position: relative;
    z-index: 10;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-rows: minmax(0, 1fr) auto;
    column-gap: 64px;
    min-height: 0;
    padding: 0 var(--gutter) 18px;
    transition: opacity var(--dur-2) linear;
  }

  .stage.dim {
    opacity: 0.3;
  }

  .span {
    grid-column: 1 / -1;
    grid-row: 1 / -1;
  }

  .center {
    display: grid;
    place-items: center;
    height: 100%;
  }

  .loader {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1px solid var(--line-strong);
    border-top-color: var(--ink);
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .message {
    justify-items: center;
    gap: 12px;
    text-align: center;
  }

  .message h2 {
    margin: 0;
    font-size: 26px;
    font-weight: 680;
    letter-spacing: -0.03em;
  }

  .detail {
    margin: 0;
    max-width: 62ch;
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 11.5px;
    line-height: 1.7;
  }

  /* --- the record --------------------------------------------------------- */

  .hero {
    grid-column: 1;
    grid-row: 1;
    align-self: end;
    max-width: 820px;
    padding-bottom: 34px;
  }

  .kicker {
    display: flex;
    align-items: baseline;
    gap: 7px;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    font-variant-numeric: tabular-nums;
  }

  .kicker .idx {
    color: var(--accent);
  }

  .kicker .sep,
  .kicker .tot {
    color: var(--ink-ghost);
  }

  .kicker .genre {
    margin-left: 12px;
    padding-left: 13px;
    border-left: 1px solid var(--line);
    color: var(--ink-faint);
  }

  h1 {
    margin: 16px 0 0;
    font-size: clamp(38px, 5.4vw, 80px);
    font-weight: 720;
    letter-spacing: -0.045em;
    line-height: 0.92;
    text-wrap: balance;
  }

  .sub {
    margin: 12px 0 0;
    color: var(--ink-dim);
    font-size: 14px;
    font-weight: 500;
  }

  /* Record table: label left, value right, one hairline per row. */
  .facts {
    grid-column: 2;
    grid-row: 1;
    align-self: end;
    width: 272px;
    margin: 0;
    padding-bottom: 34px;
  }

  .facts > div {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 24px;
    padding: 9px 0;
    border-top: 1px solid var(--line);
  }

  .facts > div:last-child {
    border-bottom: 1px solid var(--line);
  }

  dt {
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 500;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }

  dd {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 10px;
    margin-top: 30px;
  }

  .strip {
    grid-column: 1 / -1;
    grid-row: 2;
  }

  /* --- readouts ----------------------------------------------------------- */

  .stats {
    position: fixed;
    left: var(--gutter);
    top: calc(var(--rail) + 14px);
    z-index: 55;
    display: inline-flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    font-size: 11px;
  }

  .stats b {
    color: var(--ink);
    font-weight: 600;
  }

  .stats .num {
    color: var(--ink-faint);
    font-family: var(--font-mono);
  }

  .divider {
    width: 1px;
    height: 10px;
    background: var(--line);
  }

  .boot {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: flex;
    align-items: flex-end;
    padding: 0 0 28px var(--gutter);
    background: var(--bg-0);
  }

  .boot .label {
    color: var(--ink-ghost);
  }
</style>
