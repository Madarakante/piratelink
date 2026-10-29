# PirateLink

A console-style game shelf for Windows: horizontal cover carousel, frosted-glass
panels, spring physics, and UI sound — built on Tauri v2 + Svelte 5 + Rust, with a
memory budget it does **not** currently meet (see below).

**Status: Phase 1 and Phase 2 complete.** Scan a folder, confirm what it found, and
covers are fetched from SteamGridDB and cached to disk. The UI still needs its
design pass, and the memory target is not hit yet.

![The shelf](docs/shelf.png)

---

## What works right now

| Area | Detail |
| --- | --- |
| Shelf | Centre-focused carousel, spring-driven, wraps at both ends, ±2 covers decoded |
| Library | SQLite under the app data dir; scan a folder, confirm candidates, remove entries |
| Artwork | SteamGridDB cover + hero, re-encoded to WebP at ingest, served from the cache |
| Input | Keyboard (`←` `→` `Enter` `Esc` `M` `N` `F3`) **and** gamepad (Xbox / DualSense / DualShock, standard mapping, hold-to-repeat) |
| Motion | Custom critically-damped spring in `src/lib/spring.svelte.ts`; parks its `requestAnimationFrame` loop the moment it settles |
| Audio | One 27 KB OGG sprite, five cues, sliced at playback |
| Window | Frameless, custom titlebar, drag region, native min/max/close |
| Boot | ~620 ms splash, then the shelf |

| | | |
| --- | --- | --- |
| ![Empty](docs/empty.png) | ![Import](docs/import.png) | ![Details](docs/detail.png) |

## What is deliberately *not* in the box

The Goldberg / `gbe_fork` auto-injector from the original plan (Phase 3) is not
implemented and will not be. `gbe_fork` exists to satisfy Steam's ownership check
for copies the user does not own, so a one-click injector with backup/restore is
DRM circumvention rather than a launcher feature. The other four phases stand on
their own and are unaffected.

---

## Measured on this machine

| Metric | Value |
| --- | --- |
| Frontend bundle (`dist/`) | 301 KB across 20 files |
| Main JS / CSS | 77.6 KB (29 KB gzip) / 28.3 KB (6 KB gzip) |
| Release `.exe` (`tauri build --no-bundle`) | 7.1 MB (image, HTTP/TLS and SQLite linked in) |
| Decoded art on screen | 1 hero + up to 5 covers ≈ 12 MB raster |
| Renderer JS heap | 4.5 MB (`F3` overlay) |
| **Whole-process RAM** | **461 MB — over the 200 MB target** |
| Folder scan, `D:\Games` (10 installs) | ~50 ms, 13 candidates |

### Memory is not where it should be

Measured on the running release build: **55.4 MB** in the Tauri process and
**405 MB across seven WebView2 processes**, 461 MB total. The 200 MB ceiling from
the brief is not met, and this README previously implied it was without ever
having measured it.

`tools\measure-memory.ps1` samples the app plus its whole child tree, so this is a
number we can move deliberately:

```powershell
powershell -ExecutionPolicy Bypass -File tools\measure-memory.ps1 -Seconds 30
```

Suspects to test next, in order of likely payoff:

1. **The film-grain overlay** — a full-screen `feTurbulence` SVG with
   `mix-blend-mode: overlay`, which forces a composited full-screen layer.
2. **`backdrop-filter` panels** — blur cost scales with surface area; the sheet and
   overlay scrims are large.
3. **The drifting hero** — a 1920x620 layer painting continuously, even when idle.
4. WebView2 baseline itself (~120-200 MB here before any of our content).

## Requirements

- **Rust** 1.77+ — this machine builds with `x86_64-pc-windows-gnu`; `tauri-build`
  needs MinGW's `windres`. The MSVC target works if VS Build Tools are installed.
- **Node** 20+ (built on Node 24, Vite 8)
- **WebView2 runtime** — already present on Windows 11 and patched Windows 10

## Getting started

```powershell
npm install
npm run tauri:dev      # dev window with HMR (starts Vite for you)
npm run tauri:build    # release .exe + NSIS installer
```

Preview the built UI in a browser, without Tauri:

```powershell
npm run build
node tools\serve.mjs --port 4174 --root dist
```

The preview runs the real components against an in-memory backend, so the whole
flow is clickable without the app: `?preview=empty` starts with an empty library,
"Add games" walks the import sheet, and confirmations land on the shelf. That is
how the screenshots here are produced (`tools/shoot.mjs`).

Regenerate all art, audio and the icon (deterministic, offline):

```powershell
python tools\make_assets.py
npx tauri icon tools\icon-1024.png
```

### Which binary to run

| Artifact | Standalone? |
| --- | --- |
| `src-tauri/target/release/piratelink.exe` | ✅ embeds the app — double-click it |
| `npm run tauri:dev` | ✅ but it owns a Vite process |
| `src-tauri/target/debug/piratelink.exe` | ❌ **black window** |

