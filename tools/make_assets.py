#!/usr/bin/env python3
"""
Asset builder for PirateLink — Phase 1.

Everything the launcher shows or plays at runtime is generated here, so the build
has zero third-party asset licences to track and zero network dependency:

  ui-sprite.ogg / .json   UI cues synthesised from oscillators + filtered noise
  cover-*.webp            600x900 portrait covers
  bg-*.webp               1920x1080, pre-blurred + pre-darkened backdrops
  icon-1024.png           app icon source for `tauri icon`

Run:  python tools/make_assets.py
"""

from __future__ import annotations

import json
import math
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
PUBLIC = ROOT / "public"
ART = PUBLIC / "art"
AUDIO = PUBLIC / "audio"
TOOLS = ROOT / "tools"

SR = 44_100

# ---------------------------------------------------------------------------
# Audio
# ---------------------------------------------------------------------------


def _band(x: np.ndarray, lo: float, hi: float, order: int = 4) -> np.ndarray:
    """Soft-edged band filter via FFT (no scipy needed)."""
    spec = np.fft.rfft(x)
    freqs = np.fft.rfftfreq(len(x), 1 / SR)
    mask = np.ones_like(freqs)
    if lo > 0:
        mask *= 1.0 / (1.0 + (lo / np.maximum(freqs, 1.0)) ** order)
    if hi > 0:
        mask *= 1.0 / (1.0 + (freqs / hi) ** order)
    return np.fft.irfft(spec * mask, n=len(x))


def _sweep_lowpass(x: np.ndarray, f0: float, f1: float, curve: float = 1.0) -> np.ndarray:
    """One-pole lowpass whose cutoff glides from f0 to f1 across the buffer."""
    n = len(x)
    out = np.empty(n, dtype=np.float64)
    y = 0.0
    for i in range(n):
        t = (i / max(1, n - 1)) ** curve
        fc = f0 * (f1 / f0) ** t
        a = 1.0 - math.exp(-2.0 * math.pi * fc / SR)
        y += a * (x[i] - y)
        out[i] = y
    return out


def _t(dur: float) -> np.ndarray:
    return np.arange(int(dur * SR), dtype=np.float64) / SR


def _exp_env(n: int, tau: float) -> np.ndarray:
    return np.exp(-np.arange(n, dtype=np.float64) / (tau * SR))


def _attack(n: int, ms: float) -> np.ndarray:
    a = max(1, int(ms / 1000 * SR))
    env = np.ones(n)
    env[:a] = np.linspace(0, 1, a) ** 2
    return env


def _norm(x: np.ndarray, peak: float = 0.9, fade_ms: float = 6.0) -> np.ndarray:
    x = np.nan_to_num(x)
    m = np.max(np.abs(x)) or 1.0
    x = x / m * peak
    f = max(1, int(fade_ms / 1000 * SR))
    if len(x) > 2 * f:
        x[:f] *= np.linspace(0, 1, f)
        x[-f:] *= np.linspace(1, 0, f)
    return x


def cue_tick() -> np.ndarray:
    """Soft mechanical detent: a tiny transient with a wooden body."""
    t = _t(0.13)
    noise = _band(np.random.default_rng(7).normal(0, 1, len(t)), 1800, 6500)
    click = noise * _exp_env(len(t), 0.006) * 0.55
    body = np.sin(2 * math.pi * 1180 * t) * _exp_env(len(t), 0.018) * 0.32
    body += np.sin(2 * math.pi * 320 * t) * _exp_env(len(t), 0.026) * 0.14
    return _norm(click + body, 0.72, fade_ms=4)


def cue_select() -> np.ndarray:
    """Confirm: airy rise into a low thump."""
    t = _t(0.54)
    noise = np.random.default_rng(11).normal(0, 1, len(t))
    air = _sweep_lowpass(noise, 500, 5200, curve=0.7)
    air = _band(air, 120, 7000) * (_attack(len(t), 18) * _exp_env(len(t), 0.16)) * 0.5

    freq = 92 * (0.55 ** (t / t[-1]))
    phase = 2 * math.pi * np.cumsum(freq) / SR
    thump = np.sin(phase) * _exp_env(len(t), 0.11) * 0.55

    return _norm(air + thump, 0.9)


