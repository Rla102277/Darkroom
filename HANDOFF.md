# IA Darkroom: handoff for the next session

Read this first, before CLAUDE.md. CLAUDE.md is LightCraft's upstream agent guide and still applies
to the engine code (never crash, no unsafe, assets rules, how to run and test), but the product is
not LightCraft any more. It is **IA Darkroom**, Randy Archambault's own photo app (The Infinite
Arch), built on a private fork of LightCraft. Upstream parity is not the goal; Randy's workflow is.

## The person and how to work with him

- Randy Archambault, Frisco TX. Fine art photographer (theinfinitearch.com). Director of Data at
  his day job, so he is technical, but he is not here to read Rust: he wants working software.
- Voice: direct, casual, first person. No AI filler. **No em dashes, ever** (use a colon, a comma,
  or a new sentence). ALL CAPS in his messages is emphasis, not anger.
- He wants progress updates while long things run, so he does not have to ask.
- Ask before taking over his screen. Never put his Lightroom API key or any key in chat or in a
  commit; keys go in the macOS Keychain.
- When he says "GO", build; do not come back with a plan to approve.

## Hard rules (never break)

1. **Native Epson driver only.** Never AirPrint. The helper refuses AirPrint queues.
2. **Native B&W is first class**: Epson Advanced B&W Photo with every driver control.
3. **Nothing in Apple Photos is ever deleted by the app.** Hide, favorite, keyword, album: yes.
   Delete: never. Never write inside the Photos library package.
4. **Nothing is called LightCraft** in anything the user sees (window, menus, bundle, docs for him).
   Crate names and upstream internals keep their names so upstream merges stay cheap.
5. **Review before write**: anything that changes Photos or uses paper shows a count and a preview.
6. Apple Photos is the system of record. IA Darkroom reads it and writes decisions back.

## The Mac

- MacBook Pro "Whopr" (M1 Max). A second Mac is "identical" (same iCloud Photos library).
- Repo: `~/Developer/darkroom`, remote `origin` = github.com/Rla102277/Darkroom (private).
  Branches `main` and `darkroom` carry our work and are kept equal; `upstream-b3d392b` is the
  untouched LightCraft base.
- Build and install: `scripts/build-ia-darkroom.sh` (sources `~/.cargo/env` itself) puts
  `/Applications/IA Darkroom.app` together: app binary, `lightcraft-cli`, the Swift helper in
  `Contents/Helpers`, bundled Looks in `Contents/Resources/looks`, icon, Info.plist, ad hoc signed
  with `packaging/macos/entitlements.plist` (apple-events + photos-library). A Desktop launcher
  "Build IA Darkroom.command" does pull + build + log.
- Apple Photos library: about 65k assets, 652 albums. Photos access for the bundled helper needs
  the photos-library entitlement (already in the plist); after a signing change run
  `tccutil reset Photos` and allow "IA Darkroom" when asked.
- Printer: Epson SureColor P900, CUPS queue `EPSON_SC_P900_Series__IP_` (native `rastertoescpII`).
  Papers: Red River UltraPro Luster (daily), Big Bend Baryta 310, Polar Gloss Metallic 255, and
  the rest in `crates/darkroom-print/src/p900.rs`. Red River P900 ICC profiles are installed.
  `~/Desktop/p900-options.json` is the driver option dump the P900 module is built from.
- Cameras: Leica Q3 (DNG), Fuji GFX 50R (RAF Bayer), Fuji X-E5 (RAF X-Trans), Sony, Olympus.
  Panoramics are in-camera 65:24 crops. Velvia is a valid look for him, never flag it.
- Library (catalog) lives on iCloud Drive:
  `~/Library/Mobile Documents/com~apple~CloudDocs/Documents/Projects/Photos/IA Darkroom/IA Darkroom Library`.
  Linked originals are per Mac in `~/Library/Application Support/Darkroom/originals/`; the link
  table `apple-photos.json` is keyed by Apple's cloud identifier so both Macs agree.

## Where things are

