//! Turns an HTML page into: the videos on it, the pagination around it,
//! and links to sub-pages (cards) that probably hold a video each.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::net::{self, LinkKind};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoItem {
    pub url: String,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    /// "file" or "hls"
    pub kind: String,
    pub ext: Option<String>,
    pub page_url: Option<String>,
    pub size: Option<u64>,
    /// Where on the page we found it (video tag, meta, link, script…)
    pub source: String,
    /// Vertical resolution if the page or URL says (720, 1080…)
    pub quality: Option<u32>,
    /// Same value for every quality variant of one video.
    pub group: String,
}

impl VideoItem {
    /// Higher is better: resolution, then a direct file over a stream, then size.
    pub fn rank(&self) -> (u32, u8, u64) {
        (self.quality.unwrap_or(0), (self.kind == "file") as u8, self.size.unwrap_or(0))
    }
}

/// Treat everything as one video and return its best variant.
pub fn collapse_all(mut items: Vec<VideoItem>) -> Option<VideoItem> {
    if let Some(g) = items.first().map(|v| v.group.clone()) {
        items.iter_mut().for_each(|v| v.group = g.clone());
    }
    collapse_variants(items).into_iter().next()
}

/// Keep one entry per video: the best quality variant, carrying over any
/// title / thumbnail the others had. Order of first appearance is kept.
pub fn collapse_variants(items: Vec<VideoItem>) -> Vec<VideoItem> {
    let mut out: Vec<VideoItem> = Vec::new();
    for v in items {
        match out.iter_mut().find(|o| o.group == v.group) {
            Some(o) => {
                let (title, thumb) = (o.title.clone(), o.thumbnail.clone());
                if v.rank() > o.rank() {
                    *o = v;
                }
                if o.title.is_none() {
                    o.title = title;
                }
                if o.thumbnail.is_none() {
                    o.thumbnail = thumb;
                }
            }
            None => out.push(v),
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageLink {
    pub number: u32,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailLink {
    pub url: String,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageScan {
    pub url: String,
    pub title: Option<String>,
    pub image: Option<String>,
    pub videos: Vec<VideoItem>,
    pub current_page: Option<u32>,
    pub next_page: Option<String>,
    pub prev_page: Option<String>,
    /// True when `next_page` was guessed from the URL pattern, not read from a link.
    pub next_inferred: bool,
    pub pages: Vec<PageLink>,
    pub detail_links: Vec<DetailLink>,
}

macro_rules! sel {
    ($name:ident, $css:expr) => {
        static $name: LazyLock<Selector> = LazyLock::new(|| Selector::parse($css).unwrap());
    };
}

sel!(S_BASE, "base[href]");
sel!(S_TITLE, "title");
sel!(S_META, "meta");
sel!(S_VIDEO, "video");
sel!(S_SOURCE, "source");
sel!(S_A, "a[href]");
sel!(S_IMG, "img");
sel!(S_IFRAME, "iframe[src], embed[src]");
sel!(S_LINK_NEXT, "link[rel~=next][href], a[rel~=next][href]");
sel!(S_LINK_PREV, "link[rel~=prev][href], a[rel~=prev][href]");
sel!(S_JSONLD, r#"script[type="application/ld+json"]"#);

static RE_ABS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:https?:)?//[^\s"'<>()\\\[\]{}]+?\.(?:mp4|m4v|webm|mkv|mov|m3u8|flv|3gp|ogv)(?:\?[^\s"'<>()\\\[\]{}]*)?"#,
    )
    .unwrap()
});
static RE_REL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)["'](/[^\s"'<>]+?\.(?:mp4|m4v|webm|mkv|mov|m3u8)(?:\?[^\s"'<>]*)?)["']"#).unwrap()
});
static RE_PAGE_QUERY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(^|&)(page|p|pg|paged|pagenum|page_num|pn|pageno)=(\d{1,5})").unwrap());
static RE_PAGE_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/(page|p|pg)[/\-_]?(\d{1,5})(/|\.html?)?$").unwrap());
static RE_DETAIL_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)/(videos?|watch|v|embed|clips?|movies?|play|player|episodes?|media|post|reel|shorts?|stream)(/|$|\?|\.|-|_)")
        .unwrap()
});

