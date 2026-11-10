//! The libjxl conformance suite (github.com/libjxl/conformance): every test case decoded with
//! specular-jxl and compared with libjxl's reference decode.
//!
//! The corpus is gitignored and fetched by script:
//! `tools/corpus/fetch-conformance.sh src_jxl large_jxl_refs` (inputs, `test.json`, and the
//! ~0.6 GB of reference images). Without it the test prints a message and passes.
//! `SPECULAR_CONFORMANCE_DIR` overrides the corpus root (`tools/corpus/conformance`), e.g. to
//! share one copy between worktrees.
//!
//! Per case, after orientation, a case conforms when either check passes (both are printed):
//! * **reference**: when our output is in the reference's colour space (same ICC bytes, the
//!   same matrix/gamma profile, or the same encoding description, e.g. `RGB_D65_SRG_Rel_SRG`,
//!   possibly up to the transfer function, which is then re-encoded analytically), the
//!   conformance limits from `test.json` (peak and per-channel RMS error, per frame), widened
//!   by the 8-bit output's own rounding (0.5/255 peak, 1/(255·√12) RMS; twice that for
//!   associated alpha, where ours is multiplied out), against the reference clamped to
//!   `[0, 1]` (our output is display-referred);
//! * **jxl-oxide 0.12.6** (the upstream of the fork): at most 1 LSB from its `f32` render
//!   quantised to 8 bits.
//!
//! CMYK cases (`cmyk_layers`), whose reference is in the CMYK profile, compare the inks of
//! [`JxlFile::decode_cmyk`] (and alpha) with the reference under the same limits. The tests in
//! `cmyk.rs` check the 8-bit sRGB paths of the same file. `JXL_CONFORMANCE_CASES`
//! (comma-separated) restricts the run to some cases.

use std::path::{Path, PathBuf};

use specular_jxl::{JxlFile, Layout};

fn corpus() -> Option<PathBuf> {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        });
    let dir = root.join("jxl-conformance/testcases");
    dir.is_dir().then_some(dir)
}

// ------------------------------------------------------------------------------ inputs

/// The parts of `test.json` used here.
struct Desc {
    /// (peak, RMS) error limit per frame.
    limits: Vec<(f64, f64)>,
    extra_channels: Vec<String>,
}

