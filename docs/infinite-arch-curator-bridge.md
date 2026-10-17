# THE INFINITE ARCH: Curator bridge for IA Darkroom

**Status:** Proposed integration contract. No existing policy, photograph, Apple Photos asset, or curation verdict is changed by this document.  
**Darkroom source branch:** `main` at `53a64c15916f5e0c5a32dda1bfee70dce7d29799`, 17 October 2026.  
**External governance:** [Rla102277/theinfinitearch](https://github.com/Rla102277/theinfinitearch), `governance/` and `curation/`, 16 October snapshot `a4a8fa5`.

## Decision: extend the existing IA Darkroom Curator, do not build another culling app

The external repository's proposed `apps/arch-cull/` SwiftUI application should be treated as **a requirements archive**, not a second production app. This repository already contains:

| Existing code | Current evidenced responsibility |
|---|---|
| `helper/darkroom-helper/Sources/darkroom-helper/main.swift` | PhotoKit read, thumbnails, cloud ID translation, selected write operations and measurements |
| `crates/darkroom-photos/` | Strongly typed Rust client for Swift helper; PhotoKit cloud/local IDs |
| `crates/engine/src/cmd/photos.rs` | `photos.*`, per-Mac linked resource caches, catalog mapping, write-back |
| `crates/darkroom-curator/` | JSON store, rules, simple signals, stack and cull plans |
| `crates/engine/src/cmd/curator.rs` | `curator.scan/report/stacks/cull/status/plan/review/apply`, background workers |
| `crates/ui-egui/src/apple_photos.rs` | Photos album sidebar, linking and thumbnails |
| `crates/catalog/` and `crates/ui-egui/` | Existing working catalog, navigation, comparisons and review primitives |
| `crates/mcp/` | Existing agent command surface |

**Do not rewrite the Rust UI in SwiftUI.** Extend Rust `darkroom-curator`, reuse the established Swift helper, commands, egui and catalog. Current editor/print implementations already exist; the curation initiative does not authorize expanding them or modifying photo pixels.

### Source hierarchy and meanings

1. **Apple Photos**: photographic assets and native Photos metadata remain the record of the photographs. No direct writes into `.photoslibrary`. A linked original in `Application Support/Darkroom/originals` is a per-machine cache, not a new authoritative original.
2. **`theinfinitearch/governance/`**: versioned artistic protocol, Master KB and Curatorial Decision Log. Its Decision Log v1.0 is labeled **WORKING RECONSTRUCTION overall** with individually confirmed, working and disputed entries. Preserve both statuses.
3. **`theinfinitearch/curation/`**: dated, independently observed curation evidence, run history, contact-sheet verdicts and provisional selections.
4. **Darkroom Curator local store**: feature extraction, linked identities, ranking candidates, AI proposals, review history and pending plans; never silently edits or declares external governance.

The external repository is read as versioned, **pinned inputs**, not as a mutable online dependency. Imports must work offline from a local checkout or user-selected snapshot, record commit+hash+source path and run read-only. Syncing approved new decisions back into `theinfinitearch` requires a deliberate export/review workflow, never unattended rewriting.

## Source conflicts: preserve, do not resolve automatically

**Grades.** `CURATOR.md` (C5) currently proposes `S/A/B/C` and refers to “ten dimensions”, while its text lists eleven. The later Unified Protocol v1.1 explicitly requires eleven dimensions and **S, A+, A, A−, B+, B, C**. Do not use `S/A/B/C` for new canonical judgments; historical values remain readable and labeled as earlier schema. A final grade is holistic, not an arithmetic average.

**Classifications.** Exhibition Anchor, Exhibition Support, Developmental Work, Personal Archive, Exclude are separate from grades, cull decisions, and candidate priority.

**Chapters.** Protocol v1.1 has Thresholds / Companions / Witnesses / Elsewhere / Becoming; the 15–16 Oct repository operational structure also includes `6 Keeping` explicitly **provisional**. Retain these distinct statuses rather than converting Keeping into a sixth approved canonical chapter.

**Titles.** Locked, sold and submitted titles must not be changed automatically. The sold work is `Where Trolls Go to Watch the Sun Die`, not the shortened Protocol example. `Where the Blue Comes From` vs `The Azure Vein` is disputed. Proposed print-finish title pairs remain unresolved. A title suggestion is never a title write.

**Styles.** Inherited guidance may treat empty space, interiority or subdued light as absolute rules. Under Protocol v1.1 they are evidence-based diagnostics, **not grade ceilings**. A conventional-looking frame may still be artistically important; film grain or Fuji Velvia is not intrinsically bad.

## Already-curated image history is NOT an empty starting point

The 16 October `curation/verdicts/chapters/chapters-status-2026-10-16.md` reports:

- 4,893 real-camera no-face frames in the wide sweep; 1,309 **Maybe** additions: Elsewhere 581, Thresholds 335, Witnesses 283, Companions 106, Becoming 4. Elsewhere intentionally needs a stricter cut.
- The sweep excluded film and about **900 RAF/ARW files with camera model `?`**. Missing camera metadata must never remove an image from discovery.
- Film: 339 identified scans, 128 Maybe; Japan film: 68 imports, 48 reported unique + 20 duplicates, 23 Maybe.
- Finals Print album: 429 live files representing about 110 works; 37 satisfy a real-title/white-border print-ready heuristic. Tagging/moving those 37 was *proposed but not performed*. **Print-ready does not mean artistically confirmed A**.
- The existing chapter structure has `1 Tiered`, `2 Maybe`, `3 No`, and keywords `Ch:` and `Prov:`. Provisional labels must remain provisional.
- A set of Air-only pick maps and `ch*_decisions.json` files is missing from Git. Compact selections such as `X####` and `wp_compact.txt` cannot safely be resolved to photo UUIDs without their original maps.

Some older album names were visually proven false. GPS and capture date also contain documented errors, especially scans and clocks. Visual contact-sheet review and evidence-backed reconciliation must always outrank metadata-only inferences.

## Required modules and API additions

### C2a. Governance and evidence importer

Add new `darkroom-curator` module(s) behind an importer interface:

- `SourceSnapshot`: repository, commit SHA, file path, content SHA-256, import time, declared status.
- `ExternalAssetReference`: source machine, namespace, legacy UUID/pick ID/title, any original mapping files.
- `Reconciliation`: source ref -> PhotoKit cloud ID/local ID, evidence signals and `RESOLVED_VERIFIED | AMBIGUOUS | UNRESOLVED | NOT_FOUND`.
- `ExternalDecision`: source entry, status, tier, classification, chapter, title, evidence and confirmation provenance.
- `CuratorialEvent`: append-only human decisions and supersession links, source/model/policy versions.
- `ImportIssue`: unmapped `X####`, conflicting titles, broken source hash, bad date, conflicting chapter labels and unavailable asset.

Use deterministic fixture-tested parsing and import summaries; no invented UUID joins. Historical IDs may differ across Macs. PhotoKit cloud IDs are useful when present and resolvable, but must not be the sole identity evidence; fall back to unresolved cases, never silent matches. Retain legacy `curator-store.json` compatibility and make additive changes with schema-version tests.

**Cross-Mac concurrency:** the existing curator store lives on iCloud Drive and uses atomic JSON replacement. Atomic rename protects from torn writes, **not two Macs overwriting each other's state**. Add writer/lease discipline or an append-only mergeable event journal before storing newly confirmed decisions across machines. Backup before migrations, crash tests and conflict tests mandatory. Do not replace the store with SQLite merely because a prior document mentioned `curator.sqlite`; the code currently uses JSON, and this workspace's pure-Rust requirement must be respected.

### C2b. Read-only review and comparisons

Use the existing `curator.scan`, `curator.stacks` and `curator.cull` infrastructure, but separate `TechnicalWarning`, `DuplicateCandidate`, `PersonalArchiveCandidate`, `FineArtCandidate` and `CuratorialDecision`.

The current `cull.rs` labels black, white, blurry or near-identical frames as potential `Pick: No`/`Kind: Junk`. That is risky for intentionally dark, overexposed, soft film or alternate renderings. Do **not** allow that heuristic to become a silent final grade, exclusion or automatic Apple Photos write. Add provenance protection for approved work, original variants, edited/favorite images, film/RAW companion groups, and human-protected assets. Keep the current behavior inspectable until a tested alternative replaces it.

`curator.stacks` must distinguish true duplicates, photographic bursts, alternates, revisit scenes and artistically meaningful variations. Never discard every frame except the technically sharpest. Keep exact/perceptual/semantic similarity distinct.

### C3. Local image intelligence

Extend Swift helper in independent, tested read-only commands for supported Vision/Core ML features: image feature prints, classification, saliency, face presence, validated aesthetic signals. Avoid unverified exact model throughput claims or equating MPS acceleration to Neural Engine use. Use thumbnails without downloading all iCloud originals; preserve unavailable/error state and checkpointing. Support 75k+ assets, cancellation, resumability, bounded resources, and incremental invalidation by model/version.

### C5. Full fine-art critic and chapters

Implement the full eleven dimensions: composition, light, visual weight, emotional impact, symbolism, storytelling, printability, gallery appeal, commercial appeal, originality, longevity. Distinguish four perspectives: museum curator, gallery owner, national juror, collector; four roles: Scholar, Photographer, Critic, Great Curator. Grade recommendation and chapter/collection classification are independently versioned hypotheses with clear evidence and possible abstention.

Never force roads, arches, fog, frames-within-frames or `Ma` into artificial rules. Respect photographic medium and source rendering; do not identify Fujifilm recipes or Leica Looks by palette alone. Preload previously approved pieces as protected anchors but do not train exclusively on familiar motifs: **Hidden Gems** must search deliberately for new visual registers.

Cloud judgment may be an **optional, user-triggered**, cost-previewed adapter; no asset upload by default. Unknown image provenance and screenshots require an appropriate warning. Keep true model results distinguishable from simulated ones.

### C6. Decisions and Apple Photos write safety

`curator.plan` and `curator.review` remain the safe approval workflow. Before invoking `curator.apply` or `photos.push`, show **exact asset counts, target IDs, changes, conflicts, protected titles, keywords, favorites and failure handling**. Existing `photos.push` has default `favorites=true`, `keywords=true`, `titles=true`; a future curator adapter must **explicitly opt out of title and favorite changes unless independently approved**. Never call write-back as a side effect of scanning or grading.

No delete, original editing, forced favorites, automatic `Where…` title writes, or silent edition/grade mutations. Any supported Apple Photos write must use tested public APIs or the existing explicitly reviewed scripting bridge, with audit/idempotence/verification.

## Engineering gates

1. **Source fixtures:** import at least one confirmed CDL entry, one disputed title, one 16 Oct Maybe, one missing Air pick ID and one provisional Keeping assignment without status corruption; test a re-import idempotently.
2. **Identity:** test correct cloud/local match, missing cloud mapping, cross-Mac legacy UUID mismatch and two plausible original assets. Ambiguous matches never write.
3. **Review:** confirm a protected A+ photo and personal memory cannot be marked `Kind: Junk` or `Pick: No` merely because of blur or darkness.
4. **Art criticism:** eleven 1–10 dimensions and exact S-to-C tiers; no single arithmetic mean; AI cannot overwrite confirmed verdict; no title lock violation.
5. **Write safety:** scanning/searching/critique make zero Photos mutations; `apply` requires explicit reviewed confirmation and ignores missing/unmapped targets. Ensure favorites/titles remain unchanged when not explicitly approved.
6. **Concurrent state:** two-machine state changes do not silently erase decisions; an interrupted process can resume.
7. **Performance:** measure real indexed counts, memory, throughput and errors rather than trusting the projected durations in `CURATOR.md`.

## Work order, within this fork

- **First:** audit current `curator.*` and helper commands and validate tests, without any library mutations.
- **Second:** add migration-safe source importer and case fixtures from the external repo, with missing-map and ambiguous-ID handling.
- **Third:** harden read-only culling/stacking decisions and protect previously confirmed work.
- **Fourth:** add Versioned Vision/semantic features, then full eleven-dimension critique, five confirmed chapters plus provisional Keeping, and Hidden Gems.
- **Last:** integrate documented explicit Photos write approval and report/exports. Do not expand the editor or printer while implementing culling.

**Build gates:** respect `AGENTS.md`, no unsafe or panics, `cargo xtask ci` and documented macOS Swift helper builds where available. The repository's **default branch is `upstream-b3d392b`**, an untouched upstream baseline. Features and PRs must explicitly target `main` (and synchronize `darkroom` under the owner's established workflow), **never accidentally target upstream baseline**.
