//! Apple Photos for Darkroom, through the Mac helper.
//!
//! Rust cannot call PhotoKit, so everything that touches the Photos library goes through one
//! small Swift program, `helper/darkroom-helper`. This crate is its client: it finds the binary,
//! runs one command per process (`darkroom-helper <command> <json>`), and turns the JSON that
//! comes back into typed rows. The process runner is injectable ([`Runner`]) so the engine's
//! tests never start the helper.
//!
//! Identity: every row carries Apple's *cloud identifier* ([`Asset::cloud_id`]), the same for
//! the same picture on every Mac on the same iCloud account. PhotoKit's local identifier differs
//! between Macs, so Darkroom stores the cloud id and passes either kind back to the helper.
//!
//! Nothing here deletes anything. The write commands favorite, hide, keyword and file photos
//! into albums and folders; deleting stays something done in Photos by hand.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Runs the helper: arguments (command, then the JSON parameters) and a timeout; returns stdout,
/// or an error. The real one spawns the binary; tests use closures.
pub type Runner = Arc<dyn Fn(&[String], Duration) -> std::result::Result<String, Error> + Send + Sync>;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("Darkroom's Mac helper was not found; build it with `swift build -c release` in helper/darkroom-helper")]
    NoHelper,
    #[error("the helper could not be started: {0}")]
    Spawn(String),
    #[error("the helper did not answer within {0:?}")]
    Timeout(Duration),
    #[error("{0}")]
    Helper(String),
    #[error("the helper's answer was not understood: {0}")]
    Protocol(String),
    #[error("Apple Photos is only available on macOS")]
    NotMac,
}

pub type Result<T> = std::result::Result<T, Error>;

/// The helper's name on disk.
pub const HELPER_NAME: &str = "darkroom-helper";

/// Where the helper is looked for, first match wins: `$DARKROOM_HELPER`, next to the running
/// executable, inside the app bundle's `Contents/Helpers`, then the source tree's SwiftPM build
/// output (release, then debug) for development.
pub fn locate_helper() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("DARKROOM_HELPER") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let mut candidates = vec![dir.join(HELPER_NAME), dir.join("../Helpers").join(HELPER_NAME)];
    // development: <repo>/target/<profile>/lightcraft -> <repo>/helper/darkroom-helper/.build/...
    for up in [2usize, 3] {
        let mut root = dir.to_path_buf();
        for _ in 0..up {
            root = match root.parent() {
                Some(p) => p.to_path_buf(),
                None => break,
            };
        }
        for profile in ["release", "debug"] {
            candidates.push(root.join("helper").join(HELPER_NAME).join(".build").join(profile).join(HELPER_NAME));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// The real runner: spawns `helper` and waits up to the timeout, reading both pipes while it runs.
pub fn process_runner(helper: PathBuf) -> Runner {
    Arc::new(move |args: &[String], timeout: Duration| {
        use std::io::Read;
        use std::process::{Command, Stdio};
        let mut child = Command::new(&helper)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Spawn(e.to_string()))?;
        fn drain(r: Option<impl Read + Send + 'static>) -> std::io::Result<std::thread::JoinHandle<String>> {
            std::thread::Builder::new().name("darkroom-helper-output".into()).spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut r) = r {
                    let _ = r.read_to_end(&mut buf);
                }
                String::from_utf8_lossy(&buf).into_owned()
            })
        }
        let pipes = drain(child.stdout.take()).and_then(|o| Ok((o, drain(child.stderr.take())?)));
        let (out, err) = match pipes {
            Ok(p) => p,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Spawn(e.to_string()));
            }
        };
        let deadline = std::time::Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if std::time::Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::Timeout(timeout));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::Spawn(e.to_string()));
                }
            }
        };
        let stdout = out.join().unwrap_or_default();
        let stderr = err.join().unwrap_or_default();
        if status.success() {
            Ok(stdout)
        } else {
            // the helper prints {"error": ...} on its own failures
            let msg = serde_json::from_str::<Value>(stdout.trim())
                .ok()
                .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
                .unwrap_or_else(|| if stderr.trim().is_empty() { format!("the helper exited with {status}") } else { stderr.trim().to_string() });
            Err(Error::Helper(msg))
        }
    })
}

// ---------- rows

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AlbumKind {
    Folder,
    Album,
    Smart,
    Other,
}

/// One folder, album or smart album.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    #[serde(default)]
    pub cloud_id: String,
    pub title: String,
    pub kind: AlbumKind,
    /// Nesting depth in the sidebar (0 = top level).
    #[serde(default)]
    pub depth: u32,
    /// The parent folder's id; "" at the top level, "smart" for Photos' own smart albums.
    #[serde(default)]
    pub parent: String,
    /// Photos' estimate; -1 when unknown.
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub subtype: i64,
}