/// A minimal reader for the flat `test.json` files (no JSON crate in the tree).
fn read_desc(text: &str) -> Desc {
    let num = |key: &str, from: &str| -> Vec<f64> {
        let mut out = Vec::new();
        let mut rest = from;
        while let Some(i) = rest.find(&format!("\"{key}\"")) {
            rest = &rest[i + key.len() + 2..];
            let v = rest.trim_start_matches([':', ' ']);
            let end = v
                .find(|c: char| !(c.is_ascii_digit() || "+-.eE".contains(c)))
                .unwrap_or(v.len());
            out.push(v[..end].parse().expect("number in test.json"));
        }
        out
    };
    let frames = &text[text.find("\"frames\"").expect("frames")..];
    let frames = &frames[..frames.find(']').expect("end of frames")];
    let peak = num("peak_error", frames);
    let rms = num("rms_error", frames);
    assert_eq!(peak.len(), rms.len(), "test.json limits");
    let extra_channels = text
        .find("\"extra_channel_type\"")
        .map(|i| {
            let s = &text[i..];
            let s = &s[s.find('[').unwrap() + 1..s.find(']').unwrap()];
            s.split(',')
                .map(|t| t.trim().trim_matches('"').to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Desc {
        limits: peak.into_iter().zip(rms).collect(),
        extra_channels,
    }
}

/// A float32 `.npy` array: (shape, data).
fn read_npy(bytes: &[u8]) -> (Vec<usize>, Vec<f32>) {
    assert_eq!(&bytes[..6], b"\x93NUMPY", "npy magic");
    let (hlen, start) = match bytes[6] {
        1 => (u16::from_le_bytes([bytes[8], bytes[9]]) as usize, 10),
        _ => (
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize,
            12,
        ),
    };
    let header = std::str::from_utf8(&bytes[start..start + hlen]).unwrap();
    assert!(header.contains("'<f4'"), "npy dtype: {header}");
    assert!(header.contains("'fortran_order': False"), "npy order");
    let shape = &header[header.find("'shape': (").unwrap() + 10..];
    let shape: Vec<usize> = shape[..shape.find(')').unwrap()]
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let data: Vec<f32> = bytes[start + hlen..]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(data.len(), shape.iter().product::<usize>(), "npy size");
    (shape, data)
}

/// The `desc` text of an ICC profile (`mluc` or `desc` type).
fn icc_description(icc: &[u8]) -> Option<String> {
    let be32 = |o: usize| Some(u32::from_be_bytes(icc.get(o..o + 4)?.try_into().ok()?) as usize);
    let n = be32(128)?;
    for i in 0..n {
        let e = 132 + 12 * i;
        if icc.get(e..e + 4)? != b"desc" {
            continue;
        }
        let (off, size) = (be32(e + 4)?, be32(e + 8)?);
        let t = icc.get(off..off + size)?;
        return if t.starts_with(b"mluc") {
            let len = u32::from_be_bytes(t.get(20..24)?.try_into().ok()?) as usize;
            let at = u32::from_be_bytes(t.get(24..28)?.try_into().ok()?) as usize;
            let units: Vec<u16> = t
                .get(at..at + len)?
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&units).ok()
        } else {
            let len = u32::from_be_bytes(t.get(8..12)?.try_into().ok()?) as usize;
            let s = t.get(12..12 + len.saturating_sub(1))?;
            Some(String::from_utf8_lossy(s).into_owned())
        };
    }
    None
}

/// A transfer function named in a libjxl-style profile description.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Trc {
    Srgb,
    Linear,
    Bt709,
    /// Encoding exponent (`g0.45455`: the signal is linear^0.45455).
    Gamma(f64),
}

impl Trc {
    fn parse(s: &str) -> Option<Trc> {
        match s {
            "SRG" => Some(Trc::Srgb),
            "Lin" => Some(Trc::Linear),
            "709" => Some(Trc::Bt709),
            _ => s.strip_prefix('g')?.parse().ok().map(Trc::Gamma),
        }
    }

    fn decode(self, v: f64) -> f64 {
        match self {
            Trc::Srgb if v <= 0.04045 => v / 12.92,
            Trc::Srgb => ((v + 0.055) / 1.055).powf(2.4),
            Trc::Linear => v,
            Trc::Bt709 if v < 0.081 => v / 4.5,
            Trc::Bt709 => ((v + 0.099) / 1.099).powf(1.0 / 0.45),
            Trc::Gamma(g) => v.powf(1.0 / g),
        }
    }

    fn encode(self, v: f64) -> f64 {
        match self {
            Trc::Srgb if v <= 0.0031308 => v * 12.92,
            Trc::Srgb => 1.055 * v.powf(1.0 / 2.4) - 0.055,
            Trc::Linear => v,
            Trc::Bt709 if v < 0.018 => v * 4.5,
            Trc::Bt709 => 1.099 * v.powf(0.45) - 0.099,
            Trc::Gamma(g) => v.powf(g),
        }
    }
}

/// An RGB matrix/TRC profile with a pure-gamma tone curve: the white point, the three
/// colorant XYZs and the gamma, for comparing profiles that differ only in naming (libjxl
/// conformance `spot` embeds "ProPhoto RGB"; we describe the same space from the header).
fn matrix_trc(icc: &[u8]) -> Option<[f64; 13]> {
    let be32 = |o: usize| Some(u32::from_be_bytes(icc.get(o..o + 4)?.try_into().ok()?));
    let s15 = |o: usize| Some(be32(o)? as i32 as f64 / 65536.0);
    let tag = |sig: &[u8]| -> Option<usize> {
        (0..be32(128)? as usize)
            .map(|i| 132 + 12 * i)
            .find(|&e| icc.get(e..e + 4) == Some(sig))
            .and_then(|e| be32(e + 4).map(|o| o as usize))
    };
    let mut out = [0.0; 13];
    for (i, sig) in [b"wtpt", b"rXYZ", b"gXYZ", b"bXYZ"].iter().enumerate() {
        let o = tag(*sig)?;
        for j in 0..3 {
            out[3 * i + j] = s15(o + 8 + 4 * j)?;
        }
    }
    let o = tag(b"rTRC")?;
    out[12] = match icc.get(o..o + 4)? {
        b"curv" if be32(o + 8)? == 1 => {
            u16::from_be_bytes([icc[o + 12], icc[o + 13]]) as f64 / 256.0
        }
        b"para" if icc.get(o + 8..o + 10)? == [0, 0] => s15(o + 12)?,
        _ => return None,
    };
    let same = |sig: &[u8]| tag(sig).is_some_and(|p| icc.get(p..p + 14) == icc.get(o..o + 14));
    (same(b"gTRC") && same(b"bTRC")).then_some(out)
}

/// A profile description split into (colour space, white point and primaries in libjxl
/// spelling; transfer function), for the two naming schemes: libjxl's
/// (`RGB_D65_SRG_Per_709`: space, white point, primaries, intent, transfer) and jxl-oxide's
/// (`Rgb_Perceptual_D65_Srgb_Bt709`: space, intent, white point, primaries, transfer).
fn split_description(d: &str) -> Option<(String, Trc)> {
    let mut f: Vec<&str> = d.split('_').collect();
    if let Some(trc) = Trc::parse(f.last()?) {
        f.pop();
        f.pop()?; // rendering intent
        return (f.len() >= 2).then(|| (f.join("_"), trc));
    }
    // jxl-oxide: only the simple (enum, no custom xy) forms.
    let trc = match f.pop()? {
        "Srgb" => Trc::Srgb,
        "Linear" => Trc::Linear,
        "Bt709" => Trc::Bt709,
        s => {
            // `Gamma { g: 4545500, inverted: true }`: g / 10⁷, inverted when it is the
            // encoding exponent.
            let n: f64 = s
                .strip_prefix("Gamma { g: ")?
                .split(',')
                .next()?
                .parse()
                .ok()?;
            let g = n / 1e7;
            Trc::Gamma(if s.contains("inverted: true") {
                g
            } else {
                1.0 / g
            })
        }
    };
    let space = match f.first()? {
        &"Rgb" => "RGB",
        &"Gray" | &"Grey" => "Gra",
        _ => return None,
    };
    let rest = f[2..]
        .iter()
        .map(|p| match *p {
            "D65" => Some("D65"),
            "Srgb" | "Bt709" => Some("SRG"),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((
        [space]
            .into_iter()
            .chain(rest)
            .collect::<Vec<_>>()
            .join("_"),
        trc,
    ))
}

/// How the reference relates to our output's colour space: `None` when they differ in more
/// than the transfer function; `Some(None)` when they are the same; `Some(Some((from, to)))`
/// when only the transfer function differs (reference colour is re-encoded from `from` to
/// `to`). libjxl and jxl-oxide name the profiles they synthesise after the encoding
/// (`RGB_D65_SRG_Rel_SRG`: colour space, white point, primaries, rendering intent, transfer
/// function); the intent does not change the pixels of a matrix/TRC profile.
fn relation(reference: &[u8], ours: Option<&[u8]>, gray: bool) -> Option<Option<(Trc, Trc)>> {
    let ours_desc = match ours {
        Some(icc) if icc == reference => return Some(None),
        Some(icc) => icc_description(icc)?,
        None if gray => "Gra_D65_Rel_SRG".into(),
        None => "RGB_D65_SRG_Rel_SRG".into(),
    };
    if let Some(ours) = ours
        && let (Some(a), Some(b)) = (matrix_trc(reference), matrix_trc(ours))
        && a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 2e-3)
    {
        return Some(None);
    }
    let ref_desc = icc_description(reference)?;
    let ((rs, rt), (os, ot)) = (
        split_description(&ref_desc)?,
        split_description(&ours_desc)?,
    );
    (rs == os).then_some((rt != ot).then_some((rt, ot)))
}

// ------------------------------------------------------------------------------ decoders

/// EXIF orientation applied to an interleaved buffer: (pixels, width, height).
fn orient<T: Copy + Default>(
    src: &[T],
    (w, h, c): (usize, usize, usize),
    o: u32,
) -> (Vec<T>, usize, usize) {
    let (ow, oh) = if o >= 5 { (h, w) } else { (w, h) };
    let mut out = vec![T::default(); src.len()];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match o {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (h - 1 - y, x),
                7 => (h - 1 - y, w - 1 - x),
                8 => (y, w - 1 - x),
                _ => (x, y),
            };
            let (s, d) = ((y * w + x) * c, (dy * ow + dx) * c);
            out[d..d + c].copy_from_slice(&src[s..s + c]);
        }
    }
    (out, ow, oh)
}

