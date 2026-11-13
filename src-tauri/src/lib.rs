mod analyze;
mod download;
mod net;
mod scan;
#[cfg(test)]
mod e2e;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use analyze::{Analysis, DeepProgress, Probe};
use download::DownloadRequest;
use scan::{DetailLink, PageScan, VideoItem};

struct AppState {
    downloads: Arc<download::Manager>,
    scans: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

/// Paste anything: returns a single video, or a scanned page.
#[tauri::command]
async fn analyze_link(url: String) -> Result<Analysis, String> {
    analyze::analyze(&url).await
}

/// Scan one page (used for next / previous / numbered pages).
#[tauri::command]
async fn scan_page(url: String, referer: Option<String>) -> Result<PageScan, String> {
    analyze::scan_page(&url, referer.as_deref()).await
}

/// Size and type of a media link.
#[tauri::command]
async fn probe_media(url: String, referer: Option<String>) -> Result<Probe, String> {
    let u = net::normalize_input(&url)?;
    Ok(analyze::probe(&u, referer.as_deref()).await)
}

/// Visit each card link on a listing page and collect its videos.
/// Streams `deep-scan` events while it runs.
#[tauri::command]
async fn deep_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: String,
    links: Vec<DetailLink>,
    referer: Option<String>,
) -> Result<Vec<VideoItem>, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    state.scans.lock().unwrap().insert(scan_id.clone(), cancel.clone());
    let emitter = app.clone();
    let out = analyze::deep_scan(scan_id.clone(), links, referer, cancel, move |p: DeepProgress| {
        let _ = emitter.emit("deep-scan", p);
    })
    .await;
    state.scans.lock().unwrap().remove(&scan_id);
    Ok(out)
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>, scan_id: String) {
    if let Some(c) = state.scans.lock().unwrap().get(&scan_id) {
        c.store(true, Ordering::Relaxed);
    }
}

/// Starts (or resumes) a download. Progress arrives as `download` events.
#[tauri::command]
fn start_download(app: AppHandle, state: State<'_, AppState>, request: DownloadRequest) {
    let mgr = state.downloads.clone();
    let emitter = app.clone();
    tauri::async_runtime::spawn(async move {
        download::run(mgr, request, move |p| {
            let _ = emitter.emit("download", p);
        })
        .await;
    });
}

#[tauri::command]
fn pause_download(state: State<'_, AppState>, id: String) -> bool {
    state.downloads.pause(&id)
}

#[tauri::command]
fn cancel_download(state: State<'_, AppState>, id: String) -> bool {
    state.downloads.cancel(&id)
}

/// Removes a leftover partial file for a download that isn't running.
#[tauri::command]
async fn discard_partial(dir: String, id: String) {
    let d = PathBuf::from(dir);
    let _ = tokio::fs::remove_file(d.join(format!(".vidra-{id}.part"))).await;
    let _ = tokio::fs::remove_file(d.join(format!(".vidra-{id}.hls"))).await;
}

#[tauri::command]
fn default_download_dir(app: AppHandle) -> String {
    #[cfg(target_os = "android")]
    {
        // Public Download folder so files show up in the gallery and file manager.
        let public = PathBuf::from("/storage/emulated/0/Download/Vidra");
        if std::fs::create_dir_all(&public).is_ok() && std::fs::write(public.join(".probe"), b"").is_ok() {
            let _ = std::fs::remove_file(public.join(".probe"));
            return public.to_string_lossy().into_owned();
        }
        if let Ok(d) = app.path().document_dir().or_else(|_| app.path().app_data_dir()) {
            return d.join("Vidra").to_string_lossy().into_owned();
        }
    }
    let base = app
        .path()
        .download_dir()
        .or_else(|_| app.path().video_dir())
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("Vidra").to_string_lossy().into_owned()
}

#[tauri::command]
fn open_file(app: AppHandle, path: String) -> Result<(), String> {
    app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal_file(app: AppHandle, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if p.is_dir() {
        let _ = std::fs::create_dir_all(&p);
        return app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string());
    }
    app.opener().reveal_item_in_dir(p).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Android: a link shared into the app from another app (see MainActivity.kt).
/// Returns it once, then forgets it. Always `None` on desktop.
#[tauri::command]
fn take_shared_link(app: AppHandle) -> Option<String> {
    #[cfg(target_os = "android")]
    {
        let id = app.config().identifier.clone();
        let candidates = [
            format!("/data/data/{id}/files/shared_link.txt"),
            format!("/data/user/0/{id}/files/shared_link.txt"),
        ];
        for c in candidates {
            if let Ok(text) = std::fs::read_to_string(&c) {
                let _ = std::fs::remove_file(&c);
                let t = text.trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    let _ = app;
    None
}

#[derive(serde::Serialize)]
struct PlatformInfo {
    mobile: bool,
    os: &'static str,
}

#[tauri::command]
fn platform_info() -> PlatformInfo {
    PlatformInfo { mobile: cfg!(mobile), os: std::env::consts::OS }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState { downloads: Arc::new(download::Manager::new(2)), scans: Mutex::new(HashMap::new()) })
        .invoke_handler(tauri::generate_handler![
            analyze_link,
            scan_page,
            probe_media,
            deep_scan,
            cancel_scan,
            start_download,
            pause_download,
            cancel_download,
            discard_partial,
            default_download_dir,
            open_file,
            reveal_file,
            open_url,
            platform_info,
            take_shared_link,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vidra");
}
