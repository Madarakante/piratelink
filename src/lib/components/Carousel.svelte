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
   * The shelf is a filmstrip, not a fan: every cover sits on one baseline at one
   * size, and the strip slides under a fixed left edge so the selected cover
   * always aligns with the title above it.
   *
   * `target` is deliberately unbounded. Wrapping the *index* (n-1 -> 0) would
   * make the spring fly backwards through the whole library; tracking an
   * unbounded cursor lets it travel one pitch in either direction.
   */
  let target = 0;

  /**
   * Six plates fill the row edge to edge. Sizing from the content width (rather
   * than from the window) is what keeps that true at any size.
   */
  const cardW = $derived(
    Math.round(
      Math.min(208, Math.max(112, Math.min((viewport - 112) / 6.82, (viewportH - 430) / 2.4))),
    ),
  );
  const cardH = $derived(Math.round(cardW * 1.5));
  const gap = $derived(Math.round(cardW * 0.16));
  const pitch = $derived(cardW + gap);

  /** Last slot that fits inside the right gutter. */
  const maxD = $derived(
    Math.min(games.length - 0.05, (viewport - 112 - cardW) / pitch + 0.02),
  );

  $effect(() => {
    const n = games.length;
    if (!n) return;
    const settled = ((target % n) + n) % n;
    let delta = index - settled;
    if (delta > n / 2) delta -= n;
    if (delta < -n / 2) delta += n;
    target += delta;
    pos.to(target);
  });

  function styleFor(d: number, active: boolean): string {
    // A shelf, not a spotlight: every plate stays readable, and hierarchy comes
    // from the outline plus the accent index rather than from blacking the row
    // out around the selection.
    const opacity = active ? 1 : Math.max(0.38, 0.86 - (Math.abs(d) - 1) * 0.12);
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

<div
  class="shelf"
  bind:this={shelfEl}
  style="--card-w: {cardW}px; --card-h: {cardH}px;"
>
  {#each games as game, i (game.id)}
    <!--
      Three virtual copies. The strip is unbounded while it animates, so a
      wrapped move has to place the same game at i - n or i + n. Duplicates are
      only in the DOM when the library is too short to fill the row; keys are
      stable, so nothing remounts when the selection changes.
    -->
    {#each [-1, 0, 1] as k (k)}
      {@const d = i + k * games.length - pos.value}
      {#if d > -1.05 && d < maxD}
        <button
          class="card"
          class:active={i === index}
          style={styleFor(d, i === index)}
          tabindex={i === index ? 0 : -1}
          aria-label={game.title}
          aria-current={i === index}
          onclick={() => click(i)}
        >
          <span class="frame">
            {#if game.cover}
              <img class="cover" src={game.cover} alt="" draggable="false" decoding="async" />
            {:else}
              <!-- No artwork: a flat accent block with the title set small. -->
              <span class="blank" style="background: {game.accent}1c;">
                <span>{game.title}</span>
              </span>
            {/if}
          </span>
          <span class="tag">{String(i + 1).padStart(2, "0")}</span>
        </button>
      {/if}
    {/each}
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
