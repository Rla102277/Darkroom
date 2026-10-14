//! `curator.report`: the library in numbers. Every number comes from the store; the sentences
//! (Layer 3, C5) will be written from these, never the other way round.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::rules::Rules;
use crate::store::Store;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Overlap {
    pub a: String,
    pub b: String,
    pub shared: usize,
    /// Shared photos over the smaller album's size.
    pub share: f32,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub what: String,
    pub photos: usize,
    /// A rough minutes estimate for reviewing it.
    pub minutes: usize,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub scanned: Option<String>,
    pub total: usize,
    pub images: usize,
    pub videos: usize,
    pub hidden: usize,
    pub favorites: usize,
    pub edited: usize,
    pub raw: usize,
    pub live: usize,
    pub screenshots: usize,
    pub panoramas: usize,
    pub located: usize,
    pub by_year: BTreeMap<String, usize>,
    /// Camera family guessed from file names (IMG_ iPhone, L10 Leica, DSCF Fujifilm…), until the
    /// EXIF reader lands.
    pub by_device: BTreeMap<String, usize>,
    pub measured: usize,
    pub bursts: usize,
    pub burst_frames: usize,
    pub stacks: BTreeMap<String, usize>,
    pub stack_losers: usize,
    pub exact_duplicates: usize,
    pub blur: usize,
    pub black_or_blown: usize,
    pub screenshots_outside_documents: usize,
    pub albums: usize,
    pub empty_albums: usize,
    pub in_no_album: usize,
    pub overlaps: Vec<Overlap>,
    pub first: Vec<Step>,
    pub notes: Vec<String>,
}

/// The camera family a file name suggests. Coarse on purpose, and named as a guess.
pub fn device_from_filename(name: &str) -> &'static str {
    let n = name.to_ascii_uppercase();
    let has = |p: &str| n.starts_with(p);
    let digit_at = |i: usize| n.as_bytes().get(i).is_some_and(u8::is_ascii_digit);
    if has("IMG_") {
        return if n.ends_with(".PNG") { "Screenshot or graphic" } else { "iPhone" };
    }
    if has("L1") && digit_at(2) {
        return "Leica";
    }
    if has("DSCF") || has("_DSF") || has("XT0") || has("XT1") || has("XT5") || has("XXT") || has("GFX") || has("FUJIFILM") {
        return "Fujifilm";
    }
    if (has("P") && digit_at(1)) || n.ends_with(".ORF") {
        return "Olympus";
    }
    if has("DSC") || has("_DSC") || n.ends_with(".ARW") {
        return "Sony";
    }
    if n.ends_with(".CR2") || n.ends_with(".CR3") || has("_MG_") {
        return "Canon";
    }
    if n.ends_with(".NEF") {
        return "Nikon";
    }
    if has("SCREENSHOT") {
        return "Screenshot or graphic";
    }
    if n.ends_with(".MP4") || n.ends_with(".MOV") {
        return "Video, other";
    }
    "Other"
}

