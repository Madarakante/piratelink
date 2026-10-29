/**
 * One input pipeline for keyboard and gamepad.
 *
 * A PS5-style shelf is a controller-first UI, so gamepad support is part of the
 * shell, not polish: the same `Action` stream drives both devices and the UI has
 * exactly one place to react. The pipeline also reports which device was last
 * used, so the shelf can label its buttons the way that device names them.
 */

export type Action =
  | "prev"
  | "next"
  | "open" // X / A — open the highlighted game
  | "back" // Circle / B / Escape — close the overlay
  | "play" // Square / X — launch directly from the shelf
  | "mute"
  | "notify" // demo: simulate a friend coming online
  | "stats" // renderer heap readout
  | "add" // open the Add games sheet
  | "settings"; // open the settings page

export type Device = "keyboard" | "gamepad";
export type PadFamily = "playstation" | "xbox";

/** Face buttons and two system buttons, named the way the pad names them. */
export type FaceButtons = {
  confirm: string;
  back: string;
  play: string;
  chime: string;
  share: string;
  menu: string;
};

const FACE: Record<PadFamily, FaceButtons> = {
  playstation: { confirm: "×", back: "○", play: "□", chime: "△", share: "Share", menu: "Options" },
  xbox: { confirm: "A", back: "B", play: "X", chime: "Y", share: "View", menu: "Menu" },
};

export function faceButtons(family: PadFamily): FaceButtons {
  return FACE[family];
}

/** Sony pads report a vendor id in the name on Windows; everything else reads as Xbox. */
function familyOf(id: string): PadFamily {
  return /dualsense|dualshock|playstation|sony|054c/i.test(id) ? "playstation" : "xbox";
}

export function connectedPad(): { id: string; family: PadFamily } | null {
  const pads = navigator.getGamepads?.() ?? [];
  for (const pad of pads) {
    if (pad?.connected) return { id: pad.id, family: familyOf(pad.id) };
  }
  return null;
}

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
  KeyS: "settings",
};

/** Standard-mapping button indices (DualSense/DualShock/Xbox all land here). */
const PAD_BUTTON_MAP: Partial<Record<number, Action>> = {
  0: "open", // Cross / A
  1: "back", // Circle / B
  2: "play", // Square / X
  3: "notify", // Triangle / Y
  4: "prev", // L1
  5: "next", // R1
  8: "mute", // Share / View
  9: "stats", // Options / Menu
  10: "settings", // L3
  11: "add", // R3 — the only system button left free
  12: "prev", // D-pad up
  13: "next", // D-pad down
  14: "prev", // D-pad left
  15: "next", // D-pad right
};

/** Left stick is browsable too; the held map gives it the same repeat as the D-pad. */
const STICK_DEAD = 0.55;
const STICK_LEFT = -2;
const STICK_RIGHT = -1;

const DEFAULT_REPEAT_DELAY = 380;
const DEFAULT_REPEAT_RATE = 110;

type HeldButton = { since: number; next: number };

export function bindInput(
  onAction: (action: Action) => void,
  onDevice?: (device: Device) => void,
): () => void {
  const held = new Map<number, HeldButton>();
  let raf = 0;
  let lastPad = -1;
  let disposed = false;
  let device: Device | null = null;

  function fire(action: Action) {
    if (!disposed) onAction(action);
  }

  /** Only tells the UI when the device actually changes, not on every press. */
  function report(next: Device) {
    if (device === next) return;
    device = next;
    onDevice?.(next);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.repeat) return; // Browser auto-repeat is uneven; gamepad + hold logic is ours.
    const target = e.target as HTMLElement | null;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;

    const action = KEY_MAP[e.code];
    if (!action) return;
    e.preventDefault();
    report("keyboard");
    fire(action);
  }

  /** Fire on press, then repeat while held — one path for buttons and the stick. */
  function step(key: number, action: Action, down: boolean, now: number) {
    const state = held.get(key);

    if (!down) {
      if (state) held.delete(key);
      return;
    }

    report("gamepad");

    if (!state) {
      held.set(key, { since: now, next: now + DEFAULT_REPEAT_DELAY });
      fire(action);
    } else if (now >= state.next) {
      state.next = now + DEFAULT_REPEAT_RATE;
      fire(action);
    }
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
      step(index, action, button.pressed || button.value > 0.5, now);
    }

    const x = pad.axes[0] ?? 0;
    step(STICK_RIGHT, "next", x > STICK_DEAD, now);
    step(STICK_LEFT, "prev", x < -STICK_DEAD, now);
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
