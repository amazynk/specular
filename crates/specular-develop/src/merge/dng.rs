//! The merge as a floating point DNG (DNG 1.4): linear, 16-, 24- or 32-bit floats, the
//! mosaic as it is (CFA) or demosaiced (LinearRaw), deflate with the floating point
//! predictor in 256×256 tiles.
//!
//! - **Values** are the radiance divided by the brightest, so they run 0…1 against a white
//!   level of 1 (readers that clip at white keep everything); `BaselineExposure` carries
//!   log2 of that brightest value, so the file develops at the reference frame's brightness.
//! - **Colour** is the reference's: its DNG colour tags, else the camera's built-in profile or
//!   Adobe matrix ([`specular_raw::dng_colour_for`]), else one derived from the reference's
//!   camera matrix and white balance. `AsShotNeutral` is the reference's white balance.
//! - **Metadata**: the reference's Exif (and GPS), its orientation, and an XMP packet naming
//!   the source files (`spm:sources`, namespace `http://ns.specular.app/merge/1/`).
//! - **Layout**: IFD0 is an 8-bit RGB preview (long side up to 1024), the raw is its SubIFD;
//!   all directories and the XMP come before the pixel data, so the first 256 KiB of the
//!   file hold every tag ([`sources_of`] reads only that much).

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use specular_convert::exif::Entry;

use super::Merged;

/// Bits per float sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bits {
    F16,
    #[default]
    F24,
    F32,
}

impl Bits {
    fn bytes(self) -> usize {
        match self {
            Bits::F16 => 2,
            Bits::F24 => 3,
            Bits::F32 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DngOptions {
    pub bits: Bits,
    /// Keep the colour filter mosaic (true) or write demosaiced camera RGB.
    pub cfa: bool,
}

impl Default for DngOptions {
    fn default() -> Self {
        DngOptions {
            bits: Bits::F24,
            cfa: true,
        }
    }
}

/// The XMP namespace of a merge's facts.
pub const NS: &str = "http://ns.specular.app/merge/1/";
const TILE: usize = 256;
/// The longest side of the preview in IFD0.
const PREVIEW: usize = 1024;

/// What a merged file is: its name's suffix tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An exposure bracket (`-HDR`).
    Hdr,
    /// A panorama (`-Pano`).
    Pano,
    /// A focus stack (`-Stack`).
    Stack,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Hdr, Kind::Pano, Kind::Stack];

    pub fn suffix(self) -> &'static str {
        match self {
            Kind::Hdr => "HDR",
            Kind::Pano => "Pano",
            Kind::Stack => "Stack",
        }
    }
}

/// Extensions a merge is written with.
const MERGED_EXT: [&str; 5] = ["dng", "tif", "tiff", "jxl", "exr"];

/// The file name of the merge of `reference`: `IMG_0002-HDR.dng` (the writer adds `-2`,
/// `-3` when that name is taken).
pub fn file_name(reference: &Path) -> String {
    file_name_of(reference, Kind::Hdr, "dng")
}

/// The file name of a merge of `kind` named after `reference`, with extension `ext`:
/// `IMG_0002-Pano.dng`, `IMG_0002-Stack.tif`.
pub fn file_name_of(reference: &Path, kind: Kind, ext: &str) -> String {
    let stem = reference
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "merge".into());
    format!("{stem}-{}.{ext}", kind.suffix())
}

