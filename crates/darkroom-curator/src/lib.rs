//! The Curator, the part of IA Darkroom that does not need macOS (`CURATOR.md`).
//!
//! The helper (Swift, PhotoKit) reads Apple Photos; this crate keeps what it read and decides
//! what to propose. Everything here is plain data in, plain data out, so it is tested without a
//! Photos library:
//!
//! - [`store`]: the per-photo signals (Layer 1: PhotoKit's facts and Darkroom's quick measures on
//!   a preview), album membership, the stacks, the plans and every decision, keyed by Apple's
//!   cloud identifier so both Macs agree. Saved as JSON in the library folder, written to a
//!   temporary file and renamed into place, so a crash never leaves half a store. (The spec says
//!   `curator.sqlite`; SQLite would be the product's only C dependency, so the store is JSON like
//!   the catalog.)
//! - [`rules`]: `curator-rules.json`, the thresholds and names the jobs use.
//! - [`report`]: the library in numbers.
//! - [`stacks`] and [`cull`]: the C1 jobs. Each produces a [`plan::Plan`] to review; nothing is
//!   written to Photos until a plan is accepted and applied.
//!
//! The Curator never deletes. The strongest change a plan can carry is a keyword.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod cull;
pub mod plan;
pub mod report;
pub mod rules;
pub mod stacks;
pub mod store;

pub use plan::{Group, Item, Plan};
pub use rules::Rules;
pub use store::{Album, Decision, Measures, Photo, Store};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(String, std::io::Error),
    #[error("{0} is not a Curator file Darkroom understands: {1}")]
    Format(String, String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Hamming distance between two 64-bit perceptual hashes.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Writes `bytes` to `path` crash-safely: a temporary file next to it, flushed, then renamed over.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let name = |p: &std::path::Path| p.display().to_string();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::Io(name(dir), e))?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let mut f = std::fs::File::create(&tmp).map_err(|e| Error::Io(name(&tmp), e))?;
    f.write_all(bytes).map_err(|e| Error::Io(name(&tmp), e))?;
    f.sync_all().map_err(|e| Error::Io(name(&tmp), e))?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        Error::Io(name(path), e)
    })
}

#[cfg(test)]
mod tests;
