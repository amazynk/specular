//! Basis Universal texture data: ETC1S with BasisLZ supercompression and UASTC LDR 4x4.
//!
//! Both are KTX2 payloads (DFD colour models 163 and 166). [`etc1s`] decodes the BasisLZ
//! global data and slices to ETC1 blocks and those to RGBA8; [`uastc`] decodes UASTC blocks
//! straight to RGBA8 (KTX2 Zstandard supercompression is undone by the container code).

pub(crate) mod astc_pack;
pub(crate) mod etc1s;
pub(crate) mod hdr6x6i;
mod hdr6x6i_tables;
pub(crate) mod uastc;
pub(crate) mod xuastc;
mod xuastc_arith;
mod xuastc_dct;
mod xuastc_ep;
mod xuastc_tables;
pub(crate) mod xubc7;

use crate::{Basis2, Error, Format, Result};

/// Transcode one image of a Basis Universal intermediate codec (`w`×`h` texels) to the blocks
/// of `format`.
pub(crate) fn transcode2(
    codec: Basis2,
    d: &[u8],
    w: u32,
    h: u32,
    format: Format,
) -> Result<Vec<u8>> {
    match (codec, format) {
        (Basis2::Hdr6x6i, Format::Astc { w: 6, h: 6, .. }) => hdr6x6i::transcode(d, w, h),
        (Basis2::Xuastc, Format::Astc { w: bw, h: bh, .. }) => xuastc::transcode(d, w, h, bw, bh),
        (Basis2::Xubc7, Format::Bc7) => xubc7::transcode(d, w, h),
        _ => Err(Error::Unsupported(format!(
            "Basis Universal {codec:?} as {format:?}"
        ))),
    }
}

#[cfg(test)]
mod corpus_tests {
    //! Bit-exactness against the basisu transcoder's RGBA32 output, from the references written
    //! by `tools/corpus/fetch-textures.sh oracle_basis` (see tools/corpus/texture/basis.sh):
    //! `$SPECULAR_LARGE_CORPUS/texture/basis/ref/<stem>.l<level>.a<layer>.f<face>.rgba` with
    //! `<stem>.src` naming the KTX2 file. Skips silently when the corpus is absent.

    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use super::{etc1s, uastc};

    fn corpus() -> PathBuf {
        match std::env::var_os("SPECULAR_LARGE_CORPUS") {
            Some(p) => PathBuf::from(p).join("texture"),
            None => {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large/texture")
            }
        }
    }

    fn u32_at(b: &[u8], at: usize) -> usize {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize
    }

    fn u64_at(b: &[u8], at: usize) -> usize {
        u64::from_le_bytes(b[at..at + 8].try_into().unwrap()) as usize
    }

    /// Just enough KTX2 for the tests.
    struct Ktx2 {
        width: usize,
        height: usize,
        layers: usize,
        faces: usize,
        /// Level payloads, Zstandard already undone.
        levels: Vec<Vec<u8>>,
        sgd: Vec<u8>,
        /// DFD colour model: 163 ETC1S, 166 UASTC.
        model: u8,
    }

    fn parse_ktx2(data: &[u8]) -> Ktx2 {
        assert_eq!(&data[..12], b"\xABKTX 20\xBB\r\n\x1A\n");
        let levels_n = u32_at(data, 40).max(1);
        let scheme = u32_at(data, 44);
        let dfd = u32_at(data, 48);
        let (sgd_off, sgd_len) = (u64_at(data, 64), u64_at(data, 72));
        let levels = (0..levels_n)
            .map(|l| {
                let e = 80 + l * 24;
                let (off, len) = (u64_at(data, e), u64_at(data, e + 8));
                let raw = &data[off..off + len];
                if scheme == 2 {
                    let mut out = Vec::new();
                    let mut d = ruzstd::decoding::StreamingDecoder::new(raw).unwrap();
                    std::io::Read::read_to_end(&mut d, &mut out).unwrap();
                    out
                } else {
                    assert!(scheme <= 1, "supercompression {scheme}");
                    raw.to_vec()
                }
            })
            .collect();
        Ktx2 {
            width: u32_at(data, 20),
            height: u32_at(data, 24).max(1),
            layers: u32_at(data, 32).max(1),
            faces: u32_at(data, 36),
            levels,
            sgd: data[sgd_off..sgd_off + sgd_len].to_vec(),
            model: data[dfd + 12],
        }
    }

    /// Decodes every block of a `bw` x `bh` image with `decode(block_index, out, pitch)` into
    /// a `w` x `h` RGBA8 image (edge blocks cropped).
    fn assemble(w: usize, h: usize, mut decode: impl FnMut(usize, &mut [u8], usize)) -> Vec<u8> {
        let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
        let mut img = vec![0u8; w * h * 4];
        let mut tmp = [0u8; 64];
        for by in 0..bh {
            for bx in 0..bw {
                decode(by * bw + bx, &mut tmp, 16);
                for y in 0..4.min(h - by * 4) {
                    let n = 4.min(w - bx * 4) * 4;
                    let o = ((by * 4 + y) * w + bx * 4) * 4;
                    img[o..o + n].copy_from_slice(&tmp[y * 16..y * 16 + n]);
                }
            }
        }
        img
    }

