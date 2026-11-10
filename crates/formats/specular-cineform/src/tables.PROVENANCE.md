# Provenance of `tables.rs`

- What: the two run/magnitude codebooks CineForm streams use, codebook 9 (older streams,
  71 rows plus the band end code) and codebook 17 (263 rows plus the band end code; codebook
  18 has the same codes without companding). Data only; the decoder is our own.
- Source: GoPro CineForm SDK, <https://github.com/gopro/cineform-sdk>, master branch tarball
  fetched 2026-11-06 (sha256
  `162e34061e13708e44e077eacfa6f65ea966fe4687e17e59cac84092e9e608d1`):
  - `Common/table9.inc` (sha256 `b45c6b4da9fdd2947258830ceca0ee033c0ff0b7ef5f9c3ff82e5916b7da914a`):
    rows of `table9r`, band end code = last row of `table9s`.
  - `Common/table17.inc` (sha256 `678c9a99b4362d09f305e881887c7c9efbc237682e3d6389bf95b24de6087dd3`):
    rows of `table17r`, band end code = row of `table17s`.
- Licence: the SDK is dual-licensed Apache-2.0 OR MIT at the user's option. Used under MIT; the
  copyright and permission notice are reproduced in the header of `tables.rs`.
- Decision: the lane brief of 2026-11-06 allows CineForm SDK table values with provenance
  (no code copied). Codebook 17 is also the VC-5 codebook of GoPro GPR raws, which
  `specular-raw` reads through `specular_cineform::vc5_codebook` (the GPR SDK's own copy
  was folded into this one on 2026-11-08).
- GPR SDK: <https://github.com/gopro/gpr>, `source/lib/vc5_common/table17.inc` (master
  tarball fetched 2026-10-19, sha256
  `69295a78f2c733b6cf774229ea6bddec4db7b76d348148d5f6938a6ef3fd51f3`; file sha256
  `7b3190bda6039fba5c74e9342e88f32dcd0b26b9fef37df1c85477834f7c38db`; Copyright 2018
  GoPro, Inc., Apache-2.0 OR MIT, used under MIT)
  has the same 264 codes, runs and magnitudes, except that its band end row carries
  magnitude 1 where `table17s` has 0. Both decoders end a band on run length 0 alone, so
  the magnitude is never read.
- Regenerate: extract the `{size, bits, count, value}` rows in order and append the band end
  code with run length 0 (the lane used a short Python transform).
- Also from the SDK, as facts rather than tables: the tag numbers (`Codec/codec.h`), the cubic
  companding `m + m³·768/256³` and the older two-step companding (×4 above 54, then above 40),
  the default prescale tables per transform and precision (`wavelet.c`), and the alpha
  decompanding `(a − 256)·9400 / 2¹³`.