pub fn build(store: &Store, rules: &Rules) -> Report {
    let mut r = Report { scanned: store.scanned.clone(), notes: store.notes.clone(), ..Default::default() };
    let memberships = store.memberships();
    let mut hashes: HashMap<(u64, u32, u32), usize> = HashMap::new();
    for p in store.photos.values() {
        r.total += 1;
        match p.media.as_str() {
            "image" => r.images += 1,
            "video" => r.videos += 1,
            _ => {}
        }
        r.hidden += usize::from(p.hidden);
        r.favorites += usize::from(p.favorite);
        r.edited += usize::from(p.edited);
        r.raw += usize::from(p.raw);
        r.live += usize::from(p.live);
        r.screenshots += usize::from(p.screenshot);
        r.panoramas += usize::from(p.panorama);
        r.located += usize::from(p.lat.is_some());
        *r.by_year.entry(p.year().map_or("undated".into(), |y| y.to_string())).or_default() += 1;
        *r.by_device.entry(device_from_filename(&p.filename).into()).or_default() += 1;
        if !memberships.contains_key(p.id.as_str()) {
            r.in_no_album += 1;
        }
        if let Some(m) = store.measures.get(&p.id) {
            r.measured += 1;
            if p.is_image() && !p.favorite && !p.edited {
                if m.black > 0.8 && m.mean < 0.04 || m.white > 0.85 {
                    r.black_or_blown += 1;
                } else if m.sharp < rules.cull.blur && !p.panorama {
                    r.blur += 1;
                }
            }
            if let Some(h) = m.hash().filter(|_| m.lap_var >= 2.0) {
                *hashes.entry((h, p.width, p.height)).or_default() += 1;
            }
        }
        if p.screenshot {
            let in_docs = memberships
                .get(p.id.as_str())
                .is_some_and(|alb| alb.iter().any(|&i| store.albums.get(i).is_some_and(|a| rules.is_document_album(&a.path()))));
            if !in_docs {
                r.screenshots_outside_documents += 1;
            }
        }
    }
    r.exact_duplicates = hashes.values().filter(|&&c| c > 1).map(|c| c - 1).sum();
    let mut bursts: HashMap<&str, usize> = HashMap::new();
    for p in store.photos.values() {
        if let Some(b) = p.burst_id.as_deref().filter(|b| !b.is_empty()) {
            *bursts.entry(b).or_default() += 1;
        }
    }
    r.bursts = bursts.values().filter(|&&c| c > 1).count();
    r.burst_frames = bursts.values().filter(|&&c| c > 1).sum();
    for s in &store.stacks {
        *r.stacks.entry(s.kind.clone()).or_default() += 1;
        r.stack_losers += s.members.len().saturating_sub(1);
    }
    // albums
    r.albums = store.albums.len();
    r.empty_albums = store.albums.iter().filter(|a| a.members.is_empty()).count();
    let sets: Vec<HashSet<&str>> = store.albums.iter().map(|a| a.members.iter().map(String::as_str).collect()).collect();
    for (i, a) in store.albums.iter().enumerate() {
        for (j, b) in store.albums.iter().enumerate().skip(i + 1) {
            let (Some(sa), Some(sb)) = (sets.get(i), sets.get(j)) else { continue };
            let small = sa.len().min(sb.len());
            if small < 5 {
                continue;
            }
            let (x, y) = if sa.len() <= sb.len() { (sa, sb) } else { (sb, sa) };
            let shared = x.iter().filter(|id| y.contains(*id)).count();
            let share = shared as f32 / small as f32;
            if share >= 0.8 {
                r.overlaps.push(Overlap { a: a.path(), b: b.path(), shared, share });
            }
        }
    }
    r.overlaps.sort_by_key(|o| std::cmp::Reverse(o.shared));
    r.overlaps.truncate(50);
    // what it would do first: the cheapest big wins, about 2 s a frame to review
    let mins = |n: usize| (n * 2).div_ceil(60);
    let mut steps = vec![
        Step {
            what: "Stacks: one winner per burst and per duplicate".into(),
            photos: r.stack_losers.max(r.burst_frames.saturating_sub(r.bursts)),
            minutes: 0,
        },
        Step { what: "Cull black and blown frames".into(), photos: r.black_or_blown, minutes: 0 },
        Step { what: "Cull blur".into(), photos: r.blur, minutes: 0 },
        Step { what: "Screenshots out of the way".into(), photos: r.screenshots_outside_documents, minutes: 0 },
    ];
    for s in &mut steps {
        s.minutes = mins(s.photos);
    }
    steps.retain(|s| s.photos > 0);
    steps.sort_by_key(|s| std::cmp::Reverse(s.photos));
    r.first = steps;
    if r.measured < r.images {
        r.notes.push(format!(
            "{} of {} images have no quick measures yet: blur, duplicates and junk count only the measured ones",
            r.images - r.measured.min(r.images),
            r.images
        ));
    }
    r
}

impl Report {
    /// The report as plain text, for the CLI and the window.
    pub fn text(&self) -> String {
        let mut t = String::new();
        let pct = |a: usize, b: usize| if b == 0 { 0.0 } else { a as f64 * 100.0 / b as f64 };
        t.push_str(&format!(
            "{} items: {} photos, {} videos. {} favorites, {} edited, {} raw, {} hidden. {:.0}% have a location.\n",
            self.total,
            self.images,
            self.videos,
            self.favorites,
            self.edited,
            self.raw,
            self.hidden,
            pct(self.located, self.total)
        ));
        if let Some(s) = &self.scanned {
            t.push_str(&format!("Scanned {s}.\n"));
        }
        t.push_str("\nBy year\n");
        for (y, n) in &self.by_year {
            t.push_str(&format!("  {y}  {n}\n"));
        }
        t.push_str("\nBy camera (guessed from file names)\n");
        let mut dev: Vec<_> = self.by_device.iter().collect();
        dev.sort_by(|a, b| b.1.cmp(a.1));
        for (d, n) in dev {
            t.push_str(&format!("  {d}  {n}\n"));
        }
        t.push_str(&format!(
            "\nDuplicates and junk\n  {} bursts ({} frames)\n  {} exact duplicates\n  {} blurred, {} black or blown (of {} measured)\n  {} screenshots outside a documents album\n",
            self.bursts, self.burst_frames, self.exact_duplicates, self.blur, self.black_or_blown, self.measured, self.screenshots_outside_documents
        ));
        if !self.stacks.is_empty() {
            let parts: Vec<String> = self.stacks.iter().map(|(k, n)| format!("{n} {k}")).collect();
            t.push_str(&format!("  stacks: {}, {} frames under a winner\n", parts.join(", "), self.stack_losers));
        }
        t.push_str(&format!("\nAlbums\n  {} albums, {} empty, {} photos in no album\n", self.albums, self.empty_albums, self.in_no_album));
        for o in self.overlaps.iter().take(10) {
            t.push_str(&format!("  overlap: {} and {} share {} ({:.0}%)\n", o.a, o.b, o.shared, o.share * 100.0));
        }
        if !self.first.is_empty() {
            t.push_str("\nWhat I would do first\n");
            for (i, s) in self.first.iter().enumerate() {
                t.push_str(&format!("  {}. {}: {} photos, about {} min to review\n", i + 1, s.what, s.photos, s.minutes));
            }
        }
        for n in &self.notes {
            t.push_str(&format!("\nNote: {n}\n"));
        }
        t
    }
}