| What | Where |
|---|---|
| Spec v2 (organize, import, print, Curator) | Claude doc "Infinite Arch Darkroom: Spec v2" (claude.ai artifact 41b28dec-4b40-43b6-a2d9-144211e6c3dd), tab "Spec v2" |
| The Curator spec | `CURATOR.md` (same text as the doc's Curator section) |
| Build, library, first run, printing, write-back | `DARKROOM.md` |
| Proof print steps | `PRINT-PROOF.md` |
| Swift helper (PhotoKit, osascript, CUPS) | `helper/darkroom-helper/Sources/darkroom-helper/main.swift` |
| Rust client for the helper | `crates/darkroom-photos/src/lib.rs` |
| `photos.*` commands, link table, push | `crates/engine/src/cmd/photos.rs` |
| Apple Photos sidebar, background linking | `crates/ui-egui/src/apple_photos.rs` (`LinkJob`, `tick`) |
| Print engine: targets, profiles, P900 options, sheets | `crates/darkroom-print/src/{lib,p900,sheet}.rs` |
| `print.*` commands | `crates/engine/src/cmd/print.rs` |
| Print Sheet dialog (Cmd+P) | `crates/ui-egui/src/print_dialog.rs` |
| Looks: install, dedupe | `crates/engine/src/cmd/looks.rs`; cubes in `looks/Infinite Arch Looks/` |
| Look Studio (merged from Replit, not wired into the UI) | `lookstudio/` (engine py files, the v2.3.1 pack, the studio web app) |
| Camera local tone for every raw | `crates/engine/src/camera_local_tone.rs` |
| Olympus compressed ORF | `crates/raw/src/vendor/orf.rs` (`decode_compressed`) |
| Theme (Darkroom default) | `crates/ui-egui/src/theme.rs` |
| Icon | `assets/app-icon/ia-darkroom.*`, generator `packaging/ia-darkroom-icon.py` |

Commit style: author Randy Archambault <rla1022@gmail.com>, plain messages that say what changed
and why, pushed to both `main` and `darkroom`.

## What works today (as of Oct 14, 2026)

- The app builds, installs, runs as IA Darkroom with the Darkroom theme and the new icon.
- Apple Photos section lists folders and albums (cached tree, instant on launch, refresh behind).
  Clicking an album links it: originals fetched on a worker in chunks of 20, added to the catalog
  40 at a time between frames, keywords/titles/captions/favorites carried over, catalog album
  "Photos: <name>". Nothing in Photos changes.
- File > Push Decisions to Apple Photos: picks become favorites + `Pick: Select`, rejects
  `Pick: No`, keywords/titles/captions written back.
- Printing: `print.queues/options/profiles/papers/target/proof/sheet/submit/jobs`; Cmd+P Print
  Sheet dialog (paper, size, quality, color via paper profile or Advanced B&W with every control,
  signature strip "Randy Archambault · The Infinite Arch", save TIFF instead). Native driver only.
- Looks: the nine v2.3 Infinite Arch Looks are bundled and install into one group; apply from the
  Profile browser.
- Camera local tone mapping for every raw with a camera JPEG and a tone curve (checkbox under
  Profile in Edit). Olympus compressed ORF decodes.

## Unverified (built without the Mac; test first)

- The Olympus compressed ORF decoder: open a real ORF. Noise or a diagonal smear means the header
  skip or the escape path; the tests only round-trip my own encoder.
- The synthesized camera local tone map: open a RAF and a Q3 DNG, toggle the checkbox. Patchy
  brightness or a visible grid means the cell smoothing needs more; no checkbox on the Q3 means
  the DNG has no tone curve tag (add a fallback curve in `camera_local_tone.rs`).
- No proof print has gone to paper yet: `PRINT-PROOF.md` steps 4 and 5, then one real sheet.

## What is next, in order

1. Verify the three items above on the Mac.
2. The Curator, per `CURATOR.md`, starting at C0 (the `curator.sqlite` store, `curator.scan`
   Layer 1, `curator.report`), then C1 stacks and cull. The helper gets the Vision / Core ML work
   (Layer 2); Claude is Layer 3 and needs his API key in the Keychain.
3. Pixelmator / Photomator style layout pass on the interface (today it is LightCraft's layout
   in Darkroom colors; the look he wants is the Replit studio's, in `lookstudio/studio/`).
4. Look Studio as a workspace inside the app.
5. Live grading keys that push to Photos as pressed (today: grade, then Push).
6. Roll paper in the Print Sheet dialog (the command takes `rollWidth` already).
7. Load his Adobe DCPs as camera color for RAF (the GFX 50R and X-E5 have no measured matrices;
   the JPEG fit stands in). He has the zip locally; it is not in any repo.

## Things he still owes the project

The current mentor/curator instruction document (Layer 3 prompt), the list of named pieces and
series from the curator chats, the folder where his Lightroom/Photoshop finals export, the People
names as Photos spells them, macOS version on both Macs, his API key typed into the app once it
has the field, the DCP zip, one raw from each body.
