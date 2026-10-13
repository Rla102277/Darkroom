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

/// Installs every folder of cubes under the bundled dir whose group is not in the library yet.
/// Returns what was imported (empty when everything was there already or nothing is bundled).
pub fn install_bundled(s: &mut Session) -> Vec<Value> {
    let Some(dir) = bundled_dir() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut imported = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let folder = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let group = crate::preset_import::group_from_dir(&folder).unwrap_or_else(|| folder.clone());
        if s.lut_profiles.iter().any(|l| l.group.eq_ignore_ascii_case(&group)) {
            continue;
        }
        match s.execute("profile.import", &json!({"paths": [p.to_string_lossy()]})) {
            Ok(r) => {
                log::info!("looks: installed {folder}: {}", r["imported"].as_array().map_or(0, Vec::len));
                imported.push(json!({"folder": folder, "result": r}));
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
                        let group = crate::preset_import::group_from_dir(&folder).unwrap_or_else(|| folder.clone());
                        let installed = s.lut_profiles.iter().filter(|l| l.group.eq_ignore_ascii_case(&group)).count();
                        groups.push(json!({"folder": folder, "group": group, "cubes": cubes, "installed": installed}));
                    }
                }
                Ok(json!({"dir": dir.map(|d| d.to_string_lossy().to_string()), "groups": groups}))
            }),
        cmd!(query "looks.install", "Install Bundled Looks", [], None,
            "{} → {imported: [{folder, result}]} — installs the bundled Looks that are not in this library yet",
            always, |s, _| Ok(json!({"imported": install_bundled(s)}))),
    ]
}
