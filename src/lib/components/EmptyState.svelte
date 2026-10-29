<script lang="ts">
  let { onOpen }: { onOpen: () => void } = $props();

  const steps = [
    "Pick the game's .exe — one file per game",
    "PirateLink names it from the install folder, not the file name",
    "Fix the title if it guessed wrong, then add it and fetch the cover",
  ];
</script>

<div class="empty">
  <div class="grid">
    <div class="intro">
      <h2>Nothing on<br />the shelf</h2>
      <p class="lead">
        Point PirateLink at a game's executable. It reads the title from the folder the game is
        installed in — a file name like <code>eldenring.exe</code> is not a name — and you can
        correct it before anything is added.
      </p>

      <button class="btn" onclick={onOpen}>
        Add a game
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M1 6h10M7 1.6L11.4 6 7 10.4" stroke="currentColor" stroke-width="1.4" fill="none" />
        </svg>
      </button>

      <p class="note">
        No telemetry. The path stays on your machine; only titles you confirm are sent to
        SteamGridDB.
      </p>
    </div>

    <!-- The steps sit in their own column so the screen is a spread, not a pile. -->
    <div class="how">
      <p class="label head">How it works</p>
      <ol class="steps">
        {#each steps as step, i}
          <li><span class="idx num">{String(i + 1).padStart(2, "0")}</span><span>{step}</span></li>
        {/each}
      </ol>
    </div>
  </div>
</div>

<style>
  .empty {
    display: flex;
    align-items: center;
    height: 100%;
    animation: rise 380ms var(--ease-out) both;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }

  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 0.78fr);
    gap: 96px;
    align-items: start;
    width: 100%;
  }

  h2 {
    margin: 0;
    font-size: clamp(34px, 4.4vw, 60px);
    font-weight: 720;
    letter-spacing: -0.045em;
    line-height: 0.94;
    text-wrap: balance;
  }

  .lead {
    margin: 20px 0 0;
    max-width: 42ch;
    color: var(--ink-dim);
    font-size: 14px;
    line-height: 1.65;
  }

  .btn {
    margin-top: 30px;
  }

  .note {
    margin: 22px 0 0;
    max-width: 46ch;
    color: var(--ink-ghost);
    font-size: 11.5px;
    line-height: 1.6;
  }

  .how {
    padding-top: 10px;
  }

  .head {
    margin: 0 0 14px;
  }

  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .steps li {
    display: flex;
    align-items: baseline;
    gap: 16px;
    padding: 13px 0;
    border-top: 1px solid var(--line);
    color: var(--ink-dim);
    font-size: 13px;
    line-height: 1.45;
  }

  .steps li:last-child {
    border-bottom: 1px solid var(--line);
  }

  .idx {
    flex: none;
    width: 22px;
    color: var(--ink-ghost);
    font-size: 9.5px;
    letter-spacing: 0.14em;
  }

  @media (max-width: 1000px) {
    .grid {
      grid-template-columns: 1fr;
      gap: 40px;
      align-items: start;
    }
  }
</style>
