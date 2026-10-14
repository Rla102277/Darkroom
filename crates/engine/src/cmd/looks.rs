//! IA Darkroom: the bundled Infinite Arch Looks (`looks/` in the repo, `Contents/Resources/looks`
//! in the app), installed into the library's Profile browser on first open so every library has
//! them, and the `looks.*` commands.

use serde_json::{Value, json};

use super::{CommandSpec, always, cmd};
use crate::Session;

/// Where the bundled Looks are, first match wins: `$DARKROOM_LOOKS`, the app bundle's
/// `Contents/Resources/looks`, next to the executable, then the source tree's `looks/`.
pub fn bundled_dir() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("DARKROOM_LOOKS").filter(|p| !p.is_empty()) {
        let p = std::path::PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let mut candidates = vec![dir.join("../Resources/looks"), dir.join("looks")];
    for up in [2usize, 3] {
        let mut root = dir.to_path_buf();
        for _ in 0..up {
            root = root.parent()?.to_path_buf();
        }
        candidates.push(root.join("looks"));
    }
    candidates.into_iter().find(|p| p.is_dir())
}

/// The titles inside a folder's cubes (the name the Profile browser shows), or the file stems.
fn cube_titles(dir: &std::path::Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cube")) {
            continue;
        }
        let title = std::fs::read_to_string(&p)
            .ok()
            .and_then(|t| lightcraft_pipeline::lut::Lut3d::parse_cube(&t).ok())
            .and_then(|l| l.title)
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| p.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default());
        out.push(title);
    }
    out
}

/// Removes every imported profile whose name matches, in groups other than `keep_group`, and
/// extra copies inside `keep_group` beyond the first. Returns how many went.
pub fn remove_duplicates(s: &mut Session, titles: &[String], keep_group: &str) -> usize {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let doomed: Vec<String> = s
        .lut_profiles
        .iter()
        .filter(|l| titles.iter().any(|t| t == &l.name))
        .filter(|l| {
            if l.group.eq_ignore_ascii_case(keep_group) {
                !seen.insert(l.name.clone())
            } else {
                true
            }
        })
        .map(|l| l.id.clone())
        .collect();
    let n = doomed.len();
    for id in doomed {
        let _ = s.execute("profile.deleteImported", &json!({"id": id}));
    }
    n
}

/// Installs every folder of cubes under the bundled dir whose cubes are not in the library yet
/// (matched by the cube's title in the Profile browser, whatever group it sits in). Folders
/// starting with `_` are archives and are skipped. Returns what was imported.
pub fn install_bundled(s: &mut Session) -> Vec<Value> {
    let Some(dir) = bundled_dir() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut imported = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let folder = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !p.is_dir() || folder.starts_with('_') {
            continue;
        }
        let titles = cube_titles(&p);
        if titles.is_empty() || titles.iter().all(|t| s.lut_profiles.iter().any(|l| &l.name == t && l.group.eq_ignore_ascii_case(&folder))) {
            continue;
        }
        // older copies of the same Looks (an earlier version's group, or repeats) make way
        let removed = remove_duplicates(s, &titles, "");
        match s.execute("profile.import", &json!({"paths": [p.to_string_lossy()]})) {
            Ok(r) => {
                // the group is the folder's own name, not the shortened one the importer guesses
                let ids: Vec<String> = r["imported"].as_array().into_iter().flatten().filter_map(|x| x["id"].as_str().map(str::to_string)).collect();
                for l in s.lut_profiles.iter_mut().filter(|l| ids.contains(&l.id)) {
                    l.group = folder.clone();
                }
                let _ = s.save_prefs();
                log::info!("looks: installed {folder}: {} ({removed} older removed)", ids.len());
                imported.push(json!({"folder": folder, "installed": ids.len(), "removed": removed}));
            }
            Err(e) => log::warn!("looks: {folder}: {e}"),
        }
    }
    imported
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "looks.bundled", "Bundled Looks", [], None,
            "{} → {dir?, groups: [{folder, group, cubes, installed}]} — the Looks shipped with IA Darkroom and whether each group is in this library's Profile browser",
            always, |s, _| {
                let dir = bundled_dir();
                let mut groups = Vec::new();
                if let Some(d) = &dir && let Ok(rd) = std::fs::read_dir(d) {
                    for e in rd.flatten() {
                        let p = e.path();
                        if !p.is_dir() { continue; }
                        let folder = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        let cubes = std::fs::read_dir(&p).map(|r| r.flatten().filter(|f| f.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("cube"))).count()).unwrap_or(0);
                        let installed = s.lut_profiles.iter().filter(|l| l.group.eq_ignore_ascii_case(&folder)).count();
                        groups.push(json!({"folder": folder, "cubes": cubes, "installed": installed}));
                    }
                }
                Ok(json!({"dir": dir.map(|d| d.to_string_lossy().to_string()), "groups": groups}))
            }),
        cmd!(query "looks.install", "Install Bundled Looks", [], None,
            "{} → {imported: [{folder, installed, removed}]} — installs the bundled Looks that are not in this library yet, removing older copies of the same Looks",
            always, |s, _| Ok(json!({"imported": install_bundled(s)}))),
        cmd!(query "looks.cleanup", "Remove Duplicate Looks", [], None,
            "{} → {removed} — removes repeated imported Looks (same name in another group, or more than once in a group), keeping one copy in the bundled group",
            always, |s, _| {
                let names: Vec<String> = s.lut_profiles.iter().filter(|l| l.name.starts_with("IA ")).map(|l| l.name.clone()).collect();
                let removed = remove_duplicates(s, &names, "Infinite Arch Looks");
                Ok(json!({"removed": removed}))
            }),
    ]
}
