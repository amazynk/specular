//! HTJ2K (T.814 | ISO/IEC 15444-15) decoding against OpenJPEG's `opj_decompress` and
//! OpenJPH's `ojph_expand` (each skipped when not available), over the committed vectors in
//! `tests/data/htj2k` (see `tests/gen_htj2k.sh`) and the gitignored conformance corpora.

mod common;

use common::*;
use specular_jp2::{Decoder, Jp2Error, Jp2Info, Pixels, Region};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Location of `ojph_expand`: `$SPECULAR_OJPH_EXPAND` or `PATH`. Its shared library is
/// looked up in `../lib` next to the binary.
fn ojph_expand() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SPECULAR_OJPH_EXPAND") {
        return Some(PathBuf::from(p));
    }
    let ok = Command::new("ojph_expand")
        .arg("-h")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok();
    ok.then(|| PathBuf::from("ojph_expand"))
}

/// Run `ojph_expand`, skipping `reduce` resolutions; `None` if it fails or the image needs an
/// output format other than PGM/PPM.
fn ojph(bin: &Path, file: &Path, info: &Jp2Info, reduce: u8) -> Option<Pnm> {
    if info.has_alpha || !matches!(info.components.len(), 1 | 3) {
        return None;
    }
    let ext = if info.gray { "pgm" } else { "ppm" };
    let out = scratch_dir("ojph").join(format!("{}.r{reduce}.{ext}", name(file)));
    let mut cmd = Command::new(bin);
    if let Some(lib) = bin.parent().and_then(|b| b.parent()).map(|p| p.join("lib")) {
        cmd.env("LD_LIBRARY_PATH", lib);
    }
    let st = cmd
        .arg("-i")
        .arg(file)
        .arg("-o")
        .arg(&out)
        .args(["-skip_res", &format!("{reduce},{reduce}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    if !st.success() {
        return None;
    }
    read_pnm(&std::fs::read(&out).ok()?)
}

/// `ojph_expand` to planar YUV at `reduce`, split into components of the given sizes (it
/// writes subsampled components at their own resolution, unlike OpenJPEG).
fn ojph_components(
    bin: &Path,
    file: &Path,
    info: &Jp2Info,
    reduce: u8,
    sizes: &[(usize, usize)],
) -> Option<Vec<Pgx>> {
    let out = scratch_dir("ojph").join(format!("{}.r{reduce}.yuv", name(file)));
    let mut cmd = Command::new(bin);
    if let Some(lib) = bin.parent().and_then(|b| b.parent()).map(|p| p.join("lib")) {
        cmd.env("LD_LIBRARY_PATH", lib);
    }
    let st = cmd
        .arg("-i")
        .arg(file)
        .arg("-o")
        .arg(&out)
        .args(["-skip_res", &format!("{reduce},{reduce}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    if !st.success() {
        return None;
    }
    let raw = std::fs::read(&out).ok()?;
    let mut pos = 0;
    let mut comps = Vec::new();
    for (ci, &(w, h)) in info.components.iter().zip(sizes) {
        let nb = if ci.precision <= 8 { 1 } else { 2 };
        let bytes = raw.get(pos..pos + w * h * nb)?;
        pos += w * h * nb;
        let data = bytes
            .chunks_exact(nb)
            .map(|b| {
                let u = if nb == 1 {
                    b[0] as u16
                } else {
                    u16::from_le_bytes([b[0], b[1]])
                };
                if ci.signed {
                    let sh = 16 - u32::from(ci.precision);
                    (((u << sh) as i16) >> sh) as i64
                } else {
                    u as i64
                }
            })
            .collect();
        comps.push(Pgx {
            w,
            h,
            depth: u32::from(ci.precision),
            signed: ci.signed,
            data,
        });
    }
    (pos == raw.len()).then_some(comps)
}

/// HTJ2K streams under `dir` (recursively).
fn streams(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("j2k" | "j2c" | "jhc" | "jph")
            ) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// `rel` under the conformance corpus root: `$SPECULAR_CONFORMANCE_DIR` or
/// `tools/corpus/conformance`.
fn corpus(rel: &str) -> PathBuf {
    std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        })
        .join(rel)
}

/// One comparison: our decode at `reduce` against a reference decode.
struct Cmp {
    file: String,
    reduce: u8,
    oracle: &'static str,
    reversible: bool,
    /// Highest component precision (bits).
    prec: u8,
    max: u32,
    /// Mean absolute error (pixels) or worst component MSE (components).
    mean: f64,
}

/// Decode every stream at reduce 0..=min(levels, 2) and compare its components (before any
/// colour mapping, so subsampled, YCC and ICC streams are covered alike) with OpenJPEG's PGX
/// output, falling back to OpenJPH's pixel output where OpenJPEG refuses the stream.
/// Streams using features outside Part 1 + Part 15 (e.g. Part 2 wavelet kernels) must be
/// refused cleanly; they are listed and dropped from `files`.
fn compare_all(files: &mut Vec<PathBuf>, ht: bool) -> Vec<Cmp> {
    let opj_bin = opj_decompress();
    let ojph_bin = ojph_expand();
    let mut res = Vec::new();
    files.retain(|f| match Decoder::new(&std::fs::read(f).unwrap()) {
        Err(Jp2Error::Unsupported(why)) => {
            eprintln!("{:<44} refused: {why}", name(f));
            false
        }
        _ => true,
    });
    for f in files.iter() {
        let n = name(f);
        let data = std::fs::read(f).unwrap();
        let dec = Decoder::new(&data).unwrap_or_else(|e| panic!("{n}: {e}"));
        let info = dec.info().clone();
        assert_eq!(info.htj2k, ht, "{n}: HTJ2K detection");
        for reduce in 0..=info.levels.min(2) {
            let ours = decode16(&data, reduce, None);
            assert!(!ours.truncated, "{n} r{reduce}: truncated");
            let comps = dec
                .decode_components(reduce, None)
                .unwrap_or_else(|e| panic!("{n} r{reduce}: {e}"));
            // OpenJPEG converts 4:2:x streams to RGB even from a raw codestream; OpenJPH
            // writes their components as they are.
            let subsampled = info.components.iter().any(|c| (c.dx, c.dy) != (1, 1));
            let sizes: Vec<(usize, usize)> = comps
                .iter()
                .map(|c| (c.width as usize, c.height as usize))
                .collect();
            let refs = if subsampled {
                ojph_bin
                    .as_deref()
                    .and_then(|b| ojph_components(b, f, &info, reduce, &sizes))
                    .map(|r| ("openjph", r))
            } else {
                opj_bin
                    .as_deref()
                    .and_then(|b| opj_components(b, f, reduce))
                    .filter(|r| !r.is_empty())
                    .map(|r| ("openjpeg", r))
            };
            if let Some((oracle, refs)) = refs {
                assert_eq!(comps.len(), refs.len(), "{n}: component count");
                let (mut max, mut mse) = (0u64, 0f64);
                for (c, (ours, r)) in comps.iter().zip(&refs).enumerate() {
                    let prec = info.components[c].precision as u32;
                    let r = &decimate(r, ours.width as usize, ours.height as usize);
                    let (m, e) = pgx_diff(ours, prec, r).unwrap_or_else(|| {
                        panic!(
                            "{n} r{reduce} c{c}: size {}x{} vs {}x{}",
                            ours.width, ours.height, r.w, r.h
                        )
                    });
                    max = max.max(m);
                    mse = mse.max(e);
                }
                res.push(Cmp {
                    file: n.clone(),
                    reduce,
                    oracle,
                    reversible: info.reversible,
                    prec: info
                        .components
                        .iter()
                        .map(|c| c.precision)
                        .max()
                        .unwrap_or(8),
                    max: max as u32,
                    mean: mse,
                });
                continue;
            }
            let reference = ojph_bin
                .as_deref()
                .and_then(|b| ojph(b, f, &info, reduce))
                .map(|r| ("openjph", r));
            let Some((oracle, r)) = reference else {
                continue;
            };
            if (r.w, r.h) != (ours.width as usize, ours.height as usize) {
                continue;
            }
            let (max, mean) = diff(&ours, &r, r.h);
            res.push(Cmp {
                file: n.clone(),
                reduce,
                oracle,
                reversible: info.reversible,
                prec: info
                    .components
                    .iter()
                    .map(|c| c.precision)
                    .max()
                    .unwrap_or(8),
                max,
                mean,
            });
        }
    }
    for c in &res {
        eprintln!(
            "{:<44} r{} {:<8} {} max {:>3} err {:.4}",
            c.file,
            c.reduce,
            c.oracle,
            if c.reversible { "5/3" } else { "9/7" },
            c.max,
            c.mean
        );
    }
    res
}

/// Reversible streams must match exactly; irreversible ones within float rounding: 1, or 2
/// above 12 bits against OpenJPEG, whose single-precision pipeline drifts there (on
/// `rgb16_97.j2c` it is off by up to 2 with MSE 0.35, where OpenJPH and we agree within 1 with
/// MSE 0.01).
fn check(res: &[Cmp]) {
    for c in res {
        let tol = match (c.reversible, c.oracle) {
            (true, _) => 0,
            (false, "openjpeg") if c.prec > 12 => 2,
            (false, _) => 1,
        };
        assert!(
            c.max <= tol,
            "{} reduce {} vs {}: max {} err {:.4}",
            c.file,
            c.reduce,
            c.oracle,
            c.max,
            c.mean
        );
    }
}

/// A region in the middle must equal the same window of the full decode.
fn region_matches_full(data: &[u8], n: &str) {
    let info = Decoder::new(data).unwrap().info().clone();
    let (w, h) = (info.width, info.height);
    if w < 5 || h < 3 {
        return;
    }
    let reg = Region {
        x: w / 5,
        y: h / 3,
        w: w / 2,
        h: h / 3,
    };
    for reduce in [0, info.levels.min(1)] {
        let full = decode16(data, reduce, None);
        let reg = Region {
            x: reg.x >> reduce,
            y: reg.y >> reduce,
            w: (reg.w >> reduce).max(1),
            h: (reg.h >> reduce).max(1),
        };
        if reg.x + reg.w > full.width || reg.y + reg.h > full.height {
            continue;
        }
        let part = decode16(data, reduce, Some(reg));
        let (Pixels::U16(a), Pixels::U16(b)) = (&full.pixels, &part.pixels) else {
            panic!("expected 16-bit output")
        };
        let ch = full.format.channels();
        let fw = full.width as usize;
        for y in 0..reg.h as usize {
            let fo = ((reg.y as usize + y) * fw + reg.x as usize) * ch;
            let po = y * reg.w as usize * ch;
            let len = reg.w as usize * ch;
            assert_eq!(
                &a[fo..fo + len],
                &b[po..po + len],
                "{n} r{reduce} region row {y}"
            );
        }
    }
}

/// The committed OpenJPH-encoded vectors (`tests/data/htj2k/MANIFEST`): against the oracles
/// when installed, regions against full decodes, and, needing no oracle, every lossless
/// encoding of the same source (code-block shapes, precincts, levels, progressions, tiles,
/// offsets, with and without the colour transform) must decode to identical pixels.
#[test]
fn committed_vectors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/htj2k");
    let mut files = streams(&dir);
    let listed = std::fs::read_to_string(dir.join("MANIFEST")).unwrap();
    assert_eq!(
        files.len(),
        listed.lines().filter(|l| !l.starts_with('#')).count()
    );
    let all = files.clone();
    let res = compare_all(&mut files, true);
    assert_eq!(files.len(), all.len(), "a committed vector was refused");
    check(&res);
    if opj_decompress().is_some() && ojph_expand().is_some() {
        assert!(
            res.len() >= 2 * all.len(),
            "only {} comparisons ran",
            res.len()
        );
    }
    let same_source = [
        "rgb_rev.jph",
        "rgb_rev_nomct.j2c",
        "rgb_cb16.j2c",
        "rgb_cb_tall.j2c",
        "rgb_prec.j2c",
        "rgb_nl0.j2c",
        "rgb_LRCP.j2c",
        "rgb_RLCP.j2c",
        "rgb_PCRL.j2c",
        "rgb_CPRL.j2c",
        "rgb_tiles.j2c",
        "rgb_offsets.j2c",
    ];
    let first = decode16(&std::fs::read(dir.join(same_source[0])).unwrap(), 0, None);
    for n in &same_source[1..] {
        let img = decode16(&std::fs::read(dir.join(n)).unwrap(), 0, None);
        assert_eq!((img.width, img.height), (first.width, first.height), "{n}");
        assert!(
            img.pixels == first.pixels,
            "{n}: differs from {}",
            same_source[0]
        );
    }
    for f in &all {
        region_matches_full(&std::fs::read(f).unwrap(), &name(f));
    }
}

/// HTJ2K streams from openjpeg-data at every reduce level and for a region, against
/// `opj_decompress`.
#[test]
fn openjpeg_data_htj2k() {
    let mut files = streams(&corpus("openjpeg-data/input/nonregression/htj2k"));
    if files.is_empty() {
        eprintln!("openjpeg-data htj2k corpus missing; skipping");
        return;
    }
    let res = compare_all(&mut files, true);
    check(&res);
    for f in &files {
        region_matches_full(&std::fs::read(f).unwrap(), &name(f));
    }
    if opj_decompress().is_some() {
        assert!(res.len() >= 4, "only {} comparisons ran", res.len());
    }
}

/// The T.803-style HTJ2K conformance codestreams and JPH test files, and the OpenJPH test
/// streams (Kakadu-encoded, with SigProp/MagRef passes).
#[test]
fn conformance_htj2k() {
    let mut files = streams(&corpus("htj2k-codestreams"));
    files.extend(streams(&corpus("openjph-tests")));
    if files.is_empty() {
        eprintln!("HTJ2K conformance corpora missing; skipping");
        return;
    }
    let res = compare_all(&mut files, true);
    check(&res);
    for f in &files {
        region_matches_full(&std::fs::read(f).unwrap(), &name(f));
    }
}

/// The Part 1 conformance codestreams share the component paths (ROI, subsampling,
/// precincts, odd tile geometry) with HTJ2K: every component of the raw codestreams at
/// reduce 0..=2 against OpenJPEG, exact for reversible streams.
#[test]
fn part1_conformance_components() {
    let mut files = streams(&corpus("openjpeg-data/input/conformance"));
    if files.is_empty() {
        eprintln!("openjpeg-data conformance corpus missing; skipping");
        return;
    }
    let res = compare_all(&mut files, false);
    check(&res);
    if opj_decompress().is_some() {
        eprintln!("{} comparisons", res.len());
        assert!(res.len() >= 60, "only {} comparisons ran", res.len());
    }
}

/// A PGX file (T.803): `PG ML|LM [+|-]depth w h`, then big- or little-endian samples.
struct Pgx {
    w: usize,
    h: usize,
    depth: u32,
    signed: bool,
    data: Vec<i64>,
}

fn read_pgx(bytes: &[u8]) -> Option<Pgx> {
    let nl = bytes.iter().position(|&b| b == b'\n')?;
    let hdr = std::str::from_utf8(&bytes[..nl]).ok()?;
    let mut it = hdr.split_whitespace();
    (it.next()? == "PG").then_some(())?;
    let big = it.next()? == "ML";
    let mut rest: Vec<&str> = it.collect();
    let mut signed = false;
    if matches!(rest.first(), Some(&"+") | Some(&"-")) {
        signed = rest.remove(0) == "-";
    }
    let d = *rest.first()?;
    if let Some(d) = d.strip_prefix('-') {
        signed = true;
        rest[0] = d;
    } else if let Some(d) = d.strip_prefix('+') {
        rest[0] = d;
    }
    let depth: u32 = rest.first()?.parse().ok()?;
    let w: usize = rest.get(1)?.parse().ok()?;
    let h: usize = rest.get(2)?.parse().ok()?;
    let nb = match depth {
        0..=8 => 1,
        9..=16 => 2,
        _ => 4,
    };
    let body = bytes.get(nl + 1..)?;
    let data = body
        .chunks_exact(nb)
        .take(w * h)
        .map(|b| {
            let mut u = 0u64;
            for i in 0..nb {
                let byte = if big { b[i] } else { b[nb - 1 - i] };
                u = u << 8 | byte as u64;
            }
            if signed {
                let bits = nb as u32 * 8;
                ((u << (64 - bits)) as i64) >> (64 - bits)
            } else {
                u as i64
            }
        })
        .collect::<Vec<_>>();
    (data.len() == w * h).then_some(Pgx {
        w,
        h,
        depth,
        signed,
        data,
    })
}

/// Max absolute error and MSE of component `ours` against a PGX reference, in the reference's
/// units; `None` on a size mismatch.
fn pgx_diff(ours: &specular_jp2::RawComponent, prec: u32, r: &Pgx) -> Option<(u64, f64)> {
    if (ours.width as usize, ours.height as usize) != (r.w, r.h) {
        return None;
    }
    // Our samples are unsigned (signed ones offset) and reduced to 16 bits.
    let shift = prec.saturating_sub(16);
    let off = if r.signed { 1i64 << (r.depth - 1) } else { 0 };
    let (mut max, mut se) = (0u64, 0f64);
    for (&a, &b) in ours.samples.iter().zip(&r.data) {
        let d = (a as i64).abs_diff((b + off) >> shift);
        max = max.max(d);
        se += (d * d) as f64;
    }
    Some((max, se / r.data.len().max(1) as f64))
}

/// OpenJPEG upsamples subsampled components (by replication) when writing PGX: take every
/// k-th sample back when `r` is an integer multiple of `w` x `h`.
fn decimate(r: &Pgx, w: usize, h: usize) -> Pgx {
    let k = |big: usize, small: usize| {
        (1..=4).find(|&k| small > 0 && big.div_ceil(k) == small && big > small)
    };
    let (Some(kx), Some(ky)) = (
        k(r.w, w).or((r.w == w).then_some(1)),
        k(r.h, h).or((r.h == h).then_some(1)),
    ) else {
        return Pgx {
            data: r.data.clone(),
            ..*r
        };
    };
    let data = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| r.data[y * ky * r.w + x * kx])
        .collect();
    Pgx { w, h, data, ..*r }
}

