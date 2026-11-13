export function bytes(n: number | null | undefined): string {
  if (n == null || !isFinite(n)) return "";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

export function speed(bps: number): string {
  return bps > 0 ? `${bytes(bps)}/s` : "";
}

export function eta(received: number, total: number | null, bps: number, fraction?: number | null): string {
  let secs: number | null = null;
  if (total && bps > 0) secs = (total - received) / bps;
  else if (fraction && fraction > 0.02 && bps > 0) secs = (received / fraction - received) / bps;
  if (secs == null || !isFinite(secs) || secs < 0) return "";
  if (secs < 60) return `${Math.ceil(secs)}s left`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${Math.round(secs % 60)}s left`;
  return `${Math.floor(secs / 3600)}h ${Math.round((secs % 3600) / 60)}m left`;
}

export function host(u: string | null | undefined): string {
  if (!u) return "";
  try {
    return new URL(u).host.replace(/^www\./, "");
  } catch {
    return "";
  }
}

export function fileStem(u: string): string {
  try {
    const seg = decodeURIComponent(new URL(u).pathname.split("/").filter(Boolean).pop() ?? "");
    return seg.replace(/\.[a-z0-9]{2,5}$/i, "").replace(/[_-]+/g, " ").trim();
  } catch {
    return "";
  }
}

export function titleOf(v: { title: string | null; url: string }): string {
  return v.title?.trim() || fileStem(v.url) || "Untitled video";
}

export function label(v: { kind: string; ext: string | null }): string {
  if (v.kind === "hls") return "HLS";
  return (v.ext ?? "video").toUpperCase();
}

const VIDEO_RE = /\.(mp4|m4v|webm|mkv|mov|avi|flv|wmv|3gp|ogv|mpg|mpeg)(\?|#|$)/i;
const HLS_RE = /\.m3u8(\?|#|$)/i;

/** Instant, client-side guess shown while the user types. The backend has the final word. */
export function guessKind(text: string): "video" | "stream" | "page" | null {
  const t = text.trim();
  if (!t) return null;
  let u: URL;
  try {
    u = new URL(/^[a-z]+:\/\//i.test(t) ? t : `https://${t}`);
  } catch {
    return null;
  }
  if (!/^https?:$/.test(u.protocol) || !u.hostname.includes(".")) return null;
  if (HLS_RE.test(u.pathname) || /\.m3u8/i.test(u.search)) return "stream";
  if (VIDEO_RE.test(u.pathname)) return "video";
  return "page";
}

export function looksLikeUrl(text: string): boolean {
  return guessKind(text) !== null;
}

export function uid(): string {
  return Math.random().toString(36).slice(2, 10) + Date.now().toString(36).slice(-4);
}