/// One photo or video in the library.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    #[serde(default)]
    pub cloud_id: String,
    #[serde(default)]
    pub media_type: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub filenames: Vec<String>,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub modified: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub burst: bool,
    #[serde(default)]
    pub burst_id: Option<String>,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub raw: bool,
    #[serde(default)]
    pub edited: bool,
    #[serde(default = "yes")]
    pub local: bool,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
    #[serde(default)]
    pub screenshot: bool,
    #[serde(default)]
    pub panorama: bool,
    /// Photos (or you) picked this frame of its burst.
    #[serde(default)]
    pub burst_pick: bool,
    /// Seconds, videos only.
    #[serde(default)]
    pub duration: f64,
}

/// One regular album's members (asset local ids), from `album.members`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlbumMembers {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub ids: Vec<String>,
}

/// The Curator's quick measures for one photo, from `curator.measure`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub lap_var: f64,
    #[serde(default)]
    pub mean: f64,
    #[serde(default)]
    pub black: f64,
    #[serde(default)]
    pub white: f64,
    #[serde(default)]
    pub dhash: String,
    #[serde(default)]
    pub error: Option<String>,
}

fn yes() -> bool {
    true
}

impl Asset {
    /// The id to store: the cloud id when Photos has one, else the local id.
    pub fn stable_id(&self) -> &str {
        if self.cloud_id.is_empty() { &self.id } else { &self.cloud_id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetPage {
    pub items: Vec<Asset>,
    pub total: usize,
    #[serde(default)]
    pub offset: usize,
}

/// An original (or a thumbnail) fetched to disk, or why not.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FetchedFile {
    pub id: String,
    #[serde(default)]
    pub cloud_id: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub cached: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Keywords {
    pub id: String,
    #[serde(default)]
    pub cloud_id: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
}

/// One photo's keyword, title and caption change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeywordChange {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub authorized: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub library_path: Option<String>,
    #[serde(default)]
    pub assets: u64,
    #[serde(default)]
    pub albums: u64,
}

// ---------- client

/// The client: one helper, one runner.
#[derive(Clone)]
pub struct Photos {
    runner: Runner,
}

impl std::fmt::Debug for Photos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Photos")
    }
}

/// Reading a page of assets or writing a batch can take a while on an 80,000 photo library;
/// fetching originals from iCloud longer still.
pub const QUICK: Duration = Duration::from_secs(120);
pub const LONG: Duration = Duration::from_secs(3600);

impl Photos {
    /// On macOS, the helper found by [`locate_helper`]; `Err(NoHelper)` when it is not built;
    /// `Err(NotMac)` elsewhere.
    pub fn system() -> Result<Self> {
        if !cfg!(target_os = "macos") {
            return Err(Error::NotMac);
        }
        let helper = locate_helper().ok_or(Error::NoHelper)?;
        log::info!("Apple Photos helper: {}", helper.display());
        Ok(Self { runner: process_runner(helper) })
    }

    pub fn with_runner(runner: Runner) -> Self {
        Self { runner }
    }

    pub fn at(helper: &Path) -> Self {
        Self { runner: process_runner(helper.to_path_buf()) }
    }

    fn call(&self, command: &str, params: Value, timeout: Duration) -> Result<Value> {
        let args = vec![command.to_string(), params.to_string()];
        let out = (self.runner)(&args, timeout)?;
        let v: Value =
            serde_json::from_str(out.trim()).map_err(|e| Error::Protocol(format!("{e}: {}", out.chars().take(200).collect::<String>())))?;
        if let Some(e) = v.get("error").and_then(Value::as_str) {
            return Err(Error::Helper(e.to_string()));
        }
        Ok(v)
    }

    fn parse<T: for<'a> Deserialize<'a>>(v: Value) -> Result<T> {
        serde_json::from_value(v).map_err(|e| Error::Protocol(e.to_string()))
    }

    fn items<T: for<'a> Deserialize<'a>>(v: Value) -> Result<Vec<T>> {
        let items = v.get("items").cloned().unwrap_or(Value::Array(vec![]));
        Self::parse(items)
    }

    pub fn status(&self) -> Result<Status> {
        Self::parse(self.call("status", json!({}), QUICK)?)
    }

    /// The folder and album tree, depth first, then Photos' own smart albums (parent "smart").
    pub fn albums(&self) -> Result<Vec<Album>> {
        Self::items(self.call("albums", json!({}), QUICK)?)
    }

