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

  const stepLabel = $derived(
    step === "review"
      ? "Step 2 — Review"
      : step === "importing"
        ? "Step 3 — Import"
        : step === "done"
          ? "Complete"
          : "Step 1 — Folder",
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
  <div class="scrim" transition:fade={{ duration: 160 }} onclick={close} role="presentation"></div>

  <div class="wrap" role="dialog" aria-modal="true" aria-label="Add games">
    <div class="sheet" transition:fly={{ y: 14, duration: 220 }}>
      <header>
        <div class="titles">
          <p class="label">{stepLabel}</p>
          <h2>
            {#if step === "choose"}Choose a folder
            {:else if step === "scanning"}Scanning
            {:else if step === "review"}Pick what to add
            {:else if step === "importing"}Adding {done + 1} of {total}
            {:else}Done{/if}
          </h2>
        </div>
        <button class="close" aria-label="Close" onclick={close} disabled={step === "importing"}>
          <svg viewBox="0 0 12 12" width="11" height="11">
            <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </button>
      </header>

      <div class="content">
        {#if error}
          <p class="error num">{error}</p>
        {/if}

        {#if step === "choose"}
          <p class="lead">
            PirateLink looks for executables up to four folders deep, ignores installers and
            redistributables, and ranks what is left. Nothing is added without your say-so.
          </p>
          <div class="row">
            <button class="btn" onclick={chooseFolder}>Choose folder</button>
            {#if rootPath}<span class="path num">{rootPath}</span>{/if}
          </div>
        {:else if step === "scanning"}
          <div class="row">
            <span class="spinner" aria-hidden="true"></span>
            <span class="path num">Walking {rootPath}{elapsed > 0 ? ` (${elapsed}s)` : ""}</span>
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
              <button class="quiet" onclick={toggleAll}>
                {allSelected ? "Select none" : "Select all"}
              </button>
              <span class="label">{selected.length} of {selectable.length} selected</span>
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
                      <span class="sub num">{candidate.path}</span>
                      <span class="reasons num">
                        {mb(candidate.size_bytes)} · {candidate.reasons.join(" · ")}
                      </span>
                    </span>
                  </label>
                </li>
              {/each}
            </ul>

            <div class="row end">
              <button class="quiet" onclick={chooseFolder}>Another folder</button>
              <button class="btn" onclick={importSelected} disabled={selected.length === 0}>
                Add {selected.length} {selected.length === 1 ? "game" : "games"}
              </button>
            </div>
          {/if}
        {:else if step === "importing"}
          <div class="progress">
            <div class="bar"><i style="width: {(done / Math.max(1, total)) * 100}%"></i></div>
            <div class="bar-row">
              <span class="path num">{label}</span>
              <span class="label">{done} / {total}</span>
            </div>
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
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: rgba(3, 4, 7, 0.84);
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
    display: flex;
    flex-direction: column;
    width: min(720px, 94vw);
    max-height: min(700px, 86vh);
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 3px;
    box-shadow: 0 50px 130px -50px #000;
    pointer-events: auto;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 20px 24px 18px;
    border-bottom: 1px solid var(--line);
  }

  .titles {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h2 {
    margin: 0;
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.035em;
    line-height: 1.05;
  }

  .close {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    flex: none;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
    transition:
      background var(--dur-1) linear,
      color var(--dur-1) linear;
  }

  .close:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: var(--ink);
  }

  .close:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .content {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 22px 24px 24px;
  }

  .lead {
    margin: 0;
    max-width: 62ch;
    color: var(--ink-dim);
    font-size: 13.5px;
    line-height: 1.65;
  }

  .note {
    margin: 12px 0 0;
    color: var(--ink-ghost);
    font-size: 11.5px;
    line-height: 1.55;
  }

  .note code {
    padding: 1px 5px;
    background: rgba(255, 255, 255, 0.07);
    border-radius: 2px;
    font-family: var(--font-mono);
    font-size: 10.5px;
  }

  .error {
    margin: 0 0 16px;
    padding: 9px 11px;
    border-left: 2px solid #d92d20;
    background: rgba(217, 45, 32, 0.1);
    color: #ffb4a8;
    font-size: 10.5px;
    line-height: 1.6;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 22px;
    min-width: 0;
  }

  .row.end {
    justify-content: flex-end;
  }

  .path {
    overflow: hidden;
    color: var(--ink-faint);
    font-size: 10.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 20px 0 4px;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    max-height: 340px;
    border-top: 1px solid var(--line);
  }

  li {
    border-bottom: 1px solid var(--line);
  }

  li.disabled {
    opacity: 0.42;
  }

  label {
    display: flex;
    align-items: flex-start;
    gap: 13px;
    padding: 12px 2px;
    cursor: pointer;
  }

  input[type="checkbox"] {
    width: 14px;
    height: 14px;
    margin-top: 3px;
    flex: none;
    accent-color: var(--ink);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .title {
    font-size: 13.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .title em {
    margin-left: 9px;
    color: var(--ink-ghost);
    font-family: var(--font-mono);
    font-size: 9px;
    font-style: normal;
    font-weight: 500;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .sub,
  .reasons {
    overflow: hidden;
    color: var(--ink-ghost);
    font-size: 10.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .reasons {
    color: var(--ink-faint);
  }

  .progress {
    display: grid;
    gap: 12px;
    margin-top: 22px;
  }

  .bar {
    height: 1px;
    background: var(--line);
  }

  .bar i {
    display: block;
    height: 100%;
    background: var(--ink);
    transition: width 200ms var(--ease-out);
  }

  .bar-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-width: 0;
  }

  .bar-row .path {
    color: var(--ink-dim);
  }

  .spinner {
    width: 13px;
    height: 13px;
    flex: none;
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
</style>
