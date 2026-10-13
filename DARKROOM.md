# Darkroom

The Infinite Arch's photo app: Apple Photos organizing, a working catalog, and printing to the
Epson P900 through the native driver. A private fork of [LightCraft](https://github.com/storytold/lightcraft)
(branch `upstream-b3d392b` is the untouched base; `darkroom` and `main` carry our work).

The spec lives in the Darkroom doc (Spec v2). Our code stays in new crates and one new folder so
pulling LightCraft's updates stays cheap:

| Where | What |
|---|---|
| `helper/darkroom-helper/` | The Swift Mac helper: PhotoKit, Photos' scripting interface, later the print system |
| `crates/darkroom-photos/` | Rust client for the helper |
| `crates/engine/src/cmd/photos.rs` | `photos.*` commands and the link table |
| `crates/ui-egui/src/apple_photos.rs` | The Apple Photos sidebar section |

## Build on the Mac

Once, on each Mac:

```sh
xcode-select --install            # Swift and the macOS SDK (skip if Xcode is installed)
curl https://sh.rustup.rs -sSf | sh   # Rust, then open a new terminal
git clone https://github.com/Rla102277/darkroom ~/Developer/darkroom
```

Then, every time:

```sh
cd ~/Developer/darkroom
(cd helper/darkroom-helper && swift build -c release)
cargo run --release -p lightcraft
```

The first `cargo run` compiles for a while (10 to 20 minutes on an M-series Mac); after that it
is seconds. The app looks for the helper at `helper/darkroom-helper/.build/release/darkroom-helper`
(or `$DARKROOM_HELPER`), so build the helper before running the app.

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

## What is not here yet

- Writing back from the grid (favorites, hidden, keywords) is wired as commands
  (`photos.set`, `photos.setKeywords`, `photos.addToAlbum`) but has no buttons yet.
- Linking runs on the window's thread: a big album makes the window busy the first time.
- Printing: next milestone.