/// The contiguous codestream (`jp2c` box) of a JP2/JPH file; `None` for a raw codestream.
fn jp2c(data: &[u8]) -> Option<&[u8]> {
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let len = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as u64;
        let (hdr, len) = match len {
            0 => (8, (data.len() - pos) as u64),
            1 => (
                16,
                u64::from_be_bytes(data.get(pos + 8..pos + 16)?.try_into().ok()?),
            ),
            l => (8, l),
        };
        let end = pos.checked_add(usize::try_from(len).ok()?)?.min(data.len());
        if &data[pos + 4..pos + 8] == b"jp2c" {
            return data.get(pos + hdr..end);
        }
        if end <= pos {
            return None;
        }
        pos = end;
    }
    None
}

/// `opj_decompress` to PGX at `reduce`, one file per component. Container files are
/// unwrapped first so OpenJPEG writes the raw components (no chroma upsampling).
fn opj_components(bin: &Path, file: &Path, reduce: u8) -> Option<Vec<Pgx>> {
    let dir = scratch_dir("opj-pgx");
    let stem = format!("{}-r{reduce}", name(file));
    let data = std::fs::read(file).ok()?;
    let input = match jp2c(&data) {
        Some(cs) => {
            let p = dir.join(format!("{stem}.j2k"));
            std::fs::write(&p, cs).ok()?;
            p
        }
        None => file.to_path_buf(),
    };
    let st = Command::new(bin)
        .arg("-i")
        .arg(&input)
        .arg("-r")
        .arg(reduce.to_string())
        .arg("-o")
        .arg(dir.join(format!("{stem}.pgx")))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    if !st.success() {
        return None;
    }
    (0..)
        .map(|c| dir.join(format!("{stem}_{c}.pgx")))
        .map_while(|p| std::fs::read(p).ok())
        .map(|b| read_pgx(&b))
        .collect()
}

