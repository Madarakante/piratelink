/**
 * Tiny critically-damped spring.
 *
 * Why hand-rolled instead of `svelte/motion` or a motion library:
 *  - `spring` from svelte/motion keeps ticking on every frame it is subscribed;
 *    this one parks the rAF loop the moment the value settles, so an idle
 *    carousel costs zero CPU.
 *  - Framer Motion is a React library and cannot drive Svelte components.
 *  - It is ~35 lines and shares the same physics as the CSS easing curves.
 *
 * Values are unitless (carousel index, not pixels), so stiffness/damping are
 * tuned for a 0..n range rather than for 0..1000px.
 */

type SpringOptions = {
  /** Higher = snappier. 170 lands around 180ms to visual rest. */
  stiffness?: number;
  /** Critical damping for stiffness 170 is ~26. */
  damping?: number;
};

export function createSpring(initial: number, opts: SpringOptions = {}) {
  const stiffness = opts.stiffness ?? 170;
  const damping = opts.damping ?? 26;

  let value = $state(initial);
  let target = initial;
  let velocity = 0;
  let raf = 0;
  let last = 0;

  function step(now: number) {
    // Clamp dt: a backgrounded window must not produce a huge integration jump.
    const dt = Math.min(64, now - last) / 1000;
    last = now;

    const force = (target - value) * stiffness;
    velocity += (force - velocity * damping) * dt;
    value += velocity * dt;

    if (Math.abs(target - value) < 0.0005 && Math.abs(velocity) < 0.005) {
      value = target;
      velocity = 0;
      raf = 0;
      return;
    }
    raf = requestAnimationFrame(step);
  }

  function run() {
    if (raf) return;
    last = performance.now();
    raf = requestAnimationFrame(step);
  }

  return {
    get value() {
      return value;
    },
    /** Animate toward `next`. */
    to(next: number) {
      if (next === target) return;
      target = next;
      run();
    },
    /** Jump without animating (initial mount, resize, reduced motion). */
    snap(next: number) {
      target = next;
      value = next;
      velocity = 0;
      if (raf) {
        cancelAnimationFrame(raf);
        raf = 0;
      }
    },
    dispose() {
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
    },
  };
}
