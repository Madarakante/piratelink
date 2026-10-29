<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import type { Game } from "../games";
  import { credits } from "../games";

  let {
    game,
    open,
    onClose,
    onPlay,
    onRefreshArt,
    onRemove,
  }: {
    game: Game;
    open: boolean;
    onClose: () => void;
    onPlay: () => void;
    onRefreshArt?: () => void;
    onRemove?: () => void;
  } = $props();

  const lastPlayed = (value: string) =>
    !value ? "—" : /^never/i.test(value) ? "Never" : value.replace(/^played\s*/i, "");
</script>

<!--
  Details is a full page, not a floating card. A record has a back, so the page
  wipes the stage and gives the cover real size; the rail stays visible above it
  because the window is frameless and Esc is not the only way out.
-->
{#if open}
  <div
    class="wrap"
    role="dialog"
    aria-modal="true"
    aria-label="{game.title} details"
    transition:fade={{ duration: 160 }}
  >
    <div class="sheet" transition:fly={{ y: 14, duration: 240 }}>
      <header>
        <span class="label">Record</span>
        <button class="quiet" onclick={onClose}>Close — Esc</button>
      </header>

      <div class="body">
        <div class="art">
          {#if game.cover}
            <img src={game.cover} alt="" draggable="false" />
          {:else}
            <div class="art-empty" style="background: {game.accent}1c">
              <span>{game.title}</span>
            </div>
          {/if}
        </div>

        <div class="info">
          {#if game.genre}<p class="kicker">{game.genre}</p>{/if}

          <h2>{game.title}</h2>
          {#if game.subtitle}<p class="sub">{game.subtitle}</p>{/if}
          {#if game.description}<p class="desc">{game.description}</p>{/if}

          <dl class="facts">
            <div>
              <dt>Playtime</dt>
              <dd>{game.playtimeMinutes > 0 ? credits(game.playtimeMinutes) : "—"}</dd>
            </div>
            <div>
              <dt>Last played</dt>
              <dd>{lastPlayed(game.lastPlayed)}</dd>
            </div>
            {#if game.achievements.total > 0}
              <div>
                <dt>Achievements</dt>
                <dd>{game.achievements.unlocked} / {game.achievements.total}</dd>
              </div>
            {/if}
          </dl>

          {#if game.achievements.total > 0}
            <div class="progress" aria-hidden="true">
              <i style="width: {(game.achievements.unlocked / game.achievements.total) * 100}%"></i>
            </div>
          {/if}

          <div class="actions">
            <button class="btn" onclick={onPlay}>
              <svg viewBox="0 0 12 14" width="10" height="12" aria-hidden="true">
                <path d="M1 1l10 6-10 6z" fill="currentColor" />
              </svg>
              Play
            </button>
            <button class="btn btn-ghost" onclick={onClose}>Back</button>

            <span class="spacer"></span>

            {#if onRefreshArt}
              <button class="quiet" onclick={onRefreshArt} title="Re-run the SteamGridDB lookup">
                Refresh artwork
              </button>
            {/if}
            {#if onRemove}
              <button class="quiet danger" onclick={onRemove} title="Remove from library (files stay on disk)">
                Remove
              </button>
            {/if}
          </div>
        </div>
      </div>

      <!-- A footer closes the page. Without it the record floats in a void. -->
      <footer>
        <span class="path num" title={game.executable}>{game.executable || "No executable recorded"}</span>
        <span class="label">PirateLink</span>
      </footer>
    </div>
  </div>
{/if}

<style>
  .wrap {
    position: fixed;
    top: var(--rail);
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 45;
    overflow: hidden;
    background:
      radial-gradient(58% 52% at 22% 34%, var(--accent-soft), transparent 66%),
      var(--bg-0);
  }

  .sheet {
    display: grid;
    grid-template-rows: 52px minmax(0, 1fr) 48px;
    height: 100%;
    padding: 0 var(--gutter);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--line);
  }

  .body {
    display: grid;
    grid-template-columns: minmax(200px, 320px) minmax(0, 1fr);
    gap: 64px;
    align-items: start;
    overflow-y: auto;
    padding: 36px 0 28px;
  }

  .art {
    overflow: hidden;
    background: #0a0b0f;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
  }

  .art img {
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    object-fit: cover;
  }

  .art-empty {
    display: flex;
    align-items: flex-end;
    aspect-ratio: 2 / 3;
    padding: 22px;
    color: rgba(238, 240, 244, 0.72);
    font-size: 19px;
    font-weight: 650;
    line-height: 1.15;
    text-wrap: balance;
  }

  .info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    max-width: 700px;
    padding-top: 4px;
  }

  .kicker {
    margin: 0 0 14px;
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    font-size: clamp(32px, 4.2vw, 60px);
    font-weight: 720;
    letter-spacing: -0.045em;
    line-height: 0.96;
    text-wrap: balance;
  }

  .sub {
    margin: 12px 0 0;
    color: var(--ink-dim);
    font-size: 14px;
    font-weight: 500;
  }

  .desc {
    margin: 22px 0 0;
    max-width: 54ch;
    color: var(--ink-dim);
    font-size: 14px;
    line-height: 1.7;
  }

  .facts {
    display: flex;
    margin: 32px 0 0;
    padding: 16px 0 0;
    border-top: 1px solid var(--line);
  }

  .facts > div {
    margin-right: 34px;
    padding-right: 34px;
    border-right: 1px solid var(--line);
  }

  .facts > div:last-child {
    margin-right: 0;
    padding-right: 0;
    border-right: 0;
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
    margin: 8px 0 0;
    font-size: 15px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .progress {
    position: relative;
    height: 1px;
    margin-top: 20px;
    background: var(--line);
  }

  .progress i {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--accent);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 36px;
  }

  .spacer {
    flex: 1;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    border-top: 1px solid var(--line);
  }

  .path {
    overflow: hidden;
    color: var(--ink-ghost);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 900px) {
    .body {
      grid-template-columns: 1fr;
      gap: 28px;
    }
    .art {
      max-width: 220px;
    }
  }
</style>
