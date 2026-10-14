//! Darkroom: the Curator's commands (`curator.*`, `CURATOR.md` C0 and C1).
//!
//! `curator.scan` fills the store from Apple Photos through the helper (Layer 1: PhotoKit's facts,
//! album membership, and the quick measures on Photos' previews); `curator.report` is the library
//! in numbers; `curator.stacks` and `curator.cull` turn the store into plans; `curator.plan` and
//! `curator.review` are the review; `curator.apply` writes an accepted plan's keywords to Photos.
//!
//! Three rules, as in the spec: nothing is deleted (a plan only adds keywords); review before
//! write (`curator.apply` without `confirm: true` only says what it would do, and only accepted
//! groups minus what you took out are written); pictures stay on the Mac (Layers 1 and 2 are
//! local).
//!
//! The store, the rules and the plans live in the library folder (both Macs share them); without
//! a library on disk, in Darkroom's config folder.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use darkroom_curator::{Measures, Photo, Plan, Rules, Store};
use serde::Serialize;
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, cmd, str_param};
use crate::{Result, Session};

/// Photos measured per helper call, and how often a long scan saves what it has.
const MEASURE_CHUNK: usize = 48;
const SAVE_EVERY: usize = 1500;
/// Keyword writes per helper call (Photos' scripting interface does one photo at a time).
const WRITE_CHUNK: usize = 25;
/// Long edge of the preview the measures are taken on.
const MEASURE_SIZE: u32 = 1024;

/// The note a scan leaves while Photos' own database is out of reach.
pub const NO_PHOTOS_DB: &str = "Photos' own analysis (scene labels, aesthetic scores, camera EXIF) is not read yet: its database needs Full Disk Access, so cameras are guessed from file names";

/// The running or last background job (scan or apply), as the window and `curator.status` see it.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub kind: String,
    pub phase: String,
    pub done: u64,
    pub total: u64,
    pub running: bool,
    pub error: Option<String>,
    pub result: Value,
}

#[derive(Clone, Default)]
pub struct Curator {
    pub job: Arc<Mutex<JobInfo>>,
    cancel: Arc<AtomicBool>,
}

impl std::fmt::Debug for Curator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Curator")
    }
}

impl Curator {
    pub fn info(&self) -> JobInfo {
        self.job.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
    fn running(&self) -> bool {
        self.info().running
    }
    fn update(&self, f: impl FnOnce(&mut JobInfo)) {
        f(&mut self.job.lock().unwrap_or_else(PoisonError::into_inner));
    }
}

/// Unix seconds now.
fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn now_iso() -> String {
    format!("{}Z", lightcraft_catalog::dates::civil(now_secs()))
}

/// Where the Curator keeps its files.
pub fn curator_dir(s: &Session) -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join("darkroom-tests").join(format!("curator-{}", std::process::id()));
    }
    match s.library.as_ref().filter(|l| l.on_disk) {
        Some(l) => l.dir.clone(),
        None => crate::config::config_dir().unwrap_or_else(std::env::temp_dir).join("curator"),
    }
}

fn store_path(dir: &Path) -> PathBuf {
    dir.join(Store::FILE)
}

fn load_store(dir: &Path, c: &str) -> Result<Store> {
    Store::load(&store_path(dir)).map_err(|e| bad(c, e.to_string()))
}

/// The rules; the defaults are written out the first time so they can be edited.
fn load_rules(dir: &Path, c: &str) -> Result<Rules> {
    let path = dir.join(Rules::FILE);
    let rules = Rules::load(&path).map_err(|e| bad(c, e.to_string()))?;
    if !path.exists() {
        let _ = rules.save(&path);
    }
    Ok(rules)
}

fn plans_dir(dir: &Path) -> PathBuf {
    dir.join(Plan::DIR)
}

fn save_plan(dir: &Path, plan: &Plan, c: &str) -> Result<()> {
    let bytes = serde_json::to_vec(plan).map_err(|e| bad(c, e.to_string()))?;
    darkroom_curator::write_atomic(&plans_dir(dir).join(format!("{}.json", plan.id)), &bytes).map_err(|e| bad(c, e.to_string()))
}

