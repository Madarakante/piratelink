<script lang="ts">
  import { fade } from "svelte/transition";
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
</script>

{#if open}
  <div class="scrim" transition:fade={{ duration: 200 }} onclick={onClose} role="presentation"></div>

  <div class="wrap" role="dialog" aria-modal="true" aria-label="{game.title} details">
    <div class="panel glass" transition:fade={{ duration: 220 }}>
      <div class="art">
        {#if game.cover}
          <img src={game.cover} alt="" draggable="false" />
        {:else}
          <div class="art-empty" style="background: {game.accent}22">
            <span>{game.title}</span>
          </div>
        {/if}
        <div class="art-fade"></div>
      </div>

      <div class="body">
        {#if game.genre}<p class="eyebrow">{game.genre}</p>{/if}
        <h2>{game.title}</h2>
        {#if game.subtitle}<p class="sub">{game.subtitle}</p>{/if}
        {#if game.description}<p class="desc">{game.description}</p>{/if}

        <div class="stats">
          <div class="stat">
            <span class="k">Playtime</span>
            <span class="v">{credits(game.playtimeMinutes)}</span>
          </div>
          <div class="stat">
            <span class="k">Last played</span>
            <span class="v">{game.lastPlayed.replace(/^Played\s*/, "")}</span>
          </div>
          {#if game.achievements.total > 0}
            <div class="stat">
              <span class="k">Achievements</span>
              <span class="v">{game.achievements.unlocked}/{game.achievements.total}</span>
            </div>
          {/if}
        </div>

        {#if game.achievements.total > 0}
          <div class="progress" aria-hidden="true">
            <i style="width: {(game.achievements.unlocked / game.achievements.total) * 100}%"></i>
          </div>
        {/if}

        <div class="actions">
          <button class="btn" onclick={onPlay}>
            <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
              <path d="M8 5.5v13l11-6.5z" fill="currentColor" />
            </svg>
            Play
          </button>
          <button class="btn btn-ghost" onclick={onClose}>Back</button>

          <span class="spacer"></span>

          {#if onRefreshArt}
            <button class="mini" onclick={onRefreshArt} title="Re-run the SteamGridDB lookup">
              Refresh artwork
            </button>
          {/if}
          {#if onRemove}
            <button class="mini danger" onclick={onRemove} title="Remove from library (files stay on disk)">
              Remove
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgba(3, 4, 9, 0.62);
    backdrop-filter: blur(3px);
  }

  .wrap {
    position: fixed;
    inset: 0;
    z-index: 41;
    display: grid;
    place-items: center;
    padding: 32px;
    pointer-events: none;
  }

  .panel {
    display: grid;
    grid-template-columns: 250px minmax(0, 1fr);
    gap: 26px;
    width: min(880px, 92vw);
    padding: 22px;
    border-radius: var(--radius-panel);
    background: rgba(14, 16, 24, 0.72);
    border-color: var(--stroke-strong);
    box-shadow: 0 50px 120px -40px rgba(0, 0, 0, 0.95);
    pointer-events: auto;
  }

  .art {
    position: relative;
    border-radius: 14px;
    overflow: hidden;
    box-shadow: 0 20px 50px -22px rgba(0, 0, 0, 0.9);
  }

  .art img {
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    object-fit: cover;
  }

  .art-empty {
    display: grid;
    place-items: center;
    width: 100%;
    aspect-ratio: 2 / 3;
    padding: 32px 24px;
    color: rgba(244, 246, 255, 0.7);
    font-size: 18px;
    font-weight: 600;
    text-align: center;
    text-wrap: balance;
    background: radial-gradient(at 30% 20%, var(--accent-soft), transparent 50%),
      radial-gradient(at 70% 80%, var(--accent-subtle), transparent 50%);
  }

  .art-fade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, transparent 55%, rgba(10, 12, 18, 0.55));
  }

  .body {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 6px 8px 4px 0;
  }

  .eyebrow {
    margin: 0 0 6px;
    color: var(--accent);
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    font-size: clamp(28px, 3.2vw, 40px);
    font-weight: 680;
    letter-spacing: -0.025em;
    line-height: 1.04;
  }

  .sub {
    margin: 8px 0 0;
    color: var(--ink-dim);
    font-size: 14px;
    font-weight: 500;
  }

  .desc {
    margin: 18px 0 0;
    max-width: 52ch;
    color: var(--ink-dim);
    font-size: 13.8px;
    line-height: 1.62;
  }

  .stats {
    display: flex;
    gap: 26px;
    margin: 22px 0 0;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .stat .k {
    color: var(--ink-faint);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .stat .v {
    font-size: 14.5px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .progress {
    position: relative;
    height: 3px;
    margin-top: 18px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.09);
    overflow: hidden;
  }

  .progress i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 999px;
    background: var(--accent);
    box-shadow: 0 0 14px var(--accent);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: auto;
    padding-top: 26px;
  }


  .spacer {
    flex: 1;
  }

  .mini {
    border: 0;
    background: none;
    padding: 6px 8px;
    border-radius: 8px;
    color: var(--ink-faint);
    font: inherit;
    font-size: 11.5px;
    font-weight: 550;
    cursor: pointer;
    transition: background var(--dur-1) linear, color var(--dur-1) linear;
  }

  .mini:hover {
    background: rgba(255, 255, 255, 0.07);
    color: var(--ink);
  }

  .mini.danger:hover {
    background: rgba(217, 45, 32, 0.16);
    color: #ff9b8a;
  }

  @media (max-width: 760px) {
    .panel {
      grid-template-columns: 1fr;
    }
    .art {
      display: none;
    }
  }
</style>
