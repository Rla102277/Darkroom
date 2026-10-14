# Look Studio (sources)

The Infinite Arch Looks build system, as it stands on 2026-10-01, so IA Darkroom's Look Studio
grows from the real thing rather than a copy of it.

- `engine/`: the generator. `engine.py` (DCP film deltas, OKLab shaping, tone, 17-point compile,
  CUBE writer, metrics), `looks_v2.py` (v2.1 definitions), `looks_v22.py` (the mid-slope pass),
  `looks_v23_color.py`, `looks_v23_bw.py`, `looks_v23.py` (v2.3, the definition of truth:
  `python looks_v23.py` regenerates the nine cubes into out23/). `natura_amneg_defs.py` carries the
  Natura and American Negative builds. Needs numpy, scipy, matplotlib; the DCP-derived Looks also
  need the licensed M9 DCPs, which are NOT in this repo (private; see the Film Lab project).
- `leica-pack/`: Infinite_Arch_Leica_Looks_v2.3.1 as shipped: looks/<key>/look.CUBE + look.bmp,
  manifest with sha256, the Pyto installer and reloader, analysis, changelogs.

The cubes IA Darkroom installs live in `../looks/Infinite Arch Looks/` and are byte-identical to
`leica-pack/looks/*/look.CUBE`. v1.2 (the first camera-working set) is kept in
`../looks/_archive/v1.2/` as regression fixtures.

Rules carried over from the Studio: Look names are Infinite Arch themes, never film names; never
hand-edit a cube; minimum pairwise color distance 0.15 between color Looks; build at full
commitment, the Amount slider is the user's control.

## `studio/`: the Film Look Studio (merged from Replit)

`lookstudio/studio/` is the Replit project (GitHub: Rla102277/FilmSimulationPhotoLab) copied in on
2026-10-13 from its main branch: the FastAPI + React Look app, the Leica payload compiler and
validator (`core/leica`), the color graph (`core/color`), the Fuji recipe tools, the v1.2
authoritative reference and working copy, the confirmed Python sessions and packet-capture
tools, the research notes, screenshots and docs. Its palette and layout are what IA Darkroom's
theme and Look Studio workspace follow.

Left out on purpose: `exports/` (bundle zips), `attached_assets/` (Git LFS pointers without the
objects), `zipFile.zip` and the LFS `.gitattributes`. The packet captures in `codex_outputs/`
(about 50 MB) are kept because they are the FOTOS protocol evidence.

The Replit repo stays the place the studio app is edited; copy its changes here with
`git archive main | tar -x -C lookstudio/studio` from a clone of it.
Run the studio app from `lookstudio/studio` with `bash scripts/run.sh` (its own README).