/// The plans on disk, newest first.
fn list_plans(dir: &Path) -> Vec<Plan> {
    let Ok(rd) = std::fs::read_dir(plans_dir(dir)) else { return Vec::new() };
    let mut plans: Vec<Plan> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice(&b).ok())
        .collect();
    plans.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
    plans
}

/// A plan by id, or the newest one (of `job` when given).
fn find_plan(dir: &Path, p: &Value, c: &str) -> Result<Plan> {
    let id = str_param(p, "id").filter(|x| !x.is_empty());
    let job = str_param(p, "job").filter(|x| !x.is_empty());
    list_plans(dir)
        .into_iter()
        .find(|pl| id.is_none_or(|i| pl.id == i) && job.is_none_or(|j| pl.job == j))
        .ok_or_else(|| bad(c, if id.is_some() { "no plan with that id" } else { "no plan yet: run curator.stacks or curator.cull" }))
}

fn photo_from(a: &darkroom_photos::Asset) -> Photo {
    Photo {
        id: a.stable_id().to_string(),
        local_id: a.id.clone(),
        media: a.media_type.clone(),
        width: a.width,
        height: a.height,
        created: a.created.clone(),
        modified: a.modified.clone(),
        favorite: a.favorite,
        hidden: a.hidden,
        burst_id: a.burst_id.clone(),
        burst_pick: a.burst_pick,
        live: a.live,
        raw: a.raw,
        edited: a.edited,
        screenshot: a.screenshot,
        panorama: a.panorama,
        lat: a.lat,
        lon: a.lon,
        filename: a.filename.clone(),
        duration: a.duration,
    }
}

/// Folder paths for albums from the tree rows (parent = folder id).
fn album_folders(tree: &[darkroom_photos::Album]) -> std::collections::HashMap<String, String> {
    let by_id: std::collections::HashMap<&str, &darkroom_photos::Album> = tree.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut out = std::collections::HashMap::new();
    for a in tree {
        let mut parts = Vec::new();
        let mut parent = a.parent.as_str();
        // bounded: a cycle in the tree can't hang the scan
        for _ in 0..32 {
            let Some(f) = by_id.get(parent) else { break };
            parts.push(f.title.clone());
            parent = f.parent.as_str();
        }
        parts.reverse();
        out.insert(a.id.clone(), parts.join("/"));
    }
    out
}

