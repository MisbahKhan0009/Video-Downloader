//! Download engine: resumable direct-file downloads and HLS stream capture,
//! with pause / resume / cancel and throttled progress events.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use futures_util::{stream, StreamExt};
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use serde::{Deserialize, Serialize};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::Semaphore;
use url::Url;

use crate::analyze::friendly;
use crate::net;

const RUN: u8 = 0;
const PAUSE: u8 = 1;
const CANCEL: u8 = 2;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub id: String,
    pub url: String,
    /// "file" or "hls"
    pub kind: String,
    pub title: Option<String>,
    pub referer: Option<String>,
    pub dir: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub id: String,
    /// queued | downloading | paused | done | error | canceled
    pub status: String,
    pub received: u64,
    pub total: Option<u64>,
    /// 0..1 when known (HLS reports by segment)
    pub fraction: Option<f64>,
    /// bytes per second
    pub speed: f64,
    pub path: Option<String>,
    pub file_name: Option<String>,
    pub error: Option<String>,
    /// Temporary state worth showing, e.g. "Site is busy, retrying in 8s".
    pub note: Option<String>,
}

struct Ctl {
    state: AtomicU8,
}

pub struct Manager {
    tasks: Mutex<HashMap<String, Arc<Ctl>>>,
    slots: Arc<Semaphore>,
}

impl Manager {
    pub fn new(parallel: usize) -> Self {
        Self { tasks: Mutex::new(HashMap::new()), slots: Arc::new(Semaphore::new(parallel.max(1))) }
    }

    pub fn signal(&self, id: &str, what: u8) -> bool {
        if let Some(c) = self.tasks.lock().unwrap().get(id) {
            c.state.store(what, Ordering::SeqCst);
            true
        } else {
            false
        }
    }
    pub fn pause(&self, id: &str) -> bool {
        self.signal(id, PAUSE)
    }
    pub fn cancel(&self, id: &str) -> bool {
        self.signal(id, CANCEL)
    }
}

fn part_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!(".vidra-{id}.part"))
}
fn hls_state_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!(".vidra-{id}.hls"))
}

/// `name.ext`, or `name (2).ext` when taken.
fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    p
}

/// Emits progress at most ~5 times a second, with a smoothed speed.
struct Reporter<F: Fn(Progress)> {
    emit: F,
    base: Progress,
    last_emit: Instant,
    window_start: Instant,
    window_bytes: u64,
    speed: f64,
}

impl<F: Fn(Progress)> Reporter<F> {
    fn new(id: &str, emit: F) -> Self {
        let now = Instant::now();
        Self {
            emit,
            base: Progress { id: id.into(), status: "downloading".into(), ..Default::default() },
            last_emit: now - Duration::from_secs(1),
            window_start: now,
            window_bytes: 0,
            speed: 0.0,
        }
    }
    fn add(&mut self, n: u64) {
        self.base.received += n;
        self.window_bytes += n;
        let el = self.window_start.elapsed().as_secs_f64();
        if el >= 0.5 {
            let inst = self.window_bytes as f64 / el;
            self.speed = if self.speed == 0.0 { inst } else { self.speed * 0.6 + inst * 0.4 };
            self.window_start = Instant::now();
            self.window_bytes = 0;
        }
        if self.last_emit.elapsed() >= Duration::from_millis(200) {
            self.flush();
        }
    }
    fn flush(&mut self) {
        self.last_emit = Instant::now();
        let mut p = self.base.clone();
        p.speed = self.speed;
        (self.emit)(p);
    }
    fn note(&mut self, n: Option<String>) {
        self.base.note = n;
        self.flush();
    }
    fn finish(&mut self, status: &str, error: Option<String>) {
        self.base.status = status.into();
        self.base.error = error;
        self.speed = 0.0;
        self.flush();
    }
}

enum Outcome {
    Done(PathBuf),
    Paused,
    Canceled,
}

