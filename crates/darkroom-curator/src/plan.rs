//! A plan: what a job proposes, grouped for review. Review before write: a plan only reaches
//! Photos through `curator.apply`, only its accepted groups, and never what you took out.

use serde::{Deserialize, Serialize};

/// One proposed change for one photo.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    /// Stable id.
    pub id: String,
    /// 0..1: how sure the rule is.
    pub confidence: f32,
    /// One line on why, in words.
    pub reason: String,
    /// For stack losers: the frame that won.
    pub winner: Option<String>,
    /// You took it out of the plan.
    pub excluded: bool,
}

/// One rule's proposals, reviewed together.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Group {
    /// Stable key, e.g. "cull.blur".
    pub key: String,
    /// What the review sheet calls it: "Blur".
    pub label: String,
    /// The keywords applying it adds in Photos.
    pub add: Vec<String>,
    pub items: Vec<Item>,
    /// You accepted the group (only accepted groups are applied).
    pub accepted: bool,
}

impl Group {
    pub fn included(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|i| !i.excluded)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Plan {
    /// "<job>-<seconds>", also the file name.
    pub id: String,
    /// "stacks" or "cull".
    pub job: String,
    /// ISO 8601.
    pub created: String,
    pub groups: Vec<Group>,
    /// Set once applied (ISO 8601); an applied plan is kept as the record, not applied again.
    pub applied: Option<String>,
}

impl Plan {
    pub const DIR: &'static str = "curator-plans";

    /// What applying would do now: per keyword, how many photos; and the photo count.
    pub fn totals(&self) -> (usize, std::collections::BTreeMap<String, usize>) {
        let mut per: std::collections::BTreeMap<String, usize> = Default::default();
        let mut photos = std::collections::BTreeSet::new();
        for g in self.groups.iter().filter(|g| g.accepted) {
            for it in g.included() {
                photos.insert(it.id.as_str());
                for k in &g.add {
                    *per.entry(k.clone()).or_default() += 1;
                }
            }
        }
        (photos.len(), per)
    }

    /// The writes applying would make: one entry per photo with every keyword its accepted groups
    /// add (a photo in two groups gets one write).
    pub fn changes(&self) -> Vec<(String, Vec<String>)> {
        let mut by: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
        for g in self.groups.iter().filter(|g| g.accepted) {
            for it in g.included() {
                let e = by.entry(it.id.as_str()).or_default();
                for k in &g.add {
                    if !e.contains(k) {
                        e.push(k.clone());
                    }
                }
            }
        }
        by.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    /// A short summary for the window and the CLI.
    pub fn summary(&self) -> serde_json::Value {
        let (photos, per) = self.totals();
        serde_json::json!({
            "id": self.id,
            "job": self.job,
            "created": self.created,
            "applied": self.applied,
            "groups": self.groups.iter().map(|g| serde_json::json!({
                "key": g.key, "label": g.label, "add": g.add, "count": g.items.len(),
                "excluded": g.items.iter().filter(|i| i.excluded).count(), "accepted": g.accepted,
            })).collect::<Vec<_>>(),
            "toApply": {"photos": photos, "keywords": per},
        })
    }
}