/// Our decode, oriented: per frame, interleaved colour (1 or 3) + alpha, in `[0, 1]`.
struct Ours {
    w: usize,
    h: usize,
    colour: usize,
    alpha: bool,
    frames: Vec<Vec<u8>>,
    icc: Option<Vec<u8>>,
}

fn decode_ours(data: &[u8]) -> specular_jxl::Result<Ours> {
    let f = JxlFile::open(data)?;
    let info = f.info().clone();
    let mut out = Ours {
        w: 0,
        h: 0,
        colour: if info.gray { 1 } else { 3 },
        alpha: false,
        frames: Vec::new(),
        icc: info.output_icc.clone(),
    };
    for d in f.frames(1, true)? {
        let d = d?;
        let c = d.layout.channels();
        let (px, w, h) = orient(
            &d.data,
            (d.width as usize, d.height as usize, c),
            info.orientation,
        );
        (out.w, out.h) = (w, h);
        // Grey with alpha comes out as RGBA with the grey replicated: keep grey + alpha.
        let keep: &[usize] = match (d.layout, info.gray) {
            (Layout::Gray, _) => &[0],
            (Layout::Rgb, _) => &[0, 1, 2],
            (Layout::Rgba, true) => &[0, 3],
            (Layout::Rgba, false) => &[0, 1, 2, 3],
            (l, _) => panic!("unexpected layout {l:?}"),
        };
        out.alpha = d.layout == Layout::Rgba;
        out.frames.push(
            px.chunks_exact(c)
                .flat_map(|p| keep.iter().map(|&k| p[k]))
                .collect(),
        );
    }
    Ok(out)
}

