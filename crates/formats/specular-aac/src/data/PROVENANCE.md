# Provenance of the copied AAC data tables

The ISO/IEC 14496-3 data tables below are copied from permissively licensed
implementations. Only data is copied here; every line of decoder logic in this crate
is our own. No GPL or LGPL source was used. The tables were extracted by a script (numbers
only, no code) and checked as described below.

| Our table | File | Taken from | Licence |
|---|---|---|---|
| `SCALEFACTOR` (Table 4.A.1), `SPECTRAL` 1-11 (Tables 4.A.2-4.A.12) | `lc.rs` | VisualOn AAC encoder, `aac_rom.c`: `huff_ctab1`..`huff_ctab11`, `huff_ltab1_2`..`huff_ltab11`, `huff_ctabscf`, `huff_ltabscf` | Apache-2.0 |
| `SWB_LONG`, `SWB_SHORT` (Tables 4.129-4.147) | `lc.rs` | same file: `sfBandTabLong`, `sfBandTabShort` and their offset/count tables | Apache-2.0 |
| SBR Huffman trees `ENV_*`, `BAL_*`, `NOISE_*` (Tables 4.A.75-4.A.84) | `sbr.rs` | PacketVideo AAC decoder, `sbr_code_book_envlevel.cpp` | Apache-2.0 + ISO/IEC SC29 notice |
| `START_OFFSET` (k0 offsets, §4.6.18.3.2.1) | `sbr.rs` | PacketVideo, `get_sbr_startfreq.cpp`: `v_offset` (rows 0-5) | Apache-2.0 + ISO/IEC SC29 notice |
| `QMF_WINDOW` (Table 4.A.89) | `sbr.rs` | JAADec, `sbr/FilterbankTable.java`: `qmf_c` | public domain |
| `NOISE` (Table 4.A.90) | `sbr.rs` | JAADec, `sbr/NoiseTable.java`: `NOISE_TABLE` | public domain |
| PS Huffman trees `IID_*`, `ICC_*` (Tables 8.B.*) | `ps.rs` | PacketVideo, `ps_read_data.cpp` | Apache-2.0 + ISO/IEC SC29 notice |
| PS Huffman trees `IPD_*`, `OPD_*` (Tables 8.B.*) | `ps.rs` | JAADec, `ps/HuffmanTables.java`: `f_huff_ipd`, `t_huff_ipd`, `f_huff_opd`, `t_huff_opd` | public domain |
| `HYBRID8`, `HYBRID2` (hybrid filter prototypes, 20 bands) | `ps.rs` | JAADec, `ps/PSTables.java`: `p8_13_20`, `p2_13_20` | public domain |
| `HYBRID12_34`, `HYBRID8_34`, `HYBRID4_34` (prototypes, 34 bands) | `ps.rs` | JAADec, `ps/PSTables.java`: `p12_13_34`, `p8_13_34`, `p4_13_34` | public domain |
| 20-band parameter map (`[1, 0, 0, 1, 2, …]` and the QMF borders 3…64 in `../ps.rs`) | `../ps.rs` | PacketVideo, `ps_all_pass_filter_coeff.cpp`: `groupBorders`, `bins2groupMap` | Apache-2.0 + ISO/IEC SC29 notice |
| `SUB_TO_PAR_34` and the 34-band QMF borders (in `../ps.rs`) | `../ps.rs` | JAADec, `ps/PSTables.java`: `map_group2bk34`, `group_border34` | public domain |

Small parameter tables written into the code (TNS maximum bands, the PS IID and ICC
quantisation grids, the all-pass link coefficients, the stop-frequency rule) are the
standard's values; the IID grid and the stop frequencies are checked against PacketVideo's
precomputed tables in unit tests. The hybrid bands' centre frequencies and the decorrelator
phases are computed from the filterbank definitions, not copied.

## Sources

- VisualOn AAC encoder, Android Open Source Project, `platform/frameworks/av`, tag
  `android-4.4_r1` (commit `a6f2c7b1ac67075197ccbaba61e5e23277aab5f7`),
  `media/libstagefright/codecs/aacenc/src/aac_rom.c`, "Copyright 2003-2010, VisualOn, Inc.",
  Apache License 2.0.
  - `aac_rom.c` sha256 `bebae3b53f4de70055c6328a24e3fc3172f3b15d40eb9e4556d6d8c2d91cb8b6`
  - `NOTICE` sha256 `38751245389e1e23f73e6f5384b5cbe7fa972cc4410c5adc9c04b082a0b9561a`
    (the licence text in `LICENSE-APACHE-2.0.txt` is copied from it)