pub async fn run<F>(mgr: Arc<Manager>, req: DownloadRequest, emit: F)
where
    F: Fn(Progress) + Send + Sync + 'static,
{
    let ctl = Arc::new(Ctl { state: AtomicU8::new(RUN) });
    mgr.tasks.lock().unwrap().insert(req.id.clone(), ctl.clone());

    let mut rep = Reporter::new(&req.id, emit);
    rep.base.status = "queued".into();
    rep.flush();

    let permit = tokio::select! {
        p = mgr.slots.clone().acquire_owned() => p.ok(),
        _ = async {
            while ctl.state.load(Ordering::SeqCst) == RUN { tokio::time::sleep(Duration::from_millis(150)).await; }
        } => None,
    };
    let result = match permit {
        None => Ok(if ctl.state.load(Ordering::SeqCst) == CANCEL { Outcome::Canceled } else { Outcome::Paused }),
        Some(_permit) => {
            rep.base.status = "downloading".into();
            rep.flush();
            let dir = PathBuf::from(&req.dir);
            match fs::create_dir_all(&dir).await {
                Err(e) => Err(format!("Can't write to {}: {e}", dir.display())),
                Ok(()) => {
                    if req.kind == "hls" {
                        download_hls(&req, &dir, &ctl, &mut rep).await
                    } else {
                        download_file(&req, &dir, &ctl, &mut rep).await
                    }
                }
            }
        }
    };

    mgr.tasks.lock().unwrap().remove(&req.id);
    let dir = PathBuf::from(&req.dir);
    match result {
        Ok(Outcome::Done(path)) => {
            rep.base.path = Some(path.to_string_lossy().into_owned());
            rep.base.file_name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            if rep.base.total.is_none() {
                rep.base.total = Some(rep.base.received);
            }
            rep.base.fraction = Some(1.0);
            rep.finish("done", None);
        }
        Ok(Outcome::Paused) => rep.finish("paused", None),
        Ok(Outcome::Canceled) => {
            let _ = fs::remove_file(part_path(&dir, &req.id)).await;
            let _ = fs::remove_file(hls_state_path(&dir, &req.id)).await;
            rep.finish("canceled", None);
        }
        Err(e) => rep.finish("error", Some(e)),
    }
}

fn check(ctl: &Ctl) -> Option<Outcome> {
    match ctl.state.load(Ordering::SeqCst) {
        PAUSE => Some(Outcome::Paused),
        CANCEL => Some(Outcome::Canceled),
        _ => None,
    }
}

