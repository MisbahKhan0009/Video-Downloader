import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type VideoKind = "file" | "hls";

export interface VideoItem {
  url: string;
  title: string | null;
  thumbnail: string | null;
  kind: VideoKind;
  ext: string | null;
  pageUrl: string | null;
  size: number | null;
  source: string;
}

export interface PageLink {
  number: number;
  url: string;
}

export interface DetailLink {
  url: string;
  title: string | null;
  thumbnail: string | null;
}

export interface PageScan {
  url: string;
  title: string | null;
  image: string | null;
  videos: VideoItem[];
  currentPage: number | null;
  nextPage: string | null;
  prevPage: string | null;
  nextInferred: boolean;
  pages: PageLink[];
  detailLinks: DetailLink[];
}

export interface Analysis {
  kind: "video" | "page";
  video: VideoItem | null;
  page: PageScan | null;
}

export interface Probe {
  kind: string | null;
  size: number | null;
  contentType: string | null;
}

export type DownloadStatus = "queued" | "downloading" | "paused" | "done" | "error" | "canceled";

export interface Progress {
  id: string;
  status: DownloadStatus;
  received: number;
  total: number | null;
  fraction: number | null;
  speed: number;
  path: string | null;
  fileName: string | null;
  error: string | null;
}

export interface DeepProgress {
  scanId: string;
  done: number;
  total: number;
  found: number;
  videos: VideoItem[];
}

export interface DownloadRequest {
  id: string;
  url: string;
  kind: VideoKind;
  title: string | null;
  referer: string | null;
  dir: string;
}

export const api = {
  analyze: (url: string) => invoke<Analysis>("analyze_link", { url }),
  scanPage: (url: string, referer?: string | null) => invoke<PageScan>("scan_page", { url, referer: referer ?? null }),
  probe: (url: string, referer?: string | null) => invoke<Probe>("probe_media", { url, referer: referer ?? null }),
  deepScan: (scanId: string, links: DetailLink[], referer: string | null) =>
    invoke<VideoItem[]>("deep_scan", { scanId, links, referer }),
  cancelScan: (scanId: string) => invoke<void>("cancel_scan", { scanId }),
  startDownload: (request: DownloadRequest) => invoke<void>("start_download", { request }),
  pauseDownload: (id: string) => invoke<boolean>("pause_download", { id }),
  cancelDownload: (id: string) => invoke<boolean>("cancel_download", { id }),
  discardPartial: (dir: string, id: string) => invoke<void>("discard_partial", { dir, id }),
  defaultDir: () => invoke<string>("default_download_dir"),
  openFile: (path: string) => invoke<void>("open_file", { path }),
  revealFile: (path: string) => invoke<void>("reveal_file", { path }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  platform: () => invoke<{ mobile: boolean; os: string }>("platform_info"),
  takeSharedLink: () => invoke<string | null>("take_shared_link"),
};

export function onDownload(cb: (p: Progress) => void): Promise<UnlistenFn> {
  return listen<Progress>("download", (e) => cb(e.payload));
}

export function onDeepScan(cb: (p: DeepProgress) => void): Promise<UnlistenFn> {
  return listen<DeepProgress>("deep-scan", (e) => cb(e.payload));
}
