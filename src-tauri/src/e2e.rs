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
    assert_eq!(vids.len(), 2);
    assert!(vids.iter().any(|v| v.kind == "file" && v.title.as_deref() == Some("Alpha 1")));
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
    let p = fetch_to(&out, "f1", format!("{b}/media/alpha.mp4"), "file", "Alpha clip").await;
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

fn tempdir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vidra-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