/// Sleep, but wake up early if the user pauses or cancels.
async fn wait_or_stop(ctl: &Ctl, d: Duration) -> Option<Outcome> {
    let end = Instant::now() + d;
    while Instant::now() < end {
        if let Some(o) = check(ctl) {
            return Some(o);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    None
}

const RATE_LIMIT_TRIES: u32 = 8;
const RATE_LIMIT_MSG: &str = "The site is limiting requests (429). Wait a few minutes, then press retry";

async fn download_file<F: Fn(Progress)>(
    req: &DownloadRequest,
    dir: &Path,
    ctl: &Ctl,
    rep: &mut Reporter<F>,
) -> Result<Outcome, String> {
    let mut url = Url::parse(&req.url).map_err(|_| "Invalid video link".to_string())?;
    let mut referer = req.referer.clone();
    let part = part_path(dir, &req.id);
    let mut attempts = 0;
    let mut name_hint: Option<String> = None;
    let mut ext_hint: Option<String> = None;
    let mut limited = 0u32;
    // Ways to ask for the file, tried in order whenever the site answers 401/403/410.
    let mut way = Way::Media;
    let mut refreshed = false;
    let mut page_hops = 0;

    loop {
        let have = fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
        let range = format!("bytes={have}-");
        let r = match way {
            // Like a browser's <video>: always a ranged request.
            Way::Media => net::media_get(&url, referer.as_deref()).header(RANGE, range),
            // Like clicking a download link in a tab.
            Way::Navigate => {
                let r = net::nav_get(&url, referer.as_deref());
                if have > 0 { r.header(RANGE, range) } else { r }
            }
            Way::NoReferer => {
                let r = net::nav_get(&url, None);
                if have > 0 { r.header(RANGE, range) } else { r }
            }
        };
        let resp = match r.send().await {
            Ok(x) => x,
            Err(e) => {
                attempts += 1;
                if attempts > 4 {
                    return Err(friendly(&e));
                }
                tokio::time::sleep(Duration::from_millis(800 * attempts)).await;
                continue;
            }
        };
        if let Some(wait) = net::retry_wait(&resp, limited) {
            limited += 1;
            if limited > RATE_LIMIT_TRIES {
                return Err(RATE_LIMIT_MSG.into());
            }
            rep.note(Some(format!("Site is busy, retrying in {}s", wait.as_secs())));
            if let Some(o) = wait_or_stop(ctl, wait).await {
                return Ok(o);
            }
            rep.note(None);
            continue;
        }
        rep.base.note = None;
        let status = resp.status().as_u16();

        if matches!(status, 401 | 403 | 410) {
            way = match way {
                Way::Media => Way::Navigate,
                Way::Navigate if !refreshed && referer.is_some() => {
                    // Signed links expire. Read the video's page again for a fresh one.
                    refreshed = true;
                    rep.note(Some("Link refused, getting a fresh one from the page".into()));
                    if let Some(fresh) = fresh_link(referer.as_deref().unwrap(), &url).await {
                        url = fresh;
                    }
                    rep.note(None);
                    Way::Media
                }
                Way::Navigate => Way::NoReferer,
                Way::NoReferer => {
                    return Err(format!(
                        "The site refused this download ({status}). The video may need a signed-in account, \
                         or the site blocks downloads outside its own player"
                    ));
                }
            };
            continue;
        }

        // A "download" link that opens a web page: read it and follow the video inside.
        let is_html = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|c| c.to_ascii_lowercase().contains("text/html"));
        if is_html && resp.status().is_success() && have == 0 {
            page_hops += 1;
            if page_hops > 2 {
                return Err("This link opens a web page, not a video".into());
            }
            let page_url = resp.url().clone();
            let html = resp.text().await.map_err(|e| friendly(&e))?;
            let scan = crate::scan::scan_html(&page_url, &html);
            let best = crate::scan::collapse_all(scan.videos);
            match best {
                Some(v) if v.url != url.as_str() => {
                    url = Url::parse(&v.url).map_err(|_| "Invalid video link".to_string())?;
                    referer = Some(page_url.to_string());
                    way = Way::Media;
                    continue;
                }
                _ => return Err("This link opens a web page, not a video".into()),
            }
        }

        if status == 416 && have > 0 {
            // Already have every byte.
            rep.base.received = have;
            rep.base.total = Some(have);
            break;
        }
        if !resp.status().is_success() {
            return Err(format!("Server answered {status}"));
        }
        let h = resp.headers();
        if name_hint.is_none() {
            name_hint = h.get(CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()).and_then(net::filename_from_disposition);
        }
        if ext_hint.is_none() {
            ext_hint = h
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .and_then(net::ext_from_content_type)
                .map(String::from);
        }
        let resuming = status == 206 && have > 0;
        let total = if status == 206 {
            h.get(CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit('/').next())
                .and_then(|t| t.trim().parse::<u64>().ok())
        } else {
            resp.content_length()
        };
        rep.base.total = total;
        rep.base.received = if resuming { have } else { 0 };
        rep.base.fraction = None;

        let file = if resuming {
            OpenOptions::new().append(true).open(&part).await
        } else {
            File::create(&part).await
        }
        .map_err(|e| format!("Can't write file: {e}"))?;
        let mut out = BufWriter::with_capacity(512 * 1024, file);
        let mut body = resp.bytes_stream();
        let mut broke = false;
        while let Some(chunk) = body.next().await {
            match chunk {
                Ok(bytes) => {
                    out.write_all(&bytes).await.map_err(|e| format!("Disk write failed: {e}"))?;
                    rep.add(bytes.len() as u64);
                    if let Some(o) = check(ctl) {
                        out.flush().await.ok();
                        return Ok(o);
                    }
                }
                Err(_) => {
                    broke = true;
                    break;
                }
            }
        }
        out.flush().await.map_err(|e| format!("Disk write failed: {e}"))?;
        drop(out);

        let incomplete = total.is_some_and(|t| rep.base.received < t);
        if broke || incomplete {
            attempts += 1;
            if attempts > 6 {
                return Err("Connection kept dropping. Resume to try again".into());
            }
            tokio::time::sleep(Duration::from_millis(600 * attempts)).await;
            continue; // resumes from what's on disk
        }
        break;
    }

    // Pick the final name: server's suggestion, then the title, then the URL.
    let url_ext = net::ext_of(&url).filter(|e| net::VIDEO_EXTS.contains(&e.as_str()));
    let (stem, ext) = match name_hint {
        Some(n) => {
            let n = net::sanitize_filename(&n);
            match n.rsplit_once('.') {
                Some((s, e)) if !s.is_empty() && e.len() <= 5 => (s.to_string(), e.to_ascii_lowercase()),
                _ => (n, ext_hint.or(url_ext).unwrap_or_else(|| "mp4".into())),
            }
        }
        None => {
            let stem = req.title.clone().or_else(|| net::stem_from_url(&url)).unwrap_or_else(|| "video".into());
            let stem = net::sanitize_filename(strip_ext(&stem));
            (stem, url_ext.or(ext_hint).unwrap_or_else(|| "mp4".into()))
        }
    };
    let dest = unique_path(dir, &stem, &ext);
    fs::rename(&part, &dest).await.map_err(|e| format!("Couldn't save file: {e}"))?;
    Ok(Outcome::Done(dest))
}

#[derive(Clone, Copy, PartialEq)]
enum Way {
    Media,
    Navigate,
    NoReferer,
}

/// Re-scan the page a video came from and return its current link
/// (same file if still listed, else the same video's best quality).
async fn fresh_link(page: &str, old: &Url) -> Option<Url> {
    let scan = crate::analyze::scan_page(page, None).await.ok()?;
    if scan.videos.iter().any(|v| v.url == old.as_str()) {
        return Some(old.clone());
    }
    let group = net::group_key(old);
    let pick = scan
        .videos
        .iter()
        .filter(|v| v.group == group)
        .max_by_key(|v| v.rank())
        .cloned()
        .or_else(|| crate::scan::collapse_all(scan.videos))?;
    Url::parse(&pick.url).ok()
}

fn strip_ext(s: &str) -> &str {
    match s.rsplit_once('.') {
        Some((a, e)) if net::VIDEO_EXTS.contains(&e.to_ascii_lowercase().as_str()) && !a.is_empty() => a,
        _ => s,
    }
}

// ---------------------------------------------------------------- HLS

#[derive(Clone)]
struct Key {
    uri: Url,
    iv: Option<[u8; 16]>,
}

#[derive(Clone)]
struct Segment {
    url: Url,
    key: Option<Key>,
    seq: u64,
}

struct MediaPlaylist {
    init: Option<Url>,
    segments: Vec<Segment>,
}

fn attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    // Attribute lists: KEY=VALUE,KEY="VALUE, with commas"
    let mut rest = line.split_once(':')?.1;
    while !rest.is_empty() {
        let (k, after) = rest.split_once('=')?;
        let (v, next) = if let Some(stripped) = after.strip_prefix('"') {
            let end = stripped.find('"')?;
            (&stripped[..end], stripped[end + 1..].trim_start_matches(','))
        } else {
            match after.split_once(',') {
                Some((v, n)) => (v, n),
                None => (after, ""),
            }
        };
        if k.trim().eq_ignore_ascii_case(name) {
            return Some(v);
        }
        rest = next;
    }
    None
}