/// Reference PGX files of a conformance codestream (class 1: full resolution, every
/// component).
fn references(stream: &Path) -> Vec<PathBuf> {
    let n = name(stream);
    let stem = n.trim_end_matches(".j2k");
    let parts: Vec<&str> = stem.split('_').collect();
    let (dir, prefix) = match parts.as_slice() {
        ["hifi", _, nn, ..] => ("reference_class1HF_profile1", format!("hifi_{nn}")),
        [ds, _, nn, ..] if ds.starts_with("ds") => {
            let p = &ds[2..];
            (
                if p == "0" {
                    "reference_class1_profile0"
                } else {
                    "reference_class1_profile1"
                },
                format!("c1p{p}_{nn}"),
            )
        }
        _ => return Vec::new(),
    };
    let d = corpus("htj2k-codestreams").join(dir);
    (0..16)
        .map(|c| d.join(format!("{prefix}-{c}.pgx")))
        .take_while(|p| p.is_file())
        .collect()
}

/// Whether `stream` is the last (complete) member of its `_bNN` bitstream set: earlier
/// members are truncated (fewer layers or passes) and only approximate the reference decode.
fn complete_bset(stream: &Path) -> bool {
    let b = |p: &Path| -> Option<u32> {
        let s = p.file_stem()?.to_str()?;
        s.rsplit_once("_b")?.1.parse().ok()
    };
    let Some(mine) = b(stream) else {
        return true;
    };
    let dir = stream.parent().expect("in a directory");
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| b(&e.path()))
        .all(|o| o <= mine)
}

