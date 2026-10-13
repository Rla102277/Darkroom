//! Darkroom: the Apple Photos section of the sidebar.
//!
//! Lists the Photos library's folders and albums (read on a worker through the Mac helper, so
//! the window never waits on PhotoKit) and links an album into the catalog with one click:
//! `photos.link` fetches the originals into Darkroom's cache and adds them in place, carrying
//! Photos' keywords, titles, captions and favorites. Nothing in Photos changes from here.

use std::sync::mpsc;

use darkroom_photos::{Album, AlbumKind, Photos};
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
    /// The album a link is running for (the window shows it as busy).
    pub linking: Option<String>,
    /// Smart albums (Favorites, Hidden…) shown too.
    pub show_smart: bool,
}

impl Panel {
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

/// The section's rows.
pub fn rows(app: &mut LightcraftApp, ui: &mut Ui) {
    // first look: read the tree
    if app.apple_photos.albums.is_none() && app.apple_photos.error.is_none() && !app.apple_photos.is_loading() {
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
    let show_smart = app.apple_photos.show_smart;
    let linking = app.apple_photos.linking.clone();
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
        app.apple_photos.albums = None;
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
    let label = if busy { format!("{} (linking…)", a.title) } else { a.title.clone() };
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

/// Links an album: runs `photos.link` and reports. The command runs on this thread, so the window
/// is busy meanwhile (a big album takes a while the first time; the originals are cached after).
fn link_album(app: &mut LightcraftApp, ctx: &egui::Context, a: &Album) {
    app.apple_photos.linking = Some(a.id.clone());
    ctx.request_repaint();
    let result = app.run("photos.link", json!({"album": a.id, "albumName": a.title}));
    app.apple_photos.linking = None;
    match result {
        Ok(v) => {
            let linked = v["linked"].as_u64().unwrap_or(0);
            let already = v["already"].as_u64().unwrap_or(0);
            let failed = v["failed"].as_array().map_or(0, Vec::len);
            let mut msg = format!("Linked {linked} photo{} from “{}”", if linked == 1 { "" } else { "s" }, a.title);
            if already > 0 {
                msg.push_str(&format!(" ({already} already linked)"));
            }
            if failed > 0 {
                msg.push_str(&format!(", {failed} could not be fetched"));
            }
            app.toast(ctx, msg);
            if let Some(album) = v["album"].as_u64() {
                let _ = app.run("library.source", json!({"kind": "album", "id": album}));
            }
        }
        Err(e) => app.toast_error(ctx, e),
    }
}
