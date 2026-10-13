# Milestone 2: the print proof

Goal: one color sheet and one Advanced B&W sheet, both through the native Epson driver, with
every setting sent from Darkroom and none touched in a dialog. When both look right, the print
path is proven and the full Print workspace is built on it.

Everything below runs from the terminal with `lightcraft-cli` (the same engine as the app,
headless). Build it once:

```sh
cd ~/Developer/darkroom
cargo build --release -p lightcraft-cli
alias dr='~/Developer/darkroom/target/release/lightcraft-cli run --demo'
```

## 1. Which queue, which driver

```sh
dr print.queues
```

Expect the P900 with `"epsonDriver": true`. If the only P900 queue says `"airPrint": true`,
Darkroom will refuse to print to it: in System Settings > Printers & Scanners, remove it, add
the printer again, and in the "Use" menu pick **EPSON SC-P900 Series** (Epson's driver; install it
from epson.com if it is not offered), not AirPrint.

## 2. What the driver exposes

```sh
dr print.options queue=P900 > ~/Desktop/p900-options.json
```

This is the list I need: send me `p900-options.json`. It names the keys for media type, print
quality, color mode (the "Off (No Color Management)" choice), Advanced B&W and its tone
controls, High Speed, Finest Detail, Black Enhance Overcoat, paper source and size. The names
vary by driver version, which is why Darkroom reads them instead of guessing.

## 3. Paper profiles on this Mac

```sh
dr print.profiles
```

Lists every installed ICC profile with its description. Red River's P900 profiles for Ultra Pro
Luster, Big Bend Baryta 310 and Polar Gloss Metallic 255 should be here once installed.

## 4. The color sheet

Letter size, the paper's profile, the driver's color management off, media type and quality set.
The option keys and values come from step 2; these are placeholders until I see the list:

```sh
dr print.proof queue=P900 sheet=letter \
  profile="/Library/ColorSync/Profiles/<Red River P900 UltraPro Luster>.icc" \
  '{"options": {"<MediaTypeKey>": "<LusterValue>", "<QualityKey>": "<MaxQuality>", "<ColorModeKey>": "<Off>"}}'
```

What to look for: the 21-step wedge separates at both ends, the smooth ramp has no bands, the
mid-gray field has no cast, skin and sky patches look like skin and sky, the finest bars are
distinct.

## 5. The Advanced B&W sheet

Same target, no profile (the printer maps the tones), driver put into Advanced B&W Photo:

```sh
dr print.proof queue=P900 sheet=letter \
  '{"options": {"<MediaTypeKey>": "<LusterValue>", "<QualityKey>": "<MaxQuality>", "<PrintModeKey>": "<AdvancedBW>", "<ToneKey>": "<Neutral>"}}'
```

What to look for: neutral neutrals, the 1% shadow steps still separate, nothing clipped at the
top.

## 6. Reprint anything

`print.target` writes the TIFF without sending it (`~/Library/Application Support/Darkroom/print/`);
`print.submit` sends any file with any options; `print.jobs` shows the queue.

## What this proves, and what comes next

Proven here: native driver only, app-managed color, Advanced B&W driven from the app, 16-bit all
the way. Next: the Print workspace in the window (papers with saved driver presets, layouts, the
signature sheet, soft proof, test strips, the log), built on exactly these calls.
