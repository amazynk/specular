# Provenance of the copied Musepack data

Only data is copied; every line of decoder logic in this crate is our own, written from the
format's description. No GPL or LGPL source was used.

| Our data | File | Taken from | Licence |
|---|---|---|---|
| The prefix-code tables: SV7 `HDR`, `SCFI7`, `DSCF7`, `Q7_1_0` … `Q7_7_1`; SV8 `SCFI8_*`, `DSCF8_*`, `BANDS`, `RES_*`, `Q8_*` | `src/tables.rs` | libmpcdec r475 (musepack_src_r475), `libmpcdec/huffman.c`: `mpc_table_Huff*`, `mpc_huff_*` with `mpc_sym_*` | BSD-3-Clause (Musepack Development Team) |

The tables were extracted by a script that reads the numbers only and writes each code as
(code left-aligned to 16 bits, length, symbol); SV8's canonical tables (a first code per
length and a symbol list) are expanded to one entry per code word. The script checks that
every table is a complete prefix code (its Kraft sum is exactly 1) and that codes descend.

Not copied, computed: the quantiser steps and offsets, the scale factors (1.5 dB apart,
built by the same repeated products so each rounds alike), the binomial tables of SV8's
combination code and its truncated-binary lengths, the noise generator's parity, the
grouped-code digits and their magnitudes. One quirk of the reference table is kept on
purpose: the truncated-binary "lost" count for 16 of 32 is 0 there (an entry left out of
its initialiser), and encoders follow it.

The synthesis window is ISO/IEC 11172-3 Table 3-B.3, as in `specular-mpa` (`src/window.rs`).

## Checks

Against libmpcdec r475's `mpcdec` built from source (float output truncated to 16 bits):
SV7 and SV8 agree on every sample but about 1 in 1700, which differ by one step (the
reference filters in single precision, we in double). Against FFmpeg's FATE output for
`inside-mp7.mpc` and `inside-mp8.mpc` (rounded): within one step everywhere.
