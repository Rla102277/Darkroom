//! Darkroom: Apple Photos commands (`photos.*`), through the Mac helper (`darkroom-photos`).
//!
//! Apple Photos stays the system of record. These commands read it (albums, assets), *link*
//! photos into the working catalog (the original is fetched into Darkroom's cache and added in
//! place, so nothing is copied twice and nothing inside the Photos library is touched), and
//! write decisions back (favorites, hidden, keywords, albums). What is linked is remembered in
//! `apple-photos.json` in the library folder, keyed by Apple's cloud identifier, which is the
//! same on every Mac: a photo linked on the MacBook Pro is known on the Air.
//!
//! Nothing here deletes anything in Photos.

use std::collections::HashMap;

use lightcraft_catalog::PhotoId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, cmd, str_param};
use crate::{Result, Session};

/// One linked photo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// The catalog photo.
    pub photo: u64,
    /// PhotoKit's local identifier on the Mac that linked it (differs between Macs).
    #[serde(default)]
    pub local_id: String,
    /// The fetched original's path.
    #[serde(default)]
    pub path: String,
    /// What Photos knew when it was linked.
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub hidden: bool,
}

/// The link table, keyed by the stable id ([`darkroom_photos::Asset::stable_id`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Links {
    pub by_id: HashMap<String, Link>,
}

impl Links {
    pub const FILE: &'static str = "apple-photos.json";

    /// The stable id a catalog photo was linked from, if any.
    pub fn id_of(&self, photo: PhotoId) -> Option<&str> {
        self.by_id.iter().find(|(_, l)| l.photo == photo.0).map(|(k, _)| k.as_str())
    }
}

/// The keyword Darkroom puts on every linked photo, and the family for the Photos album it came
/// from.
pub const LINK_KEYWORD: &str = "Apple Photos";

fn photos_enabled(s: &Session) -> std::result::Result<(), String> {
    match &s.photos {
        Some(_) => Ok(()),
        None => Err(unavailable()),
    }
}

pub fn unavailable() -> String {
    if cfg!(target_os = "macos") {
        darkroom_photos::Error::NoHelper.to_string()
    } else {
        darkroom_photos::Error::NotMac.to_string()
    }
}

fn client(s: &Session, cmd: &str) -> Result<darkroom_photos::Photos> {
    s.photos.clone().ok_or_else(|| bad(cmd, unavailable()))
}

fn strs(p: &Value, key: &str) -> Vec<String> {
    p.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

/// Saves the link table into the library folder (no library: kept in memory only).
pub fn save_links(s: &mut Session) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(&s.photos_links).unwrap_or_default();
    s.write_library_file(Links::FILE, &bytes)
}

