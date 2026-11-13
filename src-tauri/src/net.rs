//! Shared HTTP client and small URL / naming helpers.

use std::sync::LazyLock;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_LANGUAGE};
use url::Url;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// One client for the whole app so cookies set while scanning a page
/// are sent again when its videos are downloaded.
pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .cookie_store(true)
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .pool_idle_timeout(Duration::from_secs(60))
        .build()
        .expect("failed to build HTTP client")
});

/// Video container extensions we treat as direct files.
pub const VIDEO_EXTS: &[&str] = &[
    "mp4", "m4v", "webm", "mkv", "mov", "avi", "flv", "wmv", "3gp", "ogv", "mpg", "mpeg", "ts",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    File,
    Hls,
    Unknown,
}

impl LinkKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkKind::File => "file",
            LinkKind::Hls => "hls",
            LinkKind::Unknown => "unknown",
        }
    }
}

/// Accepts what people paste: bare domains, `//cdn...`, surrounding whitespace.
pub fn normalize_input(raw: &str) -> Result<Url, String> {
    let s = raw.trim().trim_matches(|c| c == '"' || c == '\'' || c == '<' || c == '>');
    if s.is_empty() {
        return Err("Paste a link first".into());
    }
    let candidate = if s.starts_with("//") {
        format!("https:{s}")
    } else if !s.contains("://") {
        format!("https://{s}")
    } else {
        s.to_string()
    };
    let url = Url::parse(&candidate).map_err(|_| "That doesn't look like a valid link".to_string())?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        _ => Err("Only http and https links are supported".into()),
    }
}

/// Lowercase extension of the last path segment, if any.
pub fn ext_of(url: &Url) -> Option<String> {
    let seg = url.path_segments()?.next_back()?;
    let (_, ext) = seg.rsplit_once('.')?;
    let ext = ext.to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 5 {
        None
    } else {
        Some(ext)
    }
}

pub fn kind_from_url(url: &Url) -> LinkKind {
    match ext_of(url).as_deref() {
        Some("m3u8") => LinkKind::Hls,
        Some(e) if VIDEO_EXTS.contains(&e) => LinkKind::File,
        _ => {
            // Some CDNs hide the extension in a query param, e.g. ?file=clip.mp4
            let q = url.query().unwrap_or("").to_ascii_lowercase();
            if q.contains(".m3u8") {
                LinkKind::Hls
            } else if VIDEO_EXTS.iter().filter(|e| **e != "ts").any(|e| q.contains(&format!(".{e}"))) {
                LinkKind::File
            } else {
                LinkKind::Unknown
            }
        }
    }
}

/// `None` means "this is a web page (or something we can't download)".
pub fn kind_from_content_type(ct: &str) -> Option<LinkKind> {
    let ct = ct.to_ascii_lowercase();
    if ct.contains("mpegurl") {
        Some(LinkKind::Hls)
    } else if ct.starts_with("video/") {
        Some(LinkKind::File)
    } else if ct.starts_with("application/octet-stream") || ct.starts_with("binary/") {
        Some(LinkKind::Unknown)
    } else {
        None
    }
}

pub fn ext_from_content_type(ct: &str) -> Option<&'static str> {
    let ct = ct.to_ascii_lowercase();
    Some(match () {
        _ if ct.contains("mp4") => "mp4",
        _ if ct.contains("webm") => "webm",
        _ if ct.contains("matroska") => "mkv",
        _ if ct.contains("quicktime") => "mov",
        _ if ct.contains("x-flv") => "flv",
        _ if ct.contains("3gpp") => "3gp",
        _ if ct.contains("ogg") => "ogv",
        _ if ct.contains("mp2t") => "ts",
        _ if ct.contains("x-msvideo") => "avi",
        _ => return None,
    })
}

/// Filename from a Content-Disposition header, if present.
pub fn filename_from_disposition(v: &str) -> Option<String> {
    // RFC 5987 form first: filename*=UTF-8''name.mp4
    if let Some(idx) = v.to_ascii_lowercase().find("filename*=") {
        let rest = &v[idx + 10..];
        let rest = rest.split(';').next().unwrap_or("").trim();
        let enc = rest.rsplit("''").next().unwrap_or(rest);
        let decoded = percent_decode(enc.trim_matches('"'));
        if !decoded.is_empty() {
            return Some(decoded);
        }
    }
    let lower = v.to_ascii_lowercase();
    let idx = lower.find("filename=")?;
    let rest = v[idx + 9..].split(';').next()?.trim().trim_matches('"');
    if rest.is_empty() { None } else { Some(rest.to_string()) }
}

pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Last path segment without extension, decoded, as a readable name.
pub fn stem_from_url(url: &Url) -> Option<String> {
    let seg = url.path_segments()?.filter(|s| !s.is_empty()).next_back()?;
    let seg = percent_decode(seg);
    let stem = seg.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(seg);
    let stem = stem.replace(['_', '-'], " ").trim().to_string();
    if stem.is_empty() { None } else { Some(stem) }
}

/// Make a string safe to use as a file name on Windows, Android and Linux.
pub fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(|c: char| c == '.' || c == ' ');
    let mut out: String = trimmed.chars().take(140).collect();
    if out.is_empty() {
        out = "video".into();
    }
    out
}

pub fn same_site(a: &Url, b: &Url) -> bool {
    fn base(h: &str) -> &str {
        h.strip_prefix("www.").unwrap_or(h)
    }
    match (a.host_str(), b.host_str()) {
        (Some(x), Some(y)) => {
            let (x, y) = (base(x), base(y));
            x == y || x.ends_with(&format!(".{y}")) || y.ends_with(&format!(".{x}"))
        }
        _ => false,
    }
}

// ------------------------------------------------------------ quality & identity

/// Tokens that describe quality rather than *which* video it is.
fn quality_token(t: &str) -> Option<u32> {
    let t = t.to_ascii_lowercase();
    let res = |n: &str| -> Option<u32> {
        let v: u32 = n.parse().ok()?;
        matches!(v, 144 | 180 | 240 | 270 | 360 | 405 | 480 | 540 | 576 | 720 | 900 | 1080 | 1440 | 2160 | 4320).then_some(v)
    };
    // 720, 720p, 1080p60, 2160p30
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = &t[digits.len()..];
        if rest.is_empty() || rest == "p" || (rest.starts_with('p') && rest[1..].chars().all(|c| c.is_ascii_digit())) {
            if let Some(v) = res(&digits) {
                return Some(v);
            }
        }
        // 1280x720
        if let Some(h) = rest.strip_prefix('x') {
            if h.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(hv) = h.parse::<u32>() {
                    if (100..=4320).contains(&hv) {
                        return Some(hv);
                    }
                }
            }
        }
    }
    Some(match t.as_str() {
        "8k" => 4320,
        "4k" | "uhd" | "2160" => 2160,
        "2k" | "qhd" => 1440,
        "fhd" | "fullhd" => 1080,
        "hd" | "hq" | "high" => 720,
        "sd" | "medium" | "med" => 480,
        "lq" | "low" | "mobile" | "small" | "tiny" => 240,
        _ => return None,
    })
}

fn tokens(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty())
}

/// Best guess at the vertical resolution from free text (labels, attributes).
pub fn quality_of_text(s: &str) -> Option<u32> {
    tokens(s).filter_map(quality_token).max()
}

/// Resolution hinted by a URL's path or query (`/720p/`, `_1080.mp4`, `?quality=480`).
pub fn quality_of_url(u: &Url) -> Option<u32> {
    let path = percent_decode(u.path());
    let q = u.query().map(percent_decode).unwrap_or_default();
    quality_of_text(&path).max(quality_of_text(&q))
}

const GENERIC_NAMES: &[&str] = &[
    "index", "master", "playlist", "video", "videos", "stream", "streams", "manifest", "media", "file", "files",
    "download", "downloads", "source", "original", "default", "main", "chunklist", "prog", "hls", "mp4", "dash",
    "play", "get", "v", "vod", "content", "output", "out",
];

fn identity_of_segment(seg: &str) -> String {
    let seg = percent_decode(seg).to_ascii_lowercase();
    let stem = match seg.rsplit_once('.') {
        Some((a, e)) if e.len() <= 5 => a.to_string(),
        _ => seg,
    };
    tokens(&stem)
        .filter(|t| quality_token(t).is_none())
        .filter(|t| !GENERIC_NAMES.contains(t))
        .collect::<Vec<_>>()
        .join("-")
}

