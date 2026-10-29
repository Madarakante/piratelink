<script lang="ts">
  import type { Game } from "../games";
  import { createSpring } from "../spring.svelte";

  let {
    games,
    index,
    onSelect,
    onOpen,
  }: {
    games: Game[];
    index: number;
    onSelect: (i: number) => void;
    onOpen: () => void;
  } = $props();

  let viewport = $state(1440);
  let viewportH = $state(900);
  let shelfEl = $state<HTMLDivElement | null>(null);
  let wheelAt = 0;

  const pos = createSpring(0, { stiffness: 190, damping: 28 });

  /**
   * The strip is a *ring*, not a window over a list.
   *
   * Ring item at position p is drawn at
   *     p + k * ringLen - pos      (k = nearest whole turn)
   * which is invariant under pos -> pos ± ringLen. That is what lets the cursor
   * run forever: nothing has to be reset, and any card that wraps does so more
   * than a full turn from the selection, i.e. off screen.
   *
   * A ring of length L can only place plates within ±L/2 of the cursor, so a
   * short library is laid out twice (ringLen = 2n). Without that, a six-title
   * library on a six-plate row can only fill half the row — which is exactly
   * the void that showed up after the eleventh switch.
   */
  const ring = $derived.by(() => {
    const n = games.length;
    const out: { game: Game; n: number; slot: number; key: string }[] = [];
    if (!n) return out;
    const copies = n >= 12 ? 1 : 2;
    for (let c = 0; c < copies; c++) {
      for (let i = 0; i < n; i++) {
        out.push({ game: games[i], n: i, slot: c * n + i, key: `${games[i].id}:${c}` });
      }
    }
    return out;
  });

  /** How many plates the row is designed to hold. Short libraries get one plate each. */
  const slots = $derived(Math.max(1, Math.min(6, games.length)));

  const GAP_RATIO = 0.16;

  /**
   * Six plates fill the row edge to edge, so the strip never ends in blank
   * space on a wide window. The height term only takes over when the window is
   * too short for a 3:2 plate.
   */
  const cardW = $derived.by(() => {
    if (!games.length) return 120;
    const content = viewport - 112;
    const widthFill = content / (slots + GAP_RATIO * (slots - 1));
    const heightCap = (viewportH - 430) / 1.5;
    const want = Math.min(widthFill, heightCap);
    // Round the gap down, then check the real numbers: a rounded-up gap is
    // enough to push the last plate past the gutter and drop it from the row.
    const gap = Math.floor(want * GAP_RATIO);
    const fit = (content - (slots - 1) * gap) / slots;
    return Math.round(Math.max(104, Math.min(want, fit)));
  });
  const cardH = $derived(Math.round(cardW * 1.5));
  const gap = $derived(Math.floor(cardW * GAP_RATIO));
  const pitch = $derived(cardW + gap);

  /** Last slot that is still inside the right gutter. */
  const maxD = $derived(
    Math.min(ring.length / 2 - 0.05, (viewport - 112 - cardW) / pitch + 0.02),
  );

  /** Unbounded virtual position of the selection. */
  let cursor = 0;

  $effect(() => {
    const n = games.length;
    if (!n) return;
    const at = ((cursor % n) + n) % n;
    let step = index - at;
    if (step > n / 2) step -= n;
    if (step < -n / 2) step += n;
    cursor += step;
    pos.to(cursor);
  });

  /** Ring distance from the cursor, plus whether this is the selected plate. */
  function place(slot: number): { d: number; active: boolean } {
    const L = ring.length;
    if (!L) return { d: 0, active: false };
    const k = Math.round((pos.value - slot) / L);
    const at = slot + k * L;
    return { d: at - pos.value, active: at === cursor };
  }

  function styleFor(d: number, active: boolean): string {
    // A shelf, not a spotlight: every plate stays readable, and hierarchy comes
    // from the outline plus the accent index rather than from blacking the row
    // out around the selection.
    const opacity = active ? 1 : Math.max(0.58, 0.95 - (Math.abs(d) - 1) * 0.075);
    return [
      `transform: translate3d(${(d * pitch).toFixed(2)}px, 0, 0)`,
      `opacity: ${opacity.toFixed(3)}`,
      `z-index: ${Math.round(100 - Math.abs(d))}`,
    ].join("; ");
  }

  function click(i: number) {
    if (i === index) onOpen();
    else onSelect(i);
  }

  // Wheel needs a non-passive listener to suppress page scroll; Svelte's own
  // handler registration is passive for wheel events.
  $effect(() => {
    const el = shelfEl;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const now = performance.now();
      if (now - wheelAt < 220) return;
      const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
      if (Math.abs(delta) < 6) return;
      wheelAt = now;
      onSelect(index + (delta > 0 ? 1 : -1));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });
</script>

<svelte:window bind:innerWidth={viewport} bind:innerHeight={viewportH} />

<div class="shelf" bind:this={shelfEl} style="--card-w: {cardW}px; --card-h: {cardH}px;">
  {#each ring as item (item.key)}
    {@const p = place(item.slot)}
    {#if p.d > -1.05 && p.d < maxD}
      <button
        class="card"
        class:active={p.active}
        style={styleFor(p.d, p.active)}
        tabindex={p.active ? 0 : -1}
        aria-label={item.game.title}
        aria-current={p.active}
        onclick={() => click(item.n)}
      >
        <span class="frame">
          {#if item.game.cover}
            <img class="cover" src={item.game.cover} alt="" draggable="false" decoding="async" />
          {:else}
            <!-- No artwork: a flat accent block with the title set small. -->
            <span class="blank" style="background: {item.game.accent}1c;">
              <span>{item.game.title}</span>
            </span>
          {/if}
        </span>
        <span class="tag">{String(item.n + 1).padStart(2, "0")}</span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .shelf {
    position: relative;
    height: calc(var(--card-h) + 26px);
    overflow: hidden;
  }

  .card {
    position: absolute;
    top: 0;
    left: 0;
    display: block;
    width: var(--card-w);
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    contain: layout paint;
  }

  .frame {
    position: relative;
    display: block;
    height: var(--card-h);
    overflow: hidden;
    background: #0a0b0f;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    transition: box-shadow var(--dur-2) var(--ease-out);
  }

  .cover {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .blank {
    display: flex;
    align-items: flex-end;
    width: 100%;
    height: 100%;
    padding: 14px;
    color: rgba(238, 240, 244, 0.72);
    font-size: 14px;
    font-weight: 620;
    letter-spacing: -0.01em;
    line-height: 1.15;
    text-wrap: balance;
  }

  .card.active .frame {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.62);
  }

  .card:not(.active):hover .frame {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.3);
  }

  .tag {
    display: block;
    margin-top: 9px;
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 500;
    letter-spacing: 0.16em;
    color: rgba(238, 240, 244, 0.34);
    transition: color var(--dur-2) linear;
  }

  .card.active .tag {
    color: var(--accent);
  }

  .card:focus-visible {
    outline: none;
  }

  .card:focus-visible .frame {
    box-shadow: inset 0 0 0 1px var(--ink);
  }
</style>
