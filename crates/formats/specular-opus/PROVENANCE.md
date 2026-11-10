# Provenance of measured Opus data

No libopus source was read or copied. Every line of decoder logic in this crate is our own,
written from RFC 6716 and RFC 8251.

| Our data | File | How it was made | Inputs and their licence |
|---|---|---|---|
| SILK upsampler kernels `FIT_8`, `FIT_12` (8 and 12 kHz to 48 kHz) | `src/resample_fit.rs` | Measured: our Kaiser-sinc upsampler followed by a least-squares correction that maps our decode of a test vector to the reference decode shipped with it (`src/resample_study.rs`, run with `SPECULAR_OPUS_FIT_OUT`) | RFC 8251 test vectors 2 and 3 and their reference decodes (IETF Trust, Simplified BSD) |
| SILK upsampler kernel `FIT_16` (16 kHz to 48 kHz) | `src/resample_fit.rs` | Measured the same way from our decodes and libopus's (through ffmpeg, as a black box) of SILK wideband streams libopus encoded from generated noise, a chirp and moving tones (`tools/corpus/specular-opus-wb.sh`, `SPECULAR_OPUS_WB_CORPUS`) | Generated signals; the system libopus (libopus.so.0.11.1, BSD) used as a binary |

| SILK reconstruction and stereo unmixing in fixed point (`Channel::reconstruct`, `ms_to_lr`, `div_varq`, `inverse_varq`) | `src/silk.rs` | Our own code; its number formats, rounding and approximate divisions were chosen by trial against libopus's output at the internal rate until every frame matched (`src/silk_study.rs`, `tools/corpus/specular-opus-silk.sh`). RFC 6716 §4.2.7.9 and §4.2.8 give the floating-point form | Generated signals and RFC 8251 vectors 2–6 and 12, decoded by the system libopus (libopus.so.0.11.1, BSD) through ctypes as a binary |
| SILK concealment levels `UNVOICED` | `src/silk.rs` | Measured: our concealment's level against libopus's on the same lost packets, per place in the run of lost frames (`silk_study.rs`) | As above |

The kernels were refitted (401-tap corrections) after the SILK reconstruction matched the
reference, so they now model the upsampler alone.

RFC 6716 §4.2.9 leaves the SILK upsampler to the implementation. Test vectors 4, 5, 6 and 12
were not used for the fits at 16 kHz and serve as the held-out check (`tests/conformance.rs`).
