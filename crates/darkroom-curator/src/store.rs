//! The Curator's store: one row per photo (Layer 1), the quick measures, albums, stacks, plans
//! and decisions. Keyed by the stable id (Apple's cloud identifier, else the local one).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// The store's format version; a newer file is refused rather than misread.
pub const VERSION: u32 = 1;

/// What PhotoKit says about one photo or video.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Photo {
    /// Stable id: the cloud identifier, else the local identifier.
    pub id: String,
    /// PhotoKit's local identifier on the Mac that scanned last (differs between Macs).
    pub local_id: String,
    /// "image", "video" or "other".
    pub media: String,
    pub width: u32,
    pub height: u32,
    /// ISO 8601, as PhotoKit gave it.
    pub created: Option<String>,
    pub modified: Option<String>,
    pub favorite: bool,
    pub hidden: bool,
    pub burst_id: Option<String>,
    /// Photos (or you) picked this frame of its burst.
    pub burst_pick: bool,
    pub live: bool,
    pub raw: bool,
    pub edited: bool,
    pub screenshot: bool,
    pub panorama: bool,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub filename: String,
    /// Seconds, videos only.
    pub duration: f64,
}

impl Photo {
    pub fn is_image(&self) -> bool {
        self.media == "image"
    }

    /// The capture year from `created` ("2025-03-14T…").
    pub fn year(&self) -> Option<i32> {
        self.created.as_deref().and_then(|c| c.get(..4)).and_then(|y| y.parse().ok())
    }

    /// Seconds since 1970 from `created`, for "same minute" checks. `None` when unparsable.
    pub fn time(&self) -> Option<i64> {
        self.created.as_deref().and_then(parse_iso_seconds)
    }
}

/// Darkroom's quick measures on a preview (the helper computes them on a grayscale copy).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Measures {
    /// Focus score 0..1 from the Laplacian variance (see [`Measures::sharp_from_variance`]).
    pub sharp: f32,
    /// The raw Laplacian variance on 0..255 gray at the measured size.
    pub lap_var: f32,
    /// Mean luminance 0..1.
    pub mean: f32,
    /// Share of pixels at or near black (<= 4/255) and white (>= 251/255).
    pub black: f32,
    pub white: f32,
    /// 64-bit difference hash, as 16 hex digits.
    pub dhash: String,
    /// The photo's `modified` when measured: a later edit measures again.
    pub modified: Option<String>,
}

impl Measures {
    /// Maps Laplacian variance (0..255 gray at 1024 px) to 0..1 on a log scale:
    /// `(log10(1 + v) - 0.5) / 2.5`, clamped. Calibrated on real frames and softened copies of
    /// them: sharp photos measure 500 to 1000 (0.9 to 1.0); the same frames softened 4x, 8x and
    /// 16x measure about 250, 85 and 20 (0.76, 0.57, 0.33). The default blur line, 0.35, sits near
    /// a variance of 24, so only frames softer than an 8x blur are proposed: a foggy, minimal
    /// frame has little detail by design, and the review decides the rest.
    pub fn sharp_from_variance(v: f32) -> f32 {
        if !v.is_finite() || v <= 0.0 {
            return 0.0;
        }
        (((1.0 + v).log10() - 0.5) / 2.5).clamp(0.0, 1.0)
    }

    pub fn hash(&self) -> Option<u64> {
        u64::from_str_radix(&self.dhash, 16).ok()
    }
}

/// A user album, with its folder path and members (stable ids).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub title: String,
    /// "Folder/Sub" (empty at the top level).
    pub folder: String,
    pub members: Vec<String>,
}

impl Album {
    pub fn path(&self) -> String {
        if self.folder.is_empty() { self.title.clone() } else { format!("{}/{}", self.folder, self.title) }
    }
}

/// One stack: the winner and the rest.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Stack {
    pub id: String,
    /// "burst", "duplicate" or "near".
    pub kind: String,
    pub winner: String,
    pub members: Vec<String>,
    /// Why the winner won, in words.
    pub why: String,
}

/// One proposed change and what you did with it: the record the taste model learns from.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Decision {
    /// ISO 8601 time of the apply.
    pub at: String,
    pub plan: String,
    pub job: String,
    pub rule: String,
    pub id: String,
    /// The keywords the plan would add.
    pub add: Vec<String>,
    /// true: applied; false: you took it out of the plan (an override).
    pub accepted: bool,
    /// Written to Photos (false when the write failed or the plan was only accepted).
    pub written: bool,
}

