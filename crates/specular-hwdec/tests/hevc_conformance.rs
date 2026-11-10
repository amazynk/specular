//! VA-API HEVC video against the software decoder (specular-hevc) on the JCT-VC conformance
//! bitstreams (`tools/corpus/hevc-conformance.sh`, gitignored; skipped when absent), picture
//! for picture: HEVC decoding is exact, so every sample must match, and the hardware path must
//! output exactly the pictures the software decoder outputs (H.265 C.5.2: none with
//! `pic_output_flag` 0, none discarded under `NoOutputOfPriorPicsFlag`). A missing or an
//! extra picture fails.
//!
//! By default only the streams that turn scaling lists on (SPS `scaling_list_enabled_flag`):
//! SLIST_A/B/C/D (default lists, SPS and PPS lists, PPSs re-sent between pictures, prediction
//! from reference lists and defaults, DC values) and whatever else uses them, plus
//! SLPPLP (parameter sets re-sent and changed between pictures, without lists). That is where
//! a wrong `VAIQMatrixBufferHEVC` layout shows; a flat matrix hides it. Also by default the
//! streams that exercise the output process and end-of-sequence handling (BUMPING, NoOutPrior,
//! OPFLAG, RAP) and version 1 extension data in parameter sets (EXT_A_ericsson_3, PS_A_VIDYO_3).
//! `SPECULAR_HEVC_CONF=all` takes every stream, `=<substring>` the matching ones;
//! `SPECULAR_HEVC_CORPUS` points at another copy of the corpus. Streams the hardware refuses
//! (4:2:2, 4:4:4, 12-bit, too small, size changes) are listed and skipped.

use specular_heic::h265::nal::strip_emulation_prevention;
use specular_heic::h265::sps::SeqParameterSet;
use specular_heic::hevc;
use specular_hwdec::video::{VideoCodec, VideoDecoder, VideoFrame};
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let d = std::env::var_os("SPECULAR_HEVC_CORPUS").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/corpus/conformance/hevc")
                .canonicalize()
                .unwrap_or_default()
        },
        PathBuf::from,
    );
    if d.join("manifest.tsv").is_file() {
        return Some(d);
    }
    // A worktree has no corpus of its own: the main checkout's (…/.claude/worktrees/<name>).
    let main = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == ".claude"))
        .and_then(Path::parent)
        .map(|p| p.join("tools/corpus/conformance/hevc"));
    main.filter(|d| d.join("manifest.tsv").is_file())
}

/// The NAL units of an Annex B byte stream, without start codes.
fn split_annexb(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map_or(data.len(), |&n| n - 3);
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(&data[s..e]);
        }
    }
    out
}

fn nal_type(n: &[u8]) -> u8 {
    (n[0] >> 1) & 0x3f
}

/// Groups NAL units into access units (H.265 7.4.2.4.4).
fn access_units<'a>(nals: &[&'a [u8]]) -> Vec<Vec<&'a [u8]>> {
    let mut aus: Vec<Vec<&[u8]>> = Vec::new();
    let mut cur: Vec<&[u8]> = Vec::new();
    let mut seen_vcl = false;
    for &n in nals {
        if n.len() < 2 {
            continue;
        }
        let t = nal_type(n);
        let layer = ((n[0] & 1) << 5) | (n[1] >> 3);
        let starts = if t < 32 {
            layer == 0 && n.len() > 2 && n[2] & 0x80 != 0
        } else {
            matches!(t, 32..=35 | 39 | 41..=44 | 48..=55)
        };
        if starts && seen_vcl {
            aus.push(std::mem::take(&mut cur));
            seen_vcl = false;
        }
        if t < 32 && layer == 0 {
            seen_vcl = true;
        }
        cur.push(n);
    }
    if !cur.is_empty() {
        aus.push(cur);
    }
    aus
}

/// An `hvcC` record carrying `sets` (VPS/SPS/PPS NAL units), 4-byte NAL lengths.
fn hvcc(sets: &[&[u8]]) -> Vec<u8> {
    let mut v = vec![0u8; 23];
    v[0] = 1;
    v[21] = 0x03;
    let mut arrays = 0u8;
    for ty in 32..=34u8 {
        let of: Vec<&&[u8]> = sets.iter().filter(|n| nal_type(n) == ty).collect();
        if of.is_empty() {
            continue;
        }
        arrays += 1;
        v.push(0x80 | ty);
        v.extend_from_slice(&(of.len() as u16).to_be_bytes());
        for n in of {
            v.extend_from_slice(&(n.len() as u16).to_be_bytes());
            v.extend_from_slice(n);
        }
    }
    v[22] = arrays;
    v
}