/// jxl-oxide's `f32` render, oriented, all channels (spot colours not composited, as in the
/// references): (width, height, channels, frames).
fn decode_oxide(data: &[u8]) -> (usize, usize, usize, Vec<Vec<f32>>) {
    let mut img = jxl_oxide::JxlImage::builder().read(data).unwrap();
    img.set_render_spot_color(false);
    let mut frames = Vec::new();
    let (mut w, mut h, mut c) = (0, 0, 0);
    for i in 0..img.num_loaded_keyframes() {
        let fb = img.render_frame(i).unwrap().image_all_channels();
        (w, h, c) = (fb.width(), fb.height(), fb.channels());
        frames.push(fb.buf().to_vec());
    }
    (w, h, c, frames)
}

// ------------------------------------------------------------------------------ comparison

/// Channel `k` of our output in the reference's channel layout (colour, then extra channels).
fn ref_channel(ours: &Ours, k: usize, alpha_at: Option<usize>) -> Option<usize> {
    if k < ours.colour {
        Some(k)
    } else if ours.alpha && Some(k) == alpha_at {
        Some(ours.colour)
    } else {
        None
    }
}

/// (peak, max per-channel RMS) of our frame against a reference frame of `rc` channels.
/// `premultiplied`: the reference holds colour × alpha, ours is straight: ours is multiplied
/// out, and reference colour above its alpha (which no straight colour can express) is
/// clamped to it.
fn errors(
    ours: &Ours,
    frame: &[u8],
    reference: &[f32],
    rc: usize,
    alpha_at: Option<usize>,
    premultiplied: bool,
    trc: Option<(Trc, Trc)>,
) -> (f64, f64) {
    let oc = ours.colour + usize::from(ours.alpha);
    let n = ours.w * ours.h;
    let mut peak = 0f64;
    let mut rms = 0f64;
    for k in 0..rc {
        let Some(ok) = ref_channel(ours, k, alpha_at) else {
            continue;
        };
        let mut sum = 0f64;
        for i in 0..n {
            let mut v = frame[i * oc + ok] as f64 / 255.0;
            if premultiplied && ok < ours.colour {
                v *= frame[i * oc + ours.colour] as f64 / 255.0;
            }
            let mut r = (reference[i * rc + k] as f64).clamp(0.0, 1.0);
            if let Some((from, to)) = trc
                && ok < ours.colour
            {
                r = to.encode(from.decode(r)).clamp(0.0, 1.0);
            }
            if premultiplied
                && ok < ours.colour
                && let Some(a) = alpha_at
            {
                r = r.min((reference[i * rc + a] as f64).clamp(0.0, 1.0));
            }
            let e = (v - r).abs();
            peak = peak.max(e);
            sum += e * e;
        }
        rms = rms.max((sum / n as f64).sqrt());
    }
    (peak, rms)
}