async fn get_text(url: &Url, referer: Option<&str>) -> Result<String, String> {
    let mut limited = 0;
    let resp = loop {
        let resp = net::media_get(url, referer).send().await.map_err(|e| friendly(&e))?;
        match net::retry_wait(&resp, limited) {
            Some(w) if limited < RATE_LIMIT_TRIES => {
                limited += 1;
                tokio::time::sleep(w).await;
            }
            Some(_) => return Err(RATE_LIMIT_MSG.into()),
            None => break resp,
        }
    };
    if !resp.status().is_success() {
        return Err(format!("Playlist request failed ({})", resp.status().as_u16()));
    }
    resp.text().await.map_err(|e| friendly(&e))
}

async fn get_bytes(url: &Url, referer: Option<&str>) -> Result<Vec<u8>, String> {
    let mut last = String::new();
    let mut limited = 0u32;
    let mut attempt = 0u64;
    while attempt < 4 {
        match net::media_get(url, referer).send().await {
            Ok(resp) if net::retry_wait(&resp, limited).is_some() => {
                let w = net::retry_wait(&resp, limited).unwrap();
                limited += 1;
                if limited > RATE_LIMIT_TRIES {
                    return Err(RATE_LIMIT_MSG.into());
                }
                tokio::time::sleep(w).await;
                continue; // rate limiting doesn't use up a normal retry
            }
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(b) => return Ok(b.to_vec()),
                Err(e) => last = friendly(&e),
            },
            Ok(resp) => last = format!("Segment request failed ({})", resp.status().as_u16()),
            Err(e) => last = friendly(&e),
        }
        attempt += 1;
        tokio::time::sleep(Duration::from_millis(500 * attempt)).await;
    }
    Err(last)
}