const NEXT_WORDS: &[&str] = &[
    "next", "next page", "next »", "next ›", "next >", "›", "»", ">", "→", ">>", "older", "older posts",
    "older entries", "more", "load more", "show more", "next>", "siguiente", "suivant", "weiter",
];
const PREV_WORDS: &[&str] = &["prev", "previous", "previous page", "‹", "«", "<", "←", "<<", "newer", "newer posts"];

fn clean_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "We will go to the end of the ocean | ClipSite" -> "We will go to the end of the ocean"
fn strip_site_suffix(t: &str) -> String {
    for sep in [" | ", " – ", " — ", " - ", " :: ", " · "] {
        if let Some((head, tail)) = t.rsplit_once(sep) {
            if tail.chars().count() <= 32 && head.chars().count() >= 8 {
                return head.trim().to_string();
            }
        }
    }
    t.to_string()
}

fn non_empty(s: Option<&str>) -> Option<String> {
    s.map(clean_text).filter(|t| !t.is_empty())
}

fn resolve(base: &Url, href: &str) -> Option<Url> {
    let href = href.trim().replace("\\/", "/").replace("&amp;", "&");
    if href.is_empty()
        || href.starts_with('#')
        || href.starts_with("javascript:")
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
        || href.starts_with("data:")
        || href.starts_with("blob:")
    {
        return None;
    }
    let u = base.join(&href).ok()?;
    matches!(u.scheme(), "http" | "https").then(|| net::strip_fragment(u))
}

/// The first usable image URL on an `img` (handles lazy-loading attributes).
fn img_src(img: ElementRef, base: &Url) -> Option<String> {
    let v = img.value();
    for attr in ["data-src", "data-original", "data-lazy-src", "data-thumb", "data-poster", "src"] {
        if let Some(s) = v.attr(attr) {
            if !s.starts_with("data:") {
                if let Some(u) = resolve(base, s) {
                    return Some(u.to_string());
                }
            }
        }
    }
    if let Some(set) = v.attr("srcset").or_else(|| v.attr("data-srcset")) {
        let first = set.split(',').next()?.split_whitespace().next()?;
        return resolve(base, first).map(|u| u.to_string());
    }
    None
}

pub fn page_number_in_url(u: &Url) -> Option<u32> {
    if let Some(q) = u.query() {
        if let Some(c) = RE_PAGE_QUERY.captures(q) {
            return c[3].parse().ok();
        }
    }
    RE_PAGE_PATH.captures(u.path()).and_then(|c| c[2].parse().ok())
}

/// Swap the page number in a URL for `n` (only when the URL already has one).
fn with_page_number(u: &Url, n: u32) -> Option<Url> {
    if let Some(q) = u.query() {
        if let Some(c) = RE_PAGE_QUERY.captures(q) {
            let m = c.get(3).unwrap();
            let new_q = format!("{}{}{}", &q[..m.start()], n, &q[m.end()..]);
            let mut nu = u.clone();
            nu.set_query(Some(&new_q));
            return Some(nu);
        }
    }
    let path = u.path();
    let c = RE_PAGE_PATH.captures(path)?;
    let m = c.get(2).unwrap();
    let new_path = format!("{}{}{}", &path[..m.start()], n, &path[m.end()..]);
    let mut nu = u.clone();
    nu.set_path(&new_path);
    Some(nu)
}

struct Collector {
    seen: HashSet<String>,
    items: Vec<VideoItem>,
}