/// The kind of merge `path`'s name says it is ([`file_name_of`], with or without a
/// numbered suffix).
pub fn kind_of(path: &Path) -> Option<Kind> {
    let name = path.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    let (stem, ext) = lower.rsplit_once('.')?;
    if !MERGED_EXT.contains(&ext) {
        return None;
    }
    Kind::ALL.into_iter().find(|k| {
        let tag = format!("-{}", k.suffix().to_ascii_lowercase());
        stem.ends_with(&tag)
            || stem
                .rsplit_once(&format!("{tag}-"))
                .is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// Whether `path`'s name looks like a merge's of any kind.
pub fn looks_merged(path: &Path) -> bool {
    kind_of(path).is_some()
}

/// Writes `bytes` as `name` beside `reference` (never over anything; `-2`, `-3` added when
/// taken) and returns the path.
pub fn write_beside(reference: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let dir = reference
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    match specular_convert::write::write_new(
        &dir,
        name,
        bytes,
        specular_convert::Conflict::Suffix,
        &|_| false,
    )? {
        specular_convert::write::Written::Created(p) => Ok(p),
        specular_convert::write::Written::Exists(p) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            p.display().to_string(),
        )),
    }
}

/// Writes the merge next to the reference (never over anything) and returns its path.
pub fn save(m: &Merged, reference_bytes: &[u8], o: &DngOptions) -> std::io::Result<PathBuf> {
    save_as(m, reference_bytes, o, Kind::Hdr)
}

/// [`save`] for a merge of `kind`.
pub fn save_as(
    m: &Merged,
    reference_bytes: &[u8],
    o: &DngOptions,
    kind: Kind,
) -> std::io::Result<PathBuf> {
    let r = &m.shots[m.reference].path;
    let bytes = encode(m, reference_bytes, o);
    write_beside(r, &file_name_of(r, kind, "dng"), &bytes)
}

/// The source file names a merge's XMP lists (from the file's first 256 KiB), with the
/// reference's name.
pub fn sources_of(head: &[u8]) -> Option<(Vec<String>, String)> {
    let head = &head[..head.len().min(256 * 1024)];
    let find = |pat: &[u8], from: usize| {
        head.get(from..)?
            .windows(pat.len())
            .position(|w| w == pat)
            .map(|p| p + from)
    };
    let a = find(b"<x:xmpmeta", 0)?;
    let b = find(b"</x:xmpmeta>", a)? + b"</x:xmpmeta>".len();
    let text = std::str::from_utf8(&head[a..b]).ok()?;
    let doc = roxmltree::Document::parse(text).ok()?;
    let desc = doc
        .descendants()
        .find(|n| n.attribute((NS, "reference")).is_some())?;
    let reference = desc.attribute((NS, "reference"))?.to_string();
    let sources = desc
        .descendants()
        .filter(|n| n.has_tag_name(("http://www.w3.org/1999/02/22-rdf-syntax-ns#", "li")))
        .filter_map(|n| n.text().map(str::to_string))
        .collect();
    Some((sources, reference))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xmp(m: &Merged) -> Vec<u8> {
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let items: String = m
        .shots
        .iter()
        .map(|s| format!("<rdf:li>{}</rdf:li>", esc(&name(&s.path))))
        .collect();
    let exps: Vec<String> = m
        .exposures
        .iter()
        .map(|e| format!("{:.4}", e.log2()))
        .collect();
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" xmlns:spm=\"{NS}\" spm:reference=\"{}\" spm:exposures=\"{}\" spm:ghosts=\"{:.4}\">\
<spm:sources><rdf:Seq>{items}</rdf:Seq></spm:sources></rdf:Description></rdf:RDF></x:xmpmeta>\
<?xpacket end=\"w\"?>",
        esc(&name(&m.shots[m.reference].path)),
        exps.join(" "),
        m.ghosts.coverage(),
    )
    .into_bytes()
}

fn srational(tag: u16, v: &[f64]) -> Entry {
    let data = v
        .iter()
        .flat_map(|&x| {
            let n = (x * 1_000_000.0)
                .round()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
            let mut b = n.to_le_bytes().to_vec();
            b.extend_from_slice(&1_000_000i32.to_le_bytes());
            b
        })
        .collect();
    Entry {
        tag,
        ty: 10,
        count: v.len() as u32,
        data,
    }
}

fn rational(tag: u16, v: &[f64]) -> Entry {
    let data = v
        .iter()
        .flat_map(|&x| {
            let n = (x.max(0.0) * 1_000_000.0).round().min(u32::MAX as f64) as u32;
            let mut b = n.to_le_bytes().to_vec();
            b.extend_from_slice(&1_000_000u32.to_le_bytes());
            b
        })
        .collect();
    Entry {
        tag,
        ty: 5,
        count: v.len() as u32,
        data,
    }
}

fn bytes(tag: u16, v: &[u8]) -> Entry {
    Entry {
        tag,
        ty: 1,
        count: v.len() as u32,
        data: v.to_vec(),
    }
}

/// The DNG 24-bit float: sign, 7-bit exponent (bias 63), 16-bit mantissa; rounded.
fn f24(v: f32) -> [u8; 3] {
    let b = v.to_bits();
    let sign = (b >> 31) << 23;
    let e = ((b >> 23) & 0xff) as i32;
    let m = b & 0x7f_ffff;
    let out = if v.is_nan() {
        0x7f_ffff
    } else if e == 0xff {
        sign | 0x7f << 16
    } else if e == 0 {
        sign
    } else {
        let e24 = e - 127 + 63;
        if e24 >= 127 {
            sign | 0x7f << 16
        } else if e24 <= 0 {
            // Subnormal: mantissa with the hidden bit, shifted.
            let full = (m | 0x80_0000) as u64;
            let shift = 7 + (1 - e24) as u64;
            if shift > 40 {
                sign
            } else {
                let q = (full + (1 << (shift - 1))) >> shift;
                sign | q as u32
            }
        } else {
            // Round to nearest 16 mantissa bits; a carry bumps the exponent.
            let q = (m + 0x40) >> 7;
            let (e24, q) = if q == 0x1_0000 {
                (e24 + 1, 0)
            } else {
                (e24, q)
            };
            if e24 >= 127 {
                sign | 0x7f << 16
            } else {
                sign | (e24 as u32) << 16 | q
            }
        }
    };
    [(out >> 16) as u8, (out >> 8) as u8, out as u8]
}

/// One sample's bytes, most significant first.
fn sample_be(v: f32, bits: Bits, out: &mut [u8]) {
    match bits {
        Bits::F16 => out.copy_from_slice(&half::f16::from_f32(v).to_bits().to_be_bytes()),
        Bits::F24 => out.copy_from_slice(&f24(v)),
        Bits::F32 => out.copy_from_slice(&v.to_be_bytes()),
    }
}

/// Row `ty` of the tiles of `w × h × cpp` values (row-major) divided by `max`, predicted
/// (floating point, predictor 3) and deflated, left to right.
fn tile_row(
    v: &[f32],
    w: usize,
    h: usize,
    cpp: usize,
    bits: Bits,
    ty: usize,
    max: f32,
) -> Vec<Vec<u8>> {
    let nb = bits.bytes();
    let across = w.div_ceil(TILE);
    (0..across)
        .into_par_iter()
        .map(|tx| {
            let row_samples = TILE * cpp;
            let mut raw = vec![0u8; TILE * row_samples * nb];
            let mut sample = vec![0u8; nb];
            let mut plain = vec![0u8; row_samples * nb];
            for r in 0..TILE {
                // Edge tiles repeat the last row and column.
                let y = (ty * TILE + r).min(h - 1);
                for c in 0..TILE {
                    let x = (tx * TILE + c).min(w - 1);
                    for ch in 0..cpp {
                        sample_be(v[(y * w + x) * cpp + ch] / max, bits, &mut sample);
                        let i = c * cpp + ch;
                        // Byte planes, most significant first.
                        for (b, &byte) in sample.iter().enumerate() {
                            plain[b * row_samples + i] = byte;
                        }
                    }
                }
                let dst = &mut raw[r * row_samples * nb..][..row_samples * nb];
                dst.copy_from_slice(&plain);
                for i in (cpp..dst.len()).rev() {
                    dst[i] = dst[i].wrapping_sub(dst[i - cpp]);
                }
            }
            deflate(&raw)
        })
        .collect()
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; zlib_rs::compress_bound(data.len())];
    let (c, rc) = zlib_rs::compress_slice(&mut out, data, zlib_rs::DeflateConfig::new(6));
    if rc != zlib_rs::ReturnCode::Ok {
        // Cannot happen with a bound-sized buffer; store nothing rather than panic.
        return Vec::new();
    }
    let n = c.len();
    out.truncate(n);
    out
}

