<script lang="ts">
  let { gamepad = false }: { gamepad?: boolean } = $props();
</script>

<footer class="hints">
  <span class="hint"><span class="kbd">◀</span><span class="kbd">▶</span> browse</span>
  <span class="hint"><span class="kbd">Enter</span> open</span>
  <span class="hint"><span class="kbd">Esc</span> back</span>
  <span class="hint"><span class="kbd">M</span> mute</span>
  <span class="hint"><span class="kbd">N</span> test chime</span>
  <span class="hint"><span class="kbd">F3</span> stats</span>

  <span class="pad" class:on={gamepad}>
    <i></i>
    {gamepad ? "Controller connected" : "Controller ready"}
  </span>
</footer>

<style>
  .hints {
    position: relative;
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 20px;
    height: 58px;
    padding: 0 26px;
    color: var(--ink-faint);
    font-size: 11.5px;
  }

  .hint {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .pad {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    margin-left: auto;
    font-size: 11px;
    letter-spacing: 0.02em;
  }

  .pad i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.22);
  }

  .pad.on i {
    background: #4ade80;
    box-shadow: 0 0 9px #4ade80;
  }

  @media (max-width: 900px) {
    .hints {
      gap: 12px;
    }
    .hint:nth-child(n + 4) {
      display: none;
    }
  }
</style>
