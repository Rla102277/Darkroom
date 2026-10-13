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

Done on Oct 13: the dump is what `crates/darkroom-print/src/p900.rs` is built from (media types
including the Red River 19xx entries, `EPIJ_CMat=3` for color off, `EPIJ_Ink_=3` + `ColorModel=ABWP`
for Advanced B&W, the quality, feed, roll and size keys). Re-run it after a driver update.

## 3. Paper profiles on this Mac

```sh
dr print.profiles
```

Lists every installed ICC profile with its description. Red River's P900 profiles for Ultra Pro
Luster, Big Bend Baryta 310 and Polar Gloss Metallic 255 should be here once installed.

## 4. The color sheet

The driver's option names are known now (the `print.options` dump of Oct 13), so a paper id is
all it takes. Letter size on UltraPro Luster, the paper's Red River P900 profile found
automatically, driver color off, Max Quality, Black Enhance Overcoat on:

```sh
dr print.papers                                   # the paper ids and which profiles are installed
dr print.proof paper=ultrapro-luster size=letter
```

What to look for: the 21-step wedge separates at both ends, the smooth ramp has no bands, the
mid-gray field has no cast, skin and sky patches look like skin and sky, the finest bars are
distinct. If the sheet comes out plainly wrong (strong cast, very dark), the driver's color was
not off: send me the `options` block the command printed.

## 5. The Advanced B&W sheet

Same target, no profile (the printer maps the tones), Advanced B&W Photo, neutral, tone Dark:

```sh
dr print.proof paper=ultrapro-luster size=letter '{"abw": {"toning": "neutral", "tone": "dark"}}'
```

Every B&W control is available: `toning` neutral|cool|warm|sepia, `tone` darkest|darker|dark|normal|light,
`brightness`, `contrast`, `shadowTonality`, `highlightTonality` (-25..25), `maxOpticalDensity`
(-50..0), `highlightPointShift` off|on|fullPage, `wheelX`/`wheelY` (-75..75).

What to look for: neutral neutrals, the 1% shadow steps still separate, nothing clipped at the
top.

## 6. Reprint anything

`print.target` writes the TIFF without sending it (`~/Library/Application Support/Darkroom/print/`);
`print.submit` sends any file with any options; `print.jobs` shows the queue.

## What this proves, and what comes next

Proven here: native driver only, app-managed color, Advanced B&W driven from the app, 16-bit all
the way. Next: the Print workspace in the window (papers with saved driver presets, layouts, the
signature sheet, soft proof, test strips, the log), built on exactly these calls.