/// What makes two links "the same video": the file name (or the folder it sits in,
/// for names like `index.m3u8` / `720.mp4`) with quality words removed.
/// `clip_720p.mp4`, `clip_1080p.mp4` and `clip/master.m3u8` all map to `clip`.
pub fn group_key(u: &Url) -> String {
    let segs: Vec<&str> = u.path_segments().map(|s| s.filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    for seg in segs.iter().rev() {
        let id = identity_of_segment(seg);
        if !id.is_empty() {
            return id;
        }
    }
    u.as_str().to_string()
}

// ------------------------------------------------------------ rate limiting

/// How long to wait before retrying a 429 / 503 answer (honours Retry-After).
pub fn retry_wait(resp: &reqwest::Response, attempt: u32) -> Option<std::time::Duration> {
    let s = resp.status().as_u16();
    if s != 429 && s != 503 {
        return None;
    }
    let header = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    let backoff = 2u64.saturating_pow(attempt.min(6)).min(60) + (attempt as u64 % 3);
    Some(std::time::Duration::from_secs(header.unwrap_or(backoff).clamp(1, 120)))
}

/// A request that looks like a browser's <video> element fetching media.
pub fn media_get(url: &Url, referer: Option<&str>) -> reqwest::RequestBuilder {
    let mut r = CLIENT
        .get(url.clone())
        .header("Accept", "*/*")
        .header("Accept-Encoding", "identity")
        .header("Sec-Fetch-Dest", "video")
        .header("Sec-Fetch-Mode", "no-cors");
    if let Some(rf) = referer {
        let site = Url::parse(rf).ok().map(|p| if same_site(&p, url) { "same-site" } else { "cross-site" }).unwrap_or("cross-site");
        r = r.header(reqwest::header::REFERER, rf).header("Sec-Fetch-Site", site);
    }
    r
}

pub fn strip_fragment(mut u: Url) -> Url {
    u.set_fragment(None);
    u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_urls() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert_eq!(kind_from_url(&u("https://x.com/a/b.mp4?x=1")), LinkKind::File);
        assert_eq!(kind_from_url(&u("https://x.com/live/index.m3u8")), LinkKind::Hls);
        assert_eq!(kind_from_url(&u("https://x.com/videos?page=2")), LinkKind::Unknown);
        assert_eq!(kind_from_url(&u("https://x.com/get?file=clip.webm")), LinkKind::File);
    }

    #[test]
    fn normalizes_input() {
        assert_eq!(normalize_input(" example.com/x ").unwrap().as_str(), "https://example.com/x");
        assert!(normalize_input("ftp://x").is_err());
    }

    #[test]
    fn groups_quality_variants() {
        let k = |s: &str| group_key(&Url::parse(s).unwrap());
        let q = |s: &str| quality_of_url(&Url::parse(s).unwrap());
        assert_eq!(k("https://c.x/v/clip_720p.mp4"), "clip");
        assert_eq!(k("https://c.x/v/clip_1080p.mp4?token=abc"), "clip");
        assert_eq!(k("https://c.x/hls/clip/master.m3u8"), "clip");
        assert_eq!(k("https://c.x/abc123/720/index.m3u8"), "abc123");
        assert_eq!(k("https://c.x/abc123/1080.mp4"), "abc123");
        assert_ne!(k("https://c.x/a/video.mp4"), k("https://c.x/b/video.mp4"));
        assert_ne!(k("https://c.x/v/sunset.mp4"), k("https://c.x/v/sunrise.mp4"));
        assert_eq!(q("https://c.x/v/clip_1080p60.mp4"), Some(1080));
        assert_eq!(q("https://c.x/v/clip-1280x720.mp4"), Some(720));
        assert_eq!(q("https://c.x/get?quality=480"), Some(480));
        assert_eq!(q("https://c.x/v/2024/clip.mp4"), None);
        assert_eq!(quality_of_text("HD 720p"), Some(720));
        assert_eq!(quality_of_text("4K"), Some(2160));
    }

    #[test]
    fn names() {
        assert_eq!(sanitize_filename("a/b:c?.mp4"), "a b c .mp4");
        assert_eq!(
            filename_from_disposition("attachment; filename*=UTF-8''My%20Clip.mp4").as_deref(),
            Some("My Clip.mp4")
        );
        assert_eq!(filename_from_disposition("inline; filename=\"x.mp4\"").as_deref(), Some("x.mp4"));
        assert_eq!(stem_from_url(&Url::parse("https://a.b/v/my_clip-01.mp4").unwrap()).as_deref(), Some("my clip 01"));
    }
}
