<script lang="ts">
  import { faceButtons, type Device, type PadFamily } from "../input";

  let {
    gamepad = false,
    device = "keyboard",
    family = "xbox",
    index = 0,
    total = 0,
  }: {
    gamepad?: boolean;
    device?: Device;
    family?: PadFamily;
    index?: number;
    total?: number;
  } = $props();

  const pad2 = (n: number) => String(n).padStart(2, "0");

  /**
   * The bar is a caption for whichever device was used last, so a pad owner sees
   * the pad's own button names instead of a keyboard they are not touching.
   */
  const hints = $derived.by(() => {
    if (device === "gamepad") {
      const f = faceButtons(family);
      return [
        { key: "← →", label: "Browse" },
        { key: f.confirm, label: "Info" },
        { key: f.play, label: "Play" },
        { key: f.share, label: "Mute" },
        { key: f.chime, label: "Chime" },
        { key: f.menu, label: "Stats" },
      ];
    }
    return [
      { key: "← →", label: "Browse" },
      { key: "↵", label: "Info" },
      { key: "Space", label: "Play" },
      { key: "M", label: "Mute" },
      { key: "N", label: "Chime" },
      { key: "F3", label: "Stats" },
    ];
  });
</script>

<!-- Keys are plain text, not keycap boxes: the hint bar is a caption, not a toy. -->
<footer class="hints">
  {#if total > 0}
    {#each hints as hint (hint.label)}
      <span><b>{hint.key}</b>{hint.label}</span>
    {/each}
  {/if}
  <span class="spacer"></span>

  {#if total > 0}
    <span class="pos num">{pad2(index + 1)}<i>/</i>{pad2(total)}</span>
  {/if}

  <span class="pad" class:on={gamepad}>
    <i></i>
    {gamepad ? "Pad" : "No pad"}
  </span>
</footer>

<style>
  .hints {
    position: relative;
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 0 18px 0 var(--gutter);
    border-top: 1px solid var(--line);
    color: var(--ink-ghost);
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 500;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .hints > span {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }

  b {
    color: var(--ink-dim);
    font-weight: 500;
  }

  .spacer {
    flex: 1;
  }

  .pos {
    color: var(--ink-faint);
    font-size: 10px;
  }

  .pos i {
    margin: 0 4px;
    color: var(--ink-ghost);
    font-style: normal;
  }

  .pad {
    gap: 8px;
    color: var(--ink-ghost);
  }

  .pad i {
    width: 5px;
    height: 5px;
    background: var(--ink-ghost);
  }

  .pad.on {
    color: var(--ink-dim);
  }

  .pad.on i {
    background: var(--accent);
  }

  @media (max-width: 1040px) {
    .hints {
      gap: 14px;
    }
    .hints > span:nth-child(n + 5) {
      display: none;
    }
  }
</style>