fn sample(au: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    for n in au {
        v.extend_from_slice(&(n.len() as u32).to_be_bytes());
        v.extend_from_slice(n);
    }
    v
}

/// The stream's SPSs turn scaling lists on.
fn uses_scaling_lists(nals: &[&[u8]]) -> bool {
    nals.iter().filter(|n| nal_type(n) == 33).any(|n| {
        SeqParameterSet::parse(&strip_emulation_prevention(&n[2..]))
            .is_ok_and(|s| s.scaling_list_enabled_flag)
    })
}

/// Width, height and tightly packed planes.
type Picture = (u32, u32, [Vec<u8>; 3]);

/// A decoded picture as tightly packed planes (8-bit samples as bytes, deeper ones as
/// little-endian u16), the layout of [`VideoFrame`].
fn packed(f: &hevc::Frame) -> Picture {
    let wide = f.bit_depth > 8;
    let plane = |p: &[u16], stride: u32, w: u32, rows: u32| {
        let mut out = Vec::new();
        for r in 0..rows as usize {
            for &s in &p[r * stride as usize..][..w as usize] {
                if wide {
                    out.extend_from_slice(&s.to_le_bytes());
                } else {
                    out.push(s as u8);
                }
            }
        }
        out
    };
    let (cw, ch) = f.chroma.chroma_size(f.width, f.height);
    (
        f.width,
        f.height,
        [
            plane(&f.y, f.y_stride, f.width, f.height),
            plane(&f.cb, f.c_stride, cw, ch),
            plane(&f.cr, f.c_stride, cw, ch),
        ],
    )
}

/// Streams compared by default besides the scaling-list ones.
const DEFAULT: [&str; 8] = [
    "SLPPLP",
    "BUMPING",
    "NoOutPrior",
    "OPFLAG",
    "RAP_",
    "EXT_A_ericsson_3",
    "PS_A_VIDYO_3",
    "NUT_A",
];

enum Outcome {
    /// As many pictures, all identical.
    Same(usize),
    Refused(String),
    Differs(String),
}

fn compare(path: &Path) -> Outcome {
    let data = std::fs::read(path).unwrap();
    let nals = split_annexb(&data);
    let aus = access_units(&nals);
    let first: Vec<&[u8]> = aus
        .iter()
        .flatten()
        .copied()
        .take_while(|n| nal_type(n) >= 32)
        .filter(|n| matches!(nal_type(n), 32..=34))
        .collect();

    // Software, tagged with the access unit each picture came from.
    let mut sw = hevc::VideoDecoder::new(&[], 2).unwrap();
    let mut want: Vec<Option<Picture>> = vec![None; aus.len()];
    let mut take = |sw: &mut hevc::VideoDecoder| {
        while let Some(d) = sw.next_output() {
            want[d.tag as usize] = Some(packed(&d.frame));
        }
    };
    for (i, au) in aus.iter().enumerate() {
        if let Err(e) = sw.decode_tagged(au, i as i64, None) {
            return Outcome::Refused(format!("software: AU {i}: {e}"));
        }
        take(&mut sw);
    }
    sw.flush();
    take(&mut sw);

    let mut hw = match VideoDecoder::new(VideoCodec::Hevc, &hvcc(&first)) {
        Ok(d) => d,
        Err(e) => return Outcome::Refused(format!("hardware: {e}")),
    };
    // Hardware, tagged the same way: pictures come out in decoding order.
    let mut got: Vec<Option<VideoFrame>> = (0..aus.len()).map(|_| None).collect();
    let mut last = None;
    let mut misplaced = None;
    let mut take = |hw: &mut VideoDecoder| {
        while let Some((tag, f)) = hw.next_output() {
            let i = tag as usize;
            if last.is_some_and(|l| l >= i) || got[i].is_some() {
                misplaced.get_or_insert(i);
            }
            last = Some(i);
            got[i] = Some(f);
        }
    };
    for (i, au) in aus.iter().enumerate() {
        if let Err(e) = hw.decode(&sample(au), i as i64) {
            return Outcome::Refused(format!("hardware: AU {i}: {e}"));
        }
        take(&mut hw);
    }
    hw.flush();
    take(&mut hw);
    if let Some(i) = misplaced {
        return Outcome::Differs(format!("AU {i}: picture out of decoding order or twice"));
    }

    let (mut same, mut wrong, mut first_wrong) = (0, 0, None);
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        let problem = match (g, w) {
            (None, None) => continue,
            (Some(g), Some((w, h, planes))) => {
                if (g.width, g.height) == (*w, *h) && g.planes == *planes {
                    same += 1;
                    continue;
                }
                let diff = (0..3)
                    .filter(|&k| g.planes[k] != planes[k])
                    .map(|k| ["Y", "Cb", "Cr"][k])
                    .collect::<Vec<_>>()
                    .join("+");
                format!("AU {i}: {}x{} vs {w}x{h}, {diff} differ", g.width, g.height)
            }
            (None, Some(_)) => format!("AU {i}: no hardware picture"),
            (Some(_), None) => format!("AU {i}: hardware outputs a picture software does not"),
        };
        wrong += 1;
        first_wrong.get_or_insert(problem);
    }
    if wrong > 0 {
        Outcome::Differs(format!(
            "{wrong} of {} pictures; first {}",
            wrong + same,
            first_wrong.unwrap()
        ))
    } else if same == 0 {
        Outcome::Refused("hardware gave no picture".into())
    } else {
        Outcome::Same(same)
    }
}