    #[derive(Default)]
    struct Stats {
        files: usize,
        images: usize,
        bad_images: usize,
        max_err: u8,
        pixels: usize,
        etc1s_px: usize,
        etc1s_time: Duration,
        uastc_px: usize,
        uastc_time: Duration,
    }

    fn check_file(root: &Path, stem: &str, src: &str, st: &mut Stats) {
        let data = std::fs::read(root.join(src)).unwrap();
        let k = parse_ktx2(&data);
        let global = (k.model == 163).then(|| {
            let images = k.levels.len() * k.layers * k.faces;
            etc1s::Etc1sGlobal::parse(&k.sgd, images).unwrap()
        });
        st.files += 1;
        for (l, level) in k.levels.iter().enumerate() {
            let (w, h) = ((k.width >> l).max(1), (k.height >> l).max(1));
            let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
            for a in 0..k.layers {
                for f in 0..k.faces {
                    let refp = root.join(format!("basis/ref/{stem}.l{l}.a{a}.f{f}.rgba"));
                    let Ok(want) = std::fs::read(&refp) else {
                        continue;
                    };
                    let image = a * k.faces + f;
                    let t = Instant::now();
                    let got = match &global {
                        Some(g) => {
                            let desc = (l * k.layers + a) * k.faces + f;
                            let blocks = g
                                .transcode(level, desc, bw, bh)
                                .unwrap_or_else(|e| panic!("{stem} l{l} a{a} f{f}: {e:?}"));
                            let stride = if g.has_alpha() { 16 } else { 8 };
                            let img = assemble(w, h, |i, out, pitch| {
                                etc1s::decode_block(
                                    &blocks[i * stride..],
                                    g.has_alpha(),
                                    out,
                                    pitch,
                                )
                            });
                            st.etc1s_time += t.elapsed();
                            st.etc1s_px += w * h;
                            img
                        }
                        None => {
                            assert_eq!(k.model, 166, "{stem}: colour model");
                            let base = image * bw * bh * 16;
                            let img = assemble(w, h, |i, out, pitch| {
                                uastc::decode_block(&level[base + i * 16..], out, pitch)
                            });
                            st.uastc_time += t.elapsed();
                            st.uastc_px += w * h;
                            img
                        }
                    };
                    assert_eq!(got.len(), want.len(), "{stem} l{l} a{a} f{f}: size");
                    st.images += 1;
                    st.pixels += w * h;
                    let err = got
                        .iter()
                        .zip(&want)
                        .map(|(a, b)| a.abs_diff(*b))
                        .max()
                        .unwrap_or(0);
                    if err != 0 {
                        let first = got.iter().zip(&want).position(|(a, b)| a != b).unwrap() / 4;
                        eprintln!(
                            "{stem} l{l} a{a} f{f} ({w}x{h}): max err {err}, first at ({}, {}) got {:?} want {:?}",
                            first % w,
                            first / w,
                            &got[first * 4..first * 4 + 4],
                            &want[first * 4..first * 4 + 4]
                        );
                        st.bad_images += 1;
                        st.max_err = st.max_err.max(err);
                    }
                }
            }
        }
    }

    #[test]
    fn matches_basisu() {
        let root = corpus();
        let Ok(dir) = std::fs::read_dir(root.join("basis/ref")) else {
            return;
        };
        let mut srcs: Vec<_> = dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "src"))
            .collect();
        srcs.sort();
        let filter = std::env::var("BASIS_FILTER").unwrap_or_default();
        let mut st = Stats::default();
        for p in srcs {
            let stem = p.file_stem().unwrap().to_str().unwrap().to_owned();
            if !stem.contains(&filter) {
                continue;
            }
            let src = std::fs::read_to_string(&p).unwrap();
            check_file(&root, &stem, src.trim(), &mut st);
        }
        let mpix = |px: usize, t: Duration| px as f64 / 1e6 / t.as_secs_f64().max(1e-9);
        eprintln!(
            "basis: {} files, {} images, {} Mpixel; {} mismatched (max err {}); \
             ETC1S {:.1} Mpix/s, UASTC {:.1} Mpix/s (including test overhead)",
            st.files,
            st.images,
            st.pixels as f64 / 1e6,
            st.bad_images,
            st.max_err,
            mpix(st.etc1s_px, st.etc1s_time),
            mpix(st.uastc_px, st.uastc_time),
        );
        assert_eq!(st.bad_images, 0);
    }
}