def cue_back() -> np.ndarray:
    """Dismiss: short downward thump, no tail."""
    t = _t(0.30)
    freq = 150 * (0.42 ** (t / t[-1]))
    phase = 2 * math.pi * np.cumsum(freq) / SR
    body = np.sin(phase) * _exp_env(len(t), 0.07)
    tick = _band(np.random.default_rng(3).normal(0, 1, len(t)), 900, 4200) * _exp_env(len(t), 0.004) * 0.25
    return _norm(body * 0.85 + tick, 0.7)


def cue_chime() -> np.ndarray:
    """Holographic two-note chime for a friend coming online."""
    t = _t(0.95)
    out = np.zeros(len(t))
    # C6 + G6 + C7: open, bright, not shrill.
    for i, (freq, gain, tau) in enumerate(
        [(1046.5, 0.5, 0.34), (1567.98, 0.3, 0.26), (2093.0, 0.16, 0.2), (3135.96, 0.06, 0.14)]
    ):
        delay = int(i * 0.055 * SR)
        env = _exp_env(len(t), tau)
        tone = np.sin(2 * math.pi * freq * t + 0.4 * np.sin(2 * math.pi * 5.5 * t))
        tone *= env
        out[delay:] += (tone[: len(t) - delay] * gain)
    out *= _attack(len(t), 9)
    return _norm(out, 0.8)


def cue_open() -> np.ndarray:
    """Cinematic low whoosh for opening a title."""
    t = _t(0.62)
    noise = np.random.default_rng(23).normal(0, 1, len(t))
    body = _sweep_lowpass(noise, 180, 3400, curve=0.55)
    body = _band(body, 60, 6000) * (_attack(len(t), 40) * _exp_env(len(t), 0.22)) * 0.6

    freq = 66 * (0.5 ** (t / t[-1]))
    sub = np.sin(2 * math.pi * np.cumsum(freq) / SR) * _exp_env(len(t), 0.18) * 0.5
    return _norm(body + sub, 0.88)


CUES = [
    ("tick", cue_tick, 0.04),
    ("select", cue_select, 0.06),
    ("back", cue_back, 0.06),
    ("chime", cue_chime, 0.08),
    ("open", cue_open, 0.08),
]


def build_audio() -> None:
    AUDIO.mkdir(parents=True, exist_ok=True)

    # Lay cues out back to back with a fixed gap, recording exact offsets.
    cursor = 0.0
    parts: list[np.ndarray] = []
    manifest_cues: dict[str, dict[str, float]] = {}

    for name, fn, gap in CUES:
        sig = fn()
        dur = len(sig) / SR
        manifest_cues[name] = {"start": round(cursor, 4), "duration": round(dur, 4)}
        parts.append(sig)
        parts.append(np.zeros(int(gap * SR)))
        cursor += dur + gap

    sprite = np.concatenate(parts)
    total = len(sprite) / SR

    # 16-bit PCM round trip keeps the sprite small and decodes identically everywhere.
    pcm = np.clip(sprite, -1, 1)
    pcm16 = (pcm * 32767).astype(np.int16)

    wav = AUDIO / "_sprite.wav"
    import wave

    with wave.open(str(wav), "wb") as fh:
        fh.setnchannels(1)
        fh.setsampwidth(2)
        fh.setframerate(SR)
        fh.writeframes(pcm16.tobytes())

    ogg = AUDIO / "ui-sprite.ogg"

    # Opus first: Chromium's Web Audio rejects the Vorbis stream libvorbis emits
    # for this sprite (decodeAudioData -> EncodingError) even though <audio> plays
    # it. Opus is smaller anyway. Vorbis stays as a fallback, then raw WAV.
    ENCODERS = [
        (["-c:a", "libopus", "-b:a", "64k", "-ac", "1", "-ar", "48000"], "opus"),
        (["-c:a", "libvorbis", "-q:a", "4", "-ac", "1", "-ar", str(SR)], "vorbis"),
    ]

    sprite_file = None
    for args, label in ENCODERS:
        enc = subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", str(wav), *args, str(ogg)],
            capture_output=True,
            text=True,
        )
        if enc.returncode == 0 and ogg.exists():
            sprite_file = "/audio/ui-sprite.ogg"
            print(f"  encoded as {label}")
            break
        print(f"  ! {label} encode failed ({enc.stderr.strip()[:100]})")

    if sprite_file is None:
        target = AUDIO / "ui-sprite.wav"
        wav.replace(target)
        sprite_file = "/audio/ui-sprite.wav"

    wav.unlink(missing_ok=True)

    (AUDIO / "ui-sprite.json").write_text(
        json.dumps({"file": sprite_file, "duration": round(total, 4), "cues": manifest_cues}, indent=2),
        encoding="utf-8",
    )
    size = (AUDIO / Path(sprite_file).name).stat().st_size
    print(f"  ui-sprite: {total:.2f}s, {size / 1024:.1f} KB -> {sprite_file}")


