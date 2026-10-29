<script lang="ts">
  let { onOpen }: { onOpen: () => void } = $props();

  const steps = [
    "Pick the folder your games live in",
    "PirateLink scans for executables and shows what it found",
    "Choose which ones to add, then fetch their covers",
  ];
</script>

<div class="empty">
  <div class="inner">
    <p class="eyebrow">Library</p>
    <h2>Nothing on the shelf yet</h2>
    <p class="lead">
      Point PirateLink at a folder once. It ranks the executables it finds and asks before
      adding anything.
    </p>

    <ol class="steps">
      {#each steps as step, i}
        <li><span class="num">{i + 1}</span><span>{step}</span></li>
      {/each}
    </ol>

    <button class="btn" onclick={onOpen}>
      <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
        <path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" fill="none" />
      </svg>
      Add a games folder
    </button>

    <p class="note">
      No telemetry. The folder stays on your machine; only titles you confirm are sent to
      SteamGridDB.
    </p>
  </div>
</div>

<style>
  .empty {
    display: grid;
    place-items: center;
    height: 100%;
    padding: 0 32px;
    animation: rise 420ms var(--ease-out) both;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
  }

  .inner {
    max-width: 560px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    text-align: left;
  }

  .eyebrow {
    margin: 0 0 14px;
    color: var(--accent);
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.22em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    font-size: clamp(30px, 3.6vw, 46px);
    font-weight: 690;
    letter-spacing: -0.035em;
    line-height: 1.02;
  }

  .lead {
    margin: 14px 0 0;
    max-width: 46ch;
    color: var(--ink-dim);
    font-size: 15px;
    line-height: 1.6;
  }

  .steps {
    list-style: none;
    margin: 28px 0 30px;
    padding: 0;
    display: grid;
    gap: 12px;
  }

  .steps li {
    display: flex;
    align-items: baseline;
    gap: 12px;
    color: var(--ink-dim);
    font-size: 13.5px;
  }

  .num {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 50%;
    border: 1px solid var(--stroke-strong);
    color: var(--ink);
    font-size: 11px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .note {
    margin: 20px 0 0;
    color: var(--ink-faint);
    font-size: 11.5px;
    line-height: 1.5;
  }
</style>
