//! `curator.cull`: marks junk, each rule with a confidence and a reason, for review. Accepted
//! frames get `Pick: No` and `Kind: Junk` in Photos and stay visible.
//!
//! The rules (C1, Layer 1 signals only; Vision adds receipts, whiteboards and photos of screens
//! in C2):
//! - black frame: almost all of it at black;
//! - blown frame: almost all of it at white;
//! - pocket shot: dark and soft;
//! - blur: the focus score under `cull.blur`;
//! - screenshot: Photos says it is one, and it is not in a documents album;
//! - too many near-identical frames: past `cull.nearDupMax` in one near-duplicate stack.
//!
//! A photo goes into the first group that matches, in that order. Your favorites and edits are
//! never proposed, and neither are videos.

use crate::plan::{Group, Item, Plan};
use crate::rules::Rules;
use crate::store::Store;

pub const KIND_JUNK: &str = "Kind: Junk";

pub fn build(store: &Store, rules: &Rules, plan_id: &str, now: &str) -> Plan {
    let memberships = store.memberships();
    let in_documents = |id: &str| -> bool {
        memberships.get(id).is_some_and(|alb| alb.iter().any(|&i| store.albums.get(i).is_some_and(|a| rules.is_document_album(&a.path()))))
    };
    let blur = rules.cull.blur.max(0.0);
    let mut black = Vec::new();
    let mut white = Vec::new();
    let mut pocket = Vec::new();
    let mut soft = Vec::new();
    let mut shots = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for p in store.photos.values() {
        if !p.is_image() || p.favorite || p.edited {
            continue;
        }
        let item = |confidence: f32, reason: String| Item {
            id: p.id.clone(),
            confidence: confidence.clamp(0.0, 1.0),
            reason,
            winner: None,
            excluded: false,
        };
        if let Some(m) = store.measures.get(&p.id) {
            if m.mean < 0.04 && m.black > 0.8 {
                black.push(item(0.95, format!("black frame: {:.0}% of it at black", m.black * 100.0)));
                seen.insert(p.id.as_str());
                continue;
            }
            if m.white > 0.85 {
                white.push(item(0.9, format!("blown frame: {:.0}% of it at white", m.white * 100.0)));
                seen.insert(p.id.as_str());
                continue;
            }
            if m.mean < 0.15 && m.sharp < blur * 1.3 && !p.panorama {
                pocket.push(item(0.7, format!("dark (mean {:.0}%) and soft (focus {:.2}): a pocket or accidental frame", m.mean * 100.0, m.sharp)));
                seen.insert(p.id.as_str());
                continue;
            }
            if m.sharp < blur && !p.panorama {
                let c = if blur > 0.0 { 0.6 + 0.4 * (1.0 - m.sharp / blur) } else { 0.6 };
                soft.push(item(c, format!("focus score {:.2}, under the blur line {:.2}", m.sharp, blur)));
                seen.insert(p.id.as_str());
                continue;
            }
        }
        if p.screenshot && !in_documents(&p.id) {
            shots.push(item(0.85, "a screenshot, not in a documents album".into()));
            seen.insert(p.id.as_str());
        }
    }
    // too many near-identical frames: the stack's members are ranked best first
    let mut extras = Vec::new();
    let keep = rules.cull.near_dup_max.max(1);
    for s in store.stacks.iter().filter(|s| s.kind == "near") {
        let size = s.members.len();
        for (rank, id) in s.members.iter().enumerate().skip(keep) {
            let Some(p) = store.photos.get(id) else { continue };
            if p.favorite || p.edited || seen.contains(id.as_str()) {
                continue;
            }
            extras.push(Item {
                id: id.clone(),
                confidence: 0.7,
                reason: format!("frame {} of {size} near-identical ones (keeping the best {keep})", rank + 1),
                winner: Some(s.winner.clone()),
                excluded: false,
            });
        }
    }
    let group = |key: &str, label: &str, mut items: Vec<Item>| {
        // most certain first, so the review starts where it is easiest
        items.sort_by(|a, b| b.confidence.total_cmp(&a.confidence).then(a.id.cmp(&b.id)));
        Group { key: key.into(), label: label.into(), add: vec![crate::stacks::PICK_NO.into(), KIND_JUNK.into()], items, accepted: false }
    };
    Plan {
        id: plan_id.into(),
        job: "cull".into(),
        created: now.into(),
        groups: vec![
            group("cull.black", "Black frames", black),
            group("cull.white", "Blown frames", white),
            group("cull.pocket", "Pocket and accidental", pocket),
            group("cull.blur", "Blur", soft),
            group("cull.screenshot", "Screenshots", shots),
            group("cull.nearDupMax", "Too many near-identical", extras),
        ]
        .into_iter()
        .filter(|g| !g.items.is_empty())
        .collect(),
        applied: None,
    }
}
