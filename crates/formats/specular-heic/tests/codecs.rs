//! HEIF items coded other than HEVC, `tili`, `cfen` and inter-coded
//! sequences against the oracles `tools/corpus/heif-gaps.py` writes to
//! `tools/corpus/out/heif-gaps/` (gitignored; skipped when absent):
//!
//! * `planes`: the decoded planes against libavcodec's decode of the AVC /
//!   JPEG stream, or the lossless source planes of a `cfen` image;
//! * `tiles`: each grid tile likewise; `item`: the coded item before its
//!   transformative properties;
//! * `png`: our RGBA against libheif's `heif-dec -C nn` (the `unci` files of
//!   the libheif corpus; the HEVC grid a `tili` file was built from);
//! * `frames`: every picture of an inter-coded sequence, in display order,
//!   against libavcodec's;
//! * `refused`: files libheif does not decode either.
//!
//! `HEIF_GAPS_VERBOSE=1` prints a per-file line.

use std::path::{Path, PathBuf};

use specular_heic::heif::HeifFrame;
use specular_heic::seq::{SeqPlayer, Sequence, find_moov};
use specular_heic::{DecodeOptions, HeicFile};

fn out_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/out/heif-gaps")
}

struct Entry {
    name: String,
    path: PathBuf,
    kind: String,
    refs: Vec<PathBuf>,
    note: String,
}

fn manifest() -> Option<Vec<Entry>> {
    let dir = out_dir();
    let text = std::fs::read_to_string(dir.join("manifest.tsv")).ok()?;
    Some(
        text.lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                Some(Entry {
                    name: f.first()?.to_string(),
                    path: dir.join(f.get(1)?),
                    kind: f.get(2)?.to_string(),
                    refs: f.get(3)?.split_whitespace().map(|r| dir.join(r)).collect(),
                    note: f.get(4).unwrap_or(&"").to_string(),
                })
            })
            .collect(),
    )
}

fn verbose() -> bool {
    std::env::var_os("HEIF_GAPS_VERBOSE").is_some()
}

/// Largest absolute difference and the count of differing samples between our planes and a
/// raw planar reference of the same layout (8-bit bytes or 16-bit little-endian).
fn plane_diff(frame: &HeifFrame, raw: &[u8], planes: usize) -> Result<(u32, usize), String> {
    let bps = frame.format.bytes_per_sample();
    let mut off = 0;
    let (mut worst, mut count) = (0u32, 0usize);
    for i in 0..planes {
        let (pw, ph) = frame.format.plane_dims(i, frame.width, frame.height);
        let p = &frame.planes[i];
        for y in 0..ph as usize {
            for x in 0..pw as usize {
                let ours = if bps == 1 {
                    u32::from(p.data[y * p.stride + x])
                } else {
                    let o = y * p.stride + 2 * x;
                    u32::from(u16::from_le_bytes([p.data[o], p.data[o + 1]]))
                };
                let r = off + (y * pw as usize + x) * bps;
                let theirs = match raw.get(r..r + bps) {
                    Some([a]) => u32::from(*a),
                    Some([a, b]) => u32::from(u16::from_le_bytes([*a, *b])),
                    _ => return Err(format!("reference too short (plane {i})")),
                };
                let d = ours.abs_diff(theirs);
                if d > 0 {
                    count += 1;
                    worst = worst.max(d);
                }
            }
        }
        off += pw as usize * ph as usize * bps;
    }
    if off != raw.len() {
        return Err(format!(
            "reference has {} bytes, we compared {off}",
            raw.len()
        ));
    }
    Ok((worst, count))
}

fn read_png(path: &Path) -> Option<(u32, u32, Vec<[u8; 4]>)> {
    let mut dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let ch = info.color_type.samples();
    let px = buf[..info.buffer_size()]
        .chunks_exact(ch)
        .map(|c| match ch {
            1 => [c[0], c[0], c[0], 255],
            2 => [c[0], c[0], c[0], c[1]],
            3 => [c[0], c[1], c[2], 255],
            _ => [c[0], c[1], c[2], c[3]],
        })
        .collect();
    Some((info.width, info.height, px))
}

/// (largest difference, differing samples, PSNR) of RGBA pixels.
fn rgba_diff(ours: &[u8], theirs: &[[u8; 4]]) -> (u32, usize, f64) {
    let (mut worst, mut count, mut se) = (0u32, 0usize, 0f64);
    for (a, b) in ours.chunks_exact(4).zip(theirs) {
        for c in 0..4 {
            let d = u32::from(a[c].abs_diff(b[c]));
            if d > 0 {
                count += 1;
                worst = worst.max(d);
                se += f64::from(d * d);
            }
        }
    }
    let n = theirs.len() as f64 * 4.0;
    let psnr = if se == 0.0 {
        99.0
    } else {
        (10.0 * (255.0f64 * 255.0 * n / se).log10()).min(99.0)
    };
    (worst, count, psnr)
}