A Tauri *debug* binary does not embed the frontend: it points at
`http://localhost:1420` and expects `npm run dev` to be running. Launched on its
own it paints the window background and nothing else. That is a Tauri build-mode
behaviour, not an app bug — for a double-clickable debug build, use
`npx tauri build --debug --no-bundle`.

---

## Verification

Run and passing:

```
npm run check      -> 0 errors, 0 warnings
npm run build      -> 301 KB dist
cargo test         -> 41 passed, 2 ignored (live)
npx tauri build --no-bundle -> target/release/piratelink.exe
```

Two build-level traps worth recording:

- **The CSS minifier silently removed every glass panel.** `app.css` shipped both
  `backdrop-filter` and `-webkit-backdrop-filter`; Lightning CSS collapsed the pair
  to the *prefixed* form, and Chromium 155 does not implement that alias at all, so
  every frosted surface rendered flat while the source looked correct. The prefixed
  line is gone, and the built CSS is checked after each build.
- **`protocol-asset` is a crate feature, not just config.** Turning on
  `assetProtocol` in `tauri.conf.json` fails the build script until
  `tauri = { features = ["protocol-asset"] }` is also set.

**The Tauri window does not start in the environment this was built in.** The
binary panics during setup with `Access is denied (os error 5)` from WebView2
environment creation. This is not project configuration: a stock, minimal
`tauri.conf.json` with one plain decorated window fails identically here, while
`notepad.exe` and headless Edge both run fine. It reproduces only under the agent
sandbox, so `npm run tauri:dev` is expected to work on a real desktop.

Because of that, the UI is verified by serving the production bundle and driving
Edge over the DevTools Protocol. That server sends the **same CSP** the packaged
app uses, so policy regressions show up there rather than only inside the `.exe`.
This is how the `media-src blob:` hole was caught: the audio layer had moved to
blob URLs, which the original policy blocked.

Audio is verified out-of-band: the sprite decodes to 2.86 s with peak 0.902, and
every cue window contains signal at its manifest offset (tick 0.062 RMS, select
0.188, back 0.149, chime 0.189, open 0.230). Browser playback could **not** be
confirmed in this environment — this Edge build has no compressed-media decoder
for Web Audio, and media requests were answered by an intermediary with
`204 No Content`. `src/lib/audio.ts` therefore ships both engines and picks one at
runtime.

---

## Layout

```
src/
  app.css                 design tokens, glass/chip/button primitives
  App.svelte              state, action dispatch, hero + stage composition
  lib/
    games.ts              Game view model + demo entries for the browser preview
    library.ts            the only module that talks to Rust (and the preview backend)
    audio.ts              sprite loader, <audio> + Web Audio engines
    input.ts              keyboard + gamepad -> one Action stream
    spring.svelte.ts      ~50-line critically-damped spring
    components/           Backdrop, TopBar, Carousel, DetailOverlay, HintBar, Toast,
                          EmptyState, ImportSheet
src-tauri/
  src/db.rs               SQLite schema, migrations, queries
  src/scanner.rs          executable walk, scoring, title cleanup
  src/sgdb.rs             SteamGridDB client and match confidence
  src/artwork.rs          decode / downscale / re-encode to WebP
  src/config.rs           app data dir + API key
  src/lib.rs              Tauri commands, app state, command log
  tauri.conf.json         frameless window, CSP, asset protocol, bundle config
  capabilities/           window-control and dialog permissions
public/
  art/                    6 covers + 6 backdrops (generated, used by demo/preview)
  audio/                  ui-sprite.ogg + ui-sprite.json
tools/
  make_assets.py          synthesises all audio, art and the icon
  measure-memory.ps1      samples the app + WebView2 child tree over time
  serve.mjs               Range-capable static server, sends the app's CSP
  shoot.mjs               DevTools-protocol screenshots of the UI
  screenshot.ps1          captures the live Tauri window (needs it running)
```

---

## Deviations from the original spec

| Spec | Shipped | Why |
| --- | --- | --- |
| Framer Motion | Custom spring (~50 lines) | Framer Motion is React-only; it cannot drive Svelte components |
| Howler.js | Web Audio + `<audio>` fallback | `start(0, offset, duration)` *is* sprite slicing; no dependency, and the fallback covers decoders that reject compressed audio |
| Tailwind for everything | Tailwind preflight + custom CSS | A bespoke shelf is mostly transforms and `backdrop-filter`; utility soup was not smaller |
| Cloudflare Quick Tunnels | Not yet implemented | They mint a random hostname per restart and are explicitly not for production, which is the wrong fit for a presence endpoint clients must reach repeatedly |
| Local `.ogg` from Pixabay/Freesound | Generated from oscillators | Zero licences, zero attribution, reproducible from source |

Two details worth carrying into Phase 4:

- **Bincode**: pin the major version. Bincode 2 replaced `serialize`/`deserialize`
  with `Encode`/`Decode`, so 1.x examples will not compile. Cap frame size and
  version the packet enum before parsing anything off a socket.
