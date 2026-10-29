<script lang="ts">
  import { cubicOut } from "svelte/easing";
  import { fade } from "svelte/transition";
  import type { Game } from "../games";

  let {
    game,
    fit = "fit",
  }: {
    game: Game;
    /** `fit` keeps the whole hero; `fill` lets it cover the window and crop. */
    fit?: "fit" | "fill";
  } = $props();

  type Layer = { key: string; src: string };

  /**
   * Painted, fully opaque. It is only replaced once the next layer has finished
   * fading in, which is what keeps a fast scroll from flashing the page black.
   */
  let base = $state<Layer | null>(null);
  /** The layer fading in on top of `base`. At most one at a time. */
  let incoming = $state<Layer | null>(null);
  /** Sources that failed to decode, so the effect does not retry them forever. */
  const failed = new Set<string>();

  $effect(() => {
    const src = game.backdrop;
    if (!src) {
      base = null;
      incoming = null;
      return;
    }
    if (base?.src === src || incoming?.src === src || failed.has(src)) return;
    // One fade at a time. Settling re-runs this effect, so scrolling through ten
    // games converges on the last one instead of stacking ten backdrops in RAM.
    if (incoming) return;

    // Decode before showing: an <img> that is still decoding paints nothing, and
    // cross-fading two half-transparent layers lets the black page show through
    // both. Waiting for load turns the switch into a reveal over solid art.
    const layer: Layer = { key: game.id, src };
    const image = new Image();
    image.onload = () => {
      if (game.backdrop === src) incoming = layer;
    };
    image.onerror = () => failed.add(src);
    image.src = src;
  });

  function settle(layer: Layer) {
    if (incoming?.src !== layer.src) return;
    base = layer;
    incoming = null;
  }
</script>

<!--
  Background layer.

  The bitmap is pre-blurred and pre-darkened at asset-build time, so nothing is
  filtered per frame. It is also *static* now: the old version drifted a
  1920x620 layer forever, which cost a full-screen composite every frame and read
  as a screensaver.
-->
<div class="backdrop" data-fit={fit}>
  {#if base}
    <img class="layer" src={base.src} alt="" draggable="false" decoding="async" />
  {/if}

  {#if incoming}
    <img
      class="layer"
      src={incoming.src}
      alt=""
      draggable="false"
      decoding="async"
      in:fade={{ duration: 300, easing: cubicOut }}
      onintroend={() => incoming && settle(incoming)}
    />
  {/if}

  {#if !base && !incoming}
    <!-- No hero art: a static accent wash beats a broken image or a black frame. -->
    <div class="flat"></div>
  {/if}

  <div class="tint"></div>
  <div class="wash"></div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    background: var(--bg-0);
    pointer-events: none;
  }

  .layer {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  /*
    Fit mode: the hero is printed at its own aspect ratio, the width of the window,
    so nothing is cropped. Valve's heroes are ~3.1:1 while a 16:9 window is ~1.6:1,
    so covering the window would throw away half the picture - the mask ends it
    softly instead of leaving a hard seam where the bitmap stops.
  */
  .backdrop[data-fit="fit"] .layer {
    height: auto;
    object-fit: contain;
    object-position: 50% 0;
    -webkit-mask-image: linear-gradient(to bottom, #000 58%, transparent 100%);
    mask-image: linear-gradient(to bottom, #000 58%, transparent 100%);
  }

  .flat {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(80% 70% at 76% 26%, var(--accent-soft), transparent 62%),
      linear-gradient(180deg, #0a0c11 0%, var(--bg-0) 100%);
  }

  /* The per-game accent is used once, as a wide tint - not as a glow behind the
     selected card. */
  .tint {
    position: absolute;
    inset: 0;
    opacity: 0.6;
    background: radial-gradient(55% 50% at 74% 28%, var(--accent-soft), transparent 68%);
  }

  /* Two flat gradients instead of a vignette + bloom + grain stack. The left
     third is where type lives, so it stays near-solid; the right is left open
     for the art. */
  .wash {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        180deg,
        rgba(7, 8, 10, 0.92) 0%,
        rgba(7, 8, 10, 0.2) 18%,
        rgba(7, 8, 10, 0) 45%,
        rgba(7, 8, 10, 0.34) 66%,
        rgba(7, 8, 10, 0.92) 88%,
        var(--bg-0) 100%
      ),
      linear-gradient(
        90deg,
        rgba(7, 8, 10, 0.97) 0%,
        rgba(7, 8, 10, 0.9) 22%,
        rgba(7, 8, 10, 0.58) 46%,
        rgba(7, 8, 10, 0.32) 72%,
        rgba(7, 8, 10, 0.52) 100%
      );
  }
</style>