/// Demosaiced camera RGB of the merge: each missing colour the weighted mean of its
/// nearest samples of that colour (bilinear on Bayer, a 5×5 tent on X-Trans), present
/// colours kept exactly. A plain demosaic: Develop's own runs on the mosaic.
fn demosaic(m: &Merged) -> Vec<f32> {
    let (w, h) = (m.width as usize, m.height as usize);
    let r: i64 = if matches!(m.mosaic, super::Mosaic::Bayer(_)) {
        1
    } else {
        2
    };
    let mut out = vec![0f32; w * h * 3];
    out.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let own = m.mosaic.color(x as u32, y as u32) as usize;
            let mut sum = [0f32; 3];
            let mut wsum = [0f32; 3];
            for dy in -r..=r {
                let yy = y as i64 + dy;
                if yy < 0 || yy >= h as i64 {
                    continue;
                }
                for dx in -r..=r {
                    let xx = x as i64 + dx;
                    if xx < 0 || xx >= w as i64 {
                        continue;
                    }
                    let c = m.mosaic.color(xx as u32, yy as u32) as usize;
                    let wt = ((r + 1 - dx.abs()) * (r + 1 - dy.abs())) as f32;
                    sum[c] += wt * m.radiance[yy as usize * w + xx as usize];
                    wsum[c] += wt;
                }
            }
            for c in 0..3 {
                row[x * 3 + c] = if c == own {
                    m.radiance[y * w + x]
                } else if wsum[c] > 0.0 {
                    sum[c] / wsum[c]
                } else {
                    0.0
                };
            }
        }
    });
    out
}