/// The whole store.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Store {
    pub version: u32,
    /// When the last scan finished (ISO 8601) and on which Mac's identifiers.
    pub scanned: Option<String>,
    pub photos: BTreeMap<String, Photo>,
    pub measures: BTreeMap<String, Measures>,
    pub albums: Vec<Album>,
    pub stacks: Vec<Stack>,
    pub decisions: Vec<Decision>,
    /// What the last scan could not read and fell back from, in words.
    pub notes: Vec<String>,
}

impl Store {
    pub const FILE: &'static str = "curator-store.json";

    pub fn new() -> Self {
        Self { version: VERSION, ..Default::default() }
    }

    /// Reads the store; a missing file is an empty store.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::new()),
            Err(e) => return Err(Error::Io(path.display().to_string(), e)),
        };
        let store: Store = serde_json::from_slice(&bytes).map_err(|e| Error::Format(path.display().to_string(), e.to_string()))?;
        if store.version > VERSION {
            return Err(Error::Format(path.display().to_string(), format!("written by a newer Darkroom (store version {})", store.version)));
        }
        Ok(store)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec(self).map_err(|e| Error::Format(path.display().to_string(), e.to_string()))?;
        crate::write_atomic(path, &bytes)
    }

    /// Replaces the photo rows with a fresh full listing. Rows for photos no longer in Photos are
    /// dropped with their measures (the listing is the whole library); returns (added, changed,
    /// removed).
    pub fn replace_photos(&mut self, fresh: Vec<Photo>) -> (usize, usize, usize) {
        let mut next = BTreeMap::new();
        let (mut added, mut changed) = (0, 0);
        for p in fresh {
            if p.id.is_empty() {
                continue;
            }
            match self.photos.get(&p.id) {
                None => added += 1,
                Some(old) if old != &p => changed += 1,
                _ => {}
            }
            next.insert(p.id.clone(), p);
        }
        let removed = self.photos.keys().filter(|k| !next.contains_key(*k)).count();
        self.measures.retain(|k, _| next.contains_key(k));
        self.photos = next;
        (added, changed, removed)
    }

    /// Images that have no measures yet, or were edited since they were measured.
    pub fn needs_measuring(&self) -> Vec<&Photo> {
        self.photos
            .values()
            .filter(|p| p.is_image())
            .filter(|p| match self.measures.get(&p.id) {
                None => true,
                Some(m) => m.modified != p.modified,
            })
            .collect()
    }

    /// The local id → stable id map of the photos in the store.
    pub fn by_local_id(&self) -> std::collections::HashMap<&str, &str> {
        self.photos.values().filter(|p| !p.local_id.is_empty()).map(|p| (p.local_id.as_str(), p.id.as_str())).collect()
    }

    /// Albums each photo is in (stable id → album indices).
    pub fn memberships(&self) -> std::collections::HashMap<&str, Vec<usize>> {
        let mut m: std::collections::HashMap<&str, Vec<usize>> = std::collections::HashMap::new();
        for (i, a) in self.albums.iter().enumerate() {
            for id in &a.members {
                m.entry(id.as_str()).or_default().push(i);
            }
        }
        m
    }
}

/// Seconds since 1970 for "YYYY-MM-DDTHH:MM:SS" with an optional zone ("Z", "+05:30", "-0600").
/// Good enough to compare two captures; not a calendar library.
pub fn parse_iso_seconds(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |a: usize, n: usize| -> Option<i64> { s.get(a..a + n)?.parse().ok() };
    if b.len() < 19 {
        return None;
    }
    let (y, mo, d, h, mi, se) = (num(0, 4)?, num(5, 2)?, num(8, 2)?, num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    // days from civil (Howard Hinnant's algorithm)
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = if yy >= 0 { yy } else { yy - 399 } / 400;
    let yoe = yy - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let mut t = days * 86_400 + h * 3600 + mi * 60 + se;
    // zone after the seconds (and any fraction)
    let mut i = 19;
    if b.get(i) == Some(&b'.') {
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    if let Some(&c @ (b'+' | b'-')) = b.get(i) {
        let sign = if c == b'+' { 1 } else { -1 };
        let rest = s.get(i + 1..)?.replace(':', "");
        let zh: i64 = rest.get(..2)?.parse().ok()?;
        let zm: i64 = rest.get(2..4).and_then(|v| v.parse().ok()).unwrap_or(0);
        t -= sign * (zh * 3600 + zm * 60);
    }
    Some(t)
}
