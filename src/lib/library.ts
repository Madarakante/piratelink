/**
 * Library data access.
 *
 * The Rust side owns SQLite and SteamGridDB; this module is the only place that
 * talks to it. Components keep consuming the `Game` view model from `games.ts`,
 * so swapping the demo array for real rows required no component changes.
 */

import type { Game } from "./games";
import { demoLibrary } from "./games";
import { convertFileSrc } from "@tauri-apps/api/core";

export type LibrarySource = "db" | "demo" | "empty";

export type LibraryResult = {
  games: Game[];
  source: LibrarySource;
  /** True when a SteamGridDB key is configured on the Rust side. */
  canFetchArt: boolean;
};

type Row = {
  id: number;
  title: string;
  executable: string;
  install_dir: string;
  platform: Platform;
  sgdb_id: number | null;
  cover_path: string | null;
  hero_path: string | null;
  logo_path: string | null;
  accent: string | null;
  playtime_minutes: number;
  last_played_at: string | null;
  added_at: string;
};

export type Platform = "pc" | "switch";

export type GameFileInfo = {
  path: string;
  /** Best guess at the title: the install folder for a .exe, the filename for a ROM. */
  title: string;
  install_dir: string;
  platform: Platform;
  size_bytes: number;
  already_in_library: boolean;
};

/** Extensions the picker offers, in the order they appear in the dialog. */
const GAME_EXTENSIONS = ["exe", "nsp", "xci", "nro"];

/** True inside the Tauri shell; false in the browser preview, where every command is faked. */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const core = await import("@tauri-apps/api/core");
  return core.invoke<T>(cmd, args);
}

/**
 * Turns a cached file path into something an <img> can load.
 *
 * The query string is a cache-buster, not a parameter: the asset protocol reads
 * only the path. Without it a re-fetched cover keeps the same URL and the webview
 * happily shows the old bitmap, which made "Refresh artwork" look broken. The base
 * is the page load time, so nothing survives a restart; `bumpArtEpoch` moves it on
 * every successful artwork write.
 */
let artEpoch = Date.now();

function bumpArtEpoch() {
  artEpoch = Date.now();
}

function assetUrl(path: string | null): string {
  if (!path || !isTauri) return "";
  // "asset" is the protocol registered by assetProtocol; the scope in
  // tauri.conf.json is what decides which files are actually reachable.
  return `${convertFileSrc(path, "asset")}?v=${artEpoch}`;
}

/** Stable accent per title, so art-less games still get a distinct colour. */
function accentFor(title: string): string {
  let hash = 0;
  for (const ch of title) hash = (hash * 31 + ch.charCodeAt(0)) % 360;
  return hslToHex(hash, 64, 62);
}

function hslToHex(h: number, s: number, l: number): string {
  const a = (s / 100) * Math.min(l / 100, 1 - l / 100);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    const color = l / 100 - a * Math.max(-1, Math.min(k - 3, Math.min(9 - k, 1)));
    return Math.round(255 * color)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

function relativePlayed(iso: string | null): string {
  if (!iso) return "Never played";
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return "Never played";

  const minutes = Math.floor((Date.now() - then) / 60_000);
  if (minutes < 60) return "Played just now";
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Played ${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "Played yesterday";
  if (days < 30) return `Played ${days} days ago`;
  const months = Math.floor(days / 30);
  return months === 1 ? "Played last month" : `Played ${months} months ago`;
}

function toView(row: Row): Game {
  return {
    id: String(row.id),
    title: row.title,
    // No tagline in the data model. The install folder used to go here, but it
    // reads as "Game" for nested layouts, so rows simply have no subtitle.
    subtitle: "",
    genre: "",
    cover: assetUrl(row.cover_path),
    backdrop: assetUrl(row.hero_path),
    accent: row.accent ?? accentFor(row.title),
    playtimeMinutes: row.playtime_minutes,
    lastPlayed: relativePlayed(row.last_played_at),
    achievements: { unlocked: 0, total: 0 },
    description: "",
    executable: row.executable,
    dbId: row.id,
    platform: row.platform,
  };
}

// ---------------------------------------------------------------------------
// Browser preview backend
//
// `node tools/serve.mjs` serves the built UI without Tauri, which is how the
// interface is iterated on and screenshotted. In that mode every command is
// answered from memory so the empty state, the import sheet and the shelf are all
// reachable without the app (or a SteamGridDB key). Never used inside Tauri.
// ---------------------------------------------------------------------------

const previewParams =
  typeof location !== "undefined" ? new URLSearchParams(location.search) : null;

/** `?preview=empty` starts with an empty library so the onboarding state is reachable. */
let previewGames: Game[] =
  previewParams?.get("preview") === "empty"
    ? []
    : // Demo rows get an id so the same "change art" and "remove" paths work in the
      // browser preview as in the app.
      demoLibrary.map((game, index) => ({ ...game, dbId: game.dbId ?? index + 1 }));

const PREVIEW_EXECUTABLES: GameFileInfo[] = [
  {
    path: "D:\\Games\\Hades\\Hades.exe",
    title: "Hades",
    install_dir: "D:\\Games\\Hades",
    platform: "pc",
    size_bytes: 67_108_864,
    already_in_library: false,
  },
  {
    path: "D:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe",
    title: "Cyberpunk 2077",
    install_dir: "D:\\Games\\Cyberpunk 2077\\bin\\x64",
    platform: "pc",
    size_bytes: 71_303_168,
    already_in_library: false,
  },
  {
    path: "D:\\Switch\\The Legend of Zelda - Tears of the Kingdom [0100F2C0115B6000][v0].nsp",
    title: "The Legend of Zelda Tears of the Kingdom",
    install_dir: "D:\\Switch",
    platform: "switch",
    size_bytes: 16_106_127_360,
    already_in_library: false,
  },
];

const PREVIEW_ART_GAMES: ArtGame[] = [
  { id: 4997889, name: "Hades", verified: true },
  { id: 5303117, name: "The Legend of Zelda: Tears of the Kingdom", verified: true },
  { id: 5495669, name: "EA SPORTS FCT 26", verified: true },
];

/** The picker's demo page: real covers and heroes for Hades. */
const PREVIEW_ART_CHOICES: ArtChoices = {
  covers: [
    { id: 165984, width: 600, height: 900, thumb: "https://cdn2.steamgriddb.com/thumb/ce8a253393a1bbbb3d72cd2093b81ede.jpg", url: "https://cdn2.steamgriddb.com/grid/ce8a253393a1bbbb3d72cd2093b81ede.png" },
    { id: 141212, width: 600, height: 900, thumb: "https://cdn2.steamgriddb.com/thumb/4cb1c17b82ffc973aa1d52670a17c5a8.jpg", url: "https://cdn2.steamgriddb.com/grid/4cb1c17b82ffc973aa1d52670a17c5a8.png" },
    { id: 63955, width: 600, height: 900, thumb: "https://cdn2.steamgriddb.com/thumb/bfe339860b048949369fc6945cea504d.jpg", url: "https://cdn2.steamgriddb.com/grid/bfe339860b048949369fc6945cea504d.png" },
    { id: 636266, width: 600, height: 900, thumb: "https://cdn2.steamgriddb.com/thumb/fb517b661a6899f73abc714321b576c6.jpg", url: "https://cdn2.steamgriddb.com/grid/fb517b661a6899f73abc714321b576c6.png" },
    { id: 767054, width: 600, height: 900, thumb: "https://cdn2.steamgriddb.com/thumb/121bbbbd09d94972317f0b99506f344e.png", url: "https://cdn2.steamgriddb.com/grid/121bbbbd09d94972317f0b99506f344e.png" },
    { id: 348353, width: 600, height: 900, thumb: "https://cdn2.steamgriddb.com/thumb/2fa384be1daf050a816980c7413da8a0.jpg", url: "https://cdn2.steamgriddb.com/grid/2fa384be1daf050a816980c7413da8a0.png" },
    { id: 239093, width: 600, height: 900, thumb: "https://cdn2.steamgriddb.com/thumb/822a487112102b1117cd2ad8509a42c8.jpg", url: "https://cdn2.steamgriddb.com/grid/822a487112102b1117cd2ad8509a42c8.png" },
    { id: 158642, width: 600, height: 900, thumb: "https://cdn2.steamgriddb.com/thumb/12cd0b326e3445d0c19f49c1cdfaaa5d.jpg", url: "https://cdn2.steamgriddb.com/grid/12cd0b326e3445d0c19f49c1cdfaaa5d.png" },
  ],
  heroes: [
    { id: 30050, width: 1920, height: 620, thumb: "https://cdn2.steamgriddb.com/hero_thumb/56bbc93a8425b95e4ab7c789751475bf.jpg", url: "https://cdn2.steamgriddb.com/hero/56bbc93a8425b95e4ab7c789751475bf.png" },
    { id: 35720, width: 1920, height: 620, thumb: "https://cdn2.steamgriddb.com/hero_thumb/1a23efcb39da8db7ca95ea8085d096a1.jpg", url: "https://cdn2.steamgriddb.com/hero/1a23efcb39da8db7ca95ea8085d096a1.png" },
    { id: 20733, width: 1920, height: 620, thumb: "https://cdn2.steamgriddb.com/hero_thumb/67eb5e92661b494826add1d403863257.png", url: "https://cdn2.steamgriddb.com/hero/67eb5e92661b494826add1d403863257.png" },
    { id: 29936, width: 1920, height: 620, thumb: "https://cdn2.steamgriddb.com/hero_thumb/45402d4ff8981a182dcfc4813600961f.jpg", url: "https://cdn2.steamgriddb.com/hero/45402d4ff8981a182dcfc4813600961f.png" },
  ],
};

let previewChoices: ArtChoices = PREVIEW_ART_CHOICES;

export async function loadLibrary(): Promise<LibraryResult> {
  if (!isTauri) {
    return {
      games: previewGames,
      source: previewGames.length ? "demo" : "empty",
      canFetchArt: true,
    };
  }

  const [rows, status] = await Promise.all([
    invoke<Row[]>("library_list"),
    invoke<{ configured: boolean }>("sgdb_status").catch(() => ({ configured: false })),
  ]);

  return {
    games: rows.map(toView),
    source: rows.length ? "db" : "empty",
    canFetchArt: status.configured,
  };
}

/**
 * Asks for one or more game files: Windows executables or console ROMs.
 *
 * Importing used to start from a folder scan; picking the file directly is both
 * shorter and more precise, because the user has already made the judgement the
 * scanner was guessing at.
 */
export async function pickGameFiles(): Promise<string[]> {
  if (!isTauri) return PREVIEW_EXECUTABLES.map((exe) => exe.path);

  const dialog = await import("@tauri-apps/plugin-dialog");
  const picked = await dialog.open({
    multiple: true,
    title: "Choose a game executable or ROM",
    filters: [
      { name: "Games", extensions: GAME_EXTENSIONS },
      { name: "Executables", extensions: ["exe"] },
      { name: "Switch ROMs", extensions: ["nsp", "xci", "nro"] },
    ],
  });

  if (!picked) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/**
 * Reads a chosen game file and suggests a title for it.
 *
 * The suggestion is never hidden: the sheet puts it in an editable field, so a
 * filename that does not match the game costs one correction instead of a wrong
 * SteamGridDB search.
 */
export async function inspectFile(path: string): Promise<GameFileInfo> {
  if (!isTauri) {
    const known = PREVIEW_EXECUTABLES.find((exe) => exe.path === path);
    const already = previewGames.some((game) => game.executable === path);
    if (known) return { ...known, already_in_library: already };
    const file = path.split(/[\\/]/).pop() ?? path;
    return {
      path,
      title: file.replace(/\.(exe|nsp|xci|nro)$/i, ""),
      install_dir: path.replace(/[\\/][^\\/]+$/, ""),
      platform: /\.(nsp|xci|nro)$/i.test(path) ? "switch" : "pc",
      size_bytes: 0,
      already_in_library: already,
    };
  }

  return invoke<GameFileInfo>("inspect_file", { path });
}

export async function addGame(path: string, title: string): Promise<number> {
  if (!isTauri) {
    const id = Math.max(0, ...previewGames.map((game) => game.dbId ?? 0)) + 1;
    previewGames = [
      ...previewGames,
      {
        id: String(id),
        dbId: id,
        title: title || path.split(/[\\/]/).pop() || "Untitled",
        subtitle: path.split(/[\\/]/).slice(-2, -1)[0] ?? "",
        genre: "",
        cover: "",
        backdrop: "",
        accent: accentFor(title || path),
        playtimeMinutes: 0,
        lastPlayed: "Never played",
        achievements: { unlocked: 0, total: 0 },
        description: "",
        executable: path,
      },
    ];
    return id;
  }

  const row = await invoke<{ id: number }>("add_game", { path, title });
  return row.id;
}

export async function removeGame(id: number): Promise<void> {
  if (!isTauri) {
    previewGames = previewGames.filter((g) => g.dbId !== id);
    return;
  }
  await invoke("remove_game", { id });
}

export async function fetchArtwork(id: number): Promise<void> {
  if (!isTauri) return;
  await invoke("fetch_artwork", { id });
  bumpArtEpoch();
}

// ---------------------------------------------------------------------------
// Manual art picker
// ---------------------------------------------------------------------------

export type ArtGame = { id: number; name: string; verified: boolean };

export type ArtOption = {
  id: number;
  /** ~267x400 JPEG, cheap enough to lay out a page of them. */
  thumb: string;
  /** The full asset, downloaded only when the choice is applied. */
  url: string;
  width: number;
  height: number;
};

export type ArtChoices = { covers: ArtOption[]; heroes: ArtOption[] };

export async function searchArtwork(query: string): Promise<ArtGame[]> {
  if (!isTauri) {
    const needle = query.trim().toLowerCase();
    const hits = PREVIEW_ART_GAMES.filter((game) => game.name.toLowerCase().includes(needle));
    return hits.length ? hits : PREVIEW_ART_GAMES;
  }
  return invoke<ArtGame[]>("search_artwork", { query });
}

export async function artworkChoices(sgdbId: number): Promise<ArtChoices> {
  if (!isTauri) {
    // Real SteamGridDB thumbnails (Hades) so the sheet looks like itself in the
    // browser preview; no key is needed to *view* the CDN.
    previewChoices = PREVIEW_ART_CHOICES;
    return previewChoices;
  }
  return invoke<ArtChoices>("artwork_choices", { sgdbId });
}

export async function applyArtwork(
  id: number,
  sgdbId: number,
  coverId: number | null,
  heroId: number | null,
): Promise<void> {
  if (!isTauri) {
    const cover = previewChoices.covers.find((option) => option.id === coverId)?.url;
    const hero = previewChoices.heroes.find((option) => option.id === heroId)?.url;
    previewGames = previewGames.map((game) =>
      game.dbId === id
        ? { ...game, cover: cover ?? game.cover, backdrop: hero ?? game.backdrop }
        : game,
    );
    bumpArtEpoch();
    return;
  }
  await invoke("apply_artwork", { id, sgdbId, coverId, heroId });
  bumpArtEpoch();
}

export async function launchGame(dbId: number): Promise<void> {
  if (!isTauri) return;
  await invoke("launch_game", { id: dbId });
}