#[test]
fn hardware_matches_software_on_scaling_list_streams() {
    let Some(dir) = corpus() else {
        eprintln!("skipped: run tools/corpus/hevc-conformance.sh");
        return;
    };
    specular_hwdec::set_enabled(true);
    if !specular_hwdec::video::video_supported(VideoCodec::Hevc) {
        assert!(
            std::env::var_os("SPECULAR_REQUIRE_GPU").is_none(),
            "SPECULAR_REQUIRE_GPU is set but there is no VA-API HEVC decoder"
        );
        eprintln!("skipped: no VA-API HEVC decoder");
        return;
    }
    let only = std::env::var("SPECULAR_HEVC_CONF").ok();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "bit" || e == "bin"))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            only.as_deref().is_none_or(|o| o == "all" || n.contains(o))
        })
        .collect();
    files.sort();
    let (mut checked, mut refused, mut bad) = (Vec::new(), Vec::new(), Vec::new());
    for p in &files {
        let name = p.file_stem().unwrap().to_string_lossy().into_owned();
        if only.is_none() && !DEFAULT.iter().any(|d| name.starts_with(d)) {
            let data = std::fs::read(p).unwrap();
            if !uses_scaling_lists(&split_annexb(&data)) {
                continue;
            }
        }
        match compare(p) {
            Outcome::Same(n) => {
                eprintln!("{name}: {n} pictures identical");
                checked.push(name);
            }
            Outcome::Refused(e) => {
                eprintln!("{name}: not compared ({e})");
                refused.push(name);
            }
            Outcome::Differs(e) => {
                eprintln!("{name}: DIFFERS: {e}");
                bad.push(format!("{name}: {e}"));
            }
        }
    }
    eprintln!(
        "{} streams identical, {} not compared, {} differ",
        checked.len(),
        refused.len(),
        bad.len()
    );
    assert!(bad.is_empty(), "hardware differs from software: {bad:#?}");
    if only.is_none() {
        // The scaling-list streams are 8-bit 4:2:0 Main: every VA-API HEVC decoder takes them.
        for want in [
            "SLIST_A",
            "SLIST_B",
            "SLIST_C",
            "SLIST_D",
            "SLPPLP",
            "BUMPING",
            "NoOutPrior_A",
            "NoOutPrior_B",
            "OPFLAG_A",
            "OPFLAG_B",
            "OPFLAG_C",
        ] {
            assert!(
                checked.iter().any(|n| n.starts_with(want))
                    || !files.iter().any(|p| p
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(want)),
                "{want} was not compared"
            );
        }
    }
}
