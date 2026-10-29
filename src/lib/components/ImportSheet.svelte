<script lang="ts">
  import { fly, fade } from "svelte/transition";
  import {
    addGame,
    fetchArtwork,
    inspectFile,
    isTauri,
    pickGameFiles,
    type GameFileInfo,
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

  type Step = "choose" | "review" | "importing" | "done";
  /** `suggested` keeps the derived name, so clearing the field cannot fall back
   *  to the filename. */
  type Entry = GameFileInfo & { include: boolean; suggested: string };

  let step = $state<Step>("choose");
  let entries = $state<Entry[]>([]);
  let error = $state("");
  let done = $state(0);
  let total = $state(0);
  let label = $state("");
  let added = $state(0);
  let artFailed = $state(0);

  const selectable = $derived(entries.filter((entry) => !entry.already_in_library));
  const chosen = $derived(selectable.filter((entry) => entry.include));
  const allSelected = $derived(selectable.length > 0 && chosen.length === selectable.length);

  const stepLabel = $derived(
    step === "review"
      ? "Step 2 — Review"
      : step === "importing"
        ? "Step 3 — Add"
        : step === "done"
          ? "Complete"
          : "Step 1 — Game file",
  );

  function reset() {
    step = "choose";
    entries = [];
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

  /**
   * Picking the .exe means the title has to come from somewhere, and the file
   * name is usually the worst possible answer — `eldenring.exe` is not a game
   * name. The backend names it after the install folder instead, and every entry
   * lands here as an editable field, so a bad guess costs a correction rather
   * than a wrong SteamGridDB match.
   */
  async function chooseFiles() {
    error = "";
    let paths: string[] = [];
    try {
      paths = await pickGameFiles();
    } catch (err) {
      error = String(err);
      return;
    }
    if (!paths.length) return;

    const fresh: Entry[] = [];
    for (const path of paths) {
      if (entries.some((entry) => entry.path === path)) continue;
      try {
        const info = await inspectFile(path);
        fresh.push({ ...info, include: !info.already_in_library, suggested: info.title });
      } catch (err) {
        error = `${path} — ${err instanceof Error ? err.message : String(err)}`;
      }
    }

    if (!fresh.length) return;
    entries = [...entries, ...fresh];
    step = "review";
  }

  function toggleAll() {
    const next = !allSelected;
    for (const entry of selectable) entry.include = next;
  }

  function remove(path: string) {
    entries = entries.filter((entry) => entry.path !== path);
    if (!entries.length) step = "choose";
  }

  async function addSelected() {
    if (!chosen.length) return;
    step = "importing";
    error = "";
    total = chosen.length;
    done = 0;
    added = 0;
    artFailed = 0;

    for (const entry of chosen) {
      const title = entry.title.trim() || entry.suggested || "Untitled";
      label = title;
      try {
        const id = await addGame(entry.path, title);
        added += 1;

        if (canFetchArt) {
          label = `${title} — artwork`;
          try {
            await fetchArtwork(id);
          } catch {
            // A missing cover must not drop the game from the library.
            artFailed += 1;
          }
        }
      } catch (err) {
        error = `${title} — ${err instanceof Error ? err.message : String(err)}`;
      }
      done += 1;
    }

    step = "done";
    onImported();
  }

  /** A ROM is 8-20 GB, so megabytes stop being readable above a gigabyte. */
  function mb(bytes: number): string {
    if (!bytes) return "-";
    const mb = bytes / (1024 * 1024);
    if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
    return `${Math.round(mb)} MB`;
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
            {#if step === "choose"}Choose a game
            {:else if step === "review"}Name what you picked
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
            Point at the game's <code>.exe</code>, or at a Switch dump
            (<code>.nsp</code>, <code>.xci</code>, <code>.nro</code>). Executable filenames are
            often useless — <code>eldenring.exe</code>, <code>hl2.exe</code>,
            <code>Cyberpunk2077.exe</code> — so the title comes from the folder the game is
            installed in; a ROM is named after the game, so its filename is cleaned of ids
            instead. Either way you can correct it before it is added.
          </p>
          <div class="row">
            <button class="btn" onclick={chooseFiles}>Choose game file</button>
            <span class="note">
              {#if isTauri}
                Pick one, or several at once.
              {:else}
                Browser preview — these paths are demo data, so the dialog is simulated.
                Run the built app to pick real files.
              {/if}
            </span>
          </div>
        {:else if step === "review"}
          <div class="toolbar">
            {#if selectable.length}
              <button class="quiet" onclick={toggleAll}>
                {allSelected ? "Select none" : "Select all"}
              </button>
              <span class="label">{chosen.length} of {selectable.length} selected</span>
            {:else}
              <span class="label empty">Already in your library — nothing to add</span>
            {/if}
          </div>

          <ul class="list scroll-thin">
            {#each entries as entry (entry.path)}
              <li class:disabled={entry.already_in_library} class:off={!entry.include}>
                <label class="check">
                  <input
                    type="checkbox"
                    bind:checked={entry.include}
                    disabled={entry.already_in_library}
                    aria-label="Include {entry.title}"
                  />
                </label>
                <div class="text">
                  <input
                    class="name"
                    bind:value={entry.title}
                    disabled={entry.already_in_library}
                    aria-label="Title for {entry.path}"
                    spellcheck="false"
                    placeholder="Title"
                  />
                  <span class="sub num">{entry.path}</span>
                  <span class="reasons num">
                    {mb(entry.size_bytes)}
                    {#if entry.platform === "switch"}<em class="switch">switch</em>{/if}
                    {#if entry.already_in_library}<em>already added</em>{/if}
                  </span>
                </div>
                {#if !entry.already_in_library}
                  <button class="drop" aria-label="Remove {entry.title}" onclick={() => remove(entry.path)}>
                    <svg viewBox="0 0 12 12" width="9" height="9">
                      <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" />
                    </svg>
                  </button>
                {/if}
              </li>
            {/each}
          </ul>

          <div class="row end footer">
            <button class="quiet" onclick={chooseFiles}>Another file</button>
            <button class="btn" onclick={addSelected} disabled={chosen.length === 0}>
              {#if chosen.length}
                Add {chosen.length} {chosen.length === 1 ? "game" : "games"}
              {:else}
                Add games
              {/if}
            </button>
          </div>
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
              <code>config.json</code> and use Refresh artwork on the record page.
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

  /* `flex: 1` keeps the review list scrollable inside the capped sheet while the
     footer stays put. */
  .content {
    display: flex;
    flex: 1;
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

  .row .note {
    margin: 0;
  }

  .lead code,
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

  .row .btn {
    flex: none;
    white-space: nowrap;
  }

  .row .note {
    flex: 1 1 auto;
    max-width: 46ch;
    margin: 0;
  }

  .row.footer {
    margin-top: auto;
    padding-top: 20px;
  }

  .row.end {
    justify-content: flex-end;
  }

  /* The review step carries a secondary action on the left. */
  .row.end.footer {
    justify-content: space-between;
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

  .toolbar .label.empty {
    margin-left: auto;
  }

  .list {
    list-style: none;
    flex: 1 1 auto;
    min-height: 0;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    border-top: 1px solid var(--line);
  }

  li {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) 18px;
    align-items: start;
    gap: 13px;
    padding: 12px 2px;
    border-bottom: 1px solid var(--line);
  }

  li.disabled {
    opacity: 0.42;
  }

  li.off .text {
    opacity: 0.45;
  }

  .check {
    padding-top: 4px;
    cursor: pointer;
  }

  input[type="checkbox"] {
    width: 14px;
    height: 14px;
    margin: 0;
    accent-color: var(--ink);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  /* The title is a field, not a label: renaming is the whole point of this step. */
  .name {
    width: auto;
    max-width: 100%;
    field-sizing: content;
    padding: 1px 0 3px;
    border: 0;
    border-bottom: 1px solid var(--line);
    background: transparent;
    color: var(--ink);
    font: inherit;
    font-size: 13.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
    transition: border-color var(--dur-1) linear;
  }

  .name:hover:not(:disabled) {
    border-bottom-color: var(--line-strong);
  }

  .name:focus {
    outline: none;
    border-bottom-color: var(--accent);
  }

  .name::placeholder {
    color: var(--ink-ghost);
  }

  .drop {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    margin-top: 2px;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--ink-ghost);
    cursor: pointer;
    opacity: 0;
    transition:
      opacity var(--dur-1) linear,
      background var(--dur-1) linear,
      color var(--dur-1) linear;
  }

  li:hover .drop,
  .drop:focus-visible {
    opacity: 1;
  }

  .drop:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--ink);
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

  .reasons em {
    margin-left: 9px;
    color: var(--ink-ghost);
    font-family: var(--font-mono);
    font-size: 9px;
    font-style: normal;
    font-weight: 500;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  /* A boxed label, like a format stamp on a spine — not a coloured chip. */
  .reasons em.switch {
    padding: 1px 4px 0;
    border: 1px solid var(--line);
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
</style>
