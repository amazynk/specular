# specular-texture: data copied from published documents

| Data | Source | Licence |
|---|---|---|
| `src/basis/hdr6x6i_tables.rs`: the 75 block configurations and the unique 2/3-subset partition seeds of UASTC HDR 6×6i | Binomial, "UASTC HDR 6x6i (Intermediate) Specification v1.2", basis_universal wiki (Appendix A and the pseudo-code tables) | Apache-2.0 |
| `src/basis/xuastc_tables.rs` (Appendix 1, unique partition seeds per block size) and `src/basis/xuastc_cfg.bin` (Appendix 2, the 10,311 packed configurations) | Binomial, "XUASTC LDR Specification v1.0", basis_universal wiki | Apache-2.0 |
| Constants in `src/basis/xuastc_dct.rs` (quantisation steps, modified JPEG luma table) | Binomial, "XUASTC LDR - Weight Grid DCT", basis_universal wiki | Apache-2.0 |
| Tables in `src/basis/xubc7.rs` other than the BC7 partition/anchor tables | Binomial, "XUBC7 Low-Level Format", basis_universal wiki | Apache-2.0 |
| `tests/data/xuastc6x6_L3.bin`, `tests/data/xubc7_L3.bin`, `tests/data/xubc7_L6.bin`: level slices of `shader_deblocking_glfw/bin/t6x6.ktx2` and `python/shader_deblocking/kodim26_xubc7.ktx2` (basis_universal tag v2_50); the `.blocks` files next to them are the opengl_basisu 0.1.0a2 wheel's transcodes of them | BinomialLLC/basis_universal | Apache-2.0 |

The tables were extracted from the specification text by script, not retyped. No Basis Universal source code was read or copied.