# ---------------------------------------------------------------------------
# Art
# ---------------------------------------------------------------------------

FONT_DIR = Path("C:/Windows/Fonts")


def font(name: str, size: int) -> ImageFont.FreeTypeFont:
    for candidate in (name, "segoeuib.ttf", "segoeui.ttf", "arialbd.ttf", "arial.ttf"):
        p = FONT_DIR / candidate
        if p.exists():
            try:
                return ImageFont.truetype(str(p), size)
            except OSError:
                continue
    return ImageFont.load_default()


def tracked(draw: ImageDraw.ImageDraw, xy, text, fnt, fill, tracking: float) -> None:
    x, y = xy
    for ch in text:
        draw.text((x, y), ch, font=fnt, fill=fill)
        x += draw.textlength(ch, font=fnt) + tracking


def tracked_width(draw: ImageDraw.ImageDraw, text, fnt, tracking: float) -> float:
    return sum(draw.textlength(ch, font=fnt) + tracking for ch in text) - tracking


def vertical_gradient(size, top, bottom) -> Image.Image:
    w, h = size
    ramp = np.linspace(0, 1, h)[:, None]
    top_a = np.array(top, dtype=np.float64)
    bot_a = np.array(bottom, dtype=np.float64)
    rows = (top_a[None, :] * (1 - ramp) + bot_a[None, :] * ramp)[:, None, :]
    arr = np.repeat(rows, w, axis=1).astype(np.uint8)
    return Image.fromarray(arr, "RGB")


def radial_glow(size, center, radius, color, strength=1.0) -> Image.Image:
    w, h = size
    yy, xx = np.mgrid[0:h, 0:w]
    cx, cy = center[0] * w, center[1] * h
    r = np.sqrt((xx - cx) ** 2 + (yy - cy) ** 2) / radius
    falloff = np.clip(1 - r, 0, 1) ** 2.2 * strength
    layer = np.zeros((h, w, 3), dtype=np.float64)
    for i in range(3):
        layer[..., i] = color[i] * falloff
    return Image.fromarray(np.clip(layer, 0, 255).astype(np.uint8), "RGB")


def grain(size, opacity=0.055, seed=1) -> Image.Image:
    w, h = size
    rng = np.random.default_rng(seed)
    noise = rng.normal(0.5, 0.12, (h, w, 1))
    arr = np.repeat(noise, 3, axis=2)
    return Image.fromarray(np.clip(arr * 255, 0, 255).astype(np.uint8), "RGB")


def hex_rgb(s: str) -> tuple[int, int, int]:
    n = int(s.lstrip("#"), 16)
    return (n >> 16) & 255, (n >> 8) & 255, n & 255


def shade(c, k):
    return tuple(int(max(0, min(255, v * k))) for v in c)


# --- motifs ---------------------------------------------------------------


def motif_emberfall(img, d, c):
    w, h = img.size
    glow = radial_glow((w, h), (0.5, 0.34), w * 0.85, shade(c, 0.9), 0.85)
    img.paste(Image.blend(img, Image.composite(glow, img, glow.convert("L")), 0.55))
    d = ImageDraw.Draw(img)
    d.ellipse([w * 0.16, h * 0.16, w * 0.84, h * 0.5], outline=shade(c, 1.0), width=2)
    d.ellipse([w * 0.24, h * 0.2, w * 0.76, h * 0.46], outline=shade(c, 0.7), width=1)
    d.line([0, h * 0.5, w, h * 0.5], fill=shade(c, 0.55), width=1)
    rng = np.random.default_rng(5)
    for _ in range(90):
        x = rng.uniform(0, w)
        y = rng.uniform(h * 0.05, h * 0.62)
        r = rng.uniform(0.6, 2.2)
        a = rng.uniform(0.15, 0.75)
        d.ellipse([x, y, x + r, y + r], fill=shade(c, a * 1.4))


