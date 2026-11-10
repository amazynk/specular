//! Philips PAR/REC (`.PAR` text header, `.REC` voxels), versions 4, 4.1 and 4.2, read the
//! way nibabel documents the format.
//!
//! The PAR file has general lines (`.    Key : value`) and then one line per stored image:
//! slice, echo, dynamic, phase and type numbers, the image's index in the REC file, its bit
//! depth (8 or 16, little-endian unsigned), size, scaling (rescale intercept and slope, scale
//! slope), geometry and timing. Volumes are found the way nibabel does by default: each
//! repeat of a slice number starts a new volume, incomplete volumes are dropped, slices are
//! sorted by number within a volume.
//!
//! Values are the console's "displayed values": DV = PV × RS + RI, per image. Geometry is
//! the scanner frame from the midslice angulation and off-centre (Philips' ap, fh, rl axes,
//! rotations applied rl · ap · fh as nibabel found matches the scanner's own NIfTI export),
//! with the slice orientation (transverse, sagittal, coronal) giving the stored axes.

use std::path::{Path, PathBuf};

use crate::planes::{self, Src};
use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, MedVolume,
    OrientationSource, Param, ParamValue, Result, Skip,
};

/// Bytes of a file start [`sniff`] looks at.
pub const SNIFF: usize = 1024;

/// Largest PAR file read (one line per image; a long fMRI run has some 100 000).
const MAX_PAR: usize = 64 << 20;

/// Whether `data` starts like a PAR header.
pub fn sniff(data: &[u8]) -> bool {
    let head = &data[..data.len().min(SNIFF)];
    head.starts_with(b"# === DATA DESCRIPTION FILE")
        || (head.starts_with(b"#") && head.windows(21).any(|w| w == b"DATA DESCRIPTION FILE"))
}

/// Whether `path` is named like REC voxel data.
pub fn is_rec_name(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rec"))
}

/// The file beside `path` with the same name and extension `ext` (any case).
fn sibling(path: &Path, ext: &str) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    for e in [ext.to_ascii_uppercase(), ext.to_ascii_lowercase()] {
        let p = dir.join(format!("{stem}.{e}"));
        if p.is_file() {
            return Some(p);
        }
    }
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|e| {
            let p = e.path();
            let same = p.file_stem().is_some_and(|s| s.to_string_lossy() == stem)
                && p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext));
            same.then_some(p)
        })
}

/// The PAR header beside a REC file.
pub fn par_of(rec: &Path) -> Option<PathBuf> {
    sibling(rec, "par")
}

/// One image line's fields this reader uses.
#[derive(Clone, Debug, PartialEq)]
struct Image {
    slice: i64,
    rec_index: i64,
    bits: i64,
    res: [i64; 2],
    inter: f64,
    slope: f64,
    thick: f64,
    gap: f64,
    orient: i64,
    spacing: [f64; 2],
    echo_time: f64,
    flip: f64,
}

/// A parsed PAR file.
#[derive(Clone, Debug, PartialEq)]
struct Par {
    version: String,
    general: Vec<(String, String)>,
    images: Vec<Image>,
}

impl Par {
    fn get(&self, key: &str) -> Option<&str> {
        self.general
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }

    fn numbers(&self, key: &str) -> Option<Vec<f64>> {
        crate::numbers(self.get(key)?)
    }
}

/// Image-line columns per version.
fn columns(version: &str) -> Option<usize> {
    match version {
        "V4" => Some(41),
        "V4.1" => Some(48),
        "V4.2" => Some(49),
        _ => None,
    }
}