/// An sRGB preview of the merge for IFD0: (width, height, RGB8).
fn preview(m: &Merged) -> (usize, usize, Vec<u8>) {
    let b = m.mosaic.block() as usize;
    let (w, h, cpp) = (m.width as usize, m.height as usize, m.cpp as usize);
    let (bw, bh) = (w / b, h / b);
    let step = (bw.max(bh)).div_ceil(PREVIEW).max(1);
    let (pw, ph) = ((bw / step).max(1), (bh / step).max(1));
    let wb = m.meta.wb;
    let mat = m.meta.rgb_cam;
    let enc = |v: f32| {
        // Soft shoulder above 0.8, then sRGB.
        let v = if v < 0.8 {
            v.max(0.0)
        } else {
            0.8 + 0.2 * ((v - 0.8) / 0.2).tanh()
        };
        let s = if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let mut out = vec![0u8; pw * ph * 3];
    out.par_chunks_mut(pw * 3)
        .enumerate()
        .for_each(|(py, row)| {
            for px in 0..pw {
                let mut sum = [0f32; 3];
                let mut cnt = [0u32; 3];
                for y in py * step * b..((py + 1) * step * b).min(h) {
                    for x in px * step * b..((px + 1) * step * b).min(w) {
                        for ch in 0..cpp {
                            let col = if cpp == 1 {
                                m.mosaic.color(x as u32, y as u32) as usize
                            } else {
                                ch
                            };
                            sum[col] += m.radiance[(y * w + x) * cpp + ch];
                            cnt[col] += 1;
                        }
                    }
                }
                let cam: [f32; 3] = std::array::from_fn(|c| {
                    let v = if cnt[c] > 0 {
                        sum[c] / cnt[c] as f32
                    } else {
                        sum[1] / cnt[1].max(1) as f32
                    };
                    v * if cpp == 1 && matches!(m.mosaic, super::Mosaic::Mono) {
                        1.0
                    } else {
                        wb[c]
                    }
                });
                for c in 0..3 {
                    let v = mat[c][0] * cam[0] + mat[c][1] * cam[1] + mat[c][2] * cam[2];
                    row[px * 3 + c] = enc(v);
                }
            }
        });
    (pw, ph, out)
}

const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [3.240_454_2, -1.537_138_5, -0.498_531_4],
    [-0.969_266_0, 1.876_010_8, 0.041_556_0],
    [0.055_643_4, -0.204_025_9, 1.057_225_2],
];

fn inv3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if d.abs() < 1e-12 {
        return None;
    }
    let c = |r: usize, k: usize| {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (k1, k2) = ((k + 1) % 3, (k + 2) % 3);
        m[r1][k1] * m[r2][k2] - m[r1][k2] * m[r2][k1]
    };
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| c(j, i) / d)
    }))
}

/// The colour tags of the merge (see the module documentation).
fn colour(m: &Merged) -> Vec<Entry> {
    let meta = &m.meta;
    let shot = &m.shots[m.reference];
    let dng = meta
        .dng
        .clone()
        .filter(|d| d.color_matrix[0].is_some())
        .or_else(|| specular_raw::dng_colour_for(&shot.make, &shot.model));
    let mut out = Vec::new();
    match dng {
        Some(d) => {
            for k in 0..2 {
                if let Some(cm) = d.color_matrix[k] {
                    out.push(srational([50721, 50722][k], &cm));
                    out.push(Entry::short(
                        [50778, 50779][k],
                        &[d.illuminants[k].max(1) as u16],
                    ));
                }
                if let Some(fm) = d.forward_matrix[k] {
                    out.push(srational([50964, 50965][k], &fm));
                }
                if let Some(cc) = d.camera_calibration[k] {
                    out.push(srational([50723, 50724][k], &cc));
                }
            }
            if let Some(ab) = d.analog_balance {
                out.push(rational(50727, &ab));
            }
        }
        None => {
            // From the camera matrix, taking the white balance for the daylight multipliers.
            let rc: [[f64; 3]; 3] = meta.rgb_cam.map(|r| r.map(f64::from));
            if let Some(cam_rgb) = inv3(&rc) {
                let mut cm = [0.0; 9];
                for i in 0..3 {
                    for j in 0..3 {
                        let v: f64 = (0..3).map(|k| cam_rgb[i][k] * XYZ_TO_SRGB[k][j]).sum();
                        cm[i * 3 + j] = v / f64::from(meta.wb[i].max(1e-6));
                    }
                }
                out.push(srational(50721, &cm));
                out.push(Entry::short(50778, &[21]));
            }
        }
    }
    let neutral = meta
        .dng
        .as_ref()
        .and_then(|d| d.as_shot_neutral)
        .unwrap_or_else(|| {
            let g = f64::from(meta.wb[1].max(1e-6));
            std::array::from_fn(|c| g / f64::from(meta.wb[c].max(1e-6)))
        });
    out.push(rational(50728, &neutral));
    out
}

/// The file's bytes.
pub fn encode(m: &Merged, reference_bytes: &[u8], o: &DngOptions) -> Vec<u8> {
    encode_with_xmp(m, reference_bytes, o, xmp(m))
}

/// [`encode`] with another XMP packet than the merge's (a local copy's proxy,
/// [`crate::proxy`]).
pub fn encode_with_xmp(m: &Merged, reference_bytes: &[u8], o: &DngOptions, x: Vec<u8>) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    // Writing to memory cannot fail.
    let _ = write_dng(m, reference_bytes, o, x, &mut out);
    out.into_inner()
}