def motif_voidrunner(img, d, c):
    w, h = img.size
    cx, cy = w * 0.5, h * 0.42
    for i in range(7):
        rr = w * (0.14 + i * 0.075)
        d.ellipse([cx - rr, cy - rr * 0.62, cx + rr, cy + rr * 0.62],
                  outline=shade(c, 0.75 - i * 0.07), width=1)
    d.ellipse([cx - w * 0.055, cy - w * 0.055, cx + w * 0.055, cy + w * 0.055], fill=shade(c, 1.0))
    rng = np.random.default_rng(9)
    for _ in range(160):
        x, y = rng.uniform(0, w), rng.uniform(0, h * 0.85)
        r = rng.uniform(0.4, 1.5)
        d.ellipse([x, y, x + r, y + r], fill=(235, 240, 255))


def motif_sable_tide(img, d, c):
    w, h = img.size
    for i in range(16):
        y = h * 0.34 + i * 14
        amp = 9 * math.sin(i * 0.5)
        pts = [(x, y + math.sin(x / 46 + i) * amp) for x in range(0, w + 8, 8)]
        d.line(pts, fill=shade(c, 0.25 + (i % 5) * 0.09), width=1)
    d.arc([-w * 0.3, h * 0.05, w * 0.72, h * 0.62], 200, 340, fill=shade(c, 0.9), width=2)


def motif_iron_meridian(img, d, c):
    w, h = img.size
    step = w / 26
    for i in range(-12, 34):
        x = i * step
        d.line([x, 0, x + h * 0.45, h * 0.72], fill=shade(c, 0.18), width=max(1, round(step / 23)))
    cx, cy, s = w * 0.5, h * 0.38, min(w, h) * 0.19
    for dx, dy in ((-1, -1), (1, -1), (-1, 1), (1, 1)):
        x0, y0 = cx + dx * s, cy + dy * s
        d.line([x0, y0, x0 - dx * s * 0.42, y0], fill=shade(c, 0.95), width=2)
        d.line([x0, y0, x0, y0 - dy * s * 0.42], fill=shade(c, 0.95), width=2)
    d.line([cx, cy - s * 0.5, cx, cy + s * 0.5], fill=shade(c, 0.55), width=1)
    d.line([cx - s * 0.5, cy, cx + s * 0.5, cy], fill=shade(c, 0.55), width=1)


def motif_neon_requiem(img, d, c):
    w, h = img.size
    rng = np.random.default_rng(13)
    x = 0
    while x < w:
        bw = rng.integers(18, 52)
        bh = rng.integers(70, 260)
        top = h * 0.6 - bh
        d.rectangle([x, top, x + bw - 6, h * 0.62], fill=shade(c, 0.1), outline=shade(c, 0.35), width=1)
        for wy in range(int(top) + 12, int(h * 0.58), 16):
            if rng.random() > 0.55:
                d.rectangle([x + 6, wy, x + bw - 12, wy + 4], fill=shade(c, 0.75))
        x += bw
    d.line([0, h * 0.62, w, h * 0.62], fill=shade(c, 0.8), width=2)


def motif_ashfall(img, d, c):
    w, h = img.size
    r = min(w, h) * 0.032
    cols = int(w / (r * 1.75)) + 2
    rows = int(h / (r * 1.5)) + 2
    for row in range(-1, rows):
        for col in range(-1, cols):
            cx = col * r * 1.75 + (r * 0.87 if row % 2 else 0)
            cy = row * r * 1.5
            pts = [
                (
                    cx + r * math.cos(math.pi / 3 * k + math.pi / 6),
                    cy + r * math.sin(math.pi / 3 * k + math.pi / 6),
                )
                for k in range(6)
            ]
            d.polygon(pts, outline=shade(c, 0.22))
    glow = radial_glow((w, h), (0.5, 0.32), w * 0.42, shade(c, 1.0), 0.9)
    mask = glow.convert("L").point(lambda v: int(v * 0.75))
    img.paste(Image.composite(glow, img, mask), (0, 0), mask)


