import { SvelteSet } from "svelte/reactivity";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { api, onDeepScan, onDownload, type DetailLink, type PageScan, type Progress, type VideoItem, type VideoKind } from "./api";
import { better, looksLikeUrl, rank, titleOf, uid } from "./format";
import { applyTheme, randomCorner, themeById } from "./theme";

export type View = "grab" | "downloads" | "settings";

export interface Download {
  id: string;
  url: string;
  kind: VideoKind;
  title: string;
  thumbnail: string | null;
  referer: string | null;
  dir: string;
  status: Progress["status"];
  received: number;
  total: number | null;
  fraction: number | null;
  speed: number;
  path: string | null;
  fileName: string | null;
  error: string | null;
  note?: string | null;
  addedAt: number;
}

export interface Settings {
  dir: string;
  autoDeep: boolean;
  autoGrab: boolean;
  /** follow "next" pages on its own until the last page */
  autoCrawl: boolean;
  theme: string;
}

interface Toast {
  id: string;
  text: string;
  tone: "info" | "error" | "ok";
}

const LS_DOWNLOADS = "vidra.downloads.v1";
const LS_SETTINGS = "vidra.settings.v1";

function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? { ...fallback, ...JSON.parse(raw) } : fallback;
  } catch {
    return fallback;
  }
}

class Store {
  view = $state<View>("grab");
  mobile = $state(false);
  os = $state("");

  input = $state("");
  analyzing = $state(false);
  error = $state<string | null>(null);
  clipboardSuggestion = $state<string | null>(null);

  /** What the last grab turned into. */
  mode = $state<"empty" | "video" | "page">("empty");
  page = $state<PageScan | null>(null);
  videos = $state<VideoItem[]>([]);
  scannedPages = $state<string[]>([]);
  selected = new SvelteSet<string>();

  deep = $state<{ id: string; done: number; total: number; found: number } | null>(null);
  /** Live state of "scrape every page" (no limit; runs until the last page or Stop). */
  crawl = $state<{ current: number | null; stopping: boolean } | null>(null);
  /** Why the last crawl ended, shown next to the page count. */
  crawlEnded = $state<string | null>(null);
  #gen = 0; // bumps on every new grab so late results from an old one are dropped
  loadingPage = $state<string | null>(null);

  downloads = $state<Download[]>([]);
  settings = $state<Settings>({ dir: "", autoDeep: true, autoGrab: true, autoCrawl: true, theme: "ember" });
  toasts = $state<Toast[]>([]);

  #saveTimer: ReturnType<typeof setTimeout> | null = null;
  #lastClipboard = "";

  get active() {
    return this.downloads.filter((d) => d.status === "downloading" || d.status === "queued");
  }
  get totalSpeed() {
    return this.downloads.reduce((s, d) => s + (d.status === "downloading" ? d.speed : 0), 0);
  }
  get busy() {
    return this.analyzing || !!this.deep || !!this.crawl || !!this.loadingPage;
  }

