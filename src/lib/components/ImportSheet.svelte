<script lang="ts">
  import { fly, fade } from "svelte/transition";
  import {
    addGame,
    addRoot,
    fetchArtwork,
    pickFolder,
    scanLibrary,
    type Candidate,
  } from "../library";

  let {
    open,
    canFetchArt,
    onClose,
    onImported,
  }: {
    open: boolean;
    canFetchArt: boolean;
    onClose: () => void;
    onImported: () => void;
  } = $props();

  type Step = "choose" | "scanning" | "review" | "importing" | "done";

  let step = $state<Step>("choose");
  let rootPath = $state("");
  let candidates = $state<Candidate[]>([]);
  let selected = $state<string[]>([]);
  let error = $state("");
  let elapsed = $state(0);
  let done = $state(0);
  let total = $state(0);
  let label = $state("");
  let added = $state(0);
  let artFailed = $state(0);

  /** A scan of a real folder finishes in milliseconds; anything slower is a bug,
   *  and it must never leave the sheet spinning forever. */
  const SCAN_TIMEOUT_MS = 20_000;

  function withTimeout<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
    return Promise.race([
      promise,
      new Promise<T>((_, reject) =>
        setTimeout(() => reject(new Error(`${what} did not finish within ${ms / 1000}s.`)), ms),
      ),
    ]);
  }

  const selectable = $derived(candidates.filter((c) => !c.already_in_library));
  const allSelected = $derived(
    selectable.length > 0 && selected.length === selectable.length,
  );

  function reset() {
    step = "choose";
    rootPath = "";
    candidates = [];
    selected = [];
    error = "";
    done = 0;
    total = 0;
    label = "";
    added = 0;
    artFailed = 0;
  }

  function close() {
    if (step === "importing") return;
    reset();
    onClose();
  }

  async function chooseFolder() {
    error = "";
    try {
      const folder = await pickFolder();
      if (!folder) return;
      rootPath = folder;
      await addRoot(folder);
      await runScan();
    } catch (err) {
      error = String(err);
    }
  }

  async function runScan() {
    step = "scanning";
    elapsed = 0;
    const timer = setInterval(() => (elapsed += 1), 1000);

    try {
      candidates = await withTimeout(scanLibrary(), SCAN_TIMEOUT_MS, "The folder scan");
      selected = candidates.filter((c) => !c.already_in_library).map((c) => c.path);
      step = "review";
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      step = "choose";
    } finally {
      clearInterval(timer);
    }
  }

  function toggle(path: string) {
    selected = selected.includes(path)
      ? selected.filter((p) => p !== path)
      : [...selected, path];
  }

  function toggleAll() {
    selected = allSelected ? [] : selectable.map((c) => c.path);
  }

  async function importSelected() {
    if (!selected.length) return;
    step = "importing";
    error = "";
    total = selected.length;
    done = 0;
    added = 0;
    artFailed = 0;

    for (const path of selected) {
      const candidate = candidates.find((c) => c.path === path);
      if (!candidate) continue;

      label = candidate.name_hint || candidate.path;
      try {
        const id = await addGame(path, candidate.name_hint);
        added += 1;

        if (canFetchArt) {
          label = `${candidate.name_hint} — artwork`;
          try {
            await fetchArtwork(id);
          } catch {
            // A missing cover must not drop the game from the library.
            artFailed += 1;
          }
        }
      } catch (err) {
        error = String(err);
      }
      done += 1;
    }

    step = "done";
    onImported();
  }

  function mb(bytes: number): string {
    return `${Math.round(bytes / (1024 * 1024))} MB`;
  }
</script>