/// Every component of the HTJ2K conformance codestreams (including the HT/Part 1 mixed
/// `ds0_hm_*` streams) against OpenJPEG's component decode (when it accepts the stream:
/// exact for reversible streams, within 1 otherwise) and against the class 1 reference
/// decodes, at the resolution the reference was made at. Complete reversible bitstream sets
/// must match the reference exactly and complete irreversible ones stay within a loose MSE
/// bound (the reference decoder's rounding differs); truncated sets (fewer layers, and with
/// ROI possibly no background yet) are only reported.
#[test]
fn conformance_references() {
    let files = streams(&corpus("htj2k-codestreams"));
    let opj_bin = opj_decompress();
    let mut checked = 0;
    let mut bad = Vec::new();
    for f in &files {
        let refs = references(f);
        if refs.is_empty() {
            continue;
        }
        let n = name(f);
        let data = std::fs::read(f).unwrap();
        let dec = Decoder::new(&data).unwrap_or_else(|e| panic!("{n}: {e}"));
        let info = dec.info().clone();
        let full = complete_bset(f);
        let refs: Vec<Pgx> = refs
            .iter()
            .map(|rp| read_pgx(&std::fs::read(rp).unwrap()).expect("PGX"))
            .collect();
        // Some references are made at a reduced resolution: pick the matching level.
        let mut comps = None;
        for reduce in 0..=info.levels {
            match dec.decode_components(reduce, None) {
                Ok(c)
                    if c.first().map(|c| (c.width as usize, c.height as usize))
                        == refs.first().map(|r| (r.w, r.h)) =>
                {
                    comps = Some((reduce, c));
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    bad.push(format!("{n} reduce {reduce}: {e}"));
                    break;
                }
            }
        }
        let Some((reduce, comps)) = comps else {
            bad.push(format!("{n}: no resolution matches the reference size"));
            continue;
        };
        // References may cover only the first components (ds0_ht_13 has 257).
        assert!(comps.len() >= refs.len(), "{n}: component count");
        let opjc = if reduce == 0 {
            opj_bin.as_ref().and_then(|b| opj_components(b, f, 0))
        } else {
            None
        };
        for (c, (ours, r)) in comps.iter().zip(&refs).enumerate() {
            let prec = info.components[c].precision as u32;
            if let Some((m, e)) = opjc
                .as_ref()
                .and_then(|o| o.get(c))
                .and_then(|o| pgx_diff(ours, prec, o))
            {
                eprintln!("{n:<24} c{c} vs openjpeg peak {m:>4} mse {e:.4}");
                if m > u64::from(!info.reversible) {
                    bad.push(format!("{n} c{c}: peak {m} mse {e:.4} vs OpenJPEG"));
                }
            }
            let Some((max, mse)) = pgx_diff(ours, prec, r) else {
                bad.push(format!(
                    "{n} c{c}: size {}x{} vs {}x{}",
                    ours.width, ours.height, r.w, r.h
                ));
                continue;
            };
            eprintln!(
                "{n:<24} c{c} {:>2}-bit {} r{reduce} {} peak {max:>4} mse {mse:.4}",
                r.depth,
                if info.reversible { "5/3" } else { "9/7" },
                if full { "complete " } else { "truncated" },
            );
            let range = (1u64 << r.depth.min(16)) as f64;
            if full && ((info.reversible && max != 0) || mse > range * range / 1000.0) {
                bad.push(format!("{n} c{c}: peak {max} mse {mse:.4}"));
            }
            checked += 1;
        }
    }
    assert!(bad.is_empty(), "mismatches:\n{}", bad.join("\n"));
    if !files.is_empty() {
        assert!(checked >= 40, "only {checked} components compared");
    }
}