fn parse_par(data: &[u8]) -> Result<Par> {
    if data.len() > MAX_PAR {
        return Err(Error::TooLarge);
    }
    let text = String::from_utf8_lossy(data);
    let mut version = None;
    let mut general = Vec::new();
    let mut lines = Vec::new();
    #[derive(PartialEq)]
    enum State {
        Top,
        General,
        Comments,
        Images,
    }
    let mut state = State::Top;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if state == State::Top {
            if line.starts_with('#') {
                if line.contains("image export tool") {
                    version = line.split_whitespace().last().map(str::to_owned);
                }
                continue;
            }
            state = State::General;
        }
        if state == State::General {
            if let Some(rest) = line.strip_prefix('.') {
                let (k, v) = rest.split_once(':').unwrap_or((rest, ""));
                general.push((k.trim().to_owned(), v.trim().to_owned()));
                continue;
            }
            state = State::Comments;
        }
        if state == State::Comments {
            if line.starts_with('#') {
                continue;
            }
            state = State::Images;
        }
        if line.starts_with('#') {
            break;
        }
        lines.push(line);
    }
    let version = version.unwrap_or_default();
    if lines.is_empty() {
        return Err(Error::Malformed("PAR: no image lines".into()));
    }
    let n = lines[0].split_whitespace().count();
    let version = match columns(&version) {
        Some(c) if c == n => version,
        // Some exports name no version, or the wrong one: the column count tells.
        _ => match n {
            41 => "V4".into(),
            48 => "V4.1".into(),
            49 => "V4.2".into(),
            _ if version.starts_with("V3") || n < 41 => {
                return Err(Error::Unsupported(format!(
                    "PAR/REC version {}: only versions 4 to 4.2 are read",
                    if version.is_empty() {
                        "3 or older"
                    } else {
                        &version[1..]
                    }
                )));
            }
            _ => {
                return Err(Error::Malformed(format!("PAR: {n} values per image line")));
            }
        },
    };
    let want = columns(&version).unwrap_or(41);
    let mut images = Vec::with_capacity(lines.len());
    for (i, l) in lines.iter().enumerate() {
        let t: Vec<&str> = l.split_whitespace().collect();
        if t.len() < want {
            return Err(Error::Malformed(format!(
                "PAR: image line {} has {} values, not {want}",
                i + 1,
                t.len()
            )));
        }
        let f = |c: usize| -> Result<f64> {
            t[c].parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())
                .ok_or_else(|| Error::Malformed(format!("PAR: image line {}: {:?}", i + 1, t[c])))
        };
        let int = |c: usize| -> Result<i64> {
            t[c].parse::<i64>()
                .map_err(|_| Error::Malformed(format!("PAR: image line {}: {:?}", i + 1, t[c])))
        };
        images.push(Image {
            slice: int(0)?,
            rec_index: int(6)?,
            bits: int(7)?,
            res: [int(9)?, int(10)?],
            inter: f(11)?,
            slope: f(12)?,
            thick: f(22)?,
            gap: f(23)?,
            orient: int(25)?,
            spacing: [f(28)?, f(29)?],
            echo_time: f(30)?,
            flip: f(35)?,
        });
    }
    Ok(Par {
        version,
        general,
        images,
    })
}

/// The one value of `get` every image shares.
fn unique<T: PartialEq + Copy>(
    images: &[Image],
    what: &str,
    get: impl Fn(&Image) -> T,
) -> Result<T> {
    let first = get(&images[0]);
    if images.iter().any(|i| get(i) != first) {
        return Err(Error::Unsupported(format!(
            "PAR/REC images of differing {what}"
        )));
    }
    Ok(first)
}

/// The stored image (PAR line) of every output slice, volume by volume, and the slice and
/// volume counts: nibabel's default ("lax") sort.
fn sort(par: &Par) -> Result<(Vec<usize>, usize, usize)> {
    let imgs = &par.images;
    let mut distinct: Vec<i64> = imgs.iter().map(|i| i.slice).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let ns = distinct.len();
    let max = par
        .numbers("Max. number of slices/locations")
        .and_then(|v| v.first().copied())
        .map(|x| x as i64)
        .unwrap_or_else(|| distinct.last().copied().unwrap_or(0));
    if ns as i64 != max || distinct.first() != Some(&1) || distinct.last() != Some(&max) {
        return Err(Error::Malformed(format!(
            "PAR: slice numbers do not run from 1 to {max}"
        )));
    }
    // Volume number: how often the slice number came before.
    let mut seen = std::collections::HashMap::new();
    let vol: Vec<usize> = imgs
        .iter()
        .map(|i| {
            let c = seen.entry(i.slice).or_insert(0usize);
            *c += 1;
            *c - 1
        })
        .collect();
    let nvol_all = vol.iter().max().map_or(0, |v| v + 1);
    let mut count = vec![0usize; nvol_all];
    for &v in &vol {
        count[v] += 1;
    }
    let full = |v: usize| count[v] == ns;
    let mut order: Vec<usize> = (0..imgs.len()).collect();
    order.sort_by_key(|&l| (!full(vol[l]), vol[l], imgs[l].slice));
    let nv = (0..nvol_all).filter(|&v| full(v)).count();
    if nv == 0 {
        return Err(Error::Truncated("PAR/REC: no complete volume".into()));
    }
    order.truncate(ns * nv);
    Ok((order, ns, nv))
}