fn opts() -> DecodeOptions<'static> {
    DecodeOptions {
        parallel: true,
        ..Default::default()
    }
}

/// Planes compared per codec: AVC and the lossless `cfen` sources bit-exact; JPEG within the
/// IDCT rounding libavcodec and specular-jpeg may differ by.
fn tolerance(name: &str) -> u32 {
    if name.contains("jpeg") { 2 } else { 0 }
}

#[test]
fn heif_gaps_against_oracles() {
    let Some(entries) = manifest() else {
        eprintln!("heif-gaps: no reference set (tools/corpus/heif-gaps.py); skipped");
        return;
    };
    let mut failures = Vec::new();
    let mut checked = 0;
    for e in &entries {
        let Ok(bytes) = std::fs::read(&e.path) else {
            failures.push(format!("{}: unreadable {}", e.name, e.path.display()));
            continue;
        };
        let r = match e.kind.as_str() {
            "planes" => check_planes(e, &bytes),
            "tiles" => check_tiles(e, &bytes),
            "item" => check_item(e, &bytes),
            "png" => check_png(e, &bytes),
            "frames" => check_frames(e, &bytes),
            "refused" => Ok(
                match HeicFile::open(bytes).and_then(|f| f.decode(&opts())) {
                    Ok(_) => "decodes (libheif does not)".to_string(),
                    Err(err) => format!("refused: {err}"),
                },
            ),
            k => Err(format!("unknown kind {k}")),
        };
        checked += 1;
        match r {
            Ok(m) => {
                if verbose() {
                    eprintln!("{:<52} {:<8} {m}", e.name, e.kind);
                }
            }
            Err(m) => failures.push(format!("{}: {m}", e.name)),
        }
    }
    eprintln!("heif-gaps: {checked} files, {} failures", failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn decode(bytes: &[u8]) -> Result<specular_heic::Image, String> {
    HeicFile::open(bytes.to_vec())
        .and_then(|f| f.decode(&opts()))
        .map_err(|e| e.to_string())
}

fn check_planes(e: &Entry, bytes: &[u8]) -> Result<String, String> {
    let img = decode(bytes)?;
    let raw = std::fs::read(&e.refs[0]).map_err(|x| x.to_string())?;
    let planes = img.frame.format.chroma.colour_planes();
    let (worst, count) = plane_diff(&img.frame, &raw, planes)?;
    if worst > tolerance(&e.name) {
        return Err(format!(
            "{count} samples differ, by up to {worst} ({})",
            e.note
        ));
    }
    Ok(format!("{count} samples differ, by up to {worst}"))
}

/// The coded item alone (before its transformative properties).
fn check_item(e: &Entry, bytes: &[u8]) -> Result<String, String> {
    let file = HeicFile::open(bytes.to_vec()).map_err(|x| x.to_string())?;
    let t = file.decode_tile(0, &opts()).map_err(|x| x.to_string())?;
    let raw = std::fs::read(&e.refs[0]).map_err(|x| x.to_string())?;
    let (worst, count) = plane_diff(&t.frame, &raw, t.frame.format.chroma.colour_planes())?;
    if worst > tolerance(&e.name) {
        return Err(format!("{count} samples differ, by up to {worst}"));
    }
    Ok(format!("{count} samples differ, by up to {worst}"))
}

fn check_tiles(e: &Entry, bytes: &[u8]) -> Result<String, String> {
    let file = HeicFile::open(bytes.to_vec()).map_err(|x| x.to_string())?;
    let (mut worst, mut count) = (0, 0);
    for (i, r) in e.refs.iter().enumerate() {
        let t = file.decode_tile(i, &opts()).map_err(|x| x.to_string())?;
        let raw = std::fs::read(r).map_err(|x| x.to_string())?;
        let planes = t.frame.format.chroma.colour_planes();
        let (w, c) = plane_diff(&t.frame, &raw, planes)?;
        worst = worst.max(w);
        count += c;
    }
    // The composed grid holds the tiles unchanged.
    let img = file.decode(&opts()).map_err(|x| x.to_string())?;
    for (i, r) in e.refs.iter().enumerate() {
        let t = file.decode_tile(i, &opts()).map_err(|x| x.to_string())?;
        let cols = img.frame.width / t.frame.width;
        let (x0, y0) = (
            (i as u32 % cols) * t.frame.width,
            (i as u32 / cols) * t.frame.height,
        );
        for y in 0..t.frame.height {
            for x in 0..t.frame.width {
                if img.frame.sample(0, x0 + x, y0 + y) != t.frame.sample(0, x, y) {
                    return Err(format!("tile {i} ({}) not placed as decoded", r.display()));
                }
            }
        }
    }
    if worst > tolerance(&e.name) {
        return Err(format!("{count} samples differ, by up to {worst}"));
    }
    Ok(format!("{count} samples differ, by up to {worst}"))
}

fn check_png(e: &Entry, bytes: &[u8]) -> Result<String, String> {
    let img = decode(bytes)?;
    let rgba = img.to_rgba8().map_err(|x| x.to_string())?;
    let (w, h, theirs) = read_png(&e.refs[0]).ok_or("reference png")?;
    if (w, h) != (img.width(), img.height()) {
        return Err(format!("{}x{} vs {w}x{h}", img.width(), img.height()));
    }
    let (worst, count, psnr) = if e.note.contains("empty") {
        // The empty tile is grey here; libheif's grid has the tile. Compare the others.
        let mut ours = rgba.clone();
        let mut refs = theirs.clone();
        blank_tile(&mut ours, &mut refs, w, 5, 256);
        rgba_diff(&ours, &refs)
    } else {
        rgba_diff(&rgba, &theirs)
    };
    // unci decodes are exact up to libheif's YCbCr → RGB rounding; tili tiles are libheif's
    // grid tiles (its HEVC decoder and conversion).
    let ok = if e.name.starts_with("tili") {
        psnr >= 40.0
    } else if e.name.contains("YUV") || e.name.contains("VUY") || e.name.contains("YVU") {
        worst <= 2
    } else {
        worst == 0
    };
    if !ok {
        return Err(format!(
            "{count} samples differ, by up to {worst}, PSNR {psnr:.1} dB"
        ));
    }
    Ok(format!(
        "{count} samples differ, by up to {worst}, PSNR {psnr:.1} dB"
    ))
}

/// Zero tile `k` (of `size`-pixel tiles, `w` wide) in both images.
fn blank_tile(a: &mut [u8], b: &mut [[u8; 4]], w: u32, k: u32, size: u32) {
    let cols = w.div_ceil(size);
    let (x0, y0) = ((k % cols) * size, (k / cols) * size);
    let h = (b.len() as u32) / w;
    for y in y0..(y0 + size).min(h) {
        for x in x0..(x0 + size).min(w) {
            let i = (y * w + x) as usize;
            a[i * 4..i * 4 + 4].fill(0);
            b[i] = [0; 4];
        }
    }
}

fn check_frames(e: &Entry, bytes: &[u8]) -> Result<String, String> {
    let seq = find_moov(bytes)
        .and_then(Sequence::parse)
        .ok_or("no picture track")?;
    if seq.all_sync() {
        return Err("expected an inter-coded track".into());
    }
    let raw = std::fs::read(&e.refs[0]).map_err(|x| x.to_string())?;
    let t0 = std::time::Instant::now();
    let mut player = SeqPlayer::new(&seq, bytes).map_err(|x| x.to_string())?;
    let mut n = 0;
    let (mut worst, mut count) = (0, 0);
    let mut delays = Vec::new();
    while let Some(f) = player.next_frame(bytes, None) {
        let f = f.map_err(|x| x.to_string())?;
        let size = f
            .image
            .frame
            .planes
            .iter()
            .map(|p| p.data.len())
            .sum::<usize>();
        let start = n * size;
        let r = raw
            .get(start..start + size)
            .ok_or("more frames than the reference")?;
        let (w, c) = plane_diff(&f.image.frame, r, 3)?;
        worst = worst.max(w);
        count += c;
        delays.push(f.delay);
        n += 1;
    }
    let elapsed = t0.elapsed();
    if n * (raw.len() / n.max(1)) != raw.len() || n == 0 {
        return Err(format!("{n} frames for a {}-byte reference", raw.len()));
    }
    if worst > 0 {
        return Err(format!("{count} samples differ, by up to {worst}"));
    }
    if e.note.contains("10/s") && delays.iter().any(|d| d.as_millis() != 100) {
        return Err(format!("frame delays {delays:?}"));
    }
    // A second pass after a rewind gives the same first picture.
    player.rewind();
    let again = player
        .next_frame(bytes, None)
        .ok_or("nothing after rewind")?
        .map_err(|x| x.to_string())?;
    let size = raw.len() / n;
    let (w, _) = plane_diff(&again.image.frame, &raw[..size], 3).unwrap_or((1, 1));
    if w != 0 {
        return Err("first frame differs after a rewind".into());
    }
    if delays.iter().any(|d| d.is_zero()) {
        return Err(format!("frame delays {delays:?}"));
    }
    Ok(format!(
        "{n} frames, {count} samples differ, {:.1} ms/frame",
        elapsed.as_secs_f64() * 1e3 / n as f64
    ))
}