async fn load_media_playlist(start: &Url, referer: Option<&str>) -> Result<MediaPlaylist, String> {
    let mut url = start.clone();
    for _ in 0..4 {
        let text = get_text(&url, referer).await?;
        if !text.trim_start().starts_with("#EXTM3U") {
            return Err("This link isn't an HLS playlist".into());
        }
        let lines: Vec<&str> = text.lines().map(str::trim).collect();

        // Master playlist: pick the highest-bandwidth variant.
        if lines.iter().any(|l| l.starts_with("#EXT-X-STREAM-INF")) {
            let mut best: Option<(u64, Url)> = None;
            for (i, l) in lines.iter().enumerate() {
                if l.starts_with("#EXT-X-STREAM-INF") {
                    let bw = attr(l, "BANDWIDTH").and_then(|b| b.parse().ok()).unwrap_or(0);
                    if let Some(uri) = lines[i + 1..].iter().find(|x| !x.is_empty() && !x.starts_with('#')) {
                        if let Ok(u) = url.join(uri) {
                            if best.as_ref().is_none_or(|(b, _)| bw > *b) {
                                best = Some((bw, u));
                            }
                        }
                    }
                }
            }
            url = best.ok_or("Empty master playlist")?.1;
            continue;
        }

        let mut seq: u64 = 0;
        let mut key: Option<Key> = None;
        let mut init = None;
        let mut segments = Vec::new();
        for l in &lines {
            if let Some(v) = l.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
                seq = v.trim().parse().unwrap_or(0);
            } else if l.starts_with("#EXT-X-KEY") {
                let method = attr(l, "METHOD").unwrap_or("NONE");
                key = match method {
                    "NONE" => None,
                    "AES-128" => {
                        let uri = attr(l, "URI").ok_or("Encrypted stream without key URI")?;
                        let iv = attr(l, "IV").and_then(|v| {
                            let h = v.trim_start_matches("0x").trim_start_matches("0X");
                            let b = hex::decode(format!("{h:0>32}")).ok()?;
                            b.try_into().ok()
                        });
                        Some(Key { uri: url.join(uri).map_err(|_| "Bad key URI")?, iv })
                    }
                    other => return Err(format!("This stream uses {other} protection, which can't be downloaded")),
                };
            } else if l.starts_with("#EXT-X-MAP") {
                if let Some(u) = attr(l, "URI") {
                    init = url.join(u).ok();
                }
            } else if l.starts_with("#EXT-X-BYTERANGE") {
                return Err("Byte-range HLS streams aren't supported yet".into());
            } else if !l.is_empty() && !l.starts_with('#') {
                let u = url.join(l).map_err(|_| "Bad segment URI")?;
                segments.push(Segment { url: u, key: key.clone(), seq });
                seq += 1;
            }
        }
        if segments.is_empty() {
            return Err("The stream has no segments".into());
        }
        return Ok(MediaPlaylist { init, segments });
    }
    Err("Too many nested playlists".into())
}

