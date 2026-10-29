/**
 * UI audio - one sprite file, two playback engines.
 *
 * `element` (default, always available): one pooled <audio> element per cue,
 * seeking into the shared sprite with `currentTime`. Starts instantly, no decode.
 *
 * `web-audio` (upgrade path): decode the sprite once into an AudioBuffer and
 * slice it with `AudioBufferSourceNode.start(0, offset, duration)`. Sample
 * accurate. Attempted in the background and adopted only if it wins the race.
 *
 * Why not just decode up front: `decodeAudioData` can take seconds to reject in
 * environments whose Chromium lacks a compressed-media decoder, and it can never
 * be cancelled. Starting on elements means the first keypress is never silent,
 * and the worst case is that the upgrade simply never happens.
 *
 * Both paths download exactly one file. All audio is synthesised by
 * tools/make_assets.py, so there are no third-party licences involved.
 */

export type CueName = "tick" | "select" | "back" | "chime" | "open";

type Cue = { start: number; duration: number };
type Manifest = {
  file: string;
  cues: Record<CueName, Cue>;
};

type Engine = "element" | "web-audio";

const MANIFEST_URL = "/audio/ui-sprite.json";

/** Ceiling on the background decode attempt. */
const DECODE_TIMEOUT_MS = 1500;

/** Used only if the manifest request fails; mirrors tools/make_assets.py output. */
const FALLBACK_MANIFEST: Manifest = {
  file: "/audio/ui-sprite.ogg",
  cues: {
    tick: { start: 0.0, duration: 0.13 },
    select: { start: 0.17, duration: 0.54 },
    back: { start: 0.77, duration: 0.3 },
    chime: { start: 1.13, duration: 0.95 },
    open: { start: 2.16, duration: 0.62 },
  },
};

class UiAudio {
  private manifest: Manifest = FALLBACK_MANIFEST;
  private engine: Engine = "element";
  private loadPromise: Promise<void> | null = null;
  private started = false;
  private spriteUrl: string | null = null;

  // element engine
  private elements = new Map<CueName, HTMLAudioElement>();
  private stopTimers = new Map<CueName, number>();

  // web-audio engine
  private ctx: AudioContext | null = null;
  private buffer: AudioBuffer | null = null;
  private master: GainNode | null = null;
  private live = new Set<AudioBufferSourceNode>();

  private volume = 0.55;
  private muted = false;

  get mode(): Engine {
    return this.engine;
  }

  get isMuted(): boolean {
    return this.muted;
  }

  get unlocked(): boolean {
    return this.started;
  }

  load(): Promise<void> {
    if (this.loadPromise) return this.loadPromise;

    this.loadPromise = (async () => {
      try {
        this.manifest = await fetch(MANIFEST_URL).then((r) =>
          r.ok ? (r.json() as Promise<Manifest>) : FALLBACK_MANIFEST,
        );
      } catch {
        this.manifest = FALLBACK_MANIFEST;
      }

      // One fetch, one in-memory copy. Every element points at the same blob URL,
      // which loads without touching the network stack again and avoids the
      // resource-URL load stalls seen when five <audio> elements race the HTTP
      // cache for the same file.
      try {
        const res = await fetch(this.manifest.file);
        if (!res.ok) throw new Error(`sprite HTTP ${res.status}`);
        const bytes = await res.arrayBuffer();
        const type = this.manifest.file.endsWith(".wav") ? "audio/wav" : "audio/ogg";
        this.spriteUrl = URL.createObjectURL(new Blob([bytes], { type }));
        this.buildElements();

        // Background upgrade. Deliberately not awaited: playback already works.
        void this.tryWebAudio(bytes.slice(0));
      } catch (err) {
        console.warn("[audio] disabled:", err);
      }
    })();

    return this.loadPromise;
  }

  /** Mark the first user gesture; the AudioContext can only run after this. */
  async unlock(): Promise<void> {
    this.started = true;
    await this.load();
    if (this.ctx?.state === "suspended") {
      await this.ctx.resume().catch(() => {});
    }
  }

