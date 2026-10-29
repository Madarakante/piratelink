/**
 * One input pipeline for keyboard and gamepad.
 *
 * A PS5-style shelf is a controller-first UI, so gamepad support is part of the
 * shell, not polish: the same `Action` stream drives both devices and the UI has
 * exactly one place to react.
 */

export type Action =
  | "prev"
  | "next"
  | "open" // X / A — open the highlighted game
  | "back" // Circle / B / Escape — close the overlay
  | "play" // Square / X — launch directly from the shelf
  | "mute"
  | "notify" // demo: simulate a friend coming online
  | "stats"; // renderer heap readout

const KEY_MAP: Record<string, Action> = {
  ArrowLeft: "prev",
  ArrowRight: "next",
  Enter: "open",
  Space: "play",
  Escape: "back",
  Backspace: "back",
  KeyM: "mute",
  KeyN: "notify",
  F3: "stats",
};

/** Standard-mapping button indices (DualSense/DualShock/Xbox all land here). */
const PAD_BUTTON_MAP: Partial<Record<number, Action>> = {
  0: "open", // Cross / A
  1: "back", // Circle / B
  2: "play", // Square / X
  3: "notify", // Triangle / Y
  4: "prev", // L1
  5: "next", // R1
  8: "mute", // Share / Back
  12: "prev", // D-pad up
  13: "next", // D-pad down
  14: "prev", // D-pad left
  15: "next", // D-pad right
};

const DEFAULT_REPEAT_DELAY = 380;
const DEFAULT_REPEAT_RATE = 110;

type HeldButton = { since: number; next: number };

export function bindInput(onAction: (action: Action) => void): () => void {
  const held = new Map<number, HeldButton>();
  let raf = 0;
  let lastPad = -1;
  let disposed = false;

  function fire(action: Action) {
    if (!disposed) onAction(action);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.repeat) return; // Browser auto-repeat is uneven; gamepad + hold logic is ours.
    const target = e.target as HTMLElement | null;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;

    const action = KEY_MAP[e.code];
    if (!action) return;
    e.preventDefault();
    fire(action);
  }

  function poll() {
    raf = requestAnimationFrame(poll);

    const pads = navigator.getGamepads?.() ?? [];
    let pad: Gamepad | null = null;
    for (const p of pads) {
      if (p && p.connected) {
        pad = p;
        break;
      }
    }
    if (!pad) {
      held.clear();
      lastPad = -1;
      return;
    }
    if (pad.index !== lastPad) {
      lastPad = pad.index;
      held.clear();
    }

    const now = performance.now();
    for (const [index, button] of pad.buttons.entries()) {
      const action = PAD_BUTTON_MAP[index];
      if (!action) continue;

      const down = button.pressed || button.value > 0.5;
      const state = held.get(index);

      if (!down) {
        held.delete(index);
        continue;
      }

      if (!state) {
        held.set(index, { since: now, next: now + DEFAULT_REPEAT_DELAY });
        fire(action);
      } else if (now >= state.next) {
        state.next = now + DEFAULT_REPEAT_RATE;
        fire(action);
      }
    }
  }

  window.addEventListener("keydown", onKeyDown);
  raf = requestAnimationFrame(poll);

  return () => {
    disposed = true;
    window.removeEventListener("keydown", onKeyDown);
    cancelAnimationFrame(raf);
    held.clear();
  };
}

export function gamepadConnected(): boolean {
  const pads = navigator.getGamepads?.() ?? [];
  return Array.from(pads).some((p) => p?.connected);
}
