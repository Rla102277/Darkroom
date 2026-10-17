# Cursor kickoff: extend IA Darkroom's existing Curator

Copy the following prompt into Cursor Agent with the **Darkroom** repository open. Use branch `main` as the base, not the repository's upstream-default branch.

---

You are extending **IA Darkroom**, an existing Rust + egui photographic application with a Swift PhotoKit helper and an already implemented Curator C0/C1 layer. **Do not start a new app, do not create a SwiftUI shell, do not fork the existing photos catalog, and do not change the editor or printer.** Apple Photos remains the system of record. Our goal is a resilient, explainable, non-destructive culling and portfolio intelligence system for THE INFINITE ARCH's approximately 75k+ photos.

## Start from proven source

1. Run `git status`, `git branch -vv`, `git log -5 --oneline`. The repository default `upstream-b3d392b` is a frozen LightCraft base; work is on `main` and `darkroom`. Never submit the feature against the upstream baseline.
2. Read `HANDOFF.md`, `DARKROOM.md`, `CURATOR.md`, `AGENTS.md` and **`docs/infinite-arch-curator-bridge.md`**. Inspect actual code, particularly `crates/darkroom-curator/`, `crates/engine/src/cmd/curator.rs`, `crates/engine/src/cmd/photos.rs`, `crates/darkroom-photos/`, `helper/darkroom-helper/`, `crates/ui-egui/src/apple_photos.rs`, `crates/catalog/` and `crates/mcp/`.
3. Mount or clone the sibling `Rla102277/theinfinitearch` repository as a **read-only, pinned research source**; ingest versioned `governance/` and `curation/` files. Locate the Unified Protocol v1.1, Master Knowledge Base, Curatorial Decision Log, 16 Oct chapter status, v3 summary, verified album evidence and missing mapping warnings.
4. Write a concise real-code capability matrix: implemented, partially implemented, specified only, unverifiable. C0/C1 already exist. Document actual function boundaries and preserved artifact locations. Do not assume `CURATOR.md` is an implementation report. Confirm `curator-store.json`, not its older `curator.sqlite` promise.

## Non-negotiable policy

- **Exact seven grades:** S, A+, A, A−, B+, B, C. No collapsing S/A/B/C.
- **Eleven dimensions:** composition, light, visual weight, emotional impact, symbolism, storytelling, printability, gallery appeal, commercial appeal, originality, longevity.
- **Four roles** and **four analytical perspectives**, as written in Protocol v1.1.
- **Five working chapters** Thresholds, Companions, Witnesses, Elsewhere, Becoming and a distinct **provisional** sixth Keeping in operational history.
- **Five separate curatorial classifications** Exhibition Anchor, Exhibition Support, Developmental Work, Personal Archive, Exclude.
- A proposed AI grade is not a human-confirmed verdict. A provisional `Maybe`/ `Prov:` or one of the 13 Elsewhere A-check candidates is not a locked A. Preserve the Decision Log's top-level WORKING RECONSTRUCTION status along with individually confirmed records.
- Aesthetic confidence cannot be inferred from CLIP similarity, Apple scores or a made-up numeric value. Model may abstain. Grade holistic, not arithmetic mean.
- Visual invitation, meaningful Ma, subtraction and interiority are questions, **not formulaic grade ceilings**. Film grain, Velvia, dark moods and intentional softness are not automatically bad. Do not fabricate Look identity from palette or confirm unverified symbolic readings.
- **No auto-title changes**, no rewriting locked editions, no grade escalation from print-ready flags. The sold title `Where Trolls Go to Watch the Sun Die` is preserved even though Protocol example omits “Go to”.
- **Do not touch photos, hidden/favorite state, keywords or existing albums during the first phase**. The existing `photos.push` defaults to favorites/titles/keywords=true and is dangerous if called from a new agent without explicit overrides. No production writes without preview/confirmation/identity verification/idempotent audit.

## Integration architecture and review data

Create a new, isolated `darkroom-curator` governance/evidence ingestion layer; it must use **versioned source snapshots** from sibling Git, including commit SHA, path, SHA-256 and source-status. Add import record types for source photo references, mapped Photos identities, human decisions, AI proposals, import warnings, review events and conflict states. Keep `curator-store.json` migration-safe; create backups and schema-version tests. No dependency on SQLite is required or authorized merely because old documentation mentioned it. Pure Rust policies and AGENTS.md remain in force.