- **Presence needs auth before a tunnel.** Nothing in the original plan proved a
  client was a particular account, and a public WebSocket accepts anyone who finds
  the URL.

## Phase 2 — library

Wired end to end: scan -> confirm -> fetch -> shelf.

| Piece | File | Notes |
| --- | --- | --- |
| SQLite library | `db.rs` | `library.db` under the app data dir, WAL, `user_version` migrations |
| Scanner | `scanner.rs` | Depth-limited walk with scoring and *reasons*; refuses to guess silently |
| SteamGridDB | `sgdb.rs` | Bearer auth, tolerant parsing, 429 retry, 125 ms spacing |
| Artwork | `artwork.rs` | Decode, downscale to a 600 px ceiling, re-encode WebP (JPEG/PNG fallback) |
| UI | `src/lib/library.ts`, `ImportSheet.svelte` | One module owns the invoke layer; the sheet is a three-step confirm flow |

Set the API key once (outside the repo, never committed):

```powershell
# %APPDATA%\app.piratelink.launcher\config.json
{ "steamgriddb_api_key": "<your key>" }
```

Tests: **41 unit + 2 live**. The live tests are ignored by default so the suite
stays offline:

```powershell
cargo test                                     # 41 passed
$env:PIRATELINK_SGDB_KEY = "..."
cargo test --lib -- --ignored                  # 2 passed (real API, ~5 s)
```

### How a game is matched to its artwork

Two steps, and both had to be right:

1. **Which folder is the title.** The scanner takes the folder holding the
   executable only if that folder is not structural (`Game`, `bin`, `x64`,
   `Binaries`, `Win64`, `steamapps`, ...); otherwise it walks up. So
   `Elden Ring\Game\eldenring.exe` is titled **Elden Ring**, not "Game".
2. **Which search result is the game.** `sgdb::best_match` scores every result
   (exact, then prefix, then token overlap) and requires **0.6 similarity**.
   Below that, `fetch_artwork` returns an error instead of attaching something
   plausible but wrong.

Both were wrong once, in a way that silently attached an unrelated game's cover:
searching "Game" returns a title literally named *GAME*. A wrong cover is worse
than no cover — it gets cached, and nothing on screen says it is wrong.

If a cover is wrong: open the game and use **Refresh artwork**, or **Remove** and
re-add it so the title is re-derived.

### What the live API actually does

Recorded because the published spec does not say any of it:

- **`score` is `0` on every asset.** Ranking covers by "highest score" returns
  whichever element happened to be last. Trust the API's own ordering (already
  vote/relevance sorted) and filter tags instead.
- **Search results are flat** (`{id, name, types, verified, release_date}`), even
  though `components.schemas.Games` implies a nested `Game -> data` shape. The
  parser accepts both.
- **`release_date` is undocumented** and appears on real responses.
- **Heroes are `1920x620`**, not 16:9, so a hero cannot fill a 1080p window without
  being scaled and self-blurred.
- `limit` above 50 is ignored, and grids paginate (`total=255` for a popular
  title), so 50 is the ceiling per request.

## Troubleshooting

**`%APPDATA%\app.piratelink.launcher\piratelink.log`** — every command writes a
start and an exit line, plus any panic. The release build has no console, so this
is the only trace. Three failures look identical on screen:

| Log shows | Means |
| --- | --- |
| no `scan_library: start` | the command never reached Rust — IPC or permission problem |
| `start` but no `done` | still running, or it panicked (a `PANIC:` line follows) |
| `done in 12 ms, 13 candidate(s)` | the scan finished; the sheet is what is stuck |

The folder scan is fast: `D:\Games` with ten installs takes about 50 ms. The
import sheet also abandons a scan after 20 s rather than spinning forever.

## Security and privacy

- **No telemetry, no update pings.** The only network traffic is SteamGridDB when
  you add a game.
- **CSP** is set in `tauri.conf.json` (`default-src 'self'`, inline styles only,
  `blob:` allowed for audio and images).
- **`launch_game` takes a game id**, not a path: it reads the executable from the
  database, so the webview can request game N but cannot name an arbitrary `.exe`.
- **The API key stays out of the webview.** It lives in `config.json` under the app
  data directory and is only read by the Rust side; no command returns it.
- **Windows Defender will flag the release binary.** Worth stating plainly: a small
  unsigned Rust executable that walks game folders and spawns processes is a shape
  heuristics dislike. Sign the binary if you distribute it, and do not normalise
  "add this folder to exclusions" as the fix.

## Roadmap

- **Phase 1 — Shell & audio** ✅
- **Phase 2 — Library & metadata** ✅ SQLite, scanner, SteamGridDB, artwork cache,
  import flow, asset protocol
- **Memory pass** — next: get 461 MB down toward the 200 MB target, measuring each
  change
- **UI pass** — the layout is still the safe, symmetrical one; the shelf needs a
  real composition idea rather than more polish
- **Phase 3 — Co-op engine** ❌ dropped (see above)
- **Phase 4 — Social & hosting** — `axum` presence server, binary packets, auth
  before the tunnel, stable hostname
