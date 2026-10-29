<script lang="ts">
  import { fade } from "svelte/transition";
  import type { Game } from "../games";

  let { game }: { game: Game } = $props();
</script>

<!--
  Background layer.

  The bitmap is pre-blurred and pre-darkened at asset-build time, so nothing is
  filtered per frame. It is also *static* now: the old version drifted a
  1920x620 layer forever, which cost a full-screen composite every frame and read
  as a screensaver. Only the current game's backdrop exists in the DOM.
-->
<div class="backdrop">
  {#if game.backdrop}
    {#each [game] as g (g.id)}
      <img
        class="layer"
        src={g.backdrop}
        alt=""
        draggable="false"
        decoding="async"
        transition:fade={{ duration: 380 }}
      />
    {/each}
  {:else}
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
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .flat {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(80% 70% at 76% 26%, var(--accent-soft), transparent 62%),
      linear-gradient(180deg, #0a0c11 0%, var(--bg-0) 100%);
  }

  /* The per-game accent is used once, as a wide tint — not as a glow behind the
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
