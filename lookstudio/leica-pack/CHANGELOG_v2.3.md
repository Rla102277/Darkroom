# Infinite Arch Leica Looks v2.3 — S-curves, Kodachrome Ember, toned B&W, Sharpness +1

Same nine Looks, IDs, names, icons and Standard base. Owner's picks 2026-09-29: Ember K, Invitation B, Witness A, Vault B.

## Color: real S-curves (all six)
v2.2 raised the mid slope but left Threshold, Kin and Afterglow with a toe steeper than the mids (1.05 to 1.12 against 1.05 to 1.10), which reads flat next to Leica Brass (0.92 / 1.26 / 0.61). v2.3 shapes each Look with a pivot-power S around its own midpoint (slope to zero at the ends, endpoints pinned to the Look's black and white), then solves the mid gain. Hue and chroma untouched.

| Look | toe / mid / shoulder |
|---|---|
| Presence | 0.71 / 1.22 / 0.69 |
| Threshold | 0.95 / 1.16 / 0.67 (softest by intent) |
| Ember | 0.55 / 1.42 / 0.90 (hardest, slide) |
| Sanctuary | 0.69 / 1.25 / 0.92 |
| Kin | 0.91 / 1.24 / 0.66 |
| Afterglow | 0.89 / 1.22 / 0.56 |

Threshold pinks and pale blues nudged up, Kin greens nudged toward 400H, so the pair sits at 0.169. Set minimum 0.157 (Presence/Sanctuary). All monotone, second difference under 0.15.

## Ember is now Ember K
Kodachrome-leaning. No Kodachrome DCP exists; the base is Ektachrome 64 + E100VS + Ektar 25 tables with the Kodachrome signature applied by hand: red-orange bias, restrained greens, cool neutral shadows, no magenta bloom. Same tone as the Fortia/Velvia Ember (0.55 / 1.42 / 0.90). For the record, the Leica Steve McCurry Look (captured 2026-09-29) is not Kodachrome: it is Standard with a 1.25 mid slope and +7 percent chroma, zero hue work.

## B&W: toned, like every Leica mono Look
Leica ships no neutral B&W Look (Blue is cool throughout, Selenium violet in the shadows with neutral whites, Greg Williams faintly warm). Tone magnitudes here are 50 to 60 percent of Leica Selenium.
- Invitation B: warm silver in the mids (OKLab b +0.010), neutral whites. Soft Acros/Ilford scale unchanged (black 0.09, mid slope 1.05).
- Witness A: neutral, unchanged from v2.2.
- Vault B, the Ansel Look: orange-red filter mix 0.52/0.46/0.02, full Zone 0 to X scale (0.00, 0.03, 0.07, 0.12, 0.17, 0.27, 0.45, 0.68, 0.85, 0.93, 1.00), texture held at Zones II-III (toe 0.83) and VII-VIII (shoulder 0.73), selenium tone (cool violet-black in the shadows and lower mids, neutral highlights).
Invitation and Vault are flagged TONED BW in the manifest (mono false) because their cubes carry a tint; base stays Standard.

## Installer
Both installers set camera JPEG Sharpness +1 after the nine verify (SetDevicePropValue 0x5015 = 4, read back and printed). SET_SHARPNESS = None at the top of either script leaves the camera alone. This is global and applies under every Look; per-Look sharpness is not possible (see probe/).

## probe/
Two tap-to-run scripts that settled the per-Look adjustment question: a downloadable Look answers 0xA801 for D869 to D86D (built-in styles only) but accepts intensity (D868) through 0x903F. Handles are table record markers. Keep for reference; not needed to install.

Both installers nine PASS with identical embedded assets. Not camera-verified. BEFORE INSTALLING: remove the nine v2.2 IA Looks in FOTOS (same IDs).