- PacketVideo AAC decoder, Android Open Source Project, `platform/frameworks/base`, tag
  `android-2.3_r1` (commit `6471cd5f7d0125713081c9be4772922eedd2a904`),
  `media/libstagefright/codecs/aacdec/`, "Copyright (C) 1998-2009 PacketVideo", Apache
  License 2.0. The SBR and PS files also carry the ISO/IEC SC29 software notice, which must
  be kept with copies; it is in `NOTICE-ISO-IEC-SC29.txt`.
  - `sbr_code_book_envlevel.cpp` sha256 `ad98042ff5d2bc226a5ecabd485aa61379915f8bb98c1ed9ab1e98617e9657c8`
  - `ps_read_data.cpp` sha256 `1a7788633d8fafa651661c7d3e0fa834ec2e27ff618e3fef29c2b1dbfe6172c6`
  - `get_sbr_startfreq.cpp` sha256 `34de20f4080216bae66741a14ac4ab6991ce0fe5bdf7a1110532791070466cbc`
  - `ps_all_pass_filter_coeff.cpp` sha256 `5ce5a8f7c5755b587bc86cc0e97bb29ce8b58362b6ace504a9b151f7d9cb056a`
  - used for cross-checks only: `qmf_filterbank_coeff.cpp` sha256
    `5914d26591251cfc44e0e2e967f9c4c3c2e822a2d9c6db4bcfa42d37cf51be11`,
    `sbr_envelope_calc_tbl.cpp` sha256 `fdb80d9be6e20704e902eeccca934ec6b58ea849e570b0ee8b7b580c2fd66167`,
    `ps_channel_filtering.cpp` sha256 `7727f26d0785b0f37305650acc1c5de2c0e43d3645c857a6142bd78322950967`,
    `get_sbr_stopfreq.cpp` sha256 `eb8027bcdba71f12593277a085ca107d80c03142728659e00dbd5874397cb989`,
    `ps_init_stereo_mixing.cpp` sha256 `48f867c779db6f7f4459bd1b07a8795526d4b66e945745bf7906f1bee7eee7ba`
- JAADec, github.com/DV8FromTheWorld/JAADec, commit
  `28b460de7300aaaa80f5617f9ea3d47ce7462622` (2017-12-03), a fork of
  sourceforge.net/projects/jaadec; its LICENSE dedicates the work to the public domain.
  - `src/main/java/net/sourceforge/jaad/aac/sbr/FilterbankTable.java` sha256
    `c9fdb47390a3b17a9597e5e7dd7e36b871494238aea5be92da6ca6356a1744a0`
  - `.../sbr/NoiseTable.java` sha256 `36678eb2554a5bd2c8debba22e63265d04640c4b79ca238150a117b482f56949`
  - `.../ps/PSTables.java` sha256 `787cc0e3ef57f8bcb76822bc8769a64179f49a46adcff175cf8c334c68f46eca`
  - `.../ps/HuffmanTables.java` sha256 `3a2c6e8dcb6c0397a015bac23f850879454217af803d0a6fcf1791d9a731c9e7`
  - `.../sbr/HuffmanTables.java` sha256 `3b3d16c3641c2f2e96c6d98e70ee0491b35ca092a2d62f85a98c017c39289732`
    (cross-check only)

JAADec's decoder code is believed to descend from another decoder; only the numeric tables
named above, which are the standard's printed values, were taken from it. Those PacketVideo
also has (SBR trees, QMF window, noise table, 20-band prototypes) were checked against it;
the rest (IPD/OPD trees and the 34-band configuration, which the baseline PacketVideo
decoder lacks) by their structure and by the conformance streams decoding to ffmpeg's output
within 0.01 of a 16-bit step.

## Checks made when the tables were copied

- Every spectral codebook and the scalefactor codebook is a complete prefix code (Kraft sum
  exactly 1, no code a prefix of another), with 81/81/81/81/81/81/64/64/169/169/289 and 121
  entries. Each scalefactor band table starts at 0, rises, and ends at 1024 or 128.
- Every SBR and PS tree is a full binary tree (Kraft sum 1). JAADec's ten SBR trees are
  identical to PacketVideo's, and its six IID/ICC trees give the same codes. The four
  IPD/OPD trees each code the eight phase steps.
- `QMF_WINDOW`: the 320 even coefficients times 32768 match PacketVideo's 16-bit
  `sbrDecoderFilterbankCoefficients_down_smpl` within 0.51 LSB. The odd ones are checked by
  the 64-band synthesis test `analysis_synthesis_upsamples` in `../sbr/qmf.rs` (the bank's
  own reconstruction limit), and by the SBR conformance streams.
- `NOISE`: real parts match PacketVideo's float literals (`rP_LCx`) within 3e-8;
  imaginary parts match its Q15 table (`rPxx`) within 7.4e-5 (Q15 rounding).
- `HYBRID8` and `HYBRID2` match the constants in PacketVideo's `ps_channel_filtering.cpp`
  (which are 2·g(n), or 2·g(n)·cos/sin of the modulation) to the digits given. Every
  prototype's centre tap is 1/bands, so the bands of each hybrid filter sum to a delayed
  input (unit test `hybrid_filters_sum_to_a_delay`).

The extraction scripts were run once and are not part of the build.
