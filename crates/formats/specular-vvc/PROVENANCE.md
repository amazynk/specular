# Provenance of the VVC data tables

Only data from the standard is copied; every line of decoder logic in this crate is our own,
written from the Recommendation. No decoder source (VTM, FFmpeg, vvdec) was used.

| Our data | File | Taken from | Licence |
|---|---|---|---|
| MIP weights (mipSizeId 0–2), DCT-II 64, DST-VII and DCT-VIII 4–32, LFNST 4×4 and 8×8 matrices | `src/tables.rs` | ITU-T H.266 V4 (01/2026), 8.4.5.2.4, 8.7.4.3, 8.7.4.5 | ITU-T Recommendation, free of charge from itu.int |
| ALF fixed filters `AlfFixFiltCoeff`, `AlfClassToFiltMap` | `src/filter/alf_tables.rs` | ITU-T H.266 V4, equations 90 and 91 | as above |
| CABAC context initValue and shiftIdx | `src/cabac/init.rs` | ITU-T H.266 V4, Tables 51 to 126 (9.3.2.2) | as above |

Each table was extracted from `pdftotext -layout` of `T-REC-H.266-202601-I!!PDF-E` by
`tools/corpus/vvc-tables.py`, `vvc-alf-tables.py` and `vvc-ctx-tables.py`, which read the
numbers only. The ALF arrays are printed transposed in the Recommendation; the script writes
them in the equations' index order.

## Checks

Against the JVET conformance streams (`tools/corpus/fetch-vvc.sh`), compared with the system
FFmpeg's decode: frame MD5s, see `tests/conformance.rs`.
