/**
 * View model for the shelf.
 *
 * The demo array below is only used by the browser preview (`tools/serve.mjs`) and
 * as a seeded fallback; the real library comes from `library.ts`. Both produce this
 * same shape, which is why no component needed changing when SQLite took over.
 */

export type Game = {
  id: string;
  title: string;
  subtitle: string;
  genre: string;
  /** 600x900 portrait cover, cached to disk. Empty when artwork was not fetched. */
  cover: string;
  /** Wide hero art (1920x620 from SteamGridDB), cached to disk. */
  backdrop: string;
  /** Hex accent: the only colour the chrome borrows from the artwork. */
  accent: string;
  playtimeMinutes: number;
  lastPlayed: string;
  achievements: { unlocked: number; total: number };
  description: string;
  /** Set for rows that came from the database; absent for demo entries. */
  executable?: string;
  dbId?: number;
  /** `"pc"` for a Windows executable, `"switch"` for a ROM Ryujinx opens. */
  platform?: "pc" | "switch";
};

export const demoLibrary: Game[] = [
  {
    id: "emberfall",
    title: "Emberfall",
    subtitle: "Chapter II - The Long Dark",
    genre: "Action RPG",
    cover: "/art/cover-emberfall.webp",
    backdrop: "/art/bg-emberfall.webp",
    accent: "#ff7a45",
    playtimeMinutes: 3_412,
    lastPlayed: "Played 2 hours ago",
    achievements: { unlocked: 31, total: 48 },
    description:
      "A burning kingdom, six hours of ash, and one sword that remembers every hand that held it.",
  },
  {
    id: "voidrunner",
    title: "Voidrunner",
    subtitle: "Season 4 - Deep Signal",
    genre: "Roguelite",
    cover: "/art/cover-voidrunner.webp",
    backdrop: "/art/bg-voidrunner.webp",
    accent: "#6ea8ff",
    playtimeMinutes: 1_188,
    lastPlayed: "Played yesterday",
    achievements: { unlocked: 52, total: 60 },
    description:
      "Fly further than the map allows. Every run rewrites the station you are running through.",
  },
  {
    id: "sable-tide",
    title: "Sable Tide",
    subtitle: "Tidelands Expansion",
    genre: "Exploration",
    cover: "/art/cover-sable-tide.webp",
    backdrop: "/art/bg-sable-tide.webp",
    accent: "#5fd0c4",
    playtimeMinutes: 764,
    lastPlayed: "Played 3 days ago",
    achievements: { unlocked: 12, total: 40 },
    description:
      "Sail a drowned archipelago where the tide is the only clock and every island keeps a secret.",
  },
  {
    id: "iron-meridian",
    title: "Iron Meridian",
    subtitle: "Co-op Campaign",
    genre: "Tactical FPS",
    cover: "/art/cover-iron-meridian.webp",
    backdrop: "/art/bg-iron-meridian.webp",
    accent: "#c9a227",
    playtimeMinutes: 2_045,
    lastPlayed: "Played last week",
    achievements: { unlocked: 22, total: 55 },
    description:
      "Four operators, one corridor, zero respawns. Built to be played with the same three people forever.",
  },
  {
    id: "neon-requiem",
    title: "Neon Requiem",
    subtitle: "Director's Cut",
    genre: "Stealth",
    cover: "/art/cover-neon-requiem.webp",
    backdrop: "/art/bg-neon-requiem.webp",
    accent: "#ff5fa2",
    playtimeMinutes: 588,
    lastPlayed: "Played 2 weeks ago",
    achievements: { unlocked: 8, total: 36 },
    description:
      "A city that never powers down, and a courier who only moves during the blackout windows.",
  },
  {
    id: "ashfall-protocol",
    title: "Ashfall Protocol",
    subtitle: "Survival Mode",
    genre: "Survival",
    cover: "/art/cover-ashfall-protocol.webp",
    backdrop: "/art/bg-ashfall-protocol.webp",
    accent: "#9d8bff",
    playtimeMinutes: 132,
    lastPlayed: "Played last month",
    achievements: { unlocked: 3, total: 44 },
    description:
      "The reactor is failing on a schedule you can see. Everything you build has to survive it.",
  },
];

export function credits(minutes: number): string {
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `${hours}h ${rest}m` : `${hours}h`;
}