    /// The tree with each album's cloud identifier too (slower: hundreds of lookups).
    pub fn albums_with_cloud_ids(&self) -> Result<Vec<Album>> {
        Self::items(self.call("albums", json!({"cloud": true}), QUICK)?)
    }

    /// A page of assets, oldest first; `album` = None for the whole library. Hidden photos are
    /// left out unless `hidden`.
    pub fn assets(&self, album: Option<&str>, offset: usize, limit: usize, hidden: bool) -> Result<AssetPage> {
        let mut p = json!({"offset": offset, "limit": limit, "hidden": hidden});
        if let Some(a) = album {
            p["album"] = Value::String(a.to_string());
        }
        Self::parse(self.call("assets", p, LONG)?)
    }

    /// Every asset, page by page, calling `each` with every page; stops early when it returns
    /// false (cancel).
    pub fn all_assets(&self, album: Option<&str>, hidden: bool, mut each: impl FnMut(&AssetPage) -> bool) -> Result<usize> {
        let mut offset = 0;
        let page_size = 2000;
        loop {
            let page = self.assets(album, offset, page_size, hidden)?;
            let total = page.total;
            let n = page.items.len();
            if !each(&page) || n == 0 || offset + n >= total {
                return Ok(total);
            }
            offset += n;
        }
    }

    /// The whole library for the Curator, page by page: hidden photos and every frame of every
    /// burst included, oldest first.
    pub fn library_page(&self, offset: usize, limit: usize) -> Result<AssetPage> {
        Self::parse(self.call("assets", json!({"offset": offset, "limit": limit, "hidden": true, "allBursts": true}), LONG)?)
    }

    /// Every regular album's members in one call.
    pub fn album_members(&self) -> Result<Vec<AlbumMembers>> {
        Self::items(self.call("album.members", json!({}), LONG)?)
    }

    /// The Curator's quick measures on Photos' previews (`size` px on the long edge).
    pub fn measure(&self, ids: &[String], size: u32) -> Result<Vec<Measured>> {
        Self::items(self.call("curator.measure", json!({"ids": ids, "size": size}), LONG)?)
    }

    /// Originals on disk (fetched into Darkroom's cache, downloaded from iCloud when needed).
    pub fn originals(&self, ids: &[String]) -> Result<Vec<FetchedFile>> {
        Self::items(self.call("originals", json!({"ids": ids}), LONG)?)
    }

    /// JPEG thumbnails of up to `size` px on the long edge, in `dir` (None: the helper's cache).
    pub fn thumbs(&self, ids: &[String], size: u32, dir: Option<&Path>) -> Result<Vec<FetchedFile>> {
        let mut p = json!({"ids": ids, "size": size});
        if let Some(d) = dir {
            p["dir"] = Value::String(d.to_string_lossy().to_string());
        }
        Self::items(self.call("thumbs", p, LONG)?)
    }

    pub fn keywords(&self, ids: &[String]) -> Result<Vec<Keywords>> {
        Self::items(self.call("keywords", json!({"ids": ids}), LONG)?)
    }

    pub fn set_keywords(&self, changes: &[KeywordChange]) -> Result<Value> {
        self.call("keywords.set", json!({"items": changes}), LONG)
    }

    /// Favorite and/or hide; returns how many changed.
    pub fn set(&self, ids: &[String], favorite: Option<bool>, hidden: Option<bool>) -> Result<u64> {
        let mut p = json!({"ids": ids});
        if let Some(f) = favorite {
            p["favorite"] = Value::Bool(f);
        }
        if let Some(h) = hidden {
            p["hidden"] = Value::Bool(h);
        }
        let v = self.call("set", p, LONG)?;
        Ok(v.get("changed").and_then(Value::as_u64).unwrap_or(0))
    }

    /// A new album, inside `folder` when given; returns its local id.
    pub fn create_album(&self, title: &str, folder: Option<&str>) -> Result<String> {
        let mut p = json!({"title": title});
        if let Some(f) = folder {
            p["folder"] = Value::String(f.to_string());
        }
        let v = self.call("album.create", p, QUICK)?;
        Ok(v.get("id").and_then(Value::as_str).unwrap_or_default().to_string())
    }

    pub fn create_folder(&self, title: &str, parent: Option<&str>) -> Result<String> {
        let mut p = json!({"title": title});
        if let Some(f) = parent {
            p["parent"] = Value::String(f.to_string());
        }
        let v = self.call("folder.create", p, QUICK)?;
        Ok(v.get("id").and_then(Value::as_str).unwrap_or_default().to_string())
    }