Map legacy Air/Pro IDs only with verified evidence. Prefer PhotoKit `PHCloudIdentifier` mapping when actually available and confirmed; never assume a legacy SQLite UUID is equivalent to a live local ID. A missing `X_pick.json`, `*_pick.json` or `ch*_decisions.json` makes the associated compact pick ID **UNRESOLVED**, not guessed. PhotoKit cloud IDs can be absent or unmappable. Preserve source tokens, duplicate evidence, and conflicts for manual reconciliation.

Import these **historical facts** as metadata/proposals, not as approved new decisions:
- The 16 Oct wide sweep: 4,893 inspected, 1,309 Maybes split Elsewhere 581 / Thresholds 335 / Witnesses 283 / Companions 106 / Becoming 4; Elsewhere needs a hard edit.
- Film scans: 339, 128 Maybe; Japan film 68 imported / 48 unique / 20 duplicate copies, 23 Maybe.
- Camera-tag-unknown blind spot: around 900 RAF/ARW files plus film scans missed. Do not filter `camera == ?`.
- Finals Print: 429 files / about 110 distinct works / 37 meeting the title+white border readiness rule; tagging those 37 was PROPOSED, not executed.
- Historical trip-location and capture-date errors must be flagged, not silently rewritten.
- `Maybe`, `1 Tiered`, `3 No`, `Ch:`, `Prov:`, locked titles and confirmed grades carry different semantics.

Avoid a second authoritative catalog. Darkroom may cache linked originals per Mac, but PhotoKit assets remain the authoritative originals. For **75k-photo inventory**, read metadata/1024px previews through the helper without fetching every full-size original. Do not make an unbounded download request.

## Implementation slices: build, do not just write docs

**Slice A: Safety and provenance (implement first).**
- Add typed import of source snapshots, recognized decision record statuses, unresolved IDs and conflicting statuses to the `darkroom-curator` module.
- Add fixtures for one CONFIRMED A+, one DISPUTED title, one Maybe, one unknown RAW camera, one provisional Keeping, one unmapped Air pick, and a two-Mac ID conflict.
- Expose a read-only `curator.sources` or equivalent engine command returning source counts, source version and unresolved issues.
- Ensure no invoked command changes Photos during import. Respect the established unified command system, so CLI/MCP and UI see the same read-only result.
- Make imports repeatable/idempotent; backups and tests.

**Slice B: Safer compare and culling.**
- Separate `TechnicalWarning` from `RejectRecommendation` and `HumanExclusion`.
- Protect approved S/A/portfolio works, film scans, alternate exports and personal archives. Do not treat black/white/soft frame alone as Junk.
- Compare RAW+JPEG resources, scans and alternate compositions without declaring duplicates disposable.
- Preserve human overrides, capture timestamp source, missing camera and cloud availability.
- Profile candidate similarity for tens of thousands of images; avoid O(N²).

**Slice C: Local vision and art intelligence.**
- Add properly capability-gated Vision feature prints, scene labels and saliency as needed, not fake Apple Neural Engine or invented timing claims.
- Introduce versioned embeddings and local semantic search, preserving unknown/unavailable items.
- Implement full eleven-dimension critique with four lenses, evidence vs interpretation, explicit grade tier, uncertainty, chapter hypothesis and curatorial classification.
- Implement nonconformity search: Hidden Gems outside already learned motifs.
- Cloud language-model adapter is off by default; requires explicit selection and cost/privacy confirmation.

**Slice D: Portfolio and controlled Photos sync.**
- Chapter board, sequence comparison, confirmed/provisional filter, audit and reversible review events.
- Address two-Mac conflict: current atomic `curator-store.json` writes don't prevent simultaneous lost updates on an iCloud shared file; implement safe concurrency before approved decisions synchronize.
- Any proposed Apple Photos write must have an itemized preview/dry run, explicit user consent, safe bounded batching, outcome verification, no deletion and no silent favorites/titles write.
- Export reviewed decision additions as source-linked proposals to theinfinitearch. Do not mutate canonical governance in place.

## Engineering constraints / completion

Follow existing `AGENTS.md`: no panics/unsafe, command registry is authoritative, test real code, document tests and failures. Run `cargo xtask ci` when practical; on macOS additionally build Swift helper and test PhotoKit authorization with a **small, non-destructive** sample. CI/fixtures on another OS do not prove Photos integration works. Do not assume current installed app is updated until it has actually been built and launched on a Mac.

Every slice: implement -> test -> evaluate read-only safety -> record real counts/limitations -> commit a small scoped change to a branch targeting `main`. Never automatically push to `upstream-b3d392b`. Be precise about what is actually coded vs specified vs verified.

**BEGIN NOW:** perform code inspection and implement Slice A through passing tests. Do not stop at an architecture document if the execution environment supports coding.

---