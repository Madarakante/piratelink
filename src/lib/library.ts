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
  sgdb_id: number | null;
  cover_path: string | null;
  hero_path: string | null;
  logo_path: string | null;
  accent: string | null;
  playtime_minutes: number;
  last_played_at: string | null;
  added_at: string;
};

export type Candidate = {
  path: string;
  install_dir: string;
  name_hint: string;
  score: number;
  reasons: string[];
  size_bytes: number;
  already_in_library: boolean;
};

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const core = await import("@tauri-apps/api/core");
  return core.invoke<T>(cmd, args);
}

/** Turns a cached file path into something an <img> can load. */
function assetUrl(path: string | null): string {
  if (!path || !isTauri) return "";
  // "asset" is the protocol registered by assetProtocol; the scope in
  // tauri.conf.json is what decides which files are actually reachable.
  return convertFileSrc(path, "asset");
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
let previewGames: Game[] = previewParams?.get("preview") === "empty" ? [] : [...demoLibrary];

const PREVIEW_CANDIDATES: Candidate[] = [
  {
    path: "D:\\Games\\Hades\\Hades.exe",
    install_dir: "D:\\Games\\Hades",
    name_hint: "Hades",
    score: 12,
    reasons: ["named after its folder", "64 MB binary", "close to the root"],
    size_bytes: 67_108_864,
    already_in_library: false,
  },
  {
    path: "D:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe",
    install_dir: "D:\\Games\\Cyberpunk 2077\\bin\\x64",
    name_hint: "Cyberpunk 2077",
    score: 9,
    reasons: ["64 MB binary"],
    size_bytes: 71_303_168,
    already_in_library: false,
  },
  {
    path: "D:\\Games\\Sable Tide\\SableTide-Win64-Shipping.exe",
    install_dir: "D:\\Games\\Sable Tide",
    name_hint: "Sable Tide",
    score: 11,
    reasons: ["Unreal shipping build", "58 MB binary", "ships the Steam API stub"],
    size_bytes: 60_817_408,
    already_in_library: false,
  },
];

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

export async function pickFolder(): Promise<string | null> {
  if (!isTauri) return "D:\\Games";

  const dialog = await import("@tauri-apps/plugin-dialog");
  const picked = await dialog.open({ directory: true, multiple: false, title: "Add a games folder" });
  return typeof picked === "string" ? picked : null;
}

export async function addRoot(path: string): Promise<void> {
  if (!isTauri) return;
  await invoke("add_library_root", { path });
}

export async function scanLibrary(): Promise<Candidate[]> {
  if (!isTauri) return PREVIEW_CANDIDATES;
  return invoke<Candidate[]>("scan_library");
}

export async function addGame(path: string, title: string): Promise<number> {
  if (!isTauri) {
    const id = Math.floor(Math.random() * 1_000_000);
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
}

export async function launchGame(dbId: number): Promise<void> {
  if (!isTauri) return;
  await invoke("launch_game", { id: dbId });
}
