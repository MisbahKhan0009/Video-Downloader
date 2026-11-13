//! Decides whether a pasted link is a video or a page, and scans pages.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::{stream, StreamExt};
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE, REFERER};
use serde::Serialize;
use url::Url;

use crate::net::{self, LinkKind, CLIENT};
use crate::scan::{self, DetailLink, PageScan, VideoItem};

const MAX_PAGE_BYTES: usize = 15 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    /// "video" or "page"
    pub kind: String,
    pub video: Option<VideoItem>,
    pub page: Option<PageScan>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub kind: Option<&'static str>,
    pub size: Option<u64>,
    pub content_type: Option<String>,
}

fn size_from_headers(h: &reqwest::header::HeaderMap, partial: bool) -> Option<u64> {
    if partial {
        if let Some(total) = h
            .get(CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit('/').next())
            .and_then(|t| t.trim().parse().ok())
        {
            return Some(total);
        }
    }
    h.get(CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok())
}

/// Cheap check of a media URL: type and size, using a 1-byte ranged GET
/// (more servers answer that correctly than HEAD).
pub async fn probe(url: &Url, referer: Option<&str>) -> Probe {
    let mut req = CLIENT.get(url.clone()).header(RANGE, "bytes=0-0").header("Accept-Encoding", "identity");
    if let Some(r) = referer {
        req = req.header(REFERER, r);
    }
    let fut = req.send();
    match tokio::time::timeout(std::time::Duration::from_secs(12), fut).await {
        Ok(Ok(resp)) if resp.status().is_success() => {
            let partial = resp.status().as_u16() == 206;
            let ct = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(String::from);
            let size = size_from_headers(resp.headers(), partial).filter(|s| partial || *s > 1);
            let kind = ct.as_deref().and_then(net::kind_from_content_type).map(|k| k.as_str());
            Probe { kind, size, content_type: ct }
        }
        _ => Probe { kind: None, size: None, content_type: None },
    }
}

fn video_from_url(url: &Url, kind: LinkKind, referer: Option<String>) -> VideoItem {
    VideoItem {
        url: url.to_string(),
        title: net::stem_from_url(url),
        thumbnail: None,
        kind: if kind == LinkKind::Hls { "hls" } else { "file" }.into(),
        ext: net::ext_of(url),
        page_url: referer,
        size: None,
        source: "direct link".into(),
    }
}

/// Fetches a page and returns its final URL and HTML, or tells us it was media after all.
enum Fetched {
    Html(Url, String),
    Media(Url, LinkKind, Option<u64>),
}

async fn fetch(url: &Url, referer: Option<&str>) -> Result<Fetched, String> {
    let mut req = CLIENT
        .get(url.clone())
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8");
    if let Some(r) = referer {
        req = req.header(REFERER, r);
    }
    let resp = req.send().await.map_err(|e| friendly(&e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("The site answered {} {}", status.as_u16(), status.canonical_reason().unwrap_or("")));
    }
    let final_url = resp.url().clone();
    let ct = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if let Some(kind) = net::kind_from_content_type(&ct) {
        let kind = if kind == LinkKind::Unknown { LinkKind::File } else { kind };
        let size = size_from_headers(resp.headers(), false);
        drop(resp); // don't pull the whole video
        return Ok(Fetched::Media(final_url, kind, size));
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_PAGE_BYTES {
            return Err("That page is too large to scan".into());
        }
    }
    let text = resp.text().await.map_err(|e| friendly(&e))?;
    // A playlist served as text/plain is still a playlist.
    if text.trim_start().starts_with("#EXTM3U") {
        return Ok(Fetched::Media(final_url, LinkKind::Hls, None));
    }
    Ok(Fetched::Html(final_url, text))
}

pub fn friendly(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "The site took too long to respond".into()
    } else if e.is_connect() {
        "Couldn't reach the site. Check your connection".into()
    } else if e.is_redirect() {
        "Too many redirects".into()
    } else {
        let s = e.to_string();
        s.split(": ").last().unwrap_or(&s).to_string()
    }
}

pub async fn analyze(raw: &str) -> Result<Analysis, String> {
    let url = net::normalize_input(raw)?;
    let kind = net::kind_from_url(&url);

    if kind != LinkKind::Unknown {
        let mut v = video_from_url(&url, kind, None);
        let p = probe(&url, None).await;
        v.size = p.size;
        if p.kind == Some("hls") {
            v.kind = "hls".into();
        }
        return Ok(Analysis { kind: "video".into(), video: Some(v), page: None });
    }

    match fetch(&url, None).await? {
        Fetched::Media(u, k, size) => {
            let mut v = video_from_url(&u, k, None);
            v.size = size;
            Ok(Analysis { kind: "video".into(), video: Some(v), page: None })
        }
        Fetched::Html(u, html) => {
            let page = scan::scan_html(&u, &html);
            Ok(Analysis { kind: "page".into(), video: None, page: Some(page) })
        }
    }
}

pub async fn scan_page(raw: &str, referer: Option<&str>) -> Result<PageScan, String> {
    let url = net::normalize_input(raw)?;
    match fetch(&url, referer).await? {
        Fetched::Html(u, html) => Ok(scan::scan_html(&u, &html)),
        Fetched::Media(u, k, size) => {
            let mut v = video_from_url(&u, k, referer.map(String::from));
            v.size = size;
            Ok(PageScan {
                url: u.to_string(),
                title: v.title.clone(),
                image: None,
                videos: vec![v],
                current_page: None,
                next_page: None,
                prev_page: None,
                next_inferred: false,
                pages: vec![],
                detail_links: vec![],
            })
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepProgress {
    pub scan_id: String,
    pub done: usize,
    pub total: usize,
    pub found: usize,
    /// Videos found on the sub-page that just finished.
    pub videos: Vec<VideoItem>,
}

/// Opens each sub-page (video card) and collects the videos inside.
/// `on_progress` fires after each page so the UI can fill in as it goes.
pub async fn deep_scan<F>(
    scan_id: String,
    links: Vec<DetailLink>,
    referer: Option<String>,
    cancel: Arc<AtomicBool>,
    on_progress: F,
) -> Vec<VideoItem>
where
    F: Fn(DeepProgress),
{
    let total = links.len();
    let mut seen: HashSet<String> = HashSet::new();
    let mut all: Vec<VideoItem> = Vec::new();
    let mut done = 0usize;

    let mut results = stream::iter(links.into_iter().map(|link| {
        let cancel = cancel.clone();
        let referer = referer.clone();
        async move {
            if cancel.load(Ordering::Relaxed) {
                return Vec::new();
            }
            let Ok(page) = scan_page(&link.url, referer.as_deref()).await else { return Vec::new() };
            let single = page.videos.len() == 1;
            page.videos
                .into_iter()
                .map(|mut v| {
                    if v.title.is_none() || single {
                        v.title = link.title.clone().or(page.title.clone()).or(v.title);
                    }
                    if v.thumbnail.is_none() {
                        v.thumbnail = link.thumbnail.clone().or(page.image.clone());
                    }
                    v.page_url = Some(link.url.clone());
                    v
                })
                .collect::<Vec<_>>()
        }
    }))
    .buffer_unordered(6);

    while let Some(found) = results.next().await {
        done += 1;
        let fresh: Vec<VideoItem> = found.into_iter().filter(|v| seen.insert(v.url.clone())).collect();
        all.extend(fresh.iter().cloned());
        on_progress(DeepProgress { scan_id: scan_id.clone(), done, total, found: all.len(), videos: fresh });
        if cancel.load(Ordering::Relaxed) {
            break;
        }
    }
    all
}
