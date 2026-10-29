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

  type Hint = { key: string; label: string; add?: boolean; opt?: boolean };

  /**
   * The bar is a caption for whichever device was used last, so a pad owner sees
   * the pad's own button names instead of a keyboard they are not touching.
   */
  const hints = $derived.by<Hint[]>(() => {
    if (device === "gamepad") {
      const f = faceButtons(family);
      return [
        { key: "← →", label: "Browse" },
        { key: f.confirm, label: "Info" },
        { key: f.play, label: "Play" },
        { key: "R3", label: "Add", add: true },
        { key: "L3", label: "Settings", opt: true },
        { key: f.share, label: "Mute", opt: true },
        { key: f.chime, label: "Chime", opt: true },
        { key: f.menu, label: "Stats", opt: true },
      ];
    }
    return [
      { key: "← →", label: "Browse" },
      { key: "↵", label: "Info" },
      { key: "Space", label: "Play" },
      { key: "S", label: "Settings", opt: true },
      { key: "M", label: "Mute", opt: true },
      { key: "N", label: "Chime", opt: true },
      { key: "F3", label: "Stats", opt: true },
    ];
  });

  /** With an empty library, Add is the only thing worth advertising. */
  const visible = $derived(total > 0 ? hints : hints.filter((h) => h.add));
</script>

<!-- Keys are plain text, not keycap boxes: the hint bar is a caption, not a toy. -->
<footer class="hints">
  {#each visible as hint (hint.label)}
    <span class:opt={hint.opt}><b>{hint.key}</b>{hint.label}</span>
  {/each}
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
    /* Only the optional hints fold away; the pad light and the position stay,
       because at this width they are the only things confirming the pad works. */
    .hints > span.opt {
      display: none;
    }
  }
</style>
