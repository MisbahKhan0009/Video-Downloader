/** Accent palettes. Each pairs a UI accent with the corner glow behind the app. */
export interface Theme {
  id: string;
  name: string;
  /** main accent (buttons, progress, selection) */
  accent: string;
  /** lighter top stop for gradient buttons */
  accentHi: string;
  /** text/icons placed on the accent */
  ink: string;
  /** colour of the corner glow, "r, g, b" */
  glow: string;
}

export const THEMES: Theme[] = [
  { id: "ember", name: "Ember", accent: "#ff7a3d", accentHi: "#ffa06b", ink: "#1a0a02", glow: "255, 104, 38" },
  { id: "rose", name: "Rose", accent: "#ff5e8e", accentHi: "#ff8db0", ink: "#1f0510", glow: "255, 60, 120" },
  { id: "violet", name: "Ultraviolet", accent: "#a37bff", accentHi: "#c0a4ff", ink: "#12082a", glow: "132, 84, 255" },
  { id: "aqua", name: "Aqua", accent: "#2fe0c4", accentHi: "#7befdb", ink: "#03201b", glow: "20, 200, 175" },
  { id: "gold", name: "Champagne", accent: "#f0c477", accentHi: "#f7dba6", ink: "#1f1505", glow: "230, 170, 70" },
  { id: "ice", name: "Ice", accent: "#e8edf5", accentHi: "#ffffff", ink: "#0b0d12", glow: "150, 175, 215" },
];

export const CORNERS = ["0% 0%", "100% 0%", "0% 100%", "100% 100%"] as const;

function hexToRgb(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  return `${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}`;
}

export function themeById(id: string | undefined): Theme {
  return THEMES.find((t) => t.id === id) ?? THEMES[0];
}

export function applyTheme(t: Theme, corner?: string) {
  const r = document.documentElement.style;
  r.setProperty("--accent", t.accent);
  r.setProperty("--accent-rgb", hexToRgb(t.accent));
  r.setProperty("--accent-hover", t.accentHi);
  r.setProperty("--accent-grad", `linear-gradient(180deg, ${t.accentHi} 0%, ${t.accent} 100%)`);
  r.setProperty("--accent-ink", t.ink);
  r.setProperty("--glow-rgb", t.glow);
  if (corner) r.setProperty("--glow-at", corner);
}

/** A different corner every launch keeps the app from feeling static. */
export function randomCorner(): string {
  return CORNERS[Math.floor(Math.random() * CORNERS.length)];
}