MOTIFS = {
    "emberfall": motif_emberfall,
    "voidrunner": motif_voidrunner,
    "sable-tide": motif_sable_tide,
    "iron-meridian": motif_iron_meridian,
    "neon-requiem": motif_neon_requiem,
    "ashfall-protocol": motif_ashfall,
}

GAMES = [
    ("emberfall", "EMBERFALL", "ACTION RPG", "#ff7a45", "#1a0d0a", "#2b1108"),
    ("voidrunner", "VOIDRUNNER", "ROGUELITE", "#6ea8ff", "#080b18", "#101a33"),
    ("sable-tide", "SABLE TIDE", "EXPLORATION", "#5fd0c4", "#04120f", "#0a1f1c"),
    ("iron-meridian", "IRON MERIDIAN", "TACTICAL FPS", "#c9a227", "#12100a", "#231d0c"),
    ("neon-requiem", "NEON REQUIEM", "STEALTH", "#ff5fa2", "#120612", "#240a1f"),
    ("ashfall-protocol", "ASHFALL PROTOCOL", "SURVIVAL", "#9d8bff", "#0a0812", "#150f28"),
]


def render_scene(slug: str, size: tuple[int, int], accent: str, top: str, bottom: str) -> Image.Image:
    """Background gradient + motif, rendered natively at the requested size.

    The backdrop is rendered at 1920x1080 rather than upscaled from the 600x900
    poster: upscaling letterboxes the portrait art and blows the title type up
    into an unreadable smear.
    """
    c = hex_rgb(accent)
    img = vertical_gradient(size, hex_rgb(top), hex_rgb(bottom))
    MOTIFS[slug](img, ImageDraw.Draw(img), c)
    return img


def make_cover(slug: str, title: str, genre: str, accent: str, top: str, bottom: str) -> Image.Image:
    w, h = 600, 900
    c = hex_rgb(accent)
    img = render_scene(slug, (w, h), accent, top, bottom).filter(ImageFilter.GaussianBlur(0.4))

    # Bottom scrim so the type always has contrast.
    scrim = Image.new("L", (1, h))
    for y in range(h):
        t = y / h
        v = 0 if t < 0.42 else int(255 * ((t - 0.42) / 0.58) ** 1.35)
        scrim.putpixel((0, y), v)
    scrim = scrim.resize((w, h))
    img = Image.composite(Image.new("RGB", (w, h), (5, 6, 11)), img, scrim.point(lambda v: int(v * 0.84)))

    d = ImageDraw.Draw(img)

    # Genre label, top-left, tracked out.
    f_genre = font("segoeuib.ttf", 15)
    tracked(d, (40, 40), genre, f_genre, shade(c, 1.1), 3.4)

    # Accent rule + title, bottom-left. Type is measured with the tracking it is
    # actually drawn with, so nothing can touch the poster edge.
    max_w = w - 80
    tracking = -1.2
    size = 76
    title_font = font("segoeuib.ttf", size)
    while size > 36 and tracked_width(d, title, title_font, tracking) > max_w:
        size -= 2
        title_font = font("segoeuib.ttf", size)

    # Only wrap once the single line cannot shrink any further.
    if tracked_width(d, title, title_font, tracking) > max_w:
        words = title.split()
        mid = (len(words) + 1) // 2
        lines = [" ".join(words[:mid]), " ".join(words[mid:])]
        while size > 28 and max(tracked_width(d, ln, title_font, tracking) for ln in lines) > max_w:
            size -= 2
            title_font = font("segoeuib.ttf", size)
    else:
        lines = [title]

    line_h = int(title_font.size * 1.02)
    total_h = line_h * len(lines)
    ty = h - 92 - total_h

    d.line([40, ty - 26, 40 + 44, ty - 26], fill=c, width=3)

    for i, line in enumerate(lines):
        tracked(d, (40, ty + i * line_h), line, title_font, (247, 248, 255), -1.2)

    # Grain + vignette.
    img = Image.blend(img, grain((w, h), seed=len(slug)), 0.06)
    vign = radial_glow((w, h), (0.5, 0.5), w * 0.95, (0, 0, 0), 1.0).convert("L").point(lambda v: 255 - v)
    dark = Image.new("RGB", (w, h), (0, 0, 0))
    img = Image.composite(img, dark, vign.point(lambda v: int(60 + v * 0.76)))

    return img