/// Row-major 3 × 3 product.
fn mul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut m = [[0.0; 3]; 3];
    for r in 0..3 {
        for c in 0..3 {
            m[r][c] = (0..3).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    m
}

/// Voxel to scanner LPS millimetres.
fn affine(par: &Par, shape: [usize; 3], zoom: [f64; 3], orient: i64) -> Result<[[f64; 4]; 3]> {
    // Stored axes in Philips' (P, S, L) frame.
    let to_psl: [[f64; 3]; 3] = match orient {
        1 => [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
        2 => [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
        3 => [[0.0, 0.0, 1.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0]],
        o => {
            return Err(Error::Unsupported(format!(
                "PAR/REC slice orientation {o} (not transverse, sagittal or coronal)"
            )));
        }
    };
    let three = |key: &str| -> [f64; 3] {
        par.numbers(key)
            .filter(|v| v.len() == 3 && v.iter().all(|x| x.is_finite()))
            .map_or([0.0; 3], |v| [v[0], v[1], v[2]])
    };
    let [ap, fh, rl] = three("Angulation midslice(ap,fh,rl)[degr]").map(f64::to_radians);
    let off = three("Off Centre midslice(ap,fh,rl) [mm]");
    let (s, c) = ap.sin_cos();
    let mx = [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]];
    let (s, c) = fh.sin_cos();
    let my = [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]];
    let (s, c) = rl.sin_cos();
    let mz = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
    let rot = mul(mul(mz, mx), my);
    let mut m = mul(rot, to_psl);
    for row in &mut m {
        for (c, z) in row.iter_mut().zip(zoom) {
            *c *= z;
        }
    }
    let centre = shape.map(|n| (n as f64 - 1.0) / 2.0);
    let psl: [[f64; 4]; 3] = [0, 1, 2].map(|r| {
        let t = off[r] - (0..3).map(|k| m[r][k] * centre[k]).sum::<f64>();
        [m[r][0], m[r][1], m[r][2], t]
    });
    // (P, S, L) rows to (L, P, S).
    Ok([psl[2], psl[0], psl[1]])
}

/// The planes in REC order, with the volume's sizes, from a parsed PAR file.
struct Plan {
    header: Header,
    planes: Vec<Src>,
    stored: DataType,
}

fn plan(par: &Par, rec: PathBuf) -> Result<Plan> {
    let imgs = &par.images;
    let bits = unique(imgs, "bit depths", |i| i.bits)?;
    let stored = match bits {
        8 => DataType::U8,
        16 => DataType::U16,
        b => {
            return Err(Error::Unsupported(format!(
                "PAR/REC with {b}-bit images (only 8 and 16 are defined)"
            )));
        }
    };
    let res = unique(imgs, "sizes", |i| i.res)?;
    let [w, h] = res.map(|x| usize::try_from(x).unwrap_or(0));
    if w == 0 || h == 0 || w > 1 << 16 || h > 1 << 16 {
        return Err(Error::Malformed(format!(
            "PAR: image size {}×{}",
            res[0], res[1]
        )));
    }
    let spacing = unique(imgs, "pixel spacings", |i| i.spacing)?;
    let thick = unique(imgs, "slice thicknesses", |i| i.thick)?;
    let gap = unique(imgs, "slice gaps", |i| i.gap)?;
    let orient = unique(imgs, "slice orientations", |i| i.orient)?;
    let (order, ns, nv) = sort(par)?;
    let plane = w * h * stored.size();
    // REC positions: the index column when it numbers the images once each, else line order.
    let mut idx: Vec<i64> = imgs.iter().map(|i| i.rec_index).collect();
    idx.sort_unstable();
    let by_index = idx.iter().enumerate().all(|(n, &x)| x == n as i64);
    let planes: Vec<Src> = order
        .iter()
        .map(|&l| {
            let at = if by_index {
                imgs[l].rec_index as usize
            } else {
                l
            };
            Src {
                at: at * plane,
                slope: imgs[l].slope,
                inter: imgs[l].inter,
            }
        })
        .collect();
    let (dtype, scale) = planes::output(stored, &planes);
    let zoom = [spacing[0], spacing[1], thick + gap];
    let size = [w, h, ns];
    let extra: Vec<usize> = if nv > 1 { vec![nv] } else { Vec::new() };
    let layout = Layout::dense(size, &extra, 1).ok_or(Error::TooLarge)?;
    let affine = affine(par, size, zoom, orient)?;
    let mut dims = size.to_vec();
    if nv > 1 {
        dims.push(nv);
    }
    let mut params = Vec::new();
    if let Some(tr) = par
        .numbers("Repetition time [ms]")
        .or_else(|| par.numbers("Repetition time [msec]"))
        .and_then(|v| v.first().copied())
        .filter(|x| *x > 0.0)
    {
        params.push(Param {
            key: "tr",
            value: ParamValue::Number(tr, "ms"),
        });
    }
    let first = &imgs[order[0]];
    if first.echo_time > 0.0 {
        params.push(Param {
            key: "te",
            value: ParamValue::Number(first.echo_time, "ms"),
        });
    }
    if first.flip > 0.0 {
        params.push(Param {
            key: "flip",
            value: ParamValue::Number(first.flip, "°"),
        });
    }
    if let Some(d) = par.get("Examination date/time") {
        params.push(Param {
            key: "date",
            value: ParamValue::Text(d.split_whitespace().collect::<Vec<_>>().join(" ")),
        });
    }
    if let Some(t) = par.get("Technique") {
        params.push(Param {
            key: "technique",
            value: ParamValue::Text(t.to_owned()),
        });
    }
    params.push(Param {
        key: "version",
        value: ParamValue::Text(par.version.trim_start_matches('V').to_owned()),
    });
    Ok(Plan {
        header: Header {
            kind: Kind::ParRec,
            dtype,
            layout,
            affine,
            orientation: OrientationSource::ParRec,
            dims,
            spacing: zoom,
            scale,
            cal: None,
            description: par.get("Protocol name").map(str::to_owned),
            intent: None,
            anatomical: None,
            params,
            data: DataRef {
                file: DataFile::Detached(vec![rec]),
                skip: Skip::Start { lines: 0, bytes: 0 },
                encoding: Encoding::Container,
                big_endian: false,
            },
        },
        planes,
        stored,
    })
}

/// The REC file of the PAR file at `path` (`path` itself when it is the REC file). A missing
/// one is named all the same: the header still reads, the voxels then fail to.
fn rec_path(path: &Path) -> PathBuf {
    if is_rec_name(path) {
        return path.to_path_buf();
    }
    sibling(path, "rec").unwrap_or_else(|| path.with_extension("REC"))
}

/// The PAR header's text: `data` when it is one, else the PAR file beside the REC `path`.
fn par_text(data: &[u8], path: &Path) -> Result<std::borrow::Cow<'static, [u8]>> {
    if sniff(data) {
        return Ok(std::borrow::Cow::Owned(data.to_vec()));
    }
    let p = par_of(path).ok_or_else(|| {
        Error::Io(format!(
            "{}.PAR: not found beside the REC data",
            path.file_stem().unwrap_or_default().to_string_lossy()
        ))
    })?;
    let v = crate::read_named(&p)?;
    if !sniff(&v) {
        return Err(Error::Malformed("not a PAR header".into()));
    }
    Ok(std::borrow::Cow::Owned(v))
}