/// Writes the merge's DNG to `out` (any seekable writer, from position 0): the directories'
/// room first, then the preview and the tiles one row at a time as they are compressed
/// (never the whole file in memory), then the directories over their room.
pub fn write_dng<W: std::io::Write + std::io::Seek>(
    m: &Merged,
    reference_bytes: &[u8],
    o: &DngOptions,
    x: Vec<u8>,
    out: &mut W,
) -> std::io::Result<()> {
    let (w, h) = (m.width as usize, m.height as usize);
    let cfa = o.cfa && m.cpp == 1 && !matches!(m.mosaic, super::Mosaic::Mono);
    let demosaiced =
        (!(cfa || m.cpp == 3 || matches!(m.mosaic, super::Mosaic::Mono))).then(|| demosaic(m));
    let (cpp, values): (usize, &[f32]) = match &demosaiced {
        Some(d) => (3, d),
        None => (m.cpp as usize, &m.radiance),
    };
    let max = values
        .par_iter()
        .copied()
        .filter(|v| v.is_finite())
        .reduce(|| 0.0f32, f32::max)
        .max(1e-6);
    let (across, down) = (w.div_ceil(TILE), h.div_ceil(TILE));
    let n_tiles = across * down;
    let (pw, ph, pv) = preview(m);
    let shot = &m.shots[m.reference];

    let source = specular_convert::source_meta(reference_bytes);
    let exif = source.exif.unwrap_or_default();

    let build = |raw_at: u32, tile_at: &[u32], counts: &[u32], preview_at: u32| -> Vec<u8> {
        let mut out = b"II\x2a\0\x08\0\0\0".to_vec();
        // The raw's directory.
        let bps = (o.bits.bytes() * 8) as u16;
        let mut raw = vec![
            Entry::long(254, &[0]),
            Entry::long(256, &[w as u32]),
            Entry::long(257, &[h as u32]),
            Entry::short(258, &vec![bps; cpp]),
            Entry::short(259, &[8]),
            Entry::short(262, &[if cfa { 32803 } else { 34892 }]),
            Entry::short(277, &[cpp as u16]),
            Entry::short(284, &[1]),
            Entry::short(317, &[3]),
            Entry::long(322, &[TILE as u32]),
            Entry::long(323, &[TILE as u32]),
            Entry::long(324, tile_at),
            Entry::long(325, counts),
            Entry::short(339, &vec![3; cpp]),
            Entry::short(50713, &[1, 1]),
            Entry::long(50714, &vec![0; cpp]),
            Entry::long(50717, &[1]),
            Entry::long(50719, &[0, 0]),
            Entry::long(50720, &[w as u32, h as u32]),
        ];
        if cfa {
            match &m.mosaic {
                super::Mosaic::Bayer(p) => {
                    raw.push(Entry::short(33421, &[2, 2]));
                    raw.push(bytes(33422, p));
                }
                super::Mosaic::Pattern {
                    w: cw,
                    h: ch,
                    colors,
                } => {
                    raw.push(Entry::short(33421, &[*ch as u16, *cw as u16]));
                    raw.push(bytes(33422, colors));
                }
                _ => {}
            }
            raw.push(bytes(50710, &[0, 1, 2]));
            raw.push(Entry::short(50711, &[1]));
        }
        // The opcode lists that still hold, in the raw's directory.
        raw.extend(opcode_lists(m, cpp as u32));
        let at = specular_convert::exif::write_ifds(&mut out, raw, &[], &[]);
        debug_assert_eq!(at, raw_at);
        // IFD0: the preview, the DNG tags, the reference's descriptive tags and Exif.
        let mut ifd0: Vec<Entry> = exif
            .ifd0
            .iter()
            .filter(|e| {
                matches!(
                    e.tag,
                    0x010E | 0x013B | 0x8298 | 0x0131 | 0x0132 | 0x011A | 0x011B | 0x0128
                )
            })
            .cloned()
            .collect();
        let first = shot.make.split_whitespace().next().unwrap_or("");
        let camera =
            if first.is_empty() || shot.model.to_lowercase().starts_with(&first.to_lowercase()) {
                shot.model.clone()
            } else {
                format!("{first} {}", shot.model).trim().to_string()
            };
        ifd0.extend([
            Entry::long(254, &[1]),
            Entry::long(256, &[pw as u32]),
            Entry::long(257, &[ph as u32]),
            Entry::short(258, &[8, 8, 8]),
            Entry::short(259, &[1]),
            Entry::short(262, &[2]),
            Entry::long(273, &[preview_at]),
            Entry::short(274, &[m.meta.orientation.clamp(1, 8)]),
            Entry::short(277, &[3]),
            Entry::long(278, &[ph as u32]),
            Entry::long(279, &[(pw * ph * 3) as u32]),
            Entry::short(284, &[1]),
            Entry::ascii(271, &shot.make),
            Entry::ascii(272, &shot.model),
            Entry::ascii(305, "Specular"),
            Entry::long(330, &[raw_at]),
            bytes(50706, &[1, 4, 0, 0]),
            bytes(50707, &[1, 4, 0, 0]),
            Entry::ascii(50708, &camera),
            srational(50730, &[f64::from(max).log2()]),
            Entry {
                tag: 700,
                ty: 1,
                count: 0,
                data: Vec::new(),
            },
        ]);
        let x = x.clone();
        if let Some(e) = ifd0.iter_mut().find(|e| e.tag == 700) {
            e.count = x.len() as u32;
            e.data = x;
        }
        ifd0.extend(colour(m));
        let exif_dir: Vec<Entry> = exif
            .exif
            .iter()
            .filter(|e| e.ty != 13 && !matches!(e.tag, 0xA005 | 0xA002 | 0xA003 | 0x927C))
            .cloned()
            .collect();
        let gps: Vec<Entry> = exif.gps.iter().filter(|e| e.ty != 13).cloned().collect();
        let ifd0_at = specular_convert::exif::write_ifds(&mut out, ifd0, &exif_dir, &gps);
        out[4..8].copy_from_slice(&ifd0_at.to_le_bytes());
        out
    };
    // The directories' size (their values do not change it), then the pixel data.
    let dummy = vec![0u32; n_tiles];
    let head = build(8, &dummy, &dummy, 0).len();
    let preview_at = head.div_ceil(4) * 4;
    out.write_all(&vec![0u8; preview_at])?;
    out.write_all(&pv)?;
    let mut at = preview_at + pv.len();
    let mut offsets = Vec::with_capacity(n_tiles);
    let mut counts = Vec::with_capacity(n_tiles);
    for ty in 0..down {
        for t in tile_row(values, w, h, cpp, o.bits, ty, max) {
            if at % 2 == 1 {
                out.write_all(&[0])?;
                at += 1;
            }
            offsets.push(at as u32);
            counts.push(t.len() as u32);
            out.write_all(&t)?;
            at += t.len();
        }
    }
    let dirs = build(8, &offsets, &counts, preview_at as u32);
    debug_assert_eq!(dirs.len(), head);
    out.seek(std::io::SeekFrom::Start(0))?;
    out.write_all(&dirs)?;
    out.seek(std::io::SeekFrom::Start(at as u64))?;
    Ok(())
}