impl Collector {
    fn push(&mut self, u: Url, title: Option<String>, thumb: Option<String>, page: &Url, source: &str, hint: Option<LinkKind>) -> Option<usize> {
        let kind = match net::kind_from_url(&u) {
            LinkKind::Unknown => hint.unwrap_or(LinkKind::File),
            k => k,
        };
        if kind == LinkKind::Unknown {
            return None;
        }
        let key = u.as_str().to_string();
        if let Some(idx) = self.items.iter().position(|v| v.url == key) {
            let existing = &mut self.items[idx];
            // Merge in better metadata found elsewhere on the page.
            if existing.title.is_none() {
                existing.title = title;
            }
            if existing.thumbnail.is_none() {
                existing.thumbnail = thumb;
            }
            return Some(idx);
        }
        if !self.seen.insert(key.clone()) {
            return None;
        }
        self.items.push(VideoItem {
            ext: net::ext_of(&u),
            quality: net::quality_of_url(&u),
            group: net::group_key(&u),
            url: key,
            title,
            thumbnail: thumb,
            kind: kind.as_str().into(),
            page_url: Some(page.to_string()),
            size: None,
            source: source.into(),
        });
        Some(self.items.len() - 1)
    }
}

fn walk_json_ld(v: &serde_json::Value, base: &Url, page: &Url, col: &mut Collector) {
    match v {
        serde_json::Value::Array(a) => a.iter().for_each(|x| walk_json_ld(x, base, page, col)),
        serde_json::Value::Object(o) => {
            let ty = o.get("@type").map(|t| t.to_string().to_ascii_lowercase()).unwrap_or_default();
            if ty.contains("videoobject") {
                let title = o.get("name").and_then(|x| x.as_str()).map(clean_text);
                let thumb = match o.get("thumbnailUrl") {
                    Some(serde_json::Value::String(s)) => Some(s.clone()),
                    Some(serde_json::Value::Array(a)) => a.first().and_then(|x| x.as_str()).map(String::from),
                    _ => None,
                }
                .and_then(|s| resolve(base, &s))
                .map(|u| u.to_string());
                for key in ["contentUrl", "url"] {
                    if let Some(u) = o.get(key).and_then(|x| x.as_str()).and_then(|s| resolve(base, s)) {
                        if net::kind_from_url(&u) != LinkKind::Unknown {
                            col.push(u, title.clone(), thumb.clone(), page, "json-ld", None);
                        }
                    }
                }
            }
            for (_, child) in o {
                if child.is_object() || child.is_array() {
                    walk_json_ld(child, base, page, col);
                }
            }
        }
        _ => {}
    }
}

