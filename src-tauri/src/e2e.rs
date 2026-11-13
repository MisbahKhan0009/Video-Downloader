//! End-to-end checks against a local test site.
//! Run with: VIDRA_E2E=http://127.0.0.1:8765 VIDRA_E2E_ROOT=<site dir> cargo test e2e -- --ignored --nocapture

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::analyze;
use crate::download::{self, DownloadRequest, Manager, Progress};

fn base() -> String {
    std::env::var("VIDRA_E2E").expect("VIDRA_E2E not set")
}

async fn fetch_to(dir: &std::path::Path, id: &str, url: String, kind: &str, title: &str) -> Progress {
    let mgr = Arc::new(Manager::new(2));
    let last: Arc<Mutex<Option<Progress>>> = Arc::new(Mutex::new(None));
    let l2 = last.clone();
    download::run(
        mgr,
        DownloadRequest {
            id: id.into(),
            url,
            kind: kind.into(),
            title: Some(title.into()),
            referer: None,
            dir: dir.to_string_lossy().into_owned(),
        },
        move |p| *l2.lock().unwrap() = Some(p),
    )
    .await;
    let p = last.lock().unwrap().clone().unwrap();
    p
}

#[tokio::test]
#[ignore]
async fn e2e_paginated_site() {
    let b = base();
    let root = std::path::PathBuf::from(std::env::var("VIDRA_E2E_ROOT").unwrap());

    // Page link: no direct videos, pagination + cards found.
    let a = analyze::analyze(&format!("{b}/list/1.html")).await.unwrap();
    assert_eq!(a.kind, "page");
    let page = a.page.unwrap();
    println!("page: next={:?} pages={} cards={}", page.next_page, page.pages.len(), page.detail_links.len());
    assert!(page.videos.is_empty());
    assert_eq!(page.next_page.as_deref(), Some(format!("{b}/list/2.html").as_str()));
    assert_eq!(page.current_page, Some(1));
    assert_eq!(page.pages.len(), 3);
    assert_eq!(page.detail_links.len(), 2);

    // Deep scan opens the cards.
    let vids = analyze::deep_scan("s".into(), page.detail_links.clone(), Some(page.url.clone()), Arc::new(AtomicBool::new(false)), |_| {}).await;
    for v in &vids {
        println!("found: {} [{}] {:?}", v.url, v.kind, v.title);
    }
    // alpha's page offers 360p, 480p, 1080p and HLS of one clip: only the 1080p file survives.
    assert_eq!(vids.len(), 2, "one per card, best quality only");
    let alpha = vids.iter().find(|v| v.title.as_deref() == Some("Alpha 1")).expect("alpha");
    assert_eq!(alpha.url, format!("{b}/media/alpha.mp4"));
    assert_eq!(alpha.quality, Some(1080));
    assert!(vids.iter().any(|v| v.kind == "hls" && v.url.contains("master.m3u8")));

    // Page 2 via next link
    let p2 = analyze::scan_page(page.next_page.as_ref().unwrap(), Some(&page.url)).await.unwrap();
    assert_eq!(p2.current_page, Some(2));
    assert_eq!(p2.next_page.as_deref(), Some(format!("{b}/list/3.html").as_str()));

    // Direct link
    let d = analyze::analyze(&format!("{b}/media/alpha.mp4")).await.unwrap();
    assert_eq!(d.kind, "video");
    assert_eq!(d.video.as_ref().unwrap().size, Some(5_000_000));

    // File download with resume from an existing partial file.
    let out = tempdir();
    let original = std::fs::read(root.join("media/alpha.mp4")).unwrap();
    std::fs::write(out.join(".vidra-f1.part"), &original[..1_234_567]).unwrap();
    // The server answers the first request for each media file with 429 + Retry-After.
    // gamma.mp4 is a copy of alpha.mp4 that nothing has requested yet.
    let started = std::time::Instant::now();
    let p = fetch_to(&out, "f1", format!("{b}/media/gamma.mp4"), "file", "Alpha clip").await;
    assert!(started.elapsed().as_millis() >= 900, "should have waited for Retry-After");
    println!("file: {:?}", p);
    assert_eq!(p.status, "done");
    let saved = std::fs::read(p.path.as_ref().unwrap()).unwrap();
    assert_eq!(saved, original, "resumed file must match source");
    assert!(p.path.unwrap().ends_with("Alpha clip.mp4"));

    // Encrypted HLS: master -> best variant -> decrypt -> concat
    let p = fetch_to(&out, "h1", format!("{b}/media/hls/master.m3u8?v=1"), "hls", "Beta 1").await;
    println!("hls: {:?}", p);
    assert_eq!(p.status, "done");
    let got = std::fs::read(p.path.as_ref().unwrap()).unwrap();
    let want = std::fs::read(root.join("media/hls/expected.ts")).unwrap();
    assert_eq!(got.len(), want.len());
    assert_eq!(got, want, "decrypted stream must match plain segments");
    assert!(p.path.unwrap().ends_with("Beta 1.ts"));
}

#[tokio::test]
#[ignore]
async fn e2e_quality_buttons_and_403() {
    let b = base();
    let out = tempdir2();

    // Listing -> 2 card pages, each with 4 "MP4 (…)" buttons (one a duplicate mirror) + a landing link.
    let a = analyze::analyze(&format!("{b}/list2.html")).await.unwrap();
    let page = a.page.unwrap();
    assert_eq!(page.detail_links.len(), 2);
    let vids = analyze::deep_scan("q".into(), page.detail_links.clone(), Some(page.url.clone()), Arc::new(AtomicBool::new(false)), |_| {}).await;
    for v in &vids {
        println!("card video: {:?} | {:?}p | {:?} bytes | {}", v.title, v.quality, v.size, v.url);
    }
    assert_eq!(vids.len(), 2, "exactly one entry per video page");
    for v in &vids {
        let t = v.title.as_deref().unwrap();
        assert!(t.starts_with("We will go to the end of pacific ocean to see.."), "real title, got {t}");
        assert!(!t.contains("MP4"), "format label must not be the title");
        assert_eq!(v.quality, Some(1080), "best quality kept");
    }

    // The scanned link is signed and expires: wait it out, then download.
    tokio::time::sleep(std::time::Duration::from_secs(7)).await;
    let mut req_ref = vids[0].clone();
    let p = {
        let mgr = Arc::new(Manager::new(1));
        let last: Arc<Mutex<Option<Progress>>> = Arc::new(Mutex::new(None));
        let l2 = last.clone();
        download::run(
            mgr,
            DownloadRequest {
                id: "exp".into(),
                url: req_ref.url.clone(),
                kind: "file".into(),
                title: req_ref.title.take(),
                referer: req_ref.page_url.clone(),
                dir: out.to_string_lossy().into_owned(),
            },
            move |p| *l2.lock().unwrap() = Some(p),
        )
        .await;
        let p = last.lock().unwrap().clone().unwrap();
        p
    };
    println!("expired-link download: {:?}", p);
    assert_eq!(p.status, "done", "403 should be recovered by refreshing the link: {:?}", p.error);

    // A "download" link that opens a landing page with the real file inside.
    let p = fetch_to(&out, "land", format!("{b}/getfile.html?f=ocean1.mp4"), "file", "Landing").await;
    println!("landing download: {:?}", p);
    assert_eq!(p.status, "done");
    assert_eq!(p.received, 200_000);
}

fn tempdir2() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vidra-e2e2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn tempdir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vidra-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
