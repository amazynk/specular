//! Huffman lossless (SOF3) files from the public corpus against `jpeg-decoder` (the oracle):
//! the jpeg-decoder reftest set (`tools/corpus/fetch-conformance.sh src_jpeg_decoder`) and the
//! JPEG lossless streams inside the DICOM corpus (`tools/corpus/fetch-dicom.sh`). Skips what is
//! not present. The display output must match the oracle's byte for byte, `decode_wide` its raw
//! samples, and the parallel path the serial one.

use std::path::{Path, PathBuf};

use specular_jpeg::{DecodeOptions, Parallelism, decode, decode_wide, read_info};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// Every complete JPEG stream (SOI .. EOI) inside `d` whose frame is SOF3.
fn sof3_streams(d: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(s) = find(d, i, &[0xFF, 0xD8, 0xFF]) {
        let Some(e) = find(d, s, &[0xFF, 0xD9]) else {
            break;
        };
        let j = &d[s..e + 2];
        if read_info(j).is_ok_and(|i| i.sof_marker == 0xC3) {
            out.push(j.to_vec());
        }
        i = e + 2;
    }
    out
}

fn find(d: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    d.get(from..)?
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|p| p + from)
}

/// The oracle: raw samples (interleaved, as u16) and the 8-bit RGBA the old fallback made.
fn oracle(data: &[u8]) -> Option<(Vec<u16>, Vec<u8>)> {
    let mut d = jpeg_decoder::Decoder::new(data);
    let px = d.decode().ok()?;
    let info = d.info()?;
    let npx = info.width as usize * info.height as usize;
    let nc = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 | jpeg_decoder::PixelFormat::L16 => 1,
        jpeg_decoder::PixelFormat::RGB24 => 3,
        jpeg_decoder::PixelFormat::CMYK32 => 4,
    };
    let bytes = px.len() / (npx * nc);
    let raw: Vec<u16> = if bytes == 1 {
        px.iter().map(|&b| b as u16).collect()
    } else {
        px.chunks_exact(2)
            .map(|b| u16::from_ne_bytes([b[0], b[1]]))
            .collect()
    };
    let p = read_info(data).ok()?.precision as u32;
    let max = (1u32 << p) - 1;
    let s = |i: usize| -> u8 {
        if bytes == 1 {
            px[i]
        } else {
            ((raw[i] as u32).min(max) * 255 + max / 2).div_euclid(max) as u8
        }
    };
    let mut rgba = Vec::with_capacity(npx * 4);
    for i in 0..npx {
        match nc {
            1 => rgba.extend_from_slice(&[s(i), s(i), s(i), 255]),
            3 => rgba.extend_from_slice(&[s(3 * i), s(3 * i + 1), s(3 * i + 2), 255]),
            _ => {
                let k = 255 - s(4 * i + 3) as u32;
                let f = |c: u8| (((255 - c as u32) * k + 127) / 255) as u8;
                rgba.extend_from_slice(&[f(s(4 * i)), f(s(4 * i + 1)), f(s(4 * i + 2)), 255]);
            }
        }
    }
    Some((raw, rgba))
}

/// The point transform of the first scan (the oracle predicts from shifted samples when it is
/// not 0, so those files are compared against the serial path only).
fn point_transform(d: &[u8]) -> u8 {
    find(d, 2, &[0xFF, 0xDA]).map_or(0, |s| {
        let len = u16::from_be_bytes([d[s + 2], d[s + 3]]) as usize;
        d[s + 2 + len - 1] & 15
    })
}

fn restarts(d: &[u8]) -> bool {
    find(d, 2, &[0xFF, 0xDD]).is_some_and(|s| d[s + 4] != 0 || d[s + 5] != 0)
}

#[test]
fn lossless_corpus_matches_jpeg_decoder() {
    let mut files = Vec::new();
    walk(
        &root().join("conformance/jpeg-decoder/tests/reftest/images"),
        &mut files,
    );
    walk(&root().join("dicom"), &mut files);
    let (mut compared, mut own_only, mut both_fail) = (0, 0, 0);
    for path in &files {
        let Ok(d) = std::fs::read(path) else { continue };
        for (n, j) in sof3_streams(&d).iter().enumerate() {
            let what = format!("{} #{n}", path.display());
            let single = DecodeOptions {
                parallelism: Parallelism::Single,
                ..Default::default()
            };
            let mine = decode(j, &single);
            let par = decode(
                j,
                &DecodeOptions {
                    parallelism: Parallelism::Threads(4),
                    ..Default::default()
                },
            );
            assert_eq!(mine, par, "{what}: parallel differs from serial");
            let wide = decode_wide(j, None);
            let simple = point_transform(j) == 0 && !restarts(j);
            match (oracle(j), mine, wide) {
                (Some((raw, rgba)), Ok(m), Ok(w)) if simple => {
                    assert!(m.planes[0].data == rgba, "{what}: display differs");
                    let nc = w.planes.len();
                    let n = w.width as usize * w.height as usize;
                    let mut inter = vec![0u16; n * nc];
                    for (k, p) in w.planes.iter().enumerate() {
                        for (i, &v) in p.data.iter().enumerate() {
                            inter[i * nc + k] = v;
                        }
                    }
                    assert!(inter == raw, "{what}: raw samples differ");
                    compared += 1;
                }
                (None, Err(_), Err(_)) => both_fail += 1,
                (_, m, w) => {
                    assert!(
                        !simple || m.is_ok() || w.is_ok(),
                        "{what}: the oracle decodes it, we do not"
                    );
                    own_only += 1;
                }
            }
        }
    }
    eprintln!(
        "lossless corpus: {compared} streams identical to jpeg-decoder, {own_only} only checked \
         serial = parallel (restarts or point transform, where the oracle is wrong), \
         {both_fail} rejected by both"
    );
}