/// Largest 8-bit difference from jxl-oxide's render (straight alpha, clamped, rounded).
fn oxide_lsb(
    ours: &Ours,
    oxide: &(usize, usize, usize, Vec<Vec<f32>>),
    alpha_at: Option<usize>,
    premultiplied: bool,
) -> Result<(u8, String), String> {
    let (w, h, c, frames) = oxide;
    if (*w, *h, frames.len()) != (ours.w, ours.h, ours.frames.len()) {
        return Err(format!(
            "jxl-oxide {w}×{h}×{} frames vs ours {}×{}×{}",
            frames.len(),
            ours.w,
            ours.h,
            ours.frames.len()
        ));
    }
    let oc = ours.colour + usize::from(ours.alpha);
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let mut worst = 0u8;
    let mut at = String::new();
    for (f, (frame, x)) in ours.frames.iter().zip(frames).enumerate() {
        for i in 0..w * h {
            let a = alpha_at.map(|k| x[i * c + k].clamp(0.0, 1.0));
            for k in 0..*c {
                let Some(ok) = ref_channel(ours, k, alpha_at) else {
                    continue;
                };
                // Straight colour under associated alpha that rounds to 0 is ill-conditioned
                // (colour / ~0) and invisible.
                if premultiplied && ok < ours.colour && a.is_some_and(|a| q(a) == 0) {
                    continue;
                }
                let mut v = x[i * c + k];
                if premultiplied && ok < ours.colour {
                    v = match a {
                        Some(a) if a > 0.0 => v / a,
                        _ => v,
                    };
                }
                let d = q(v).abs_diff(frame[i * oc + ok]);
                if d > worst {
                    worst = d;
                    at = format!(
                        " (frame {f} pixel {},{} channel {k}: ours {}, jxl-oxide {v} alpha {a:?})",
                        i % w,
                        i / w,
                        frame[i * oc + ok]
                    );
                }
            }
        }
    }
    Ok((worst, at))
}

