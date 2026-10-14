//! `curator.stacks`: bursts, exact duplicates and near duplicates collapse into stacks with one
//! winner on top. In Photos, losers get the keyword `Pick: No` and nothing else.
//!
//! Relations (any of them joins two frames into one stack):
//! - the same burst (Apple's burst id);
//! - exact duplicates: the same pixel size, the same 64-bit difference hash and the same
//!   brightness and focus to within 0.2% (a photo imported twice);
//! - near duplicates: hashes within `stacks.nearBits` bits and taken within
//!   `stacks.nearSeconds` of each other. (C2 replaces this with Vision feature prints.)
//!
//! Featureless frames (black, white, flat) are left out of the hash relations: every black frame
//! hashes alike, and they are the cull's business, not duplicates.
//!
//! The winner: your favorite or edit always wins; then Photos' own burst pick; then the sharpest,
//! best exposed frame; the earliest breaks a tie.

use std::collections::HashMap;

use crate::plan::{Group, Item, Plan};
use crate::rules::Rules;
use crate::store::{Measures, Photo, Stack, Store};

pub const PICK_NO: &str = "Pick: No";

struct Uf(Vec<usize>);

impl Uf {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }
    fn find(&mut self, mut a: usize) -> usize {
        while let Some(&p) = self.0.get(a) {
            if p == a {
                break;
            }
            let gp = self.0.get(p).copied().unwrap_or(p);
            if let Some(slot) = self.0.get_mut(a) {
                *slot = gp;
            }
            a = p;
        }
        a
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb
            && let Some(slot) = self.0.get_mut(ra.max(rb))
        {
            *slot = ra.min(rb);
        }
    }
}

/// A frame with enough structure for its hash to mean something.
fn hashable(m: &Measures) -> Option<u64> {
    let h = m.hash()?;
    (m.lap_var >= 2.0 && m.black < 0.9 && m.white < 0.9 && h != 0 && h != u64::MAX).then_some(h)
}

/// How good a frame is to keep, and the word for why.
fn score(p: &Photo, m: Option<&Measures>) -> (f32, &'static str) {
    let mut s = 0.0;
    let mut why = "the sharpest, best exposed";
    if let Some(m) = m {
        s += m.sharp * 100.0 - (m.black + m.white) * 60.0;
    }
    if p.raw {
        s += 3.0;
    }
    if p.burst_pick {
        s += 40.0;
        why = "Photos' pick of the burst";
    }
    if p.edited {
        s += 500.0;
        why = "edited";
    }
    if p.favorite {
        s += 1000.0;
        why = "your favorite";
    }
    (s, why)
}

