//! Darkroom: the Apple Photos section of the sidebar.
//!
//! Lists the Photos library's folders and albums (read on a worker through the Mac helper, so
//! the window never waits on PhotoKit) and links an album into the catalog with one click: a
//! worker fetches the originals into Darkroom's cache while the window adds them in place a few
//! at a time ([`tick`]), carrying Photos' keywords, titles, captions and favorites. Nothing in
//! Photos changes from here.

use std::sync::mpsc;

use darkroom_photos::{Album, AlbumKind, Asset, FetchedFile, Photos};
use egui::Ui;
use serde_json::json;

use crate::LightcraftApp;
use crate::icons::Icon;
use crate::panels::left::row;

#[derive(Default)]
pub struct Panel {
    /// The tree, depth first, once read.
    pub albums: Option<Vec<Album>>,
    /// Why the tree could not be read (the helper is missing, Photos access is denied…).
    pub error: Option<String>,
    /// The read in flight.
    loading: Option<mpsc::Receiver<Result<Vec<Album>, String>>>,
    /// The link in progress, if any.
    pub link: Option<LinkJob>,
    /// Smart albums (Favorites, Hidden…) shown too.
    pub show_smart: bool,
}

impl Panel {
    /// Where the last tree read is kept, so the section shows at once on the next launch
    /// (a fresh read runs behind it).
    fn cache_path() -> Option<std::path::PathBuf> {
        lightcraft_engine::config::config_dir().map(|d| d.join("apple-photos-albums.json"))
    }

    /// Shows the cached tree, if there is one. True when something was shown.
    pub fn load_cached(&mut self) -> bool {
        let Some(path) = Self::cache_path() else { return false };
        let Ok(bytes) = std::fs::read(&path) else { return false };
        match serde_json::from_slice::<Vec<Album>>(&bytes) {
            Ok(a) if !a.is_empty() => {
                self.albums = Some(a);
                true
            }
            _ => false,
        }
    }

    fn save_cache(albums: &[Album]) {
        let Some(path) = Self::cache_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec(albums) {
            let _ = std::fs::write(path, bytes);
        }
    }

    /// Starts reading the tree on a worker; nothing happens while one is already running.
    pub fn refresh(&mut self, photos: Option<Photos>) {
        if self.loading.is_some() {
            return;
        }
        let Some(photos) = photos else {
            self.error = Some(lightcraft_engine::cmd::photos::unavailable());
            self.albums = None;
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.loading = Some(rx);
        self.error = None;
        let _ = std::thread::Builder::new().name("apple-photos-albums".into()).spawn(move || {
            let _ = tx.send(photos.albums().map_err(|e| e.to_string()));
        });
    }

    /// Picks up a finished read.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(Ok(albums)) => {
                    Self::save_cache(&albums);
                    self.albums = Some(albums);
                    self.loading = None;
                }
                Ok(Err(e)) => {
                    self.error = Some(e);
                    self.loading = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("the helper stopped before answering".into());
                    self.loading = None;
                }
            }
        }
    }

    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    /// Rows the section lists at its top level (for the folded header's count).
    pub fn top_level_rows(&self) -> usize {
        self.albums.as_ref().map_or(0, |a| a.iter().filter(|x| x.depth == 0 && x.parent.is_empty()).count())
    }
}

/// What the link worker reports.
enum LinkEvent {
    /// The album's asset rows, in order.
    Listed(Vec<Asset>),
    /// Originals for these asset ids are on disk now (or failed: `path` is None).
    Fetched(Vec<FetchedFile>),
    Failed(String),
    Done,
}

/// Linking an album, kept off the window's thread: a worker lists the album and fetches the
/// originals into the cache in small chunks; between frames the window adds the fetched ones
/// to the catalog in small batches, so it stays usable the whole time.
pub struct LinkJob {
    pub album_id: String,
    pub album_title: String,
    rx: mpsc::Receiver<LinkEvent>,
    assets: Vec<Asset>,
    /// asset id -> fetched (cached or failed)
    fetched: std::collections::HashMap<String, bool>,
    /// how many of `assets` (in order) have been handed to `photos.link`
    added: usize,
    pub listed: bool,
    pub worker_done: bool,
    pub total: usize,
    pub linked: u64,
    pub already: u64,
    pub failed: u64,
    catalog_album: Option<u64>,
    errors: Vec<String>,
}

impl LinkJob {
    const FETCH_CHUNK: usize = 20;
    const ADD_BATCH: usize = 40;