/// [`save_as`] streaming the file to disk (a hidden temporary beside the reference, then
/// linked to its name; never over anything): the merge's pixels are compressed a row of
/// tiles at a time.
pub fn save_streamed(
    m: &Merged,
    reference_bytes: &[u8],
    o: &DngOptions,
    kind: Kind,
    x: Option<Vec<u8>>,
) -> std::io::Result<PathBuf> {
    use std::io::Write;
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let r = &m.shots[m.reference].path;
    let dir = r
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let name = file_name_of(r, kind, "dng");
    let tmp = dir.join(format!(
        ".{name}.specular-tmp-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let res = (|| {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        let mut b = std::io::BufWriter::with_capacity(1 << 20, f);
        write_dng(m, reference_bytes, o, x.unwrap_or_else(|| xmp(m)), &mut b)?;
        b.flush()?;
        b.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        for k in 1..10_000u32 {
            let path = dir.join(specular_convert::write::suffixed(&name, k));
            match std::fs::hard_link(&tmp, &path) {
                Ok(()) => return Ok(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                // No hard links here (some network and FAT volumes): rename when free.
                Err(_) if !path.exists() => {
                    std::fs::rename(&tmp, &path)?;
                    return Ok(path);
                }
                Err(_) => continue,
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            name.clone(),
        ))
    })();
    let _ = std::fs::remove_file(&tmp);
    res
}

// ---- DNG opcode lists carried over ---------------------------------------------------------

/// The reference's opcodes that still hold for the merge, rewritten for the merged area
/// (DNG 1.4 §6): list 1 bad-pixel lists (the merge reads the stored samples unrepaired),
/// list 2 gain maps and per-row/column scales (multiplicative, so they commute with the
/// merge's weighted sums), and list 3 warps, radial vignetting, gain maps and scales (they
/// act on the developed reference geometry). Tables, polynomials and deltas assume the
/// frame's own levels and bad-pixel constants mark stored values: left out.
fn opcode_lists(m: &Merged, cpp: u32) -> Vec<Entry> {
    let mut out = Vec::new();
    for (b, tag) in opcode_list_bytes(m, cpp)
        .into_iter()
        .zip([51008u16, 51009, 51022])
    {
        if let Some(b) = b {
            out.push(Entry {
                tag,
                ty: 7,
                count: b.len() as u32,
                data: b,
            });
        }
    }
    out
}

/// The merged DNG's three opcode lists as stored (`None`: absent): the reference's that
/// still hold ([`opcode_lists`]), then in list 3 its maker-note lens corrections fitted to
/// `WarpRectilinear` and `FixVignetteRadial` ([`super::lens_fit`]) when it has no warp of
/// its own.
pub(super) fn opcode_list_bytes(m: &Merged, cpp: u32) -> [Option<Vec<u8>>; 3] {
    std::array::from_fn(|li| {
        let mut ops: Vec<Vec<u8>> = m.meta.opcodes[li]
            .iter()
            .filter_map(|o| carry_opcode(o, li, m.crop, cpp))
            .collect();
        if li == 2 {
            ops.extend(lens_opcodes(m));
        }
        if ops.is_empty() {
            return None;
        }
        let mut b = (ops.len() as u32).to_be_bytes().to_vec();
        for o in ops {
            b.extend_from_slice(&o);
        }
        Some(b)
    })
}

/// The reference's maker-note lens tables as list-3 opcodes (warp, then vignetting), unless
/// its own list 3 already warps.
fn lens_opcodes(m: &Merged) -> Vec<Vec<u8>> {
    use specular_raw::sensor::OpcodeInfo;
    let Some(lens) = m.meta.lens.as_deref() else {
        return Vec::new();
    };
    if m.meta.opcodes[2]
        .iter()
        .any(|o| matches!(o, OpcodeInfo::Warp { .. }))
    {
        return Vec::new();
    }
    let Some(fit) = super::lens_fit::fit_lens(lens, m.width, m.height) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(k) = fit.warp {
        let mut b = Be(Vec::new());
        b.u32(3);
        for kp in k {
            for v in kp {
                b.f64(v);
            }
            b.f64(0.0).f64(0.0);
        }
        b.f64(0.5).f64(0.5);
        out.push(opcode(1, b.0));
    }
    if let Some(k) = fit.vignette {
        let mut b = Be(Vec::new());
        for v in k {
            b.f64(v);
        }
        b.f64(0.5).f64(0.5);
        out.push(opcode(3, b.0));
    }
    out
}

struct Be(Vec<u8>);

impl Be {
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn f64(&mut self, v: f64) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn f32(&mut self, v: f32) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn rect(&mut self, r: specular_raw::Rect) -> &mut Self {
        self.u32(r.y).u32(r.x).u32(r.y + r.h).u32(r.x + r.w)
    }
}

/// One opcode: id, DNG version 1.3, flags (optional: readers that lack it still open the
/// file), parameters.
fn opcode(id: u32, params: Vec<u8>) -> Vec<u8> {
    let mut b = Be(Vec::with_capacity(16 + params.len()));
    b.u32(id).u32(0x0103_0000).u32(1).u32(params.len() as u32);
    b.0.extend_from_slice(&params);
    b.0
}

/// `r` (absolute) in the merged area's coordinates, clipped to it; `None` when empty.
fn moved(r: specular_raw::Rect, crop: specular_raw::Rect) -> Option<specular_raw::Rect> {
    let x0 = r.x.max(crop.x);
    let y0 = r.y.max(crop.y);
    let x1 = (r.x + r.w).min(crop.x + crop.w);
    let y1 = (r.y + r.h).min(crop.y + crop.h);
    (x1 > x0 && y1 > y0).then(|| specular_raw::Rect {
        x: x0 - crop.x,
        y: y0 - crop.y,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// The squared distance from `c` (absolute) to the farthest corner of `b`.
fn far2(c: (f64, f64), b: specular_raw::Rect) -> f64 {
    let (x0, x1) = (f64::from(b.x), f64::from(b.x + b.w));
    let (y0, y1) = (f64::from(b.y), f64::from(b.y + b.h));
    [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
        .iter()
        .map(|&(x, y)| (x - c.0).powi(2) + (y - c.1).powi(2))
        .fold(0.0, f64::max)
}

/// `o` (list `li` of the reference, stored coordinates) for the merged DNG, or `None` when
/// it no longer holds or leaves the merged area.
fn carry_opcode(
    o: &specular_raw::sensor::OpcodeInfo,
    li: usize,
    crop: specular_raw::Rect,
    cpp: u32,
) -> Option<Vec<u8>> {
    use specular_raw::sensor::{OpcodeInfo, PointKind};
    match o {
        OpcodeInfo::FixBadPixelsList { points, rects, .. } if li < 2 => {
            let pts: Vec<(u32, u32)> = points
                .iter()
                .filter(|&&(y, x)| {
                    x >= crop.x && y >= crop.y && x < crop.x + crop.w && y < crop.y + crop.h
                })
                .map(|&(y, x)| (y - crop.y, x - crop.x))
                .collect();
            let rs: Vec<_> = rects.iter().filter_map(|&r| moved(r, crop)).collect();
            if pts.is_empty() && rs.is_empty() {
                return None;
            }
            let mut b = Be(Vec::new());
            b.u32(0).u32(pts.len() as u32).u32(rs.len() as u32);
            for (y, x) in pts {
                b.u32(y).u32(x);
            }
            for r in rs {
                b.rect(r);
            }
            Some(opcode(5, b.0))
        }
        OpcodeInfo::Warp {
            fisheye,
            k,
            centre,
            bounds,
        } if li == 2 => {
            // Centre to the merged area; radii normalised by its own farthest corner.
            let c = (
                f64::from(bounds.x) + centre.0 * f64::from(bounds.w),
                f64::from(bounds.y) + centre.1 * f64::from(bounds.h),
            );
            let (m_old, m_new) = (far2(c, *bounds).sqrt(), far2(c, crop).sqrt());
            if m_old <= 0.0 || m_new <= 0.0 {
                return None;
            }
            let s = m_new / m_old;
            // A fisheye's angles don't rescale: kept only when the area is unchanged.
            if *fisheye && (s - 1.0).abs() > 1e-9 {
                return None;
            }
            // List 3 acts on the developed planes: three for a mosaic too.
            let planes = if cpp == 1 { 3 } else { cpp.min(3) as usize };
            let mut b = Be(Vec::new());
            b.u32(planes as u32);
            for kp in k.iter().take(planes) {
                if *fisheye {
                    for v in kp.iter().take(4) {
                        b.f64(*v);
                    }
                } else {
                    for (i, v) in kp.iter().take(4).enumerate() {
                        b.f64(v * s.powi(2 * i as i32));
                    }
                    b.f64(kp[4] * s).f64(kp[5] * s);
                }
            }
            b.f64((c.0 - f64::from(crop.x)) / f64::from(crop.w))
                .f64((c.1 - f64::from(crop.y)) / f64::from(crop.h));
            Some(opcode(if *fisheye { 2 } else { 1 }, b.0))
        }
        OpcodeInfo::Point {
            rect,
            plane,
            planes,
            row_pitch,
            col_pitch,
            bounds,
            kind,
        } if li > 0 => {
            let (rp, cp) = ((*row_pitch).max(1), (*col_pitch).max(1));
            // The first row and column on the pitch inside the merged area.
            let align = |start: u32, lo: u32, pitch: u32| -> u32 {
                if start >= lo {
                    start
                } else {
                    start + (lo - start).div_ceil(pitch) * pitch
                }
            };
            let (x0, y0) = (align(rect.x, crop.x, cp), align(rect.y, crop.y, rp));
            let r = moved(
                specular_raw::Rect {
                    x: x0,
                    y: y0,
                    w: (rect.x + rect.w).saturating_sub(x0),
                    h: (rect.y + rect.h).saturating_sub(y0),
                },
                crop,
            )?;
            let mut b = Be(Vec::new());
            b.rect(r).u32(*plane).u32(*planes).u32(rp).u32(cp);
            let rel = |a: f64, lo: u32, from: u32, n: u32, to: u32| -> f64 {
                (f64::from(from) + a * f64::from(n) - f64::from(lo)) / f64::from(to)
            };
            let id = match kind {
                PointKind::GainMap {
                    points_v,
                    points_h,
                    spacing_v,
                    spacing_h,
                    origin_v,
                    origin_h,
                    planes: gp,
                    gains,
                } => {
                    b.u32(*points_v).u32(*points_h);
                    b.f64(spacing_v * f64::from(bounds.h) / f64::from(crop.h))
                        .f64(spacing_h * f64::from(bounds.w) / f64::from(crop.w));
                    b.f64(rel(*origin_v, crop.y, bounds.y, bounds.h, crop.h))
                        .f64(rel(*origin_h, crop.x, bounds.x, bounds.w, crop.w));
                    b.u32(*gp);
                    for g in gains {
                        b.f32(*g);
                    }
                    9
                }
                PointKind::ScalePerRow(v) | PointKind::ScalePerColumn(v) => {
                    let row = matches!(kind, PointKind::ScalePerRow(_));
                    // Entries before the merged area are dropped.
                    let skip = if row {
                        (y0 - rect.y) / rp
                    } else {
                        (x0 - rect.x) / cp
                    } as usize;
                    let v: Vec<f32> = v.iter().skip(skip).copied().collect();
                    b.u32(v.len() as u32);
                    for x in v {
                        b.f32(x);
                    }
                    if row { 12 } else { 13 }
                }
                PointKind::Vignette { k, centre, inv_m2 } if li == 2 => {
                    let m2_new = far2(*centre, crop);
                    if *inv_m2 <= 0.0 || m2_new <= 0.0 {
                        return None;
                    }
                    let s2 = m2_new * inv_m2;
                    let mut v = Be(Vec::new());
                    for (i, x) in k.iter().enumerate() {
                        v.f64(x * s2.powi(i as i32 + 1));
                    }
                    v.f64((centre.0 - f64::from(crop.x)) / f64::from(crop.w))
                        .f64((centre.1 - f64::from(crop.y)) / f64::from(crop.h));
                    return Some(opcode(3, v.0));
                }
                _ => return None,
            };
            Some(opcode(id, b.0))
        }
        _ => None,
    }
}