/// The volume header of the PAR file (or of the REC file beside its PAR) at `path`, whose
/// bytes start with `data`.
pub fn parse(data: &[u8], path: &Path) -> Result<Header> {
    let par = parse_par(&par_text(data, path)?)?;
    Ok(plan(&par, rec_path(path))?.header)
}

/// Reads the REC voxels `header` (from [`parse`]) describes.
pub fn load(header: Header, data: &[u8], path: &Path) -> Result<MedVolume> {
    let par = parse_par(&par_text(data, path)?)?;
    let DataFile::Detached(files) = &header.data.file else {
        return Err(Error::Malformed("PAR/REC: no REC file".into()));
    };
    let rec = files.first().cloned().unwrap_or_default();
    let p = plan(&par, rec.clone())?;
    if p.header.dtype != header.dtype || p.header.layout != header.layout {
        return Err(Error::Malformed(
            "PAR header changed since it was read".into(),
        ));
    }
    let bytes = crate::read_named(&rec)?;
    let [w, h, _] = header.layout.size;
    let buf = planes::gather(
        &bytes,
        p.stored,
        false,
        [w, h],
        &p.planes,
        header.dtype,
        [false; 2],
    )?;
    MedVolume::from_bytes(header, buf, 0, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn par(version: &str, lines: &[&str]) -> Vec<u8> {
        let mut s = String::from("# === DATA DESCRIPTION FILE ===\n#\n");
        s += &format!("# CLINICAL TRYOUT   Research image export tool     {version}\n#\n");
        s += ".    Max. number of slices/locations    :   2\n";
        s += ".    Angulation midslice(ap,fh,rl)[degr]:   0 0 0\n";
        s += ".    Off Centre midslice(ap,fh,rl) [mm] :   0 0 0\n";
        s += "#\n# === IMAGE INFORMATION ===\n";
        for l in lines {
            s += l;
            s += "\n";
        }
        s += "# === END OF DATA DESCRIPTION FILE ===\n";
        s.into_bytes()
    }

    fn line(slice: u32, idx: u32, slope: f64) -> String {
        let mut v: Vec<String> = vec![
            slice.to_string(),
            "1".into(),
            "1".into(),
            "1".into(),
            "0".into(),
            "2".into(),
            idx.to_string(),
            "16".into(),
            "100".into(),
            "4".into(),
            "3".into(),
            "0".into(),
            slope.to_string(),
            "1".into(),
        ];
        v.resize(41, "1".into());
        v[25] = "1".into();
        v.join(" ")
    }

    #[test]
    fn volumes_follow_repeated_slice_numbers() {
        let l = [
            line(2, 1, 1.0),
            line(1, 0, 1.0),
            line(1, 2, 1.0),
            line(2, 3, 1.0),
            line(1, 4, 1.0),
        ];
        let refs: Vec<&str> = l.iter().map(String::as_str).collect();
        let p = parse_par(&par("V4", &refs)).unwrap();
        assert_eq!(p.version, "V4");
        let (order, ns, nv) = sort(&p).unwrap();
        assert_eq!((ns, nv), (2, 2));
        // Volume 0: lines 1 (slice 1) and 0 (slice 2); volume 1: lines 2 and 3; line 4 is a
        // third volume's lone slice, dropped.
        assert_eq!(order, [1, 0, 2, 3]);
    }

    #[test]
    fn differing_scales_become_float() {
        let l = [line(1, 0, 1.0), line(2, 1, 2.0)];
        let refs: Vec<&str> = l.iter().map(String::as_str).collect();
        let p = parse_par(&par("V4", &refs)).unwrap();
        let pl = plan(&p, PathBuf::from("x.REC")).unwrap();
        assert_eq!(pl.header.dtype, DataType::F32);
        assert_eq!(pl.header.layout.size, [4, 3, 2]);
        let l = [line(1, 0, 2.0), line(2, 1, 2.0)];
        let refs: Vec<&str> = l.iter().map(String::as_str).collect();
        let p = parse_par(&par("V4", &refs)).unwrap();
        let pl = plan(&p, PathBuf::from("x.REC")).unwrap();
        assert_eq!(pl.header.dtype, DataType::U16);
        assert_eq!(pl.header.scale, Some((2.0, 0.0)));
    }

    #[test]
    fn old_versions_are_refused_with_the_reason() {
        let l = "1 1 1 1 0 2 0 16 100 4 3 0 1 1";
        let e = parse_par(&par("V3", &[l])).unwrap_err();
        assert!(e.to_string().contains("only versions 4 to 4.2"), "{e}");
    }

    #[test]
    fn sniffs_the_banner() {
        assert!(sniff(b"# === DATA DESCRIPTION FILE ======\n"));
        assert!(!sniff(b"# a shell script\n"));
    }
}
