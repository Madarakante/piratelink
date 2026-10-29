<script lang="ts">
  import { fade } from "svelte/transition";
  import { untrack } from "svelte";
  import type { Game } from "../games";
  import {
    applyArtwork,
    artworkChoices,
    searchArtwork,
    type ArtChoices,
    type ArtGame,
    type ArtOption,
  } from "../library";

  let {
    open,
    game,
    canFetchArt,
    onClose,
    onApplied,
  }: {
    open: boolean;
    game: Game;
    canFetchArt: boolean;
    onClose: () => void;
    onApplied: (slot: "cover" | "hero") => void;
  } = $props();

  let query = $state("");
  let results = $state<ArtGame[]>([]);
  let picked = $state<ArtGame | null>(null);
  let choices = $state<ArtChoices | null>(null);
  let searching = $state(false);
  let loadingChoices = $state(false);
  let applying = $state(0);
  let error = $state("");
  let applied = $state<{ cover: number | null; hero: number | null }>({
    cover: null,
    hero: null,
  });

  /** A search is identified by its term, so a slow reply cannot overwrite a
   *  newer one. */
  let searchToken = 0;
  let choiceToken = 0;
  /** Plain flag, not `$state`: the reset must fire on *open*, not when the game
   *  prop is replaced by a library reload after applying art. */
  let opened = false;

  $effect(() => {
    if (!open) {
      opened = false;
      return;
    }
    if (opened) return;
    opened = true;

    untrack(() => {
      // Opening is a fresh visit: the previous game's search must not linger.
      query = game.title;
      results = [];
      picked = null;
      choices = null;
      error = "";
      applied = { cover: null, hero: null };
      if (canFetchArt) void runSearch(game.title);
    });
  });

  async function runSearch(term: string) {
    const token = ++searchToken;
    searching = true;
    error = "";
    try {
      const hits = await searchArtwork(term);
      if (token !== searchToken) return;
      results = hits;
      if (hits.length === 0) {
        error = `SteamGridDB has nothing for “${term.trim()}”. Try the console or regional title.`;
        return;
      }
      // Show the first hit straight away; the list is there to correct it.
      await pick(hits[0]);
    } catch (err) {
      if (token === searchToken) error = String(err);
    } finally {
      if (token === searchToken) searching = false;
    }
  }

  async function pick(candidate: ArtGame) {
    const token = ++choiceToken;
    picked = candidate;
    choices = null;
    applied = { cover: null, hero: null };
    loadingChoices = true;
    error = "";
    try {
      const next = await artworkChoices(candidate.id);
      if (token !== choiceToken) return;
      choices = next;
      if (next.covers.length === 0 && next.heroes.length === 0) {
        error = `“${candidate.name}” has no artwork on SteamGridDB.`;
      }
    } catch (err) {
      if (token === choiceToken) error = String(err);
    } finally {
      if (token === choiceToken) loadingChoices = false;
    }
  }

  async function use(
    slot: "cover" | "hero",
    option: ArtOption,
  ) {
    if (game.dbId == null || !picked) return;
    applying = option.id;
    error = "";
    try {
      await applyArtwork(
        game.dbId,
        picked.id,
        slot === "cover" ? option.id : null,
        slot === "hero" ? option.id : null,
      );
      applied = { ...applied, [slot]: option.id };
      onApplied(slot);
    } catch (err) {
      error = String(err);
    } finally {
      applying = 0;
    }
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (query.trim().length < 2) return;
    void runSearch(query);
  }
</script>

<!--
  Manual artwork picker.

  Two columns, like a catalogue index: the search and its matches on the left, the
  artwork the entry offers on the right. A click applies, because a preview that
  needs a second confirm is slower than trying the one next to it.