/// What a scan does, wherever it runs. `progress(phase, done, total)`; stops between chunks when
/// `cancel` is set, keeping what it has.
pub fn run_scan(
    photos: &darkroom_photos::Photos,
    dir: &Path,
    measure_limit: Option<usize>,
    cancel: &AtomicBool,
    progress: &dyn Fn(&str, u64, u64),
) -> std::result::Result<Value, String> {
    let mut store = Store::load(&store_path(dir)).map_err(|e| e.to_string())?;
    store.version = darkroom_curator::store::VERSION;
    // 1. every photo and video
    let mut rows = Vec::new();
    let mut offset = 0;
    loop {
        let page = photos.library_page(offset, 5000).map_err(|e| e.to_string())?;
        let n = page.items.len();
        rows.extend(page.items.iter().map(photo_from));
        progress("Reading Apple Photos", rows.len() as u64, page.total as u64);
        if n == 0 || offset + n >= page.total || cancel.load(Ordering::Relaxed) {
            break;
        }
        offset += n;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled before the listing finished; the store is unchanged".into());
    }
    let (added, changed, removed) = store.replace_photos(rows);
    // 2. albums
    progress("Reading albums", 0, 1);
    let tree = photos.albums().map_err(|e| e.to_string())?;
    let folders = album_folders(&tree);
    let members = photos.album_members().map_err(|e| e.to_string())?;
    {
        let by_local = store.by_local_id();
        store.albums = members
            .iter()
            .map(|m| darkroom_curator::Album {
                id: m.id.clone(),
                title: m.title.clone(),
                folder: folders.get(&m.id).cloned().unwrap_or_default(),
                members: m.ids.iter().filter_map(|l| by_local.get(l.as_str()).map(|s| s.to_string())).collect(),
            })
            .collect();
    }
    // 3. quick measures on what is new or edited
    let mut todo: Vec<(String, String, Option<String>)> =
        store.needs_measuring().iter().map(|p| (p.local_id.clone(), p.id.clone(), p.modified.clone())).collect();
    if let Some(limit) = measure_limit {
        todo.truncate(limit);
    }
    let total = todo.len() as u64;
    let (mut measured, mut failed, mut since_save) = (0u64, 0u64, 0usize);
    let mut errors: Vec<String> = Vec::new();
    for chunk in todo.chunks(MEASURE_CHUNK) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let ids: Vec<String> = chunk.iter().map(|c| c.0.clone()).collect();
        let by_local: std::collections::HashMap<&str, (&str, &Option<String>)> = chunk.iter().map(|c| (c.0.as_str(), (c.1.as_str(), &c.2))).collect();
        match photos.measure(&ids, MEASURE_SIZE) {
            Ok(rows) => {
                for r in rows {
                    let Some(&(stable, modified)) = by_local.get(r.id.as_str()) else { continue };
                    if let Some(e) = r.error {
                        failed += 1;
                        if errors.len() < 5 && !errors.contains(&e) {
                            errors.push(e);
                        }
                        continue;
                    }
                    let lap = r.lap_var as f32;
                    store.measures.insert(
                        stable.to_string(),
                        Measures {
                            sharp: Measures::sharp_from_variance(lap),
                            lap_var: lap,
                            mean: r.mean as f32,
                            black: r.black as f32,
                            white: r.white as f32,
                            dhash: r.dhash,
                            modified: modified.clone(),
                        },
                    );
                    measured += 1;
                }
            }
            Err(e) => {
                failed += chunk.len() as u64;
                if errors.len() < 5 {
                    errors.push(e.to_string());
                }
            }
        }
        since_save += chunk.len();
        progress("Measuring previews", measured + failed, total);
        if since_save >= SAVE_EVERY {
            since_save = 0;
            store.save(&store_path(dir)).map_err(|e| e.to_string())?;
        }
    }
    store.notes = vec![NO_PHOTOS_DB.to_string()];
    let cancelled = cancel.load(Ordering::Relaxed);
    if cancelled {
        store.notes.push(format!("the last scan was stopped with {} previews left to measure; scan again to finish", total - measured - failed));
    }
    store.scanned = Some(now_iso());
    store.save(&store_path(dir)).map_err(|e| e.to_string())?;
    Ok(json!({
        "photos": store.photos.len(), "added": added, "changed": changed, "removed": removed,
        "albums": store.albums.len(), "measured": measured, "measureFailed": failed, "toMeasure": total,
        "measuredTotal": store.measures.len(), "errors": errors, "cancelled": cancelled,
        "store": store_path(dir).to_string_lossy(),
    }))
}

/// Writes an accepted plan's keywords to Photos, records every decision and marks the plan
/// applied. Returns {written, failed, photos}.
pub fn run_apply(
    photos: &darkroom_photos::Photos,
    dir: &Path,
    mut plan: Plan,
    cancel: &AtomicBool,
    progress: &dyn Fn(&str, u64, u64),
) -> std::result::Result<Value, String> {
    let changes = plan.changes();
    let total = changes.len() as u64;
    let mut written: std::collections::HashSet<String> = Default::default();
    let mut failures: Vec<Value> = Vec::new();
    for chunk in changes.chunks(WRITE_CHUNK) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let batch: Vec<darkroom_photos::KeywordChange> =
            chunk.iter().map(|(id, add)| darkroom_photos::KeywordChange { id: id.clone(), add: add.clone(), ..Default::default() }).collect();
        match photos.set_keywords(&batch) {
            Ok(v) => {
                let bad_ids: std::collections::HashSet<String> =
                    v["failed"].as_array().map(|a| a.iter().filter_map(|f| f["id"].as_str().map(str::to_string)).collect()).unwrap_or_default();
                for f in v["failed"].as_array().into_iter().flatten() {
                    failures.push(f.clone());
                }
                for (id, _) in chunk {
                    if !bad_ids.contains(id) {
                        written.insert(id.clone());
                    }
                }
            }
            Err(e) => {
                for (id, _) in chunk {
                    failures.push(json!({"id": id, "error": e.to_string()}));
                }
            }
        }
        progress("Writing keywords to Apple Photos", (written.len() + failures.len()) as u64, total);
    }
    // the record: every item of the plan, accepted or taken out
    let at = now_iso();
    let mut store = Store::load(&store_path(dir)).map_err(|e| e.to_string())?;
    for g in &plan.groups {
        for it in &g.items {
            let accepted = g.accepted && !it.excluded;
            if !g.accepted && !it.excluded {
                continue; // a group never reviewed is not a decision
            }
            store.decisions.push(darkroom_curator::Decision {
                at: at.clone(),
                plan: plan.id.clone(),
                job: plan.job.clone(),
                rule: g.key.clone(),
                id: it.id.clone(),
                add: g.add.clone(),
                accepted,
                written: accepted && written.contains(&it.id),
            });
        }
    }
    store.save(&store_path(dir)).map_err(|e| e.to_string())?;
    let cancelled = cancel.load(Ordering::Relaxed);
    if !cancelled {
        plan.applied = Some(at);
    }
    let bytes = serde_json::to_vec(&plan).map_err(|e| e.to_string())?;
    darkroom_curator::write_atomic(&plans_dir(dir).join(format!("{}.json", plan.id)), &bytes).map_err(|e| e.to_string())?;
    Ok(json!({"photos": total, "written": written.len(), "failed": failures, "cancelled": cancelled, "plan": plan.id}))
}