  async init() {
    const info = await api.platform().catch(() => ({ mobile: false, os: "" }));
    this.mobile = info.mobile;
    this.os = info.os;

    this.settings = load(LS_SETTINGS, this.settings);
    applyTheme(themeById(this.settings.theme), randomCorner());
    if (!this.settings.dir) this.settings.dir = await api.defaultDir().catch(() => "");

    const saved = load<{ items: Download[] }>(LS_DOWNLOADS, { items: [] }).items;
    // Anything running when the app closed can be resumed from its partial file.
    this.downloads = saved.map((d) =>
      d.status === "downloading" || d.status === "queued" ? { ...d, status: "paused", speed: 0 } : { ...d, speed: 0 },
    );

    await onDownload((p) => this.#onProgress(p));
    await onDeepScan((p) => {
      if (!this.deep || p.scanId !== this.deep.id) return;
      this.deep = { ...this.deep, done: p.done, total: p.total, found: p.found };
      this.#addVideos(p.videos);
    });
    this.checkClipboard();
    this.checkShared();
  }

  // ---------------------------------------------------------------- grab

  /** Android share sheet: "Share → Vidra" from a browser lands here. */
  async checkShared() {
    if (!this.mobile) return;
    try {
      const text = await api.takeSharedLink();
      const link = text?.match(/https?:\/\/\S+/i)?.[0] ?? text;
      if (link && looksLikeUrl(link)) {
        this.clipboardSuggestion = null;
        this.grab(link);
      }
    } catch {
      /* nothing shared */
    }
  }

  async checkClipboard() {
    try {
      const t = (await readText())?.trim() ?? "";
      if (t && t !== this.#lastClipboard && t !== this.input.trim() && looksLikeUrl(t) && t.length < 2048) {
        this.clipboardSuggestion = t;
      }
      this.#lastClipboard = t;
    } catch {
      /* clipboard may be empty or unavailable */
    }
  }

  async pasteFromClipboard() {
    try {
      const t = (await readText())?.trim() ?? "";
      if (t) {
        this.input = t;
        this.clipboardSuggestion = null;
        if (this.settings.autoGrab && looksLikeUrl(t)) this.grab();
      }
    } catch {
      this.toast("Clipboard is empty", "info");
    }
  }

  useSuggestion() {
    if (!this.clipboardSuggestion) return;
    this.input = this.clipboardSuggestion;
    this.clipboardSuggestion = null;
    this.grab();
  }

  reset() {
    this.stopAll();
    this.#gen++; // the old crawl sees this and quits without touching new state
    this.crawl = null;
    this.crawlEnded = null;
    this.mode = "empty";
    this.page = null;
    this.videos = [];
    this.scannedPages = [];
    this.selected.clear();
    this.error = null;
  }

  async grab(text?: string) {
    const link = (text ?? this.input).trim();
    if (!link || this.analyzing) return;
    this.input = link;
    this.reset();
    this.analyzing = true;
    this.view = "grab";
    try {
      const res = await api.analyze(link);
      if (res.kind === "video" && res.video) {
        this.mode = "video";
        this.videos = [res.video];
      } else if (res.page) {
        this.mode = "page";
        this.#showPage(res.page, false);
        this.analyzing = false;
        await this.#maybeDeep(res.page);
        if (this.settings.autoCrawl && this.page?.nextPage) this.crawlAll();
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.analyzing = false;
    }
  }

  #showPage(p: PageScan, append: boolean) {
    this.page = p;
    if (append) {
      this.#addVideos(p.videos);
      if (!this.scannedPages.includes(p.url)) this.scannedPages = [...this.scannedPages, p.url];
    } else {
      this.videos = [];
      this.selected.clear();
      this.#addVideos(p.videos);
      this.scannedPages = [p.url];
    }
  }

  /** One entry per video: a better-quality variant replaces the one already listed. */
  #addVideos(list: VideoItem[]) {
    if (!list.length) return;
    const next = [...this.videos];
    let changed = false;
    for (const v of list) {
      const i = next.findIndex((o) => o.url === v.url || (o.group && o.group === v.group));
      if (i === -1) {
        next.push(v);
        changed = true;
      } else if (next[i].url !== v.url && better(rank(v), rank(next[i]))) {
        const old = next[i];
        next[i] = { ...v, title: v.title ?? old.title, thumbnail: v.thumbnail ?? old.thumbnail };
        if (this.selected.has(old.url)) {
          this.selected.delete(old.url);
          this.selected.add(v.url);
        }
        changed = true;
      }
    }
    if (changed) this.videos = next;
  }