-->
{#if open}
  <div
    class="wrap"
    role="dialog"
    aria-modal="true"
    aria-label="Artwork for {game.title}"
    transition:fade={{ duration: 160 }}
  >
    <header>
      <span class="label">Artwork</span>
      <span class="target">{game.title}</span>
      <button class="quiet" onclick={onClose}>Close — Esc</button>
    </header>

    <div class="body">
      <aside>
        <form onsubmit={submit}>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="search"
            bind:value={query}
            placeholder="Game title on SteamGridDB"
            spellcheck="false"
            autocomplete="off"
            onkeydown={(event) => event.stopPropagation()}
          />
          <button class="btn btn-ghost" type="submit" disabled={!canFetchArt || searching}>
            {searching ? "Searching" : "Search"}
          </button>
        </form>

        <p class="hint">
          {#if !canFetchArt}
            No SteamGridDB key. Add <code>steamgriddb_api_key</code> to
            <code>config.json</code> in the app data folder.
          {:else}
            Pick the entry SteamGridDB has for the game — titles are not always spelled
            the way the folder is.
          {/if}
        </p>

        <ul class="matches">
          {#each results as candidate (candidate.id)}
            <li>
              <button
                class="match"
                class:on={picked?.id === candidate.id}
                onclick={() => pick(candidate)}
                disabled={applying !== 0}
              >
                <span class="name">{candidate.name}</span>
                {#if candidate.verified}<span class="label verified">verified</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      </aside>

      <section class="assets">
        {#if error}
          <p class="error num">{error}</p>
        {/if}

        {#if loadingChoices}
          <p class="status label">Looking up artwork</p>
        {:else if choices}
          {#if choices.covers.length}
            <div class="group">
              <h3 class="label">Covers <span class="count">{choices.covers.length}</span></h3>
              <div class="covers">
                {#each choices.covers as option (option.id)}
                  <button
                    class="tile"
                    class:busy={applying === option.id}
                    class:on={applied.cover === option.id}
                    onclick={() => use("cover", option)}
                    disabled={applying !== 0}
                    title="{option.width}×{option.height}"
                  >
                    <img src={option.thumb} alt="" draggable="false" loading="lazy" />
                    {#if applied.cover === option.id}<span class="stamp">in use</span>{/if}
                  </button>
                {/each}
              </div>
            </div>
          {/if}

          {#if choices.heroes.length}
            <div class="group">
              <h3 class="label">Heroes <span class="count">{choices.heroes.length}</span></h3>
              <div class="heroes">
                {#each choices.heroes as option (option.id)}
                  <button
                    class="tile wide"
                    class:busy={applying === option.id}
                    class:on={applied.hero === option.id}
                    onclick={() => use("hero", option)}
                    disabled={applying !== 0}
                    title="{option.width}×{option.height}"
                  >
                    <img src={option.thumb} alt="" draggable="false" loading="lazy" />
                    {#if applied.hero === option.id}<span class="stamp">in use</span>{/if}
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        {:else}
          <p class="status label">
            {canFetchArt ? "Search to load covers and heroes" : "Artwork needs a SteamGridDB key"}
          </p>
        {/if}
      </section>
    </div>

    <footer>
      <span class="label">SteamGridDB</span>
      <span class="note">Click a cover or a hero to apply it. Files stay in the local cache.</span>
    </footer>
  </div>
{/if}

<style>
  .wrap {
    position: fixed;
    top: var(--rail);
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 55;
    display: grid;
    grid-template-rows: 52px minmax(0, 1fr) 44px;
    padding: 0 var(--gutter);
    background: var(--bg-0);
  }

  header,
  footer {
    display: flex;
    align-items: center;
    gap: 18px;
    border-bottom: 1px solid var(--line);
  }

  footer {
    justify-content: space-between;
    border-top: 1px solid var(--line);
    border-bottom: 0;
  }

  .target {
    flex: 1;
    overflow: hidden;
    color: var(--ink-dim);
    font-size: 12px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .body {
    display: grid;
    grid-template-columns: 268px minmax(0, 1fr);
    gap: 44px;
    overflow: hidden;
    padding: 28px 0 24px;
  }

  aside {
    display: flex;
    flex-direction: column;
    min-height: 0;
    gap: 14px;
  }

  form {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .search {
    flex: 1;
    min-width: 0;
    padding: 0 0 6px;
    border: 0;
    border-bottom: 1px solid var(--line);
    border-radius: 0;
    background: transparent;
    color: var(--ink);
    font: 500 13px/1.4 var(--font-display);
  }

  .search::placeholder {
    color: var(--ink-ghost);
  }

  .search:focus {
    outline: none;
    border-bottom-color: var(--accent);
  }

  form .btn {
    height: 30px;
    padding: 0 12px;
    font-size: 9.5px;
  }

  .hint {
    margin: 0;
    color: var(--ink-ghost);
    font-size: 11px;
    line-height: 1.6;
  }

  .hint code {
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .matches {
    overflow-y: auto;
    margin: 4px 0 0;
    padding: 0;
    list-style: none;
    scrollbar-width: thin;
  }

  .match {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    width: 100%;
    padding: 9px 10px 9px 0;
    border: 0;
    border-top: 1px solid var(--line);
    background: none;
    color: var(--ink-dim);
    font: 500 12.5px/1.3 var(--font-display);
    text-align: left;
    cursor: pointer;
  }

  .match:hover {
    color: var(--ink);
  }

  .match:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .match.on {
    color: var(--ink);
  }

  .match.on .name {
    text-decoration: underline;
    text-decoration-color: var(--accent);
    text-underline-offset: 4px;
  }

  .match .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .verified {
    flex: none;
    font-size: 8.5px;
    letter-spacing: 0.14em;
  }

  .assets {
    overflow-y: auto;
    padding-right: 6px;
    scrollbar-width: thin;
  }

  .group + .group {
    margin-top: 30px;
  }

  .group h3 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0 0 12px;
    color: var(--ink-faint);
  }

  .count {
    color: var(--ink-ghost);
    font-size: 9px;
  }

  .covers {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: 10px;
  }

  .heroes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(232px, 1fr));
    gap: 10px;
  }

  .tile {
    position: relative;
    display: block;
    overflow: hidden;
    padding: 0;
    border: 1px solid var(--line);
    background: #0a0b0f;
    cursor: pointer;
    transition:
      border-color var(--dur-1) linear,
      opacity var(--dur-1) linear;
  }

  .tile img {
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    object-fit: cover;
  }

  .tile.wide img {
    aspect-ratio: 3 / 1;
  }

  .tile:hover:not(:disabled) {
    border-color: var(--line-strong);
  }

  .tile:disabled {
    cursor: default;
  }

  .tile.on {
    border-color: var(--ink);
    cursor: default;
  }

  .tile.busy {
    opacity: 0.45;
  }

  .stamp {
    position: absolute;
    top: 0;
    left: 0;
    padding: 3px 5px 2px;
    background: var(--ink);
    color: #07080a;
    font: 600 8.5px/1 var(--font-mono);
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .status {
    margin: 0;
    color: var(--ink-ghost);
  }

  .error {
    margin: 0 0 18px;
    max-width: 60ch;
    color: #ff9b8a;
    font-size: 11px;
    line-height: 1.6;
  }

  .note {
    color: var(--ink-ghost);
    font-size: 10.5px;
  }

  @media (max-width: 980px) {
    .body {
      grid-template-columns: 1fr;
      grid-template-rows: auto minmax(0, 1fr);
      gap: 20px;
    }
    .matches {
      max-height: 108px;
    }
  }
</style>