/// Runs `work` on a worker with a row in the activity stack (the app), or right here (CLI,
/// scripts, tests: `wait`).
fn start_job(
    s: &mut Session,
    kind: &'static str,
    label: &str,
    wait: bool,
    work: impl FnOnce(&AtomicBool, &dyn Fn(&str, u64, u64)) -> std::result::Result<Value, String> + Send + 'static,
) -> Result<Value> {
    const C: &str = "curator";
    if s.curator.running() {
        return Err(bad(C, "the Curator is already running a job; see curator.status"));
    }
    let cur = s.curator.clone();
    cur.cancel.store(false, Ordering::Relaxed);
    cur.update(|j| *j = JobInfo { kind: kind.into(), phase: "Starting".into(), running: true, ..Default::default() });
    let guard = s.activity.start("curator", label, crate::activity::Cancel::Flag(cur.cancel.clone()));
    let run = move || {
        let progress = |phase: &str, done: u64, total: u64| {
            guard.detail(phase);
            guard.progress(done, total);
            cur.update(|j| {
                j.phase = phase.to_string();
                j.done = done;
                j.total = total;
            });
        };
        let r = work(&cur.cancel, &progress);
        cur.update(|j| {
            j.running = false;
            match &r {
                Ok(v) => j.result = v.clone(),
                Err(e) => j.error = Some(e.clone()),
            }
        });
        drop(guard);
        r
    };
    #[cfg(not(target_arch = "wasm32"))]
    if !wait {
        std::thread::Builder::new().name(format!("curator-{kind}")).spawn(run).map_err(|e| bad(C, e.to_string()))?;
        return Ok(json!({"started": kind}));
    }
    let _ = wait;
    run().map_err(|e| bad(C, e))
}

fn wait_param(s: &Session, p: &Value) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    let background = s.apple_photos_background;
    #[cfg(target_arch = "wasm32")]
    let background = {
        let _ = s;
        false
    };
    p.get("wait").and_then(Value::as_bool).unwrap_or(!background)
}

fn photos_enabled(s: &Session) -> std::result::Result<(), String> {
    if s.photos.is_some() { Ok(()) } else { Err(super::photos::unavailable()) }
}

/// One plan item with what the review sheet needs to show it.
fn item_row(store: &Store, it: &darkroom_curator::Item) -> Value {
    let p = store.photos.get(&it.id);
    let m = store.measures.get(&it.id);
    json!({
        "id": it.id, "localId": p.map(|p| p.local_id.as_str()), "filename": p.map(|p| p.filename.as_str()),
        "created": p.and_then(|p| p.created.as_deref()), "confidence": it.confidence, "reason": it.reason,
        "winner": it.winner, "excluded": it.excluded, "sharp": m.map(|m| m.sharp), "mean": m.map(|m| m.mean),
    })
}