/// `photos.link`: fetch originals and add them in place, with Photos' keywords, title, caption
/// and favorite state, into a catalog album named after the Photos album.
fn link(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "photos.link";
    let photos = client(s, C)?;
    let album = str_param(p, "album").map(str::to_string).filter(|a| !a.is_empty());
    let ids = strs(p, "ids");
    let with_meta = p.get("withKeywords").and_then(Value::as_bool).unwrap_or(true);
    let include_hidden = p.get("hidden").and_then(Value::as_bool).unwrap_or(false);
    if album.is_none() && ids.is_empty() {
        return Err(bad(C, "album (a Photos album id) or ids (asset ids) required"));
    }
    // the assets: named, or everything in the album
    let mut assets: Vec<darkroom_photos::Asset> = Vec::new();
    let mut album_title: Option<String> = str_param(p, "albumName").map(str::to_string).filter(|n| !n.is_empty());
    if ids.is_empty() {
        let album_id = album.as_deref().unwrap_or("");
        if album_title.is_none() {
            album_title = photos
                .albums()
                .map_err(|e| bad(C, e.to_string()))?
                .into_iter()
                .find(|a| a.id == album_id || a.cloud_id == album_id)
                .map(|a| a.title);
        }
        photos
            .all_assets(Some(album_id), include_hidden, |page| {
                assets.extend(page.items.iter().cloned());
                true
            })
            .map_err(|e| bad(C, e.to_string()))?;
    } else {
        // named ids: one page per 2000 is not available by id, so rows come from `originals`
        // (identity) and keywords; favorite/hidden come from `assets` of the whole library only
        // when asked, which is slow: skip them here.
        assets = ids.iter().map(|id| darkroom_photos::Asset { id: id.clone(), ..Default::default() }).collect();
    }
    // skip what is already linked and still in the catalog
    let already: usize = assets
        .iter()
        .filter(|a| s.photos_links.by_id.get(a.stable_id()).is_some_and(|l| s.catalog.photo(PhotoId(l.photo)).is_some_and(|ph| !ph.deleted)))
        .count();
    let wanted: Vec<String> = assets
        .iter()
        .filter(|a| a.media_type != "video")
        .filter(|a| !s.photos_links.by_id.get(a.stable_id()).is_some_and(|l| s.catalog.photo(PhotoId(l.photo)).is_some_and(|ph| !ph.deleted)))
        .map(|a| a.id.clone())
        .collect();
    let skipped_video = assets.iter().filter(|a| a.media_type == "video").count();
    if wanted.is_empty() {
        return Ok(json!({"linked": 0, "already": already, "skippedVideo": skipped_video, "failed": []}));
    }
    // originals into the cache, in batches so a stall is visible
    let mut by_path: HashMap<String, darkroom_photos::FetchedFile> = HashMap::new();
    let mut failed: Vec<Value> = Vec::new();
    for chunk in wanted.chunks(200) {
        let fetched = photos.originals(chunk).map_err(|e| bad(C, e.to_string()))?;
        for f in fetched {
            match (&f.path, &f.error) {
                (Some(path), _) => {
                    by_path.insert(path.clone(), f);
                }
                (None, err) => failed.push(json!({"id": f.id, "error": err.clone().unwrap_or_else(|| "no original".into())})),
            }
        }
    }
    let paths: Vec<String> = by_path.keys().cloned().collect();
    if paths.is_empty() {
        return Ok(json!({"linked": 0, "already": already, "skippedVideo": skipped_video, "failed": failed}));
    }
    // add in place
    let opts = crate::import::ImportOptions { mode: crate::import::ImportMode::Add, keywords: vec![LINK_KEYWORD.into()], ..Default::default() };
    let undo0 = s.undo.len();
    let report = crate::import::import_with(s, &paths, &opts)?;
    let album_name = album_title.map(|t| format!("Photos: {t}"));
    let report = super::library::import_batch_done(s, report, None, album_name.as_deref())?;
    let ids_of = |k: &str| -> Vec<u64> { report[k].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default() };
    let mut linked: Vec<u64> = ids_of("imported");
    linked.extend(ids_of("restored"));
    // duplicates of files already in the catalog count as linked too (same original, found by content)
    let mut dup_linked = 0;
    if let Some(d) = report["duplicates"].as_array() {
        for dup in d {
            if let (Some(path), Some(existing)) = (dup["path"].as_str(), dup["existing"].as_u64())
                && by_path.contains_key(path)
            {
                linked.push(existing);
                dup_linked += 1;
            }
        }
    }
    // remember the links
    let by_asset: HashMap<&str, &darkroom_photos::Asset> = assets.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut linked_ids: Vec<(u64, String)> = Vec::new();
    for id in &linked {
        let Some(ph) = s.catalog.photo(PhotoId(*id)) else { continue };
        let lightcraft_catalog::Source::File { path } = &ph.source else { continue };
        let Some(f) = by_path.get(path.as_str()) else { continue };
        let a = by_asset.get(f.id.as_str());
        let stable = if f.cloud_id.is_empty() { f.id.clone() } else { f.cloud_id.clone() };
        s.photos_links.by_id.insert(
            stable.clone(),
            Link {
                photo: *id,
                local_id: f.id.clone(),
                path: path.clone(),
                favorite: a.is_some_and(|a| a.favorite),
                hidden: a.is_some_and(|a| a.hidden),
            },
        );
        linked_ids.push((*id, f.id.clone()));
    }
    let saved = save_links(s).err().map(|e| e.to_string());
    // Photos' own metadata onto the catalog records
    let mut meta_set = 0;
    if with_meta && !linked_ids.is_empty() {
        let local_ids: Vec<String> = linked_ids.iter().map(|(_, l)| l.clone()).collect();
        let photo_of: HashMap<&str, u64> = linked_ids.iter().map(|(p, l)| (l.as_str(), *p)).collect();
        for chunk in local_ids.chunks(500) {
            let rows = match photos.keywords(chunk) {
                Ok(r) => r,
                Err(e) => {
                    failed.push(json!({"error": format!("keywords: {e}")}));
                    break;
                }
            };
            for row in rows {
                let Some(pid) = photo_of.get(row.id.as_str()) else { continue };
                let mut params = json!({"ids": [pid]});
                if !row.keywords.is_empty() {
                    params["addKeywords"] = json!(row.keywords);
                }
                if !row.title.is_empty() {
                    params["title"] = json!(row.title);
                }
                if !row.description.is_empty() {
                    params["caption"] = json!(row.description);
                }
                if params.as_object().is_some_and(|o| o.len() > 1) {
                    s.execute("photo.setMeta", &params)?;
                    meta_set += 1;
                }
            }
        }
        // favorites become picks
        let favs: Vec<u64> = linked_ids.iter().filter(|(_, l)| by_asset.get(l.as_str()).is_some_and(|a| a.favorite)).map(|(p, _)| *p).collect();
        if !favs.is_empty() {
            s.execute("photo.flag", &json!({"flag": "pick", "ids": favs}))?;
        }
    }
    let n = s.undo.len().saturating_sub(undo0);
    s.merge_undo(n, &format!("Link {} Photo{} from Apple Photos", linked.len(), if linked.len() == 1 { "" } else { "s" }));
    Ok(json!({
        "linked": linked.len(),
        "fresh": ids_of("imported").len(),
        "alreadyInCatalog": dup_linked,
        "already": already,
        "skippedVideo": skipped_video,
        "metadataSet": meta_set,
        "failed": failed,
        "album": report["album"],
        "linksSaved": saved.is_none(),
        "linksError": saved,
    }))
}