def make_backdrop(slug: str, accent: str, top: str, bottom: str) -> Image.Image:
    """1920x1080, blurred and darkened once at build time (never at runtime).

    Uses its own brighter palette: reusing the poster's near-black gradient and
    then blurring it produces a frame that is indistinguishable from #000, which
    is exactly what a full-screen background must not be.
    """
    w, h = 1920, 1080
    c = hex_rgb(accent)
    base = np.array([9, 11, 18], dtype=np.float64)

    def mix(k: float) -> tuple[int, int, int]:
        return tuple(int(v) for v in (np.array(c, dtype=np.float64) * k + base * (1 - k)))

    img = render_scene(slug, (w, h), accent, "#{:02x}{:02x}{:02x}".format(*mix(0.4)), "#{:02x}{:02x}{:02x}".format(*mix(0.07)))
    img = img.filter(ImageFilter.GaussianBlur(12))
    img = Image.eval(img, lambda v: int(v * 0.86))

    glow = radial_glow((w, h), (0.5, 0.34), w * 0.55, shade(c, 0.85), 0.55)
    img = Image.blend(img, Image.composite(glow, img, glow.convert("L")), 0.38)

    # Vignette: mask is 255 at the edges, 0 in the centre (composite() takes the
    # first image where the mask is bright, so this must be edge-weighted).
    edge = radial_glow((w, h), (0.5, 0.5), w * 0.8, (0, 0, 0), 1.0).convert("L")
    edge = edge.point(lambda v: int((255 - v) * 0.7))
    return Image.composite(img, Image.new("RGB", (w, h), (4, 5, 10)), edge)


def build_art() -> None:
    ART.mkdir(parents=True, exist_ok=True)
    for slug, title, genre, accent, top, bottom in GAMES:
        cover = make_cover(slug, title, genre, accent, top, bottom)
        cover.save(ART / f"cover-{slug}.webp", "WEBP", quality=84, method=6)

        bg = make_backdrop(slug, accent, top, bottom)
        bg.save(ART / f"bg-{slug}.webp", "WEBP", quality=80, method=6)

        cs = (ART / f"cover-{slug}.webp").stat().st_size
        bs = (ART / f"bg-{slug}.webp").stat().st_size
        print(f"  {slug:16s} cover {cs / 1024:5.1f} KB   bg {bs / 1024:6.1f} KB")


def build_icon() -> None:
    """1024px app icon: rounded slab, accent sweep, monogram."""
    s = 1024
    img = vertical_gradient((s, s), (14, 17, 28), (6, 7, 12))
    d = ImageDraw.Draw(img)

    glow = radial_glow((s, s), (0.36, 0.3), s * 0.72, (122, 92, 255), 0.9)
    img = Image.composite(glow, img, glow.convert("L").point(lambda v: int(v * 0.7)))
    d = ImageDraw.Draw(img)

    d.rounded_rectangle([40, 40, s - 40, s - 40], radius=210, outline=(255, 255, 255, 40), width=6)

    f = font("segoeuib.ttf", 400)
    text = "PL"
    tw = d.textlength(text, font=f)
    d.text(((s - tw) / 2, s * 0.5 - 250), text, font=f, fill=(245, 247, 255))

    d.line([s * 0.3, s * 0.72, s * 0.7, s * 0.72], fill=(122, 160, 255), width=10)

    img.convert("RGB").save(TOOLS / "icon-1024.png", "PNG")


def main() -> None:
    print("audio:")
    build_audio()
    print("art:")
    build_art()
    print("icon:")
    build_icon()
    print(f"  -> {TOOLS / 'icon-1024.png'}")


if __name__ == "__main__":
    main()