fn strs(p: &Value, key: &str) -> Vec<String> {
    p.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "curator.status", "Curator Status", [], None,
        "{} → {dir, store: {photos, measured, albums, stacks, scanned, notes}, job: {kind, phase, done, total, running, error?, result}, plans: [summaries, newest first]} — what the Curator knows and is doing",
        always, |s, _| {
            let dir = curator_dir(s);
            let store = load_store(&dir, "curator.status")?;
            Ok(json!({
                "dir": dir.to_string_lossy(),
                "store": {"photos": store.photos.len(), "measured": store.measures.len(), "albums": store.albums.len(),
                          "stacks": store.stacks.len(), "decisions": store.decisions.len(), "scanned": store.scanned, "notes": store.notes},
                "job": s.curator.info(),
                "plans": list_plans(&dir).iter().map(Plan::summary).collect::<Vec<_>>(),
            }))
        }),
        cmd!(query "curator.scan", "Scan Apple Photos for the Curator", ["Library", "Curator"], None,
        "{measureLimit?: n (measure at most n previews this run), wait?: bool (default: in the background in the app, waits in the CLI)} → {photos, added, changed, removed, albums, measured, measureFailed, toMeasure, measuredTotal, errors, cancelled} or {started} — Layer 1: every photo's PhotoKit facts, album membership, and quick measures (focus, exposure, a perceptual hash) on Photos' 1024 px previews; only new and edited photos are measured again. Reads only; nothing in Photos changes.",
        photos_enabled, |s, p| {
            let photos = s.photos.clone().ok_or_else(|| bad("curator.scan", super::photos::unavailable()))?;
            let dir = curator_dir(s);
            load_rules(&dir, "curator.scan")?;
            let limit = p.get("measureLimit").and_then(Value::as_u64).map(|n| n as usize);
            let wait = wait_param(s, p);
            start_job(s, "scan", "Curator: scanning Apple Photos", wait, move |cancel, progress| run_scan(&photos, &dir, limit, cancel, progress))
        }),
        cmd!(query "curator.cancel", "Stop the Curator", ["Curator"], None,
        "{} → {cancelling} — stops the running scan or apply between chunks; what is done is kept",
        always, |s, _| {
            let running = s.curator.running();
            s.curator.cancel.store(true, Ordering::Relaxed);
            Ok(json!({"cancelling": running}))
        }),
        cmd!(query "curator.report", "Curator Library Report", ["Library", "Curator"], None,
        "{} → {report: {total, images, videos, byYear, byDevice, bursts, exactDuplicates, blur, blackOrBlown, albums, overlaps, first: [{what, photos, minutes}], notes, …}, text} — the library in numbers, from the store (run curator.scan first)",
        always, |s, _| {
            let dir = curator_dir(s);
            let store = load_store(&dir, "curator.report")?;
            if store.photos.is_empty() {
                return Err(bad("curator.report", "the Curator has not scanned yet: run curator.scan"));
            }
            let rules = load_rules(&dir, "curator.report")?;
            let r = darkroom_curator::report::build(&store, &rules);
            Ok(json!({"report": r, "text": r.text()}))
        }),
        cmd!(query "curator.stacks", "Curator: Find Stacks", ["Library", "Curator"], None,
        "{} → {plan summary, stacks} — bursts, exact and near duplicates become stacks with one winner (your favorite or edit always wins); the losers become a plan to review (Pick: No). Nothing is written to Photos.",
        always, |s, _| {
            const C: &str = "curator.stacks";
            let dir = curator_dir(s);
            let mut store = load_store(&dir, C)?;
            if store.photos.is_empty() {
                return Err(bad(C, "the Curator has not scanned yet: run curator.scan"));
            }
            let rules = load_rules(&dir, C)?;
            let (stacks, plan) = darkroom_curator::stacks::build(&store, &rules, &format!("stacks-{}", now_secs()), &now_iso());
            store.stacks = stacks;
            store.save(&store_path(&dir)).map_err(|e| bad(C, e.to_string()))?;
            save_plan(&dir, &plan, C)?;
            let mut v = plan.summary();
            v["stacks"] = json!(store.stacks.len());
            Ok(v)
        }),
        cmd!(query "curator.cull", "Curator: Cull", ["Library", "Curator"], None,
        "{} → {plan summary} — black, blown, pocket and blurred frames, screenshots outside a documents album, and too many near-identical frames (run curator.stacks first for that rule), as a plan to review (Pick: No, Kind: Junk). Favorites and edits are never proposed. Nothing is written to Photos.",
        always, |s, _| {
            const C: &str = "curator.cull";
            let dir = curator_dir(s);
            let store = load_store(&dir, C)?;
            if store.photos.is_empty() {
                return Err(bad(C, "the Curator has not scanned yet: run curator.scan"));
            }
            let rules = load_rules(&dir, C)?;
            let plan = darkroom_curator::cull::build(&store, &rules, &format!("cull-{}", now_secs()), &now_iso());
            save_plan(&dir, &plan, C)?;
            Ok(plan.summary())
        }),
        cmd!(query "curator.plan", "Curator Plan", [], None,
        "{id?: plan id (default: the newest), job?: stacks|cull, group?: key, offset?: 0, limit?: 60} → {plan summary, items?: [{id, localId, filename, created, confidence, reason, winner, excluded, sharp, mean}]} — a plan for review; with group, a page of its items",
        always, |s, p| {
            const C: &str = "curator.plan";
            let dir = curator_dir(s);
            let plan = find_plan(&dir, p, C)?;
            let mut v = plan.summary();
            if let Some(key) = str_param(p, "group").filter(|g| !g.is_empty()) {
                let g = plan.groups.iter().find(|g| g.key == key).ok_or_else(|| bad(C, format!("no group `{key}` in {}", plan.id)))?;
                let store = load_store(&dir, C)?;
                let offset = p.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
                let limit = p.get("limit").and_then(Value::as_u64).unwrap_or(60).clamp(1, 5000) as usize;
                v["items"] = json!(g.items.iter().skip(offset).take(limit).map(|it| item_row(&store, it)).collect::<Vec<_>>());
                v["group"] = json!(key);
            }
            Ok(v)
        }),
        cmd!(query "curator.review", "Curator Review", [], None,
        "{id?: plan id (default: the newest), group: key, accept?: bool, exclude?: [photo ids], include?: [photo ids]} → {plan summary} — accept or reject a group and take frames out of it (or put them back); nothing is written to Photos",
        always, |s, p| {
            const C: &str = "curator.review";
            let dir = curator_dir(s);
            let mut plan = find_plan(&dir, p, C)?;
            if plan.applied.is_some() {
                return Err(bad(C, format!("{} was applied already; run the job again for a new plan", plan.id)));
            }
            let key = str_param(p, "group").filter(|g| !g.is_empty()).ok_or_else(|| bad(C, "group required"))?;
            let g = plan.groups.iter_mut().find(|g| g.key == key).ok_or_else(|| bad(C, format!("no group `{key}`")))?;
            if let Some(a) = p.get("accept").and_then(Value::as_bool) {
                g.accepted = a;
            }
            let (ex, inc) = (strs(p, "exclude"), strs(p, "include"));
            for it in &mut g.items {
                if ex.contains(&it.id) {
                    it.excluded = true;
                }
                if inc.contains(&it.id) {
                    it.excluded = false;
                }
            }
            save_plan(&dir, &plan, C)?;
            Ok(plan.summary())
        }),
        cmd!(query "curator.apply", "Curator: Apply to Apple Photos", ["Curator"], None,
        "{id?: plan id (default: the newest), confirm?: false, wait?: bool} → without confirm: {plan summary, wouldWrite: {photos, keywords}} and nothing changes; with confirm: true, writes the accepted groups' keywords (minus what you took out) to Photos and records every decision → {photos, written, failed} or {started}. Never deletes or hides.",
        photos_enabled, |s, p| {
            const C: &str = "curator.apply";
            let dir = curator_dir(s);
            let plan = find_plan(&dir, p, C)?;
            if plan.applied.is_some() {
                return Err(bad(C, format!("{} was applied already", plan.id)));
            }
            let (photos_n, per) = plan.totals();
            if p.get("confirm").and_then(Value::as_bool) != Some(true) {
                let mut v = plan.summary();
                v["wouldWrite"] = json!({"photos": photos_n, "keywords": per});
                v["confirmed"] = json!(false);
                return Ok(v);
            }
            if photos_n == 0 {
                return Err(bad(C, "nothing accepted: accept a group with curator.review first"));
            }
            let photos = s.photos.clone().ok_or_else(|| bad(C, super::photos::unavailable()))?;
            let wait = wait_param(s, p);
            start_job(s, "apply", "Curator: writing to Apple Photos", wait, move |cancel, progress| run_apply(&photos, &dir, plan, cancel, progress))
        }),
    ]
}

#[cfg(test)]
mod tests;
