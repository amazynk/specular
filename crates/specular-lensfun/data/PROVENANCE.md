# Provenance of the Lensfun lens database

- What: Lensfun's lens database, the XML files of its `data/db` directory (56 files: mounts,
  cameras, lenses and their distortion, TCA and vignetting calibrations). Data only.
- Source: <https://github.com/lensfun/lensfun>, commit
  `52cf1a7b0e82e07e0ecaa3391ee5106a6dd1ee54` (master, 2026-11-04, "Add Nikkor Z 24-70mm f/2.8
  S II"), fetched 2026-11-06 by `tools/fetch-lensfun.sh`, which checks every file's SHA-256
  (listed in the script) and `data/COPYING.CC_BY-SA_3.0`.
- Licence: Creative Commons Attribution-ShareAlike 3.0 Unported (CC BY-SA 3.0),
  <https://creativecommons.org/licenses/by-sa/3.0/>; the text is `COPYING.CC_BY-SA_3.0` here.
  © the Lensfun contributors. Credited in About, on the help's Licence page and in
  `packaging/THIRD-PARTY.md`.
- What we changed: nothing in the files' contents. They are joined in name order, each as
  `name\0content\0`, and compressed with zstd (level 19) into `lensfun-db.zst`
  (uncompressed bundle: 5,217,812 bytes, SHA-256
  `3d803792d0a0e44318414278c47ef6cdf6e28749cd8abd5d1e904c56c760db87`). The DTD and XSD files
  and `timestamp.txt` are left out. Any change we make to the data is shared under the same
  licence.
- Not used: the Lensfun library (LGPL) and its source. `specular-lensfun` parses the XML
  with its own code, from Lensfun's published documentation of the format and the models.
- Update: change `COMMIT` in `tools/fetch-lensfun.sh`, run it with `--sums`, review and paste
  the new lines into its `FILES` list, run it again, and update this file.