async fn download_hls<F: Fn(Progress)>(
    req: &DownloadRequest,
    dir: &Path,
    ctl: &Ctl,
    rep: &mut Reporter<F>,
) -> Result<Outcome, String> {
    let url = Url::parse(&req.url).map_err(|_| "Invalid stream link".to_string())?;
    let referer = req.referer.as_deref();
    let pl = load_media_playlist(&url, referer).await?;
    let total = pl.segments.len();
    let part = part_path(dir, &req.id);
    let state = hls_state_path(dir, &req.id);

    // Resume: "<segments done> <bytes written>"
    let (mut done, written) = match fs::read_to_string(&state).await {
        Ok(s) => {
            let mut it = s.split_whitespace().filter_map(|x| x.parse::<u64>().ok());
            (it.next().unwrap_or(0) as usize, it.next().unwrap_or(0))
        }
        Err(_) => (0, 0),
    };
    let on_disk = fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    let file = if done > 0 && on_disk >= written && done <= total {
        let f = OpenOptions::new().write(true).open(&part).await.map_err(|e| e.to_string())?;
        f.set_len(written).await.map_err(|e| e.to_string())?;
        drop(f);
        OpenOptions::new().append(true).open(&part).await
    } else {
        done = 0;
        File::create(&part).await
    }
    .map_err(|e| format!("Can't write file: {e}"))?;
    let mut out = BufWriter::with_capacity(1024 * 1024, file);
    let mut bytes_written = if done > 0 { written } else { 0 };
    rep.base.received = bytes_written;
    rep.base.fraction = Some(done as f64 / total as f64);
    rep.flush();

    if done == 0 {
        if let Some(init) = &pl.init {
            let b = get_bytes(init, referer).await?;
            out.write_all(&b).await.map_err(|e| e.to_string())?;
            bytes_written += b.len() as u64;
            rep.add(b.len() as u64);
        }
    }

    let mut keys: HashMap<String, Vec<u8>> = HashMap::new();
    let todo: Vec<Segment> = pl.segments.iter().skip(done).cloned().collect();
    let owned_ref: Option<String> = req.referer.clone();
    let mut fetched = stream::iter(todo.into_iter().map(move |s| {
        let rf = owned_ref.clone();
        async move {
            let data = get_bytes(&s.url, rf.as_deref()).await;
            (s, data)
        }
    }))
    .buffered(3);

    while let Some((seg, data)) = fetched.next().await {
        let mut data = data?;
        if let Some(k) = &seg.key {
            let key = match keys.get(k.uri.as_str()) {
                Some(v) => v.clone(),
                None => {
                    let v = get_bytes(&k.uri, referer).await?;
                    keys.insert(k.uri.to_string(), v.clone());
                    v
                }
            };
            if key.len() != 16 {
                return Err("Stream key has an unexpected size".into());
            }
            let iv = k.iv.unwrap_or_else(|| {
                let mut b = [0u8; 16];
                b[8..].copy_from_slice(&seg.seq.to_be_bytes());
                b
            });
            let dec = cbc::Decryptor::<aes::Aes128>::new_from_slices(&key, &iv).map_err(|_| "Bad stream key")?;
            data = dec.decrypt_padded_vec_mut::<Pkcs7>(&data).map_err(|_| "Couldn't decrypt a segment")?;
        }
        out.write_all(&data).await.map_err(|e| format!("Disk write failed: {e}"))?;
        bytes_written += data.len() as u64;
        done += 1;
        rep.base.fraction = Some(done as f64 / total as f64);
        rep.add(data.len() as u64);

        if done % 8 == 0 || done == total {
            out.flush().await.ok();
            fs::write(&state, format!("{done} {bytes_written}")).await.ok();
        }
        if let Some(o) = check(ctl) {
            out.flush().await.ok();
            fs::write(&state, format!("{done} {bytes_written}")).await.ok();
            return Ok(o);
        }
    }
    out.flush().await.map_err(|e| e.to_string())?;
    drop(out);
    let _ = fs::remove_file(&state).await;

    let ext = if pl.init.is_some() { "mp4" } else { "ts" };
    let stem = req.title.clone().or_else(|| net::stem_from_url(&url)).unwrap_or_else(|| "stream".into());
    let stem = net::sanitize_filename(strip_ext(&stem));
    let dest = unique_path(dir, &stem, ext);
    fs::rename(&part, &dest).await.map_err(|e| format!("Couldn't save file: {e}"))?;
    Ok(Outcome::Done(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attrs() {
        let l = r#"#EXT-X-STREAM-INF:BANDWIDTH=1280000,CODECS="avc1.4d401f,mp4a.40.2",RESOLUTION=1280x720"#;
        assert_eq!(attr(l, "BANDWIDTH"), Some("1280000"));
        assert_eq!(attr(l, "CODECS"), Some("avc1.4d401f,mp4a.40.2"));
        assert_eq!(attr(l, "RESOLUTION"), Some("1280x720"));
        let k = r#"#EXT-X-KEY:METHOD=AES-128,URI="key.bin",IV=0x0000000000000000000000000000000A"#;
        assert_eq!(attr(k, "URI"), Some("key.bin"));
        assert_eq!(attr(k, "IV"), Some("0x0000000000000000000000000000000A"));
    }

    #[test]
    fn strips_video_ext() {
        assert_eq!(strip_ext("clip.mp4"), "clip");
        assert_eq!(strip_ext("v1.2 final"), "v1.2 final");
    }
}