    /// Adds to an album and returns how many are really in it afterwards (Photos can skip
    /// silently, so the helper counts).
    pub fn add_to_album(&self, album: &str, ids: &[String]) -> Result<(u64, u64)> {
        let v = self.call("album.add", json!({"album": album, "ids": ids}), LONG)?;
        Ok((v.get("added").and_then(Value::as_u64).unwrap_or(0), v.get("requested").and_then(Value::as_u64).unwrap_or(0)))
    }
    // ---------- printing (the same helper reaches the macOS print system)

    /// The print queues on this Mac, each flagged `airPrint` or `epsonDriver` from its PPD.
    pub fn print_queues(&self) -> Result<Value> {
        self.call("print.queues", json!({}), QUICK)
    }

    /// Every option the queue's driver exposes (`lpoptions -l`), with the PPD's labels.
    pub fn print_options(&self, queue: &str) -> Result<Value> {
        self.call("print.options", json!({"queue": queue}), QUICK)
    }

    /// Hands a prepared file to the queue with driver options. Refused for AirPrint queues.
    pub fn print_submit(&self, queue: &str, file: &Path, title: &str, copies: u32, options: &Value) -> Result<Value> {
        self.call(
            "print.submit",
            json!({"queue": queue, "file": file.to_string_lossy(), "title": title, "copies": copies, "options": options}),
            QUICK,
        )
    }

    pub fn print_jobs(&self, queue: Option<&str>) -> Result<Value> {
        self.call("print.jobs", json!({"queue": queue.unwrap_or("")}), QUICK)
    }
}

/// A runner that answers every call with the same JSON (tests).
pub fn fake_runner(answer: impl Fn(&str, &Value) -> Result<Value> + Send + Sync + 'static) -> Runner {
    Arc::new(move |args: &[String], _| {
        let cmd = args.first().map(String::as_str).unwrap_or("");
        let params: Value = args.get(1).and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null);
        answer(cmd, &params).map(|v| v.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_parse_with_missing_fields() {
        let p = Photos::with_runner(fake_runner(|cmd, p| match cmd {
            "albums" => Ok(json!({"items": [
                {"id": "F1/L0/020", "cloudId": "c1", "title": "01_Trips", "kind": "folder", "depth": 0, "parent": "", "count": 3},
                {"id": "A1/L0/040", "cloudId": "c2", "title": "Iceland", "kind": "album", "depth": 1, "parent": "F1/L0/020", "count": 412, "subtype": 2},
            ]})),
            "assets" => {
                assert_eq!(p["offset"], 0);
                Ok(json!({"items": [{"id": "X/L0/001", "cloudId": "cx", "width": 6000, "height": 4000, "favorite": true}], "total": 1}))
            }
            "status" => Ok(json!({"authorized": true, "status": "authorized", "assets": 80000})),
            _ => Err(Error::Helper("no".into())),
        }));
        let albums = p.albums().unwrap();
        assert_eq!(albums.len(), 2);
        assert_eq!(albums[0].kind, AlbumKind::Folder);
        assert_eq!(albums[1].parent, "F1/L0/020");
        let page = p.assets(None, 0, 10, false).unwrap();
        assert_eq!(page.total, 1);
        assert!(page.items[0].favorite);
        assert!(page.items[0].local);
        assert_eq!(page.items[0].stable_id(), "cx");
        assert_eq!(p.status().unwrap().assets, 80000);
    }

    #[test]
    fn helper_errors_come_back_as_messages() {
        let p = Photos::with_runner(Arc::new(|_: &[String], _| Ok("{\"error\": \"Photos access is denied\"}".to_string())));
        assert_eq!(p.albums(), Err(Error::Helper("Photos access is denied".into())));
        let p = Photos::with_runner(Arc::new(|_: &[String], _| Ok("not json".to_string())));
        assert!(matches!(p.albums(), Err(Error::Protocol(_))));
    }

    #[test]
    fn all_assets_pages_until_the_total() {
        let p = Photos::with_runner(fake_runner(|_, p| {
            let offset = p["offset"].as_u64().unwrap() as usize;
            let items: Vec<Value> = (offset..(offset + 2000).min(4500)).map(|i| json!({"id": format!("{i}/L0/001")})).collect();
            Ok(json!({"items": items, "total": 4500, "offset": offset}))
        }));
        let mut seen = 0;
        let total = p
            .all_assets(None, false, |page| {
                seen += page.items.len();
                true
            })
            .unwrap();
        assert_eq!((seen, total), (4500, 4500));
    }
}