    fn start(photos: Photos, album_id: String, album_title: String) -> Self {
        let (tx, rx) = mpsc::channel();
        let id = album_id.clone();
        let _ = std::thread::Builder::new().name("apple-photos-link".into()).spawn(move || {
            let mut assets: Vec<Asset> = Vec::new();
            let listed = photos.all_assets(Some(&id), false, |page| {
                assets.extend(page.items.iter().cloned());
                true
            });
            if let Err(e) = listed {
                let _ = tx.send(LinkEvent::Failed(e.to_string()));
                return;
            }
            let ids: Vec<String> = assets.iter().filter(|a| a.media_type != "video").map(|a| a.id.clone()).collect();
            if tx.send(LinkEvent::Listed(assets)).is_err() {
                return;
            }
            for chunk in ids.chunks(Self::FETCH_CHUNK) {
                match photos.originals(chunk) {
                    Ok(files) => {
                        if tx.send(LinkEvent::Fetched(files)).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(LinkEvent::Failed(e.to_string()));
                        return;
                    }
                }
            }
            let _ = tx.send(LinkEvent::Done);
        });
        Self {
            album_id,
            album_title,
            rx,
            assets: Vec::new(),
            fetched: Default::default(),
            added: 0,
            listed: false,
            worker_done: false,
            total: 0,
            linked: 0,
            already: 0,
            failed: 0,
            catalog_album: None,
            errors: Vec::new(),
        }
    }

    /// Fetched so far (for the progress line).
    pub fn fetched_count(&self) -> usize {
        self.fetched.len()
    }

    pub fn added_count(&self) -> usize {
        self.added
    }

    /// Takes in the worker's events; false once the worker has stopped (done or failed).
    fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(LinkEvent::Listed(a)) => {
                    self.total = a.iter().filter(|x| x.media_type != "video").count();
                    self.assets = a;
                    self.listed = true;
                }
                Ok(LinkEvent::Fetched(files)) => {
                    for f in files {
                        self.fetched.insert(f.id, f.path.is_some());
                    }
                }
                Ok(LinkEvent::Failed(e)) => {
                    self.errors.push(e);
                    self.worker_done = true;
                }
                Ok(LinkEvent::Done) => self.worker_done = true,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.worker_done = true;
                    break;
                }
            }
        }
        !self.worker_done
    }

    /// The next batch of assets whose originals are on disk, in album order; None when the
    /// next one is not fetched yet.
    fn next_batch(&mut self) -> Option<Vec<Asset>> {
        let mut out = Vec::new();
        while self.added < self.assets.len() && out.len() < Self::ADD_BATCH {
            let a = &self.assets[self.added];
            if a.media_type == "video" {
                self.added += 1;
                continue;
            }
            match self.fetched.get(&a.id) {
                Some(true) => {
                    out.push(a.clone());
                    self.added += 1;
                }
                Some(false) => {
                    self.failed += 1;
                    self.added += 1;
                }
                None if self.worker_done => {
                    // the worker stopped before reaching this one
                    self.failed += 1;
                    self.added += 1;
                }
                None => break,
            }
        }
        if out.is_empty() { None } else { Some(out) }
    }

    pub fn finished(&self) -> bool {
        self.worker_done && self.added >= self.assets.len()
    }
}

/// Runs between frames: moves a link along by one batch. Call it every frame while a link is
/// in progress.
pub fn tick(app: &mut LightcraftApp, ctx: &egui::Context) {
    let Some(mut job) = app.apple_photos.link.take() else { return };
    job.pump();
    if let Some(batch) = job.next_batch() {
        let r = app.run(
            "photos.link",
            json!({
                "assets": batch,
                "albumName": job.album_title,
                "catalogAlbum": job.catalog_album,
            }),
        );
        match r {
            Ok(v) => {
                job.linked += v["linked"].as_u64().unwrap_or(0);
                job.already += v["already"].as_u64().unwrap_or(0);
                job.failed += v["failed"].as_array().map_or(0, |f| f.len() as u64);
                if let Some(a) = v["album"].as_u64() {
                    if job.catalog_album.is_none() {
                        // the first batch made the album: show it
                        let _ = app.run("library.source", json!({"kind": "album", "id": a}));
                    }
                    job.catalog_album = Some(a);
                }
            }
            Err(e) => job.errors.push(e),
        }
    }
    if job.finished() {
        let mut msg = format!("Linked {} photo{} from \u{201c}{}\u{201d}", job.linked, if job.linked == 1 { "" } else { "s" }, job.album_title);
        if job.already > 0 {
            msg.push_str(&format!(" ({} already linked)", job.already));
        }
        if job.failed > 0 {
            msg.push_str(&format!(", {} could not be fetched", job.failed));
        }
        if job.errors.is_empty() {
            app.toast(ctx, msg);
        } else {
            app.toast_error(ctx, format!("{msg}. {}", job.errors.join("; ")));
        }
        if let Some(a) = job.catalog_album {
            let _ = app.run("library.source", json!({"kind": "album", "id": a}));
        }
    } else {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
        app.apple_photos.link = Some(job);
    }
}