  async #maybeDeep(p: PageScan, force = false) {
    if (!p.detailLinks.length) return;
    if (force || (this.settings.autoDeep && p.videos.length === 0)) {
      await this.deepScan(p.detailLinks, p.url);
    }
  }

  async deepScan(links: DetailLink[], referer: string | null) {
    if (this.deep) return;
    const id = uid();
    const gen = this.#gen;
    this.deep = { id, done: 0, total: links.length, found: 0 };
    try {
      const vids = await api.deepScan(id, links, referer);
      if (gen === this.#gen) this.#addVideos(vids);
    } catch (e) {
      this.toast(String(e), "error");
    } finally {
      this.deep = null;
    }
  }

  scanLinkedPages() {
    if (this.page) this.deepScan(this.page.detailLinks, this.page.url);
  }

  stopDeep() {
    if (this.deep) api.cancelScan(this.deep.id);
  }

  async goToPage(url: string) {
    if (this.loadingPage || !this.page) return;
    this.stopAll();
    this.#gen++; // the old crawl sees this and quits without touching new state
    this.crawl = null;
    this.crawlEnded = null;
    const referer = this.page.url;
    this.loadingPage = url;
    try {
      const p = await api.scanPage(url, referer);
      this.#showPage(p, false);
      this.loadingPage = null;
      await this.#maybeDeep(p);
      if (this.settings.autoCrawl && this.page?.nextPage) this.crawlAll();
    } catch (e) {
      this.toast(String(e), "error");
    } finally {
      this.loadingPage = null;
    }
  }

  /**
   * Scrape every page: keep following "next" until the site runs out of pages
   * or the user presses Stop. Results stream into the grid as each page lands.
   */
  async crawlAll() {
    if (!this.page?.nextPage || this.crawl) return;
    const gen = this.#gen;
    const alive = () => gen === this.#gen && this.crawl && !this.crawl.stopping;
    this.crawl = { current: this.page.currentPage, stopping: false };
    this.crawlEnded = null;
    let reason = "Reached the last page";
    try {
      while (true) {
        if (!alive()) {
          reason = "Stopped";
          break;
        }
        const from = this.page;
        const next = from?.nextPage;
        if (!from || !next) break;
        if (this.scannedPages.includes(next)) break; // the site looped back
        let p: PageScan | null = null;
        let lastErr = "";
        for (let attempt = 0; attempt < 3 && !p; attempt++) {
          try {
            p = await api.scanPage(next, from.url);
          } catch (e) {
            lastErr = String(e);
            if (from.nextInferred) break; // a guessed page that fails is just the end
            await sleep(1500 * (attempt + 1));
          }
        }
        if (gen !== this.#gen) return; // a new grab replaced this one
        if (!p) {
          if (!from.nextInferred) reason = `Stopped on an error: ${lastErr}`;
          break;
        }
        // A guessed "next page" that comes back empty means we ran past the end.
        if (from.nextInferred && !p.videos.length && !p.detailLinks.length) break;
        this.#showPage(p, true);
        if (this.crawl) this.crawl = { ...this.crawl, current: p.currentPage };
        if (p.videos.length === 0 && this.settings.autoDeep && p.detailLinks.length && alive()) {
          await this.deepScan(p.detailLinks, p.url);
        }
        await sleep(250); // be polite between pages
      }
    } finally {
      if (gen === this.#gen) {
        this.crawl = null;
        this.crawlEnded = reason;
      }
    }
  }

  stopAll() {
    if (this.crawl) this.crawl = { ...this.crawl, stopping: true };
    this.stopDeep();
  }

  toggle(url: string) {
    if (this.selected.has(url)) this.selected.delete(url);
    else this.selected.add(url);
  }

  selectAll() {
    if (this.selected.size === this.videos.length) this.selected.clear();
    else this.videos.forEach((v) => this.selected.add(v.url));
  }

  // ---------------------------------------------------------------- downloads

  statusOf(url: string): Download | undefined {
    return this.downloads.find((d) => d.url === url && d.status !== "canceled");
  }

  download(v: VideoItem, title?: string) {
    const existing = this.statusOf(v.url);
    if (existing && existing.status !== "error") {
      if (existing.status === "paused") this.resume(existing.id);
      return existing;
    }
    const d: Download = {
      id: uid(),
      url: v.url,
      kind: v.kind,
      title: title?.trim() || titleOf(v),
      thumbnail: v.thumbnail,
      referer: v.pageUrl,
      dir: this.settings.dir,
      status: "queued",
      received: 0,
      total: v.size,
      fraction: null,
      speed: 0,
      path: null,
      fileName: null,
      error: null,
      addedAt: Date.now(),
    };
    this.downloads = [d, ...this.downloads.filter((x) => x.url !== v.url)];
    this.#start(d);
    this.#save();
    return d;
  }

  downloadSelected() {
    const list = this.videos.filter((v) => this.selected.has(v.url));
    list.forEach((v) => this.download(v));
    this.selected.clear();
    if (list.length) this.toast(`${list.length} added to downloads`, "ok");
  }

  downloadAll() {
    this.videos.forEach((v) => this.download(v));
    if (this.videos.length) this.toast(`${this.videos.length} added to downloads`, "ok");
  }

  #start(d: Download) {
    api
      .startDownload({ id: d.id, url: d.url, kind: d.kind, title: d.title, referer: d.referer, dir: d.dir })
      .catch((e) => this.#onProgress({ ...d, status: "error", error: String(e), speed: 0, note: null }));
  }

  resume(id: string) {
    const d = this.downloads.find((x) => x.id === id);
    if (!d) return;
    d.status = "queued";
    d.error = null;
    this.#start(d);
  }

  pause(id: string) {
    api.pauseDownload(id).then((running) => {
      const d = this.downloads.find((x) => x.id === id);
      if (!running && d) d.status = "paused";
    });
  }

  cancel(id: string) {
    const d = this.downloads.find((x) => x.id === id);
    api.cancelDownload(id).then((running) => {
      if (!running && d) {
        api.discardPartial(d.dir, d.id);
        this.downloads = this.downloads.filter((x) => x.id !== id);
        this.#save();
      }
    });
  }

  remove(id: string) {
    const d = this.downloads.find((x) => x.id === id);
    if (d && d.status !== "done") {
      this.cancel(id);
      return;
    }
    this.downloads = this.downloads.filter((x) => x.id !== id);
    this.#save();
  }

  clearFinished() {
    this.downloads = this.downloads.filter((d) => d.status !== "done" && d.status !== "canceled");
    this.#save();
  }

  pauseAll() {
    this.active.forEach((d) => this.pause(d.id));
  }

  resumeAll() {
    this.downloads.filter((d) => d.status === "paused" || d.status === "error").forEach((d) => this.resume(d.id));
  }

  #onProgress(p: Progress) {
    const d = this.downloads.find((x) => x.id === p.id);
    if (!d) return;
    if (p.status === "canceled") {
      this.downloads = this.downloads.filter((x) => x.id !== p.id);
      this.#save();
      return;
    }
    const prev = d.status;
    d.status = p.status;
    d.received = p.received;
    if (p.total != null) d.total = p.total;
    d.fraction = p.fraction;
    d.speed = p.speed;
    d.error = p.error;
    d.note = p.note;
    if (p.path) d.path = p.path;
    if (p.fileName) d.fileName = p.fileName;
    if (prev !== p.status) {
      if (p.status === "done") this.toast(`Saved ${p.fileName ?? d.title}`, "ok");
      if (p.status === "error") this.toast(p.error ?? "Download failed", "error");
      this.#save();
    } else {
      this.#saveSoon();
    }
  }

  // ---------------------------------------------------------------- settings & misc

  setTheme(id: string) {
    this.settings.theme = id;
    applyTheme(themeById(id));
    this.saveSettings();
  }

  saveSettings() {
    try {
      localStorage.setItem(LS_SETTINGS, JSON.stringify(this.settings));
    } catch {
      /* storage may be unavailable */
    }
  }

  #save() {
    try {
      const items = this.downloads.map((d) => ({ ...d, speed: 0 }));
      localStorage.setItem(LS_DOWNLOADS, JSON.stringify({ items }));
    } catch {
      /* storage may be unavailable */
    }
  }

  #saveSoon() {
    if (this.#saveTimer) return;
    this.#saveTimer = setTimeout(() => {
      this.#saveTimer = null;
      this.#save();
    }, 3000);
  }

  toast(text: string, tone: Toast["tone"] = "info") {
    const id = uid();
    this.toasts = [...this.toasts, { id, text, tone }].slice(-3);
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), tone === "error" ? 5200 : 3200);
  }

  async copy(text: string) {
    try {
      const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
      await writeText(text);
      this.#lastClipboard = text;
      this.toast("Link copied", "ok");
    } catch {
      this.toast("Couldn't copy", "error");
    }
  }
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

export const app = new Store();