  play(cue: CueName, gain = 1): void {
    if (this.muted) return;
    const slice = this.manifest.cues[cue];
    if (!slice) return;

    if (this.engine === "web-audio" && this.buffer) this.playBuffer(slice, gain);
    else this.playElement(cue, slice, gain);
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
    if (this.master && this.ctx) {
      this.master.gain.setTargetAtTime(muted ? 0 : this.volume, this.ctx.currentTime, 0.02);
    }
    for (const el of this.elements.values()) el.volume = muted ? 0 : this.volume;
  }

  toggleMuted(): boolean {
    this.setMuted(!this.muted);
    return this.muted;
  }

  // -- element engine -------------------------------------------------------

  private buildElements(): void {
    if (!this.spriteUrl) return;
    for (const cue of Object.keys(this.manifest.cues) as CueName[]) {
      if (this.elements.has(cue)) continue;
      const el = new Audio(this.spriteUrl);
      el.preload = "auto";
      el.volume = this.muted ? 0 : this.volume;
      // Errors are non-fatal: the cue simply stays silent.
      el.addEventListener("error", () => {
        if (el.error) console.warn(`[audio] cue "${cue}" failed: code ${el.error.code}`);
      });
      this.elements.set(cue, el);
    }
  }

  private playElement(cue: CueName, slice: Cue, gain: number): void {
    const el = this.elements.get(cue);
    if (!el) return;

    const pending = this.stopTimers.get(cue);
    if (pending) clearTimeout(pending);

    el.volume = Math.min(1, this.volume * gain);

    const fire = () => {
      try {
        el.currentTime = slice.start;
      } catch {
        /* ignore: best effort */
      }
      void el.play().catch(() => {
        /* autoplay policy, or superseded by a newer cue */
      });
      const timer = window.setTimeout(() => {
        el.pause();
        this.stopTimers.delete(cue);
      }, slice.duration * 1000);
      this.stopTimers.set(cue, timer);
    };

    // Seeking before metadata exists aborts the element's load (readyState falls
    // back to HAVE_NOTHING and the cue never plays). Wait for metadata instead.
    if (el.readyState >= HTMLMediaElement.HAVE_METADATA) {
      fire();
      return;
    }
    el.addEventListener("loadedmetadata", fire, { once: true });
  }

  // -- web-audio engine -----------------------------------------------------

  private async tryWebAudio(bytes: ArrayBuffer): Promise<void> {
    try {
      const ctx = new AudioContext({ latencyHint: "interactive" });
      const master = ctx.createGain();
      master.gain.value = this.muted ? 0 : this.volume;
      master.connect(ctx.destination);

      const decoded = await Promise.race([
        ctx.decodeAudioData(bytes).catch((err: unknown) => {
          console.warn("[audio] Web Audio decode unavailable:", err);
          return null;
        }),
        new Promise<null>((resolve) => setTimeout(() => resolve(null), DECODE_TIMEOUT_MS)),
      ]);

      if (!decoded) {
        void ctx.close().catch(() => {});
        return;
      }

      this.ctx = ctx;
      this.master = master;
      this.buffer = decoded;
      this.engine = "web-audio";

      if (this.started && ctx.state === "suspended") await ctx.resume().catch(() => {});
    } catch (err) {
      // Keep the element engine.
      console.warn("[audio] staying on <audio> elements:", err);
    }
  }

  private playBuffer(slice: Cue, gain: number): void {
    if (!this.ctx || !this.buffer || !this.master) return;
    if (this.ctx.state !== "running") return;

    if (this.live.size > 8) {
      for (const old of this.live) {
        try {
          old.stop();
        } catch {
          /* already ended */
        }
      }
      this.live.clear();
    }

    const src = this.ctx.createBufferSource();
    src.buffer = this.buffer;
    const g = this.ctx.createGain();
    g.gain.value = gain;
    src.connect(g).connect(this.master);
    src.start(0, slice.start, slice.duration);

    this.live.add(src);
    src.onended = () => this.live.delete(src);
  }
}

export const audio = new UiAudio();

// Dev-only inspection hook (used by tools/shoot.mjs to assert the active engine).
if (import.meta.env.DEV) {
  (window as unknown as Record<string, unknown>).__plAudio = audio;
}