/// Builds the stacks and the review plan from the store.
pub fn build(store: &Store, rules: &Rules, plan_id: &str, now: &str) -> (Vec<Stack>, Plan) {
    let photos: Vec<&Photo> = store.photos.values().filter(|p| p.is_image()).collect();
    let n = photos.len();
    let mut uf = Uf::new(n);
    // kind of the strongest relation that joined each pair: 2 burst, 1 duplicate, 0 near
    let mut strength = vec![0u8; n];
    let mut joined = vec![false; n];
    let link = |uf: &mut Uf, strength: &mut Vec<u8>, joined: &mut Vec<bool>, a: usize, b: usize, k: u8| {
        uf.union(a, b);
        for i in [a, b] {
            if let Some(s) = strength.get_mut(i) {
                *s = (*s).max(k);
            }
            if let Some(j) = joined.get_mut(i) {
                *j = true;
            }
        }
    };
    // bursts
    let mut by_burst: HashMap<&str, usize> = HashMap::new();
    for (i, p) in photos.iter().enumerate() {
        if let Some(b) = p.burst_id.as_deref().filter(|b| !b.is_empty()) {
            match by_burst.get(b) {
                Some(&first) => link(&mut uf, &mut strength, &mut joined, first, i, 2),
                None => {
                    by_burst.insert(b, i);
                }
            }
        }
    }
    // exact duplicates
    let measures: Vec<Option<&Measures>> = photos.iter().map(|p| store.measures.get(&p.id)).collect();
    let mut by_hash: HashMap<(u64, u32, u32), Vec<usize>> = HashMap::new();
    for (i, p) in photos.iter().enumerate() {
        if let Some(h) = measures.get(i).copied().flatten().and_then(hashable) {
            by_hash.entry((h, p.width, p.height)).or_default().push(i);
        }
    }
    for idx in by_hash.values() {
        for w in idx.windows(2) {
            let (Some(&a), Some(&b)) = (w.first(), w.get(1)) else { continue };
            let (Some(Some(ma)), Some(Some(mb))) = (measures.get(a), measures.get(b)) else { continue };
            // a photo imported twice measures the same; two frames of a moment do not
            if (ma.mean - mb.mean).abs() < 0.002 && (ma.sharp - mb.sharp).abs() < 0.002 {
                link(&mut uf, &mut strength, &mut joined, a, b, 1);
            }
        }
    }
    // near duplicates: sorted by time, compare within the window
    let mut timed: Vec<(i64, usize, u64)> =
        photos.iter().enumerate().filter_map(|(i, p)| Some((p.time()?, i, measures.get(i).copied().flatten().and_then(hashable)?))).collect();
    timed.sort_unstable();
    for (k, &(t, i, h)) in timed.iter().enumerate() {
        for &(t2, j, h2) in timed.iter().skip(k + 1) {
            if t2 - t > rules.stacks.near_seconds {
                break;
            }
            if crate::hamming(h, h2) <= rules.stacks.near_bits {
                link(&mut uf, &mut strength, &mut joined, i, j, 0);
            }
        }
    }
    // collect
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        if joined.get(i).copied().unwrap_or(false) {
            let r = uf.find(i);
            groups.entry(r).or_default().push(i);
        }
    }
    let mut roots: Vec<usize> = groups.keys().copied().collect();
    roots.sort_unstable();
    let mut stacks = Vec::new();
    let (mut burst, mut dup, mut near) = (Vec::new(), Vec::new(), Vec::new());
    for r in roots {
        let Some(members) = groups.get(&r) else { continue };
        if members.len() < 2 {
            continue;
        }
        let kind = match members.iter().filter_map(|&i| strength.get(i)).max() {
            Some(2) => "burst",
            Some(1) => "duplicate",
            _ => "near",
        };
        let mut ranked: Vec<(f32, i64, &str, usize, &'static str)> = members
            .iter()
            .filter_map(|&i| {
                let p = photos.get(i)?;
                let (s, why) = score(p, measures.get(i).copied().flatten());
                Some((s, p.time().unwrap_or(i64::MAX), p.id.as_str(), i, why))
            })
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
        let Some(&(_, _, winner_id, wi, why)) = ranked.first() else { continue };
        let winner_name = photos.get(wi).map(|p| p.filename.as_str()).unwrap_or("");
        let id = format!("{kind}-{winner_id}");
        stacks.push(Stack {
            id,
            kind: kind.into(),
            winner: winner_id.to_string(),
            members: ranked.iter().map(|r| r.2.to_string()).collect(),
            why: format!("{} won: {why}", if winner_name.is_empty() { winner_id } else { winner_name }),
        });
        let size = ranked.len();
        for &(_, _, id, i, _) in ranked.iter().skip(1) {
            let Some(p) = photos.get(i) else { continue };
            // a favorite or an edit is never proposed as a loser
            if p.favorite || p.edited {
                continue;
            }
            let (confidence, label, bucket) = match kind {
                "burst" => (0.9, "burst", &mut burst),
                "duplicate" => (0.98, "exact duplicate", &mut dup),
                _ => (0.75, "near duplicate", &mut near),
            };
            bucket.push(Item {
                id: id.to_string(),
                confidence,
                reason: format!("{label} in a stack of {size}; {} won ({why})", if winner_name.is_empty() { "another frame" } else { winner_name }),
                winner: Some(winner_id.to_string()),
                excluded: false,
            });
        }
    }
    let group =
        |key: &str, label: &str, items: Vec<Item>| Group { key: key.into(), label: label.into(), add: vec![PICK_NO.into()], items, accepted: false };
    let plan = Plan {
        id: plan_id.into(),
        job: "stacks".into(),
        created: now.into(),
        groups: vec![
            group("stacks.burst", "Burst losers", burst),
            group("stacks.duplicate", "Exact duplicates", dup),
            group("stacks.near", "Near duplicates", near),
        ]
        .into_iter()
        .filter(|g| !g.items.is_empty())
        .collect(),
        applied: None,
    };
    (stacks, plan)
}
