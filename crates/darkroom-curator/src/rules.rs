//! `curator-rules.json`: the thresholds and names the jobs use, shared by both Macs through the
//! library folder. Every field has a default, so a partial file (or none) works.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Home {
    pub lat: f64,
    pub lon: f64,
    pub radius_km: f64,
}

impl Default for Home {
    fn default() -> Self {
        // Frisco, TX
        Self { lat: 33.15, lon: -96.82, radius_km: 50.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Trips {
    pub gap_hours: f64,
    pub jump_km: f64,
    pub prefix: String,
}

impl Default for Trips {
    fn default() -> Self {
        Self { gap_hours: 36.0, jump_km: 150.0, prefix: "01_".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Cull {
    /// Focus score (0..1, [`crate::Measures::sharp`]) below which a frame counts as blurred.
    pub blur: f32,
    /// More near-identical frames than this in one stack: the extras are junk.
    pub near_dup_max: usize,
    /// Hide accepted junk in Photos (off: only the keywords).
    pub hide_junk: bool,
    /// Album titles (or folder names) that hold documents on purpose: screenshots and documents
    /// in them are never culled.
    pub document_albums: Vec<String>,
}

impl Default for Cull {
    fn default() -> Self {
        Self { blur: 0.35, near_dup_max: 4, hide_junk: false, document_albums: vec!["Documents".into(), "Receipts".into(), "Screenshots".into()] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct StackRules {
    /// Two frames are near duplicates when their hashes differ in at most this many bits...
    pub near_bits: u32,
    /// ...and they were taken at most this many seconds apart.
    pub near_seconds: i64,
}

impl Default for StackRules {
    fn default() -> Self {
        Self { near_bits: 6, near_seconds: 60 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Rules {
    pub home: Home,
    pub trips: Trips,
    pub kinds: Vec<String>,
    pub cull: Cull,
    pub stacks: StackRules,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            home: Home::default(),
            trips: Trips::default(),
            kinds: [
                "Landscape",
                "City and street",
                "Arches and thresholds",
                "People",
                "Events and performances",
                "Travel snapshot",
                "Documents",
                "Food",
                "Pets",
                "Junk",
            ]
            .map(String::from)
            .to_vec(),
            cull: Cull::default(),
            stacks: StackRules::default(),
        }
    }
}

impl Rules {
    pub const FILE: &'static str = "curator-rules.json";

    /// Reads the rules; a missing file gives the defaults. Unknown fields (the spec's later
    /// sections: memories, grade, judgment) are kept out of the way, not refused.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| Error::Format(path.display().to_string(), e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::Io(path.display().to_string(), e)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| Error::Format(path.display().to_string(), e.to_string()))?;
        crate::write_atomic(path, &bytes)
    }

    /// Whether an album path names a documents album.
    pub fn is_document_album(&self, path: &str) -> bool {
        let p = path.to_lowercase();
        self.cull.document_albums.iter().any(|d| !d.is_empty() && p.split('/').any(|part| part.trim() == d.to_lowercase()))
    }
}
