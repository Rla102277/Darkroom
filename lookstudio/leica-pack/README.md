# Infinite Arch Leica Looks v2.3.1

Nine Looks, all Standard base (D866=0). Three B&W first, then color, in menu order:

| ID | Look | What it is |
|---|---|---|
| 1001 | IA Invitation | B&W, soft, warm-silver toned. Anchored to the photographer's 103 Acros/Ilford scans: black 0.09, long silver scale |
| 1002 | IA Witness | B&W, red-filter density, hard toe (keeper) |
| 1003 | IA Vault | B&W, the Ansel Look. Orange-red filter, full Zone 0 to X scale, selenium-toned print |
| 1004 | IA Presence | Escura Edo 400 (keeper) |
| 1005 | IA Threshold | Meyerowitz dusk: pastel, luminous, soft blacks, pinks and pale blues |
| 1006 | IA Ember | Kodachrome-leaning slide: red-orange bias, restrained greens, cool shadows, hard S |
| 1007 | IA Sanctuary | Marix 800T at night, measured from 36 Natura Classica scans |
| 1008 | IA Kin | 400H (keeper, tweaked) |
| 1009 | IA Afterglow | the photographer's own ADOX/CineStill rolls, measured from 152 scans: warm sunny negative, lifted blacks |

Run ia_reload9_pyto.py in Pyto to remove any IA Looks already on the camera and install all nine (ia_final9_upload_pyto.py is the plain installer; it expects a clean camera). Self-contained. Join camera Wi-Fi, connect using FOTOS, force-quit FOTOS while remaining connected. uploader.py (folder version) also included. Both support --list.

BEFORE INSTALLING: remove every IA Look on the camera in FOTOS (same IDs, the camera will not overwrite in place). Not yet camera-verified. Previews and full build source in v2.3_Comparisons (looks_v23.py is the definition of truth). See CHANGELOG_v2.3.md and CHANGELOG_v2.3.1.md (icons). Installers also set camera JPEG Sharpness +1 (SET_SHARPNESS at the top of each script; None to skip).