/// One case: `Ok(summary)` when it conforms, `Err(summary)` otherwise.
fn check(dir: &Path, name: &str) -> Result<String, String> {
    let data = std::fs::read(dir.join("input.jxl")).unwrap();
    if JxlFile::open(&data[..]).is_ok_and(|f| f.info().cmyk) {
        return check_cmyk(dir, &data);
    }
    let decoded = std::panic::catch_unwind(|| decode_ours(&data))
        .map_err(|_| "specular-jxl panicked".to_string())?;
    let ours = decoded.map_err(|e| format!("decode failed: {e}"))?;
    let desc = read_desc(&std::fs::read_to_string(dir.join("test.json")).unwrap());
    let ncolour_ref = if ours.colour == 1 { 1 } else { 3 };
    let alpha_at = desc
        .extra_channels
        .iter()
        .position(|t| t == "Alpha")
        .map(|i| ncolour_ref + i);
    // Only alpha_premultiplied has associated alpha in this suite; the header says so too.
    let premultiplied = name == "alpha_premultiplied";

    // Against the reference, where it is in our colour space.
    let mut notes = Vec::new();
    let mut conforms = false;
    let reference = std::fs::read(dir.join("reference_image.npy")).ok();
    let ref_icc = std::fs::read(dir.join("reference.icc")).ok();
    match (&reference, &ref_icc) {
        (Some(npy), Some(icc))
            if let Some(trc) = relation(icc, ours.icc.as_deref(), ours.colour == 1) =>
        {
            let (shape, refdata) = read_npy(npy);
            let [nf, h, w, rc] = shape[..] else {
                return Err(format!("reference shape {shape:?}"));
            };
            if (nf, h, w) != (ours.frames.len(), ours.h, ours.w) {
                return Err(format!(
                    "reference {nf} frames {w}×{h}, ours {} frames {}×{}",
                    ours.frames.len(),
                    ours.w,
                    ours.h
                ));
            }
            // 8-bit rounding: ±0.5/255, uniform. Premultiplied, our colour × alpha carries the
            // rounding of both factors.
            let q = if premultiplied { 2.0 } else { 1.0 };
            let (q_peak, q_rms) = (
                q * 0.5 / 255.0 + 1e-5,
                f64::sqrt(q) / (255.0 * 12f64.sqrt()) + 1e-5,
            );
            let mut fail = None;
            let (mut worst_peak, mut worst_rms) = (0f64, 0f64);
            for (i, frame) in ours.frames.iter().enumerate() {
                let r = &refdata[i * h * w * rc..(i + 1) * h * w * rc];
                let (peak, rms) = errors(&ours, frame, r, rc, alpha_at, premultiplied, trc);
                let (lp, lr) = desc.limits[i.min(desc.limits.len() - 1)];
                (worst_peak, worst_rms) = (worst_peak.max(peak), worst_rms.max(rms));
                if (peak > lp + q_peak || rms > lr + q_rms) && fail.is_none() {
                    fail = Some(format!(
                        "frame {i}: peak {peak:.6} (limit {lp} + q), rms {rms:.6} (limit {lr} + q)"
                    ));
                }
            }
            match fail {
                None => {
                    conforms = true;
                    let via = match trc {
                        Some((a, b)) => format!(" via {a:?} -> {b:?}"),
                        None => String::new(),
                    };
                    notes.push(format!(
                        "reference{via}: peak {worst_peak:.6} rms {worst_rms:.6} (limits {:?})",
                        desc.limits[0]
                    ));
                }
                Some(f) => notes.push(format!("reference FAILED {f}")),
            }
        }
        (Some(_), Some(icc)) => notes.push(format!(
            "reference in another colour space ({:?}, ours {:?})",
            icc_description(icc).unwrap_or_default(),
            ours.icc.as_deref().map_or("sRGB".into(), |i| {
                icc_description(i).unwrap_or_default()
            })
        )),
        _ => notes.push("no reference files".into()),
    }

    // Against jxl-oxide (always reported; decisive when the reference is not usable).
    let oxide = decode_oxide(&data);
    match oxide_lsb(&ours, &oxide, alpha_at, premultiplied) {
        Ok((lsb, at)) => {
            conforms |= lsb <= 1;
            notes.push(format!(
                "jxl-oxide: max {lsb} LSB{}",
                if lsb > 1 { &at } else { "" }
            ));
        }
        Err(e) => notes.push(e),
    }
    if conforms {
        Ok(notes.join("; "))
    } else {
        Err(notes.join("; "))
    }
}