pub fn specs() -> Vec<CommandSpec> {
    let mut v = vec![
        cmd!(query "photos.status", "Apple Photos Status", [], None,
            "{} → {available, helper?, authorized?, status?, libraryPath?, assets?, albums?, linked} — whether the Mac helper is there and Photos access is granted",
            always, |s, _| {
                let linked = s.photos_links.by_id.len();
                let Some(photos) = s.photos.clone() else {
                    return Ok(json!({"available": false, "reason": unavailable(), "linked": linked}));
                };
                let helper = darkroom_photos::locate_helper().map(|p| p.to_string_lossy().to_string());
                match photos.status() {
                    Ok(st) => {
                        let mut v = serde_json::to_value(st).unwrap_or_default();
                        v["available"] = json!(true);
                        v["helper"] = json!(helper);
                        v["linked"] = json!(linked);
                        Ok(v)
                    }
                    Err(e) => Ok(json!({"available": true, "helper": helper, "authorized": false, "reason": e.to_string(), "linked": linked})),
                }
            }),
        cmd!(query "photos.albums", "Apple Photos Albums", [], None,
            "{} → {items: [{id, cloudId, title, kind: folder|album|smart, depth, parent, count}]} — the folder and album tree, depth first; Photos' own smart albums have parent \"smart\"",
            photos_enabled, |s, _| {
                let photos = client(s, "photos.albums")?;
                let items = photos.albums().map_err(|e| bad("photos.albums", e.to_string()))?;
                Ok(json!({"items": items}))
            }),
        cmd!(query "photos.assets", "Apple Photos Assets", [], None,
            "{album?: albumId, offset?: 0, limit?: 2000, hidden?: false} → {items: [{id, cloudId, filename, width, height, created, favorite, hidden, burst, live, raw, edited, mediaType, linked?: photoId}], total, offset} — a page of the library (or an album), oldest first",
            photos_enabled, |s, p| {
                let photos = client(s, "photos.assets")?;
                let album = str_param(p, "album").filter(|a| !a.is_empty());
                let offset = p.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
                let limit = p.get("limit").and_then(Value::as_u64).unwrap_or(2000).clamp(1, 20000) as usize;
                let hidden = p.get("hidden").and_then(Value::as_bool).unwrap_or(false);
                let page = photos.assets(album, offset, limit, hidden).map_err(|e| bad("photos.assets", e.to_string()))?;
                let items: Vec<Value> = page.items.iter().map(|a| {
                    let mut v = serde_json::to_value(a).unwrap_or_default();
                    if let Some(l) = s.photos_links.by_id.get(a.stable_id()) {
                        v["linked"] = json!(l.photo);
                    }
                    v
                }).collect();
                Ok(json!({"items": items, "total": page.total, "offset": page.offset}))
            }),
        cmd!(query "photos.links", "Apple Photos Links", [], None,
            "{} → {items: [{id (cloud id), photo, localId, path}]} — which catalog photos came from Apple Photos",
            always, |s, _| {
                let items: Vec<Value> = s.photos_links.by_id.iter().map(|(k, l)| json!({"id": k, "photo": l.photo, "localId": l.local_id, "path": l.path})).collect();
                Ok(json!({"items": items}))
            }),
        cmd!(query "photos.link", "Link from Apple Photos", ["File"], None,
            "{album?: Photos album id, ids?: [asset ids], albumName?: catalog album name (default \"Photos: <album title>\"), withKeywords?: true, hidden?: false} → {linked, fresh, alreadyInCatalog, already, skippedVideo, metadataSet, failed: [{id?, error}], album?: catalog album id} — fetches the originals into Darkroom's cache and adds them in place; Photos' keywords, title and caption are copied and favorites become picks. Nothing in Photos changes.",
            photos_enabled, link),
        cmd!(query "photos.set", "Set in Apple Photos", [], None,
            "{ids?: catalog photo ids (default: the selection), favorite?: bool, hidden?: bool} → {changed, requested, notLinked} — writes favorite / hidden state back to Photos for linked photos",
            photos_enabled, |s, p| {
                const C: &str = "photos.set";
                let photos = client(s, C)?;
                let favorite = p.get("favorite").and_then(Value::as_bool);
                let hidden = p.get("hidden").and_then(Value::as_bool);
                if favorite.is_none() && hidden.is_none() {
                    return Err(bad(C, "favorite or hidden required"));
                }
                let targets = s.targets(p);
                let mut ids = Vec::new();
                let mut not_linked = 0;
                for t in &targets {
                    match s.photos_links.id_of(*t) {
                        Some(id) => ids.push(id.to_string()),
                        None => not_linked += 1,
                    }
                }
                if ids.is_empty() {
                    return Ok(json!({"changed": 0, "requested": 0, "notLinked": not_linked}));
                }
                let changed = photos.set(&ids, favorite, hidden).map_err(|e| bad(C, e.to_string()))?;
                for id in &ids {
                    if let Some(l) = s.photos_links.by_id.get_mut(id) {
                        if let Some(f) = favorite { l.favorite = f; }
                        if let Some(h) = hidden { l.hidden = h; }
                    }
                }
                let _ = save_links(s);
                Ok(json!({"changed": changed, "requested": ids.len(), "notLinked": not_linked}))
            }),
        cmd!(query "photos.setKeywords", "Set Keywords in Apple Photos", [], None,
            "{ids?: catalog photo ids (default: the selection), add?: [..], remove?: [..], title?, caption?} → {changed, failed: [..], notLinked} — keywords, title and caption written to Photos for linked photos",
            photos_enabled, |s, p| {
                const C: &str = "photos.setKeywords";
                let photos = client(s, C)?;
                let add = strs(p, "add");
                let remove = strs(p, "remove");
                let title = str_param(p, "title").map(str::to_string);
                let caption = str_param(p, "caption").map(str::to_string);
                if add.is_empty() && remove.is_empty() && title.is_none() && caption.is_none() {
                    return Err(bad(C, "add, remove, title or caption required"));
                }
                let targets = s.targets(p);
                let mut changes = Vec::new();
                let mut not_linked = 0;
                for t in &targets {
                    match s.photos_links.id_of(*t) {
                        Some(id) => changes.push(darkroom_photos::KeywordChange { id: id.to_string(), add: add.clone(), remove: remove.clone(), title: title.clone(), description: caption.clone() }),
                        None => not_linked += 1,
                    }
                }
                if changes.is_empty() {
                    return Ok(json!({"changed": 0, "failed": [], "notLinked": not_linked}));
                }
                let mut v = photos.set_keywords(&changes).map_err(|e| bad(C, e.to_string()))?;
                v["notLinked"] = json!(not_linked);
                Ok(v)
            }),
        cmd!(query "photos.createAlbum", "Create Apple Photos Album", [], None,
            "{title, folder?: Photos folder id} → {id}",
            photos_enabled, |s, p| {
                const C: &str = "photos.createAlbum";
                let photos = client(s, C)?;
                let title = str_param(p, "title").filter(|t| !t.trim().is_empty()).ok_or_else(|| bad(C, "title required"))?;
                let id = photos.create_album(title, str_param(p, "folder").filter(|f| !f.is_empty())).map_err(|e| bad(C, e.to_string()))?;
                Ok(json!({"id": id}))
            }),
        cmd!(query "photos.createFolder", "Create Apple Photos Folder", [], None,
            "{title, parent?: Photos folder id} → {id}",
            photos_enabled, |s, p| {
                const C: &str = "photos.createFolder";
                let photos = client(s, C)?;
                let title = str_param(p, "title").filter(|t| !t.trim().is_empty()).ok_or_else(|| bad(C, "title required"))?;
                let id = photos.create_folder(title, str_param(p, "parent").filter(|f| !f.is_empty())).map_err(|e| bad(C, e.to_string()))?;
                Ok(json!({"id": id}))
            }),
        cmd!(query "photos.addToAlbum", "Add to Apple Photos Album", [], None,
            "{album: Photos album id, ids?: catalog photo ids (default: the selection)} → {added, requested, notLinked} — the helper counts what is really in the album afterwards, because Photos can skip silently",
            photos_enabled, |s, p| {
                const C: &str = "photos.addToAlbum";
                let photos = client(s, C)?;
                let album = str_param(p, "album").filter(|a| !a.is_empty()).ok_or_else(|| bad(C, "album required"))?;
                let targets = s.targets(p);
                let mut ids = Vec::new();
                let mut not_linked = 0;
                for t in &targets {
                    match s.photos_links.id_of(*t) {
                        Some(id) => ids.push(id.to_string()),
                        None => not_linked += 1,
                    }
                }
                if ids.is_empty() {
                    return Ok(json!({"added": 0, "requested": 0, "notLinked": not_linked}));
                }
                let (added, requested) = photos.add_to_album(album, &ids).map_err(|e| bad(C, e.to_string()))?;
                Ok(json!({"added": added, "requested": requested, "notLinked": not_linked}))
            }),
    ];
    v.retain(|c| !c.id.is_empty());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_fake(answer: impl Fn(&str, &Value) -> darkroom_photos::Result<Value> + Send + Sync + 'static) -> Session {
        let mut s = Session::new();
        s.photos = Some(darkroom_photos::Photos::with_runner(darkroom_photos::fake_runner(answer)));
        s
    }

    #[test]
    fn status_without_helper_says_why() {
        let mut s = Session::new();
        s.photos = None;
        let v = s.execute("photos.status", &Value::Null).unwrap();
        assert_eq!(v["available"], false);
        assert!(s.execute("photos.albums", &Value::Null).is_err());
    }

    #[test]
    fn albums_and_assets_pass_through_with_link_marks() {
        let mut s = session_with_fake(|cmd, p| match cmd {
            "albums" => Ok(json!({"items": [{"id": "A/L0/040", "cloudId": "ca", "title": "Iceland", "kind": "album", "depth": 0, "parent": "", "count": 2}]})),
            "assets" => {
                assert_eq!(p["album"], "A/L0/040");
                Ok(json!({"items": [{"id": "X/L0/001", "cloudId": "cx"}, {"id": "Y/L0/001", "cloudId": "cy"}], "total": 2}))
            }
            _ => Err(darkroom_photos::Error::Helper("unexpected".into())),
        });
        s.photos_links.by_id.insert("cy".into(), Link { photo: 7, local_id: "Y/L0/001".into(), path: "".into(), favorite: false, hidden: false });
        let a = s.execute("photos.albums", &Value::Null).unwrap();
        assert_eq!(a["items"][0]["title"], "Iceland");
        let v = s.execute("photos.assets", &json!({"album": "A/L0/040"})).unwrap();
        assert_eq!(v["total"], 2);
        assert!(v["items"][0].get("linked").is_none());
        assert_eq!(v["items"][1]["linked"], 7);
    }

    #[test]
    fn set_only_reaches_linked_photos() {
        let mut s = session_with_fake(|cmd, p| match cmd {
            "set" => {
                assert_eq!(p["ids"], json!(["cx"]));
                assert_eq!(p["favorite"], true);
                Ok(json!({"changed": 1, "requested": 1}))
            }
            _ => Err(darkroom_photos::Error::Helper("unexpected".into())),
        });
        s.photos_links.by_id.insert("cx".into(), Link { photo: 1, local_id: "X/L0/001".into(), path: "".into(), favorite: false, hidden: false });
        let v = s.execute("photos.set", &json!({"ids": [1, 2], "favorite": true})).unwrap();
        assert_eq!(v["changed"], 1);
        assert_eq!(v["notLinked"], 1);
        assert!(s.photos_links.by_id["cx"].favorite);
    }
}
