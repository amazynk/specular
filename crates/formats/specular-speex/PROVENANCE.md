# Provenance of specular-speex's data

The decoding logic is this crate's own. The tables below are data copied from libspeex
(BSD 3-clause), as data only.

Source: libspeex 1.2.1,
<https://downloads.xiph.org/releases/speex/speex-1.2.1.tar.gz>,
SHA-256 `4b44d4f2b38a370a2d98a78329fefc56a0cf93d1c1be70029217baae6628feea`.
`tools/corpus/speex-tables.py` writes `src/tables.rs` from the unpacked `libspeex/` folder.

| Table (ours) | libspeex file | libspeex name |
|---|---|---|
| `EXC_10_16`, `EXC_10_32`, `EXC_20_32`, `EXC_5_256`, `EXC_5_64`, `EXC_8_128` | `exc_*_table.c` | `exc_*_table` |
| `GAIN_NB`, `GAIN_LBR` | `gain_table.c`, `gain_table_lbr.c` | `gain_cdbk_nb`, `gain_cdbk_lbr` |
| `HEXC`, `HEXC_10_32` | `hexc_table.c`, `hexc_10_32_table.c` | `hexc_table`, `hexc_10_32_table` |
| `HIGH_LSP`, `HIGH_LSP2` | `high_lsp_tables.c` | `high_lsp_cdbk`, `high_lsp_cdbk2` |
| `LSP_NB`, `LSP_NB_LOW1`, `LSP_NB_LOW2`, `LSP_NB_HIGH1`, `LSP_NB_HIGH2` | `lsp_tables_nb.c` | `cdbk_nb*` |
| `H0` | `sb_celp.c` | `h0` (floating point) |
| constants in `nb.rs` and `sb.rs` (gain steps, high-pass, pitch interpolation and stereo constants, mode parameters) | `nb_celp.c`, `sb_celp.c`, `filters.c`, `modes.c`, `modes_wb.c`, `stereo.c`, `math_approx.h`, `quant_lsp.c` | as named there |

Licence (BSD 3-clause), as in libspeex's `COPYING`:

```
Copyright 2002-2008 Xiph.org Foundation
Copyright 2002-2008 Jean-Marc Valin
Copyright 2005-2007 Analog Devices Inc.
Copyright 2005-2008 Commonwealth Scientific and Industrial Research Organisation (CSIRO)
Copyright 1993, 2002, 2006 David Rowe
Copyright 2003 EpicGames
Copyright 1992-1994 Jutta Degener, Carsten Bormann

Redistribution and use in source and binary forms, with or without modification, are
permitted provided that the following conditions are met:

- Redistributions of source code must retain the above copyright notice, this list of
  conditions and the following disclaimer.

- Redistributions in binary form must reproduce the above copyright notice, this list of
  conditions and the following disclaimer in the documentation and/or other materials
  provided with the distribution.

- Neither the name of the Xiph.org Foundation nor the names of its contributors may be used
  to endorse or promote products derived from this software without specific prior written
  permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS ``AS IS'' AND ANY EXPRESS
OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
FOUNDATION OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
OF THE POSSIBILITY OF SUCH DAMAGE.
```