/// The section's rows.
pub fn rows(app: &mut LightcraftApp, ui: &mut Ui) {
    // first look: the cached tree at once, then a fresh read behind it
    if app.apple_photos.albums.is_none() && app.apple_photos.error.is_none() && !app.apple_photos.is_loading() {
        app.apple_photos.load_cached();
        let photos = app.session.photos.clone();
        app.apple_photos.refresh(photos);
    }
    app.apple_photos.poll();
    if app.apple_photos.is_loading() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
    if let Some(e) = app.apple_photos.error.clone() {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(18.0);
            ui.small(e);
        });
        if row(app, ui, "applePhotos:retry", Icon::Photos, crate::i18n::tr("Try again"), None, false, 0.0).clicked() {
            app.apple_photos.error = None;
            let photos = app.session.photos.clone();
            app.apple_photos.refresh(photos);
        }
        return;
    }
    let Some(albums) = app.apple_photos.albums.clone() else {
        if app.apple_photos.is_loading() {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.spinner();
                ui.small(crate::i18n::tr("Reading your Photos library…"));
            });
        }
        return;
    };
    if app.apple_photos.is_loading() {
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.spinner();
            ui.small(crate::i18n::tr("Refreshing from Photos…"));
        });
    }
    let show_smart = app.apple_photos.show_smart;
    let linking = app.apple_photos.link.as_ref().map(|j| j.album_id.clone());
    for a in albums.iter().filter(|a| a.parent != "smart") {
        album_row(app, ui, a, linking.as_deref());
    }
    let smart: Vec<&Album> = albums.iter().filter(|a| a.parent == "smart").collect();
    if !smart.is_empty() {
        let label = if show_smart { crate::i18n::tr("Hide Photos' smart albums") } else { crate::i18n::tr("Show Photos' smart albums") };
        if row(app, ui, "applePhotos:smart", Icon::SmartAlbum, label, Some(smart.len()), false, 0.0).clicked() {
            app.apple_photos.show_smart = !show_smart;
        }
        if show_smart {
            for a in smart {
                album_row(app, ui, a, linking.as_deref());
            }
        }
    }
    if row(app, ui, "applePhotos:refresh", Icon::Photos, crate::i18n::tr("Refresh from Photos"), None, false, 0.0).clicked() {
        let photos = app.session.photos.clone();
        app.apple_photos.refresh(photos);
    }
}

fn album_row(app: &mut LightcraftApp, ui: &mut Ui, a: &Album, linking: Option<&str>) {
    let indent = 16.0 * a.depth as f32 + if a.parent == "smart" { 16.0 } else { 0.0 };
    let icon = match a.kind {
        AlbumKind::Folder => Icon::Folder,
        AlbumKind::Smart => Icon::SmartAlbum,
        AlbumKind::Album | AlbumKind::Other => Icon::Album,
    };
    let count = if a.count >= 0 { Some(a.count as usize) } else { None };
    let busy = linking == Some(a.id.as_str());
    let label = if busy {
        match &app.apple_photos.link {
            Some(j) if !j.listed => format!("{} (reading…)", a.title),
            Some(j) => format!("{} ({} of {} fetched, {} added)", a.title, j.fetched_count(), j.total, j.added_count()),
            None => a.title.clone(),
        }
    } else {
        a.title.clone()
    };
    let id = format!("applePhotos:{}", a.id);
    let r = row(app, ui, &id, icon, &label, count, busy, indent);
    if a.kind == AlbumKind::Folder {
        return;
    }
    let r = r.on_hover_text(crate::i18n::tr("Click to link this album into the catalog (originals are fetched once; nothing in Photos changes)"));
    if r.clicked() && linking.is_none() {
        link_album(app, ui.ctx(), a);
    }
}

/// Starts linking an album (see [`LinkJob`]); the window keeps drawing while it runs.
fn link_album(app: &mut LightcraftApp, ctx: &egui::Context, a: &Album) {
    let Some(photos) = app.session.photos.clone() else {
        app.toast_error(ctx, lightcraft_engine::cmd::photos::unavailable());
        return;
    };
    app.apple_photos.link = Some(LinkJob::start(photos, a.id.clone(), a.title.clone()));
    ctx.request_repaint();
}