pub fn scan_html(page_url: &Url, html: &str) -> PageScan {
    let doc = Html::parse_document(html);

    let base = doc
        .select(&S_BASE)
        .next()
        .and_then(|b| b.value().attr("href"))
        .and_then(|h| page_url.join(h).ok())
        .unwrap_or_else(|| page_url.clone());

    // Page metadata
    let mut metas: HashMap<String, String> = HashMap::new();
    for m in doc.select(&S_META) {
        let v = m.value();
        let key = v.attr("property").or_else(|| v.attr("name")).or_else(|| v.attr("itemprop"));
        if let (Some(k), Some(c)) = (key, v.attr("content")) {
            metas.entry(k.to_ascii_lowercase()).or_insert_with(|| c.to_string());
        }
    }
    let page_title = non_empty(metas.get("og:title").map(|s| s.as_str()))
        .or_else(|| non_empty(metas.get("twitter:title").map(|s| s.as_str())))
        .or_else(|| doc.select(&S_TITLE).next().and_then(|t| non_empty(Some(&t.text().collect::<String>()))).map(|t| strip_site_suffix(&t)));
    let page_image = metas
        .get("og:image")
        .or_else(|| metas.get("twitter:image"))
        .and_then(|s| resolve(&base, s))
        .map(|u| u.to_string());

    let mut col = Collector { seen: HashSet::new(), items: Vec::new() };

    // 1. <video> and <source>
    for v in doc.select(&S_VIDEO) {
        let el = v.value();
        let title = non_empty(el.attr("title")).or_else(|| non_empty(el.attr("aria-label"))).or_else(|| non_empty(el.attr("data-title")));
        let poster = el.attr("poster").or_else(|| el.attr("data-poster")).and_then(|p| resolve(&base, p)).map(|u| u.to_string());
        // Every source inside one <video> is the same clip in another quality/format.
        let mut in_tag: Vec<usize> = Vec::new();
        for attr in ["src", "data-src"] {
            if let Some(u) = el.attr(attr).and_then(|s| resolve(&base, s)) {
                in_tag.extend(col.push(u, title.clone(), poster.clone(), page_url, "video tag", Some(LinkKind::File)));
            }
        }
        for s in v.select(&S_SOURCE) {
            let se = s.value();
            let hint = match se.attr("type").map(|t| t.to_ascii_lowercase()) {
                Some(t) if t.contains("mpegurl") => Some(LinkKind::Hls),
                _ => Some(LinkKind::File),
            };
            // Players label qualities in many ways: size="1080", res="720", label="720p", title="HD"…
            let label_q = ["size", "res", "data-res", "data-quality", "quality", "label", "title", "data-label", "height"]
                .iter()
                .filter_map(|a| se.attr(a))
                .filter_map(net::quality_of_text)
                .max();
            for attr in ["src", "data-src"] {
                if let Some(u) = se.attr(attr).and_then(|x| resolve(&base, x)) {
                    if let Some(i) = col.push(u, title.clone(), poster.clone(), page_url, "video tag", hint) {
                        if col.items[i].quality.is_none() {
                            col.items[i].quality = label_q;
                        }
                        in_tag.push(i);
                    }
                }
            }
        }
        if let Some(&first) = in_tag.first() {
            let g = col.items[first].group.clone();
            for &i in &in_tag {
                col.items[i].group = g.clone();
            }
        }
    }

    // 2. Open Graph / Twitter player streams
    for key in ["og:video:secure_url", "og:video:url", "og:video", "twitter:player:stream", "contenturl"] {
        if let Some(u) = metas.get(key).and_then(|s| resolve(&base, s)) {
            if net::kind_from_url(&u) != LinkKind::Unknown
                || metas.get("og:video:type").is_some_and(|t| t.starts_with("video/"))
            {
                col.push(u, page_title.clone(), page_image.clone(), page_url, "meta tag", Some(LinkKind::File));
            }
        }
    }

    // 3. JSON-LD VideoObject
    for s in doc.select(&S_JSONLD) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s.text().collect::<String>()) {
            walk_json_ld(&v, &base, page_url, &mut col);
        }
    }

    // 4. Iframes/embeds pointing straight at a file
    let mut embed_links: Vec<DetailLink> = Vec::new();
    for f in doc.select(&S_IFRAME) {
        if let Some(u) = f.value().attr("src").and_then(|s| resolve(&base, s)) {
            if net::kind_from_url(&u) != LinkKind::Unknown {
                col.push(u, page_title.clone(), page_image.clone(), page_url, "embed", None);
            } else if net::same_site(&u, page_url) {
                embed_links.push(DetailLink { url: u.to_string(), title: page_title.clone(), thumbnail: page_image.clone() });
            }
        }
    }

    // 5. Anchors: direct video links, pagination and detail cards
    let current_from_url = page_number_in_url(page_url);
    let mut numbered: Vec<PageLink> = Vec::new();
    // rel="next"/"prev" is the most reliable signal when a site provides it.
    let mut next_by_text: Option<Url> =
        doc.select(&S_LINK_NEXT).find_map(|e| e.value().attr("href").and_then(|h| resolve(&base, h)));
    let mut prev_by_text: Option<Url> =
        doc.select(&S_LINK_PREV).find_map(|e| e.value().attr("href").and_then(|h| resolve(&base, h)));
    let mut current_marked: Option<u32> = None;
    let mut details: Vec<(i32, DetailLink)> = Vec::new();
    let mut detail_seen: HashSet<String> = HashSet::new();
    let page_key = page_url.as_str().trim_end_matches('/').to_string();

    for a in doc.select(&S_A) {
        let el = a.value();
        let Some(u) = el.attr("href").and_then(|h| resolve(&base, h)) else { continue };
        let text = clean_text(&a.text().collect::<String>());
        let lower = text.to_ascii_lowercase();
        let aria = el.attr("aria-label").unwrap_or("").to_ascii_lowercase();
        let class = el.attr("class").unwrap_or("").to_ascii_lowercase();
        let img = a.select(&S_IMG).next();

        let kind = net::kind_from_url(&u);
        if kind != LinkKind::Unknown {
            // Text like "MP4 (1080p, h264, 516.37 MB)" describes a quality, not the video.
            let label = net::format_label(&text)
                .or_else(|| el.attr("title").and_then(net::format_label))
                .or_else(|| el.attr("download").and_then(|d| net::format_label(&d.replace(['_', '-'], " "))));
            let title = if label.is_some() {
                // A real file name in download="…" still beats the page title.
                non_empty(el.attr("download")).filter(|d| net::format_label(&d.replace(['_', '-', '.'], " ")).is_none())
            } else {
                non_empty(el.attr("title"))
                    .or_else(|| (!text.is_empty() && text.len() < 160).then(|| text.clone()))
                    .or_else(|| img.and_then(|i| non_empty(i.value().attr("alt"))))
                    .or_else(|| non_empty(el.attr("download")))
            };
            let thumb = img.and_then(|i| img_src(i, &base));
            let source = if label.is_some() { "download button" } else { "link" };
            if let Some(i) = col.push(u, title, thumb, page_url, source, None) {
                if let Some((q, size)) = label {
                    let v = &mut col.items[i];
                    v.source = source.into();
                    if v.quality.is_none() {
                        v.quality = q;
                    }
                    if v.size.is_none() {
                        v.size = size;
                    }
                }
            }
            continue;
        }
        if !net::same_site(&u, page_url) {
            continue;
        }

        // Pagination: numbered links
        let is_number = !lower.is_empty() && lower.len() <= 5 && lower.chars().all(|c| c.is_ascii_digit());
        if is_number {
            if let Ok(n) = lower.parse::<u32>() {
                let marked_current = el.attr("aria-current").is_some()
                    || class.split_whitespace().any(|c| c == "active" || c == "current" || c.ends_with("-current") || c.ends_with("--active"));
                let parent_current = a.parent().and_then(ElementRef::wrap).is_some_and(|p| {
                    let pc = p.value().attr("class").unwrap_or("").to_ascii_lowercase();
                    pc.split_whitespace().any(|c| c == "active" || c == "current")
                });
                if marked_current || parent_current || u.as_str().trim_end_matches('/') == page_key {
                    current_marked.get_or_insert(n);
                }
                if n > 0 && !numbered.iter().any(|p| p.number == n) {
                    numbered.push(PageLink { number: n, url: u.to_string() });
                }
                continue;
            }
        }

        // Pagination: next / previous words
        let looks_next = NEXT_WORDS.contains(&lower.as_str())
            || aria.contains("next")
            || class.split(|c: char| !c.is_alphanumeric()).any(|c| c == "next");
        let looks_prev = PREV_WORDS.contains(&lower.as_str())
            || aria.contains("prev")
            || class.split(|c: char| !c.is_alphanumeric()).any(|c| c == "prev" || c == "previous");
        if looks_next && !looks_prev {
            next_by_text.get_or_insert(u);
            continue;
        }
        if looks_prev {
            prev_by_text.get_or_insert(u);
            continue;
        }

        // Detail cards: links that wrap a thumbnail or look like a video page
        if u.as_str().trim_end_matches('/') == page_key || page_number_in_url(&u).is_some() {
            continue;
        }
        let mut score = 0;
        if img.is_some() {
            score += 2;
        }
        if RE_DETAIL_PATH.is_match(u.path()) {
            score += 2;
        }
        if u.path().len() <= 1 {
            score -= 4;
        }
        if score >= 2 && detail_seen.insert(u.as_str().to_string()) {
            let title = non_empty(el.attr("title"))
                .or_else(|| img.and_then(|i| non_empty(i.value().attr("alt"))))
                .or_else(|| (!text.is_empty() && text.len() < 160).then(|| text.clone()));
            let thumbnail = img.and_then(|i| img_src(i, &base));
            details.push((score, DetailLink { url: u.to_string(), title, thumbnail }));
        } else if score >= 2 {
            // Same card linked twice (image + title): fill in what's missing.
            if let Some((_, d)) = details.iter_mut().find(|(_, d)| d.url == u.as_str()) {
                if d.title.is_none() && !text.is_empty() && text.len() < 160 {
                    d.title = Some(text.clone());
                }
                if d.thumbnail.is_none() {
                    d.thumbnail = img.and_then(|i| img_src(i, &base));
                }
            }
        }
    }

    // 6. Raw text sweep: catches player configs and JSON inside scripts
    let unescaped = html.replace("\\/", "/").replace("\\u002F", "/").replace("\\u002f", "/").replace("&amp;", "&");
    for m in RE_ABS.find_iter(&unescaped) {
        if let Some(u) = resolve(&base, m.as_str()) {
            col.push(u, None, None, page_url, "script", None);
        }
    }
    for c in RE_REL.captures_iter(&unescaped) {
        if let Some(u) = resolve(&base, &c[1]) {
            col.push(u, None, None, page_url, "script", None);
        }
    }

    // A page built around one player (one <video>, or an og:video) shows one clip:
    // everything found outside plain download links is that clip in other qualities.
    let players = doc.select(&S_VIDEO).count();
    let has_buttons = col.items.iter().any(|v| v.source == "download button");
    let single_player = players == 1 || (players == 0 && (metas.contains_key("og:video") || has_buttons));
    if single_player {
        if let Some(g) = col.items.iter().find(|v| v.source != "link").map(|v| v.group.clone()) {
            for v in col.items.iter_mut().filter(|v| v.source != "link") {
                v.group = g.clone();
            }
        }
    }

    // Collapse quality variants (720p/1080p files, MP4 + HLS of one clip) to the best one.
    col.items = collapse_variants(std::mem::take(&mut col.items));

    // A single video on a page is almost always "the" video: give it the page's identity.
    if col.items.len() == 1 {
        let v = &mut col.items[0];
        if v.title.is_none() {
            v.title = page_title.clone();
        }
        if v.thumbnail.is_none() {
            v.thumbnail = page_image.clone();
        }
    }
    // When a page offers the same clip as HLS and as a file, both are kept; the UI groups by title.

    // Resolve pagination
    numbered.sort_by_key(|p| p.number);
    let current = current_marked.or(current_from_url).or_else(|| (!numbered.is_empty()).then_some(1));
    let mut next_inferred = false;
    let next_page = next_by_text
        .map(|u| u.to_string())
        .or_else(|| {
            let c = current?;
            numbered.iter().find(|p| p.number == c + 1).map(|p| p.url.clone())
        })
        .or_else(|| {
            let n = current_from_url?;
            next_inferred = true;
            with_page_number(page_url, n + 1).map(|u| u.to_string())
        });
    let prev_page = prev_by_text.map(|u| u.to_string()).or_else(|| {
        let c = current?;
        if c <= 1 {
            return None;
        }
        numbered.iter().find(|p| p.number == c - 1).map(|p| p.url.clone())
    });
    if let Some(c) = current {
        if !numbered.iter().any(|p| p.number == c) && !numbered.is_empty() {
            numbered.push(PageLink { number: c, url: page_url.to_string() });
            numbered.sort_by_key(|p| p.number);
        }
    }

    details.sort_by(|a, b| b.0.cmp(&a.0));
    let mut detail_links: Vec<DetailLink> = embed_links;
    detail_links.extend(details.into_iter().map(|(_, d)| d));
    detail_links.truncate(150);

    PageScan {
        url: page_url.to_string(),
        title: page_title,
        image: page_image,
        videos: col.items,
        current_page: current,
        next_page,
        prev_page,
        next_inferred,
        pages: numbered,
        detail_links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = r#"
    <html><head><title>Clips - page 2</title></head><body>
      <div class="grid">
        <a href="/video/alpha"><img data-src="/thumbs/a.jpg" alt="Alpha clip"></a>
        <a href="/video/alpha">Alpha clip</a>
        <a href="/video/beta"><img src="/thumbs/b.jpg" alt="Beta"></a>
        <a href="/files/direct.mp4" download="Direct One.mp4">Download</a>
      </div>
      <nav class="pagination">
        <a href="/clips?page=1">1</a>
        <span class="current"><a href="/clips?page=2">2</a></span>
        <a href="/clips?page=3">3</a>
        <a href="/clips?page=3" class="next">Next</a>
      </nav>
      <a href="/about">About</a>
    </body></html>"#;

    #[test]
    fn scans_listing() {
        let url = Url::parse("https://example.com/clips?page=2").unwrap();
        let s = scan_html(&url, LISTING);
        assert_eq!(s.videos.len(), 1);
        assert_eq!(s.videos[0].title.as_deref(), Some("Direct One.mp4"));
        assert_eq!(s.current_page, Some(2));
        assert_eq!(s.next_page.as_deref(), Some("https://example.com/clips?page=3"));
        assert_eq!(s.prev_page.as_deref(), Some("https://example.com/clips?page=1"));
        assert_eq!(s.pages.len(), 3);
        assert_eq!(s.detail_links.len(), 2);
        assert_eq!(s.detail_links[0].thumbnail.as_deref(), Some("https://example.com/thumbs/a.jpg"));
    }

    #[test]
    fn scans_detail_page() {
        let html = r#"<html><head><meta property="og:title" content="Alpha clip">
          <meta property="og:image" content="https://cdn.x.com/a.jpg"></head><body>
          <video poster="/p.jpg"><source src="https://cdn.x.com/a/720.mp4" type="video/mp4"></video>
          <script>var cfg = {"hls":"https:\/\/cdn.x.com\/a\/master.m3u8"};</script>
        </body></html>"#;
        let url = Url::parse("https://x.com/video/alpha").unwrap();
        let s = scan_html(&url, html);
        // One player: the 720p MP4 and the HLS master are the same clip, keep one.
        assert_eq!(s.videos.len(), 1);
        assert_eq!(s.videos[0].kind, "file");
        assert_eq!(s.videos[0].quality, Some(720));
        assert_eq!(s.videos[0].url, "https://cdn.x.com/a/720.mp4");
        assert_eq!(s.videos[0].thumbnail.as_deref(), Some("https://x.com/p.jpg"));
    }

    #[test]
    fn keeps_best_quality_variant() {
        let html = r#"<html><body>
          <a href="/f/sunset_480p.mp4">Sunset 480p</a>
          <a href="/f/sunset_1080p.mp4">Sunset 1080p</a>
          <a href="/f/sunset_720p.mp4?sig=1">Sunset 720p</a>
          <a href="/f/harbour.mp4">Harbour</a>
          <video><source src="/s/rain/360.mp4" label="360p"><source src="/s/rain/hd.mp4" label="1080p"></video>
          <video><source src="/s/tide.webm" size="720"></video>
        </body></html>"#;
        let url = Url::parse("https://x.com/list").unwrap();
        let s = scan_html(&url, html);
        let urls: Vec<&str> = s.videos.iter().map(|v| v.url.as_str()).collect();
        assert_eq!(s.videos.len(), 4, "{urls:?}");
        assert!(urls.contains(&"https://x.com/f/sunset_1080p.mp4"));
        assert!(urls.contains(&"https://x.com/f/harbour.mp4"));
        assert!(urls.contains(&"https://x.com/s/rain/hd.mp4"));
        assert!(urls.contains(&"https://x.com/s/tide.webm"));
    }

    #[test]
    fn infers_next_from_path() {
        let url = Url::parse("https://x.com/videos/page/4/").unwrap();
        let s = scan_html(&url, "<html><body>nothing</body></html>");
        assert_eq!(s.next_page.as_deref(), Some("https://x.com/videos/page/5/"));
        assert!(s.next_inferred);
    }
}
