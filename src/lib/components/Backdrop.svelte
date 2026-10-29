<script lang="ts">
  import { fade } from "svelte/transition";
  import type { Game } from "../games";

  let { game }: { game: Game } = $props();
</script>

<!--
  Background layer.

  Two decisions worth noting for the RAM budget:
  1. The bitmap is pre-blurred and pre-darkened at asset-build time. Running a live
     CSS `filter: blur()` over a 1920x1080 layer costs a full-screen GPU pass every
     frame; baking it costs nothing at runtime.
  2. Only the current game's backdrop is in the DOM. Keyed on game.id, the outgoing
     <img> fades out and is then removed, releasing its decoded bitmap.
-->
<div class="backdrop" style="--accent: {game.accent};">
  {#if game.backdrop}
    {#each [game] as g (g.id)}
      <img
        class="layer"
        src={g.backdrop}
        alt=""
        draggable="false"
        decoding="async"
        transition:fade={{ duration: 620 }}
      />
    {/each}
  {:else}
    <!-- No hero art: a static accent wash beats a broken image or a black frame. -->
    <div class="flat"></div>
  {/if}

  <div class="wash"></div>
  <div class="vignette"></div>
  <div class="grain"></div>
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
    inset: -4%;
    width: 108%;
    height: 108%;
    object-fit: cover;
    /* Slow drift keeps a static shot from reading as a screenshot. */
    animation: drift 46s var(--ease-out) infinite alternate;
    will-change: transform;
  }

  @keyframes drift {
    from {
      transform: scale(1.02) translate3d(-0.6%, -0.4%, 0);
    }
    to {
      transform: scale(1.1) translate3d(0.8%, 0.6%, 0);
    }
  }

  .flat {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(90% 70% at 50% 18%, var(--accent-soft, rgba(127, 176, 255, 0.22)), transparent 70%),
      linear-gradient(180deg, #0a0d18 0%, #04050a 100%);
  }

  .wash {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(180deg, rgba(4, 5, 10, 0.82) 0%, rgba(4, 5, 10, 0.24) 28%, rgba(4, 5, 10, 0.55) 62%, rgba(4, 5, 10, 0.96) 100%),
      radial-gradient(120% 90% at 50% 18%, transparent 30%, rgba(4, 5, 10, 0.5) 100%);
  }

  .vignette {
    position: absolute;
    inset: 0;
    background: radial-gradient(130% 100% at 50% 50%, transparent 46%, rgba(0, 0, 0, 0.72) 100%);
  }

  /* Subtle texture — hand-crafted grain that reads as material, not algorithm. */
  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.08;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'><rect width='100%' height='100%' fill='%23000'/><circle cx='50%' cy='50%' r='42' fill='%230f0'/></svg>");
  }
</style>
