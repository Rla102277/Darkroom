# IA Darkroom

The Infinite Arch's photo app: Apple Photos organizing, a working catalog, and printing to the
Epson P900 through the native driver. A private fork of the open-source LightCraft code (github.com/storytold/lightcraft)
(branch `upstream-b3d392b` is the untouched base; `darkroom` and `main` carry our work).

The spec lives in the Darkroom doc (Spec v2). Our code stays in new crates and one new folder so
pulling upstream updates stays cheap:

| Where | What |
|---|---|
| `helper/darkroom-helper/` | The Swift Mac helper: PhotoKit, Photos' scripting interface, later the print system |
| `crates/darkroom-photos/` | Rust client for the helper |
| `crates/engine/src/cmd/photos.rs` | `photos.*` commands and the link table |
| `crates/ui-egui/src/apple_photos.rs` | The Apple Photos sidebar section |

## Build on the Mac

Once, on each Mac (Xcode command line tools, Rust, the code):

```sh
xcode-select --install
curl https://sh.rustup.rs -sSf | sh
git clone https://github.com/Rla102277/darkroom ~/Developer/darkroom
```

Then one command builds everything and installs **IA Darkroom.app** into /Applications:

```sh
cd ~/Developer/darkroom && git pull origin main && scripts/build-ia-darkroom.sh
```

The first build takes 10 to 20 minutes on an M-series Mac; after that a minute or two. The app
carries the Swift helper (Contents/Helpers), the command-line tool and the bundled Looks, so it is
self-contained. It is ad hoc signed: the first launch is right-click > Open.

For development, `cargo run --release -p lightcraft` runs the app straight from the tree (it finds
the helper in `helper/darkroom-helper/.build/release/` and the Looks in `looks/`).

## The library

The library lives on iCloud Drive when iCloud Drive exists:
`iCloud Drive/Documents/Projects/Photos/IA Darkroom/IA Darkroom Library`, so both Macs open the
same catalog. Linked originals are not in it: each Mac fetches its own copies into
`~/Library/Application Support/Darkroom/originals/`, and the link table is keyed by Apple's cloud
identifier, which is the same everywhere. Open a different folder with File > Open Library or
`--library DIR`.

## First run

1. The window opens as Darkroom with an **Apple Photos** section at the top of the left sidebar.
2. The first time the section reads your library, macOS asks whether Darkroom Helper may access
   Photos. Allow full access (System Settings > Privacy & Security > Photos if the prompt was missed).
3. Click an album in the section. Darkroom fetches the originals once into
   `~/Library/Application Support/Darkroom/originals/`, adds them to the catalog in place, copies
   Photos' keywords, titles and captions, turns favorites into picks, and opens the result as a
   catalog album named `Photos: <album>`. Nothing in Photos changes.

The link table is `apple-photos.json` in the library folder, keyed by Apple's cloud identifier,
so the same photo is recognized on the other Mac.

## Try the helper on its own

```sh
H=helper/darkroom-helper/.build/release/darkroom-helper
$H status
$H albums | head -c 2000
$H assets '{"limit": 3}'
$H keywords '{"ids": ["<an id from assets>"]}'
```

## Printing

File > Print Sheet… (Cmd+P) prints the selected photo as the signature sheet (13x19, 0.25 inch
margins, 0.4 inch strip with the caption) or a plain sheet, on any paper the P900 driver lists, in
color (IA Darkroom converts into the paper's Red River profile, the driver's color is off) or in
Advanced B&W Photo with every one of the driver's B&W controls. Native Epson driver only; an
AirPrint queue is refused. The same thing from the terminal is `print.sheet`; see PRINT-PROOF.md
for the proof targets and the driver option names.

## Camera local tone mapping, for every raw

LightCraft renders Apple ProRAW's per-region gain map when the Profile section's "Camera local
tone mapping" is on. IA Darkroom extends that to every raw with an embedded camera JPEG: the
global camera look is fitted to the JPEG as before, and what the look cannot explain at low
spatial frequency (Fuji D-Range Priority, Leica's rendering, Sony DRO, Canon ALO, Nikon ADL) is
measured and stored as a gain map of the same shape (`crates/engine/src/camera_local_tone.rs`:
16 × 12 cells, 17 brightness points each). Off is the plain raw, as Lightroom Classic shows it;
on is what the camera showed. The option is per photo and part of the develop settings.

## Write-back to Photos

File > Push Decisions to Apple Photos: picks become favorites plus the keyword `Pick: Select`,
rejects get `Pick: No`, and keywords, titles and captions go back to the linked Photos items.
Nothing is hidden or deleted by IA Darkroom.

## What is not here yet

- Linking runs on the window's thread: a big album makes the window busy the first time.
- Grading keys that write back live (today: grade, then Push).
- The Look Studio workspace (the Looks apply from the Profile browser today).
- Roll paper from the dialog (the command takes `rollWidth`; the dialog does not yet).