/// A CMYK case: the reference holds the colour channels, black and alpha as coded (1.0 = no
/// ink), in the CMYK profile; ours are [`JxlFile::decode_cmyk`]'s 8-bit inks (0 = no ink).
fn check_cmyk(dir: &Path, data: &[u8]) -> Result<String, String> {
    let file = JxlFile::open(data).map_err(|e| format!("open: {e}"))?;
    let c = std::panic::catch_unwind(|| file.decode_cmyk(true))
        .map_err(|_| "specular-jxl panicked".to_string())?
        .map_err(|e| format!("decode_cmyk failed: {e}"))?;
    let desc = read_desc(&std::fs::read_to_string(dir.join("test.json")).unwrap());
    let Ok(npy) = std::fs::read(dir.join("reference_image.npy")) else {
        return Err("no reference files".into());
    };
    let (shape, r) = read_npy(&npy);
    let [_, h, w, rc] = shape[..] else {
        return Err(format!("reference shape {shape:?}"));
    };
    if (w, h) != (c.width as usize, c.height as usize) || rc < 4 {
        return Err(format!(
            "reference {w}×{h}×{rc}, ours {}×{}",
            c.width, c.height
        ));
    }
    let black = desc.extra_channels.iter().position(|t| t == "Black");
    let alpha = desc.extra_channels.iter().position(|t| t == "Alpha");
    let Some(black) = black else {
        return Err("no Black in test.json".into());
    };
    // Peak over all channels, RMS per channel (C, M, Y, K, alpha).
    let (mut peak, mut sq, mut off) = (0f64, [0f64; 5], 0usize);
    for p in 0..w * h {
        let px = &r[p * rc..(p + 1) * rc];
        let mut pairs = [0, 1, 2, 3 + black]
            .map(|k| px[k])
            .into_iter()
            .zip(
                c.ink[p * 4..p * 4 + 4]
                    .iter()
                    .map(|&i| 1.0 - f32::from(i) / 255.0),
            )
            .collect::<Vec<_>>();
        if let (Some(a), Some(ours)) = (alpha, &c.alpha) {
            pairs.push((px[3 + a], f32::from(ours[p]) / 255.0));
        }
        for (k, (want, got)) in pairs.into_iter().enumerate() {
            let d = f64::from((want.clamp(0.0, 1.0) - got).abs());
            peak = peak.max(d);
            sq[k] += d * d;
            off += usize::from((want.clamp(0.0, 1.0) * 255.0).round() != (got * 255.0).round());
        }
    }
    let rms = sq
        .iter()
        .map(|s| (s / (w * h) as f64).sqrt())
        .fold(0f64, f64::max);
    let (lp, lr) = desc.limits[0];
    let (q_peak, q_rms) = (0.5 / 255.0 + 1e-5, 1.0 / (255.0 * 12f64.sqrt()) + 1e-5);
    let summary = format!(
        "CMYK inks: peak {peak:.6} rms {rms:.6} (limits {:?}), {off} 8-bit samples off",
        desc.limits[0]
    );
    if peak <= lp + q_peak && rms <= lr + q_rms {
        Ok(summary)
    } else {
        Err(summary)
    }
}

#[test]
fn libjxl_conformance() {
    let Some(dir) = corpus() else {
        eprintln!(
            "SKIPPED: libjxl conformance corpus not found; fetch it with \
             tools/corpus/fetch-conformance.sh src_jxl large_jxl_refs"
        );
        return;
    };
    let mut cases: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("input.jxl").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    cases.sort();
    if let Ok(only) = std::env::var("JXL_CONFORMANCE_CASES") {
        cases.retain(|c| only.split(',').any(|o| o == c));
    }
    assert!(!cases.is_empty(), "no test cases under {}", dir.display());
    let mut failures = Vec::new();
    for case in &cases {
        match check(&dir.join(case), case) {
            Ok(s) => eprintln!("ok    {case:28} {s}"),
            Err(s) => {
                eprintln!("FAIL  {case:28} {s}");
                failures.push(case.clone());
            }
        }
    }
    assert!(failures.is_empty(), "non-conforming: {failures:?}");
}
