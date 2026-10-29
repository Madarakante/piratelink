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

  const pos = createSpring(0, { stiffness: 170, damping: 26 });

  // Drive the spring from the selected index.
  $effect(() => {
    pos.to(index);
  });

  // Card metrics scale with the window, but stay inside a readable band, and
  // are height-capped so the shelf never gets clipped at 720p.
  const cardW = $derived(
    Math.round(Math.min(296, Math.max(132, Math.min(viewport * 0.181, (viewportH - 360) / 1.5)))),
  );
  const cardH = $derived(Math.round(cardW * 1.5));
  const gap = $derived(Math.round(cardW * 1.03));

  /** Rendered at most ±3 from the selection; everything else is parked off-canvas. */
  function cardStyle(i: number): string {
    const d = i - pos.value;
    const ad = Math.abs(d);
    if (ad > 4.2) return "visibility: hidden;";

    const scale = Math.max(0.7, 1 - Math.max(0, ad) * 0.135);
    const lift = Math.min(ad, 3) * -6;
    const rot = Math.max(-9, Math.min(9, -d * 4.2));
    const opacity = Math.max(0.18, 1 - ad * 0.24);

    return [
      `transform: translate3d(${(d * gap).toFixed(2)}px, ${lift.toFixed(2)}px, 0) scale(${scale.toFixed(4)}) rotateY(${rot.toFixed(2)}deg)`,
      `opacity: ${opacity.toFixed(3)}`,
      `z-index: ${Math.round(100 - ad)}`,
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
  <div class="glow" aria-hidden="true"></div>

  {#each games as game, i (game.id)}
    {@const nearby = Math.abs(i - index) <= 2}
    <button
      class="card"
      class:active={i === index}
      style={cardStyle(i)}
      tabindex={i === index ? 0 : -1}
      aria-label={game.title}
      aria-current={i === index}
      onclick={() => click(i)}
    >
      <!--
        Only ±2 neighbours get a decoded image. Anything further away unmounts its
        <img>, which is what keeps a 60-title library inside the memory ceiling.
      -->
      {#if nearby && game.cover}
        <img class="cover" src={game.cover} alt="" draggable="false" decoding="async" />
      {:else}
        <div class="cover placeholder" style="background: {game.accent}14">
          <span class="fallback-title">{game.title}</span>
        </div>
      {/if}

      <div class="sheen" aria-hidden="true"></div>

      {#if i === index}
        <div class="ring" style="--c: {game.accent}"></div>
      {/if}
    </button>
  {/each}
</div>

<style>
  .shelf {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: calc(var(--card-h) + 64px);
    perspective: 1400px;
    perspective-origin: 50% 40%;
  }

  .glow {
    position: absolute;
    top: 50%;
    left: 50%;
    width: calc(var(--card-w) * 2);
    height: calc(var(--card-w) * 1.8);
    transform: translate(-50%, -56%);
    background: radial-gradient(closest-side, var(--accent-soft, rgba(106, 156, 255, 0.12)), transparent 75%);
    filter: blur(4px);
    opacity: 0.7;
    pointer-events: none;
    transition: opacity var(--dur-3) var(--ease-out);
  }

  .card {
    position: absolute;
    left: 50%;
    width: var(--card-w);
    height: var(--card-h);
    margin-left: calc(var(--card-w) / -2);
    padding: 0;
    border: 0;
    border-radius: var(--radius-card);
    background: #0b0d14;
    overflow: hidden;
    cursor: pointer;
    transform-origin: 50% 50%;
    will-change: transform, opacity;
    contain: layout paint style;
    backface-visibility: hidden;
    box-shadow: 0 24px 60px -28px rgba(0, 0, 0, 0.9);
    transition: box-shadow var(--dur-2) var(--ease-out);
  }

  .card:focus-visible {
    outline: none;
  }

  .card.active {
    box-shadow:
      0 40px 90px -34px rgba(0, 0, 0, 0.95),
      0 0 0 1px rgba(255, 255, 255, 0.09);
  }

  .cover {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    /* Slight zoom hides the 1px seam on fractional scaling. */
    transform: scale(1.012);
  }

  .cover.placeholder {
    display: grid;
    place-items: center;
    padding: 18px;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.05);
  }

  .fallback-title {
    color: rgba(255, 255, 255, 0.82);
    font-size: 17px;
    font-weight: 680;
    letter-spacing: -0.01em;
    line-height: 1.15;
    text-align: center;
    text-wrap: balance;
  }

  .sheen {
    position: absolute;
    inset: 0;
    background: linear-gradient(160deg, rgba(255, 255, 255, 0.14), transparent 42%, rgba(0, 0, 0, 0.24));
    pointer-events: none;
  }

  .ring {
    position: absolute;
    inset: 0;
    border-radius: var(--radius-card);
    box-shadow: inset 0 0 0 2px var(--c);
    opacity: 0.9;
    pointer-events: none;
    animation: ring-in var(--dur-3) var(--ease-out);
  }

  @keyframes ring-in {
    from {
      opacity: 0;
    }
  }
</style>