{#if open}
  <div class="scrim" transition:fade={{ duration: 180 }} onclick={close} role="presentation"></div>

  <div class="wrap" role="dialog" aria-modal="true" aria-label="Add games">
    <div class="sheet glass" transition:fly={{ y: 18, duration: 240 }}>
      <header>
        <div>
          <p class="eyebrow">Add games</p>
          <h2>
            {#if step === "choose"}Choose a folder
            {:else if step === "scanning"}Scanning…
            {:else if step === "review"}Pick what to add
            {:else if step === "importing"}Adding {done + 1} of {total}
            {:else}Done{/if}
          </h2>
        </div>
        <button class="icon" aria-label="Close" onclick={close} disabled={step === "importing"}>
          <svg viewBox="0 0 12 12" width="12" height="12">
            <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.3" />
          </svg>
        </button>
      </header>

      {#if error}
        <p class="error">{error}</p>
      {/if}

      {#if step === "choose"}
        <p class="lead">
          PirateLink looks for executables up to four folders deep, ignores installers and
          redistributables, and ranks what is left. Nothing is added without your say-so.
        </p>
        <div class="row">
          <button class="btn" onclick={chooseFolder}>Choose folder…</button>
          {#if rootPath}<span class="path">{rootPath}</span>{/if}
        </div>
      {:else if step === "scanning"}
        <div class="row">
          <span class="spinner" aria-hidden="true"></span>
          <span class="path">Walking {rootPath}… {elapsed > 0 ? `(${elapsed}s)` : ""}</span>
        </div>
        <p class="note">A normal scan finishes in well under a second.</p>
      {:else if step === "review"}
        {#if candidates.length === 0}
          <p class="lead">No executables worth suggesting in that folder. Try the parent folder.</p>
          <div class="row">
            <button class="btn btn-ghost" onclick={chooseFolder}>Choose another folder</button>
          </div>
        {:else}
          <div class="toolbar">
            <button class="link" onclick={toggleAll}>
              {allSelected ? "Select none" : "Select all"}
            </button>
            <span class="count">{selected.length} of {selectable.length} selected</span>
          </div>

          <ul class="list scroll-thin">
            <!-- Keyed by index, not path: a duplicate path (junctions can produce
                 one) would throw on a keyed each and freeze the sheet mid-update. -->
            {#each candidates as candidate, i (i)}
              {@const checked = selected.includes(candidate.path)}
              <li class:disabled={candidate.already_in_library}>
                <label>
                  <input
                    type="checkbox"
                    checked={checked}
                    disabled={candidate.already_in_library}
                    onchange={() => toggle(candidate.path)}
                  />
                  <span class="text">
                    <span class="title">
                      {candidate.name_hint || candidate.path.split(/[\\/]/).pop()}
                      {#if candidate.already_in_library}<em>already added</em>{/if}
                    </span>
                    <span class="sub">{candidate.path}</span>
                    <span class="reasons">
                      {mb(candidate.size_bytes)} · {candidate.reasons.join(" · ")}
                    </span>
                  </span>
                </label>
              </li>
            {/each}
          </ul>

          <div class="row end">
            <button class="btn btn-ghost" onclick={chooseFolder}>Another folder</button>
            <button class="btn" onclick={importSelected} disabled={selected.length === 0}>
              Add {selected.length} {selected.length === 1 ? "game" : "games"}
            </button>
          </div>
        {/if}
      {:else if step === "importing"}
        <div class="progress">
          <div class="bar"><i style="width: {(done / Math.max(1, total)) * 100}%"></i></div>
          <span class="label">{label}</span>
          <span class="count">{done} / {total}</span>
        </div>
      {:else}
        <p class="lead">
          Added {added} {added === 1 ? "game" : "games"}.
          {#if artFailed > 0}
            {artFailed} had no artwork found — they are still in your library.
          {/if}
        </p>
        {#if !canFetchArt}
          <p class="note">
            No SteamGridDB key configured, so covers were skipped. Add one to
            <code>config.json</code> and re-scan to fetch them.
          </p>
        {/if}
        <div class="row end">
          <button class="btn" onclick={close}>Done</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: rgba(3, 4, 9, 0.66);
    backdrop-filter: blur(4px);
  }

  .wrap {
    position: fixed;
    inset: 0;
    z-index: 71;
    display: grid;
    place-items: center;
    padding: 32px;
    pointer-events: none;
  }

  .sheet {
    width: min(760px, 94vw);
    max-height: min(720px, 88vh);
    display: flex;
    flex-direction: column;
    padding: 22px;
    border-radius: var(--radius-panel);
    background: rgba(16, 18, 28, 0.78);
    border-color: var(--stroke-subtle);
    box-shadow: 0 40px 100px -40px rgba(0, 0, 0, 0.85);
    pointer-events: auto;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }

  .eyebrow {
    margin: 0 0 6px;
    color: var(--accent);
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    font-size: 24px;
    font-weight: 680;
    letter-spacing: -0.02em;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 1px solid var(--stroke);
    border-radius: 9px;
    background: transparent;
    color: var(--ink-dim);
    cursor: pointer;
  }

  .icon:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: var(--ink);
  }

  .icon:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .lead {
    margin: 18px 0 0;
    color: var(--ink-dim);
    font-size: 13.5px;
    line-height: 1.6;
    max-width: 62ch;
  }

  .note {
    margin: 10px 0 0;
    color: var(--ink-faint);
    font-size: 12px;
    line-height: 1.5;
  }

  .note code {
    background: rgba(255, 255, 255, 0.08);
    padding: 1px 5px;
    border-radius: 4px;
    font-size: 11.5px;
  }

  .error {
    margin: 14px 0 0;
    padding: 9px 12px;
    border-radius: 9px;
    background: rgba(217, 45, 32, 0.14);
    border: 1px solid rgba(217, 45, 32, 0.35);
    color: #ffb4a8;
    font-size: 12.5px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 22px;
  }

  .row.end {
    justify-content: flex-end;
  }

  .path {
    color: var(--ink-faint);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toolbar {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin: 18px 0 8px;
  }

  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }

  .count {
    color: var(--ink-faint);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    max-height: 380px;
    border-top: 1px solid var(--stroke);
  }

  li {
    border-bottom: 1px solid var(--stroke);
  }

  li.disabled {
    opacity: 0.5;
  }

  label {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 4px;
    cursor: pointer;
  }

  input[type="checkbox"] {
    margin-top: 3px;
    accent-color: var(--accent);
    width: 15px;
    height: 15px;
    flex: none;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .title {
    font-size: 14px;
    font-weight: 600;
  }

  .title em {
    margin-left: 8px;
    color: var(--ink-faint);
    font-size: 11px;
    font-style: normal;
    font-weight: 500;
  }

  .sub {
    color: var(--ink-faint);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .reasons {
    color: var(--ink-dim);
    font-size: 11.5px;
  }

  .progress {
    margin-top: 22px;
    display: grid;
    gap: 10px;
  }

  .bar {
    height: 4px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.09);
    overflow: hidden;
  }

  .bar i {
    display: block;
    height: 100%;
    border-radius: 999px;
    background: var(--accent);
    transition: width 200ms var(--ease-out);
  }

  .label {
    color: var(--ink-dim);
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.18);
    border-top-color: var(--accent);
    animation: spin 700ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
