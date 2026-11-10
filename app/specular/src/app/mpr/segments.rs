//! Each segment as its own surface for 3-D printing: the segments of the segmentations found
//! over the volume (DICOM SEG, RTSTRUCT, the TMTV region), or the labels of a label volume
//! (a NIfTI or other research volume of small whole numbers).
//!
//! A segment's mask becomes a closed surface ([`label_surface`]: through the midpoints
//! between inside and outside voxel centres, in patient millimetres), smoothed keeping its
//! volume when the 3D view's "Smooth surface" is on, and decimated to the triangle count
//! chosen. Each is written as binary STL named by its label into a new folder, with one 3MF
//! file holding them all with their names and colours. Every surface is checked to be closed
//! (each edge shared by exactly two triangles) before it is written.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_dicom::labels::Labels;
use specular_dicom::volume::Volume;
use specular_i18n::t;
use specular_render_volume::{Mesh, label_surface};

/// Triangle counts per segment the export cycles through (`None`: no decimation).
pub(crate) const DETAILS: [Option<usize>; 4] = [Some(250_000), Some(1_000_000), None, Some(50_000)];
/// Smoothing passes (Taubin pairs).
const SMOOTH_PASSES: u32 = 10;
/// Labels a label volume may have, at most.
pub(crate) const MAX_LABELS: usize = 255;
/// Triangles over all segments beyond which no 3MF is written (its XML would pass 4 GiB).
const MAX_3MF_TRIANGLES: usize = 20_000_000;
/// Working memory per triangle while a surface is built and decimated, about.
const BYTES_PER_TRIANGLE: u64 = 220;

/// Which voxels of a layer are the segment's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pick {
    /// Voxels of this value (a packed layer's segment index + 1, or a label).
    Value(f32),
    /// A fractional layer: at or above half.
    Half,
}

/// One surface to build: its name, colour, voxels and how many they are (for the memory
/// estimate).
#[derive(Clone, Debug)]
pub(crate) struct Job {
    pub name: String,
    pub colour: [u8; 3],
    pub layers: Vec<(Arc<Volume>, Pick)>,
    pub voxels: u64,
}

/// What an export did.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Report {
    pub dir: PathBuf,
    pub written: usize,
    pub empty: usize,
    pub triangles: usize,
    /// Surfaces that failed the closed check (written anyway).
    pub open: usize,
    pub three_mf: bool,
}

/// The jobs for the segments of `sets` (each layer shared by its segments).
pub(crate) fn jobs_of_labels(sets: &[&Labels]) -> Vec<Job> {
    let mut out = Vec::new();
    for labels in sets {
        let layers: Vec<(Arc<Volume>, Option<usize>)> = labels
            .layers
            .iter()
            .map(|l| (Arc::new(l.volume.clone()), l.fraction))
            .collect();
        for (i, seg) in labels.segments.iter().enumerate() {
            let picks: Vec<(Arc<Volume>, Pick)> = layers
                .iter()
                .filter_map(|(v, f)| match f {
                    Some(k) if *k == i => Some((v.clone(), Pick::Half)),
                    Some(_) => None,
                    None => Some((v.clone(), Pick::Value((i + 1) as f32))),
                })
                .collect();
            let name = if seg.label.trim().is_empty() {
                format!("segment-{}", seg.number)
            } else {
                seg.label.clone()
            };
            out.push(Job {
                name,
                colour: seg.colour,
                layers: picks,
                voxels: seg.voxels,
            });
        }
    }
    out
}

/// The labels of a label volume: every whole value above 0, by voxel count. Refused (with a
/// reason) when the values are not whole or there are more than [`MAX_LABELS`].
pub(crate) fn jobs_of_label_volume(vol: Arc<Volume>) -> Result<Vec<Job>, String> {
    let [w, h, d] = vol.geometry.dims;
    let mut counts: std::collections::BTreeMap<u32, u64> = Default::default();
    for k in 0..d {
        if !vol.loaded[k as usize] {
            continue;
        }
        for j in 0..h {
            for i in 0..w {
                let v = vol.value(i, j, k);
                if v <= 0.0 {
                    if v < 0.0 {
                        return Err(t!("seg-stl-not-labels"));
                    }
                    continue;
                }
                if v.fract() != 0.0 || v > 65535.0 {
                    return Err(t!("seg-stl-not-labels"));
                }
                *counts.entry(v as u32).or_default() += 1;
                if counts.len() > MAX_LABELS {
                    return Err(t!("seg-stl-not-labels"));
                }
            }
        }
    }
    Ok(counts
        .into_iter()
        .map(|(v, n)| Job {
            name: format!("label-{v}"),
            colour: palette(v as usize),
            layers: vec![(vol.clone(), Pick::Value(v as f32))],
            voxels: n,
        })
        .collect())
}

/// A distinct colour per label (sRGB).
fn palette(i: usize) -> [u8; 3] {
    const C: [[u8; 3]; 8] = [
        [230, 159, 0],
        [86, 180, 233],
        [0, 158, 115],
        [240, 228, 66],
        [0, 114, 178],
        [213, 94, 0],
        [204, 121, 167],
        [180, 180, 180],
    ];
    C[i % C.len()]
}

/// A file name from a segment's label: path separators, reserved characters and control
/// characters replaced, trimmed, at most 80 characters; never empty or a device name.
pub(crate) fn file_stem(label: &str) -> String {
    let mut s: String = label
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(80)
        .collect();
    s = s.trim().trim_matches('.').trim().to_owned();
    let reserved = ["con", "prn", "aux", "nul", "com1", "lpt1"];
    if s.is_empty() || reserved.contains(&s.to_ascii_lowercase().as_str()) {
        s = format!("segment {s}").trim().to_owned();
    }
    s
}

/// A new folder `<stem>-segments` (or `-2`, `-3`…) in `dir`.
fn new_folder(dir: &Path, stem: &str) -> Result<PathBuf, String> {
    for n in 1..1000 {
        let name = if n == 1 {
            format!("{stem}-segments")
        } else {
            format!("{stem}-segments-{n}")
        };
        let p = dir.join(name);
        match std::fs::create_dir(&p) {
            Ok(()) => return Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(crate::error_text::io_reason(&e)),
        }
    }
    Err(t!("error-reason-internal"))
}

/// The surface of job `j`, smoothed and decimated as asked.
pub(crate) fn surface(j: &Job, smooth: bool, target: Option<usize>) -> Mesh {
    let mut mesh = Mesh::default();
    for (vol, pick) in &j.layers {
        let inside: Box<dyn Fn(f32) -> bool + Sync> = match *pick {
            Pick::Value(v) => Box::new(move |x| x == v),
            Pick::Half => Box::new(|x| x >= 32768.0),
        };
        let part = label_surface(vol, &*inside);
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend(part.vertices);
        mesh.triangles
            .extend(part.triangles.iter().map(|t| t.map(|i| i + base)));
    }
    if smooth {
        mesh.smooth_keeping_volume(SMOOTH_PASSES);
    }
    if let Some(n) = target {
        mesh.decimate(n);
    }
    mesh
}

/// Builds and writes every job's surface into a new folder in `dir`.
pub(crate) fn export(
    jobs: &[Job],
    dir: &Path,
    stem: &str,
    smooth: bool,
    target: Option<usize>,
) -> Result<Report, String> {
    let folder = new_folder(dir, stem)?;
    let mut report = Report {
        dir: folder.clone(),
        written: 0,
        empty: 0,
        triangles: 0,
        open: 0,
        three_mf: false,
    };
    let mut names: Vec<String> = Vec::new();
    let mut parts: Vec<(String, [u8; 3], Mesh)> = Vec::new();
    for j in jobs {
        // The working memory of the surface, estimated from the segment's size.
        let faces = 12.0 * (j.voxels as f64).powf(2.0 / 3.0) + 64.0;
        let _reserved =
            specular_core::budget::reserve((faces as u64).saturating_mul(BYTES_PER_TRIANGLE))
                .map_err(crate::error_text::needs_memory_reason)?;
        let mesh = surface(j, smooth, target);
        if mesh.triangles.is_empty() {
            report.empty += 1;
            continue;
        }
        if !mesh.is_closed() {
            report.open += 1;
            specular_log::warn!(target: specular_log::DECODE, "segment {:?}: surface not closed", j.name);
        }
        let mut stem = file_stem(&j.name);
        let base = stem.clone();
        let mut n = 2;
        while names.iter().any(|x| x.eq_ignore_ascii_case(&stem)) {
            stem = format!("{base} ({n})");
            n += 1;
        }
        names.push(stem.clone());
        let path = folder.join(format!("{stem}.stl"));
        write_atomic(&path, |f| {
            mesh.write_stl_titled(f, &format!("Specular segment {}, patient LPS mm", j.name))
        })?;
        report.written += 1;
        report.triangles += mesh.triangles.len();
        parts.push((j.name.clone(), j.colour, mesh));
    }
    if !parts.is_empty() && report.triangles <= MAX_3MF_TRIANGLES {
        let bytes = three_mf(&parts);
        write_atomic(&folder.join("segments.3mf"), |f| f.write_all(&bytes))?;
        report.three_mf = true;
    }
    if report.written == 0 {
        let _ = std::fs::remove_dir(&folder);
    }
    Ok(report)
}

use std::io::Write;

fn write_atomic(
    path: &Path,
    w: impl FnOnce(&mut std::io::BufWriter<std::fs::File>) -> std::io::Result<()>,
) -> Result<(), String> {
    let tmp = path.with_extension("part");
    let run = || -> std::io::Result<()> {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        w(&mut f)?;
        f.flush()?;
        f.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        std::fs::rename(&tmp, path)
    };
    run().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        crate::error_text::io_reason(&e)
    })
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A 3MF package of the parts (millimetres, patient LPS: z superior), each an object named
/// by its label with its colour as a base material.
pub(crate) fn three_mf(parts: &[(String, [u8; 3], Mesh)]) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut m = String::with_capacity(parts.iter().map(|p| p.2.triangles.len() * 100).sum());
    m.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    m.push_str("<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n");
    m.push_str("<metadata name=\"Application\">Specular</metadata>\n<resources>\n<basematerials id=\"1\">\n");
    for (name, c, _) in parts {
        let _ = writeln!(
            m,
            "<base name=\"{}\" displaycolor=\"#{:02X}{:02X}{:02X}\"/>",
            xml_escape(name),
            c[0],
            c[1],
            c[2]
        );
    }
    m.push_str("</basematerials>\n");
    for (k, (name, _, mesh)) in parts.iter().enumerate() {
        let _ = writeln!(
            m,
            "<object id=\"{}\" type=\"model\" name=\"{}\" pid=\"1\" pindex=\"{k}\"><mesh><vertices>",
            k + 2,
            xml_escape(name)
        );
        for v in &mesh.vertices {
            let _ = writeln!(
                m,
                "<vertex x=\"{:.4}\" y=\"{:.4}\" z=\"{:.4}\"/>",
                v[0], v[1], v[2]
            );
        }
        m.push_str("</vertices><triangles>\n");
        for t in &mesh.triangles {
            let _ = writeln!(
                m,
                "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
                t[0], t[1], t[2]
            );
        }
        m.push_str("</triangles></mesh></object>\n");
    }
    m.push_str("</resources>\n<build>\n");
    for k in 0..parts.len() {
        let _ = writeln!(m, "<item objectid=\"{}\"/>", k + 2);
    }
    m.push_str("</build>\n</model>\n");
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut z = specular_diag::zip::ZipWriter::new(unix);
    z.add(
        "[Content_Types].xml",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>\n",
    );
    z.add(
        "_rels/.rels",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>\n",
    );
    z.add("3D/3dmodel.model", m.as_bytes());
    z.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::specular_mesh;
    use specular_dicom::labels::{LabelSource, Layer, Segment};
    use specular_dicom::volume::{Geometry, VoxelKind};

    /// Two segments packed in one layer (a ball and a slab) and a fractional one.
    fn labels() -> Labels {
        let dims = [40u32, 36, 24];
        let pos: Vec<[f64; 3]> = (0..dims[2])
            .map(|k| [-20.0, -18.0, f64::from(k) * 1.2])
            .collect();
        let g = Geometry::new(
            &pos,
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.9, 0.9],
            [dims[0], dims[1]],
        );
        let mut packed = Volume::new(g.clone(), VoxelKind::U16, 1).unwrap();
        let mut frac = Volume::new(g, VoxelKind::U16, 1).unwrap();
        let mut counts = [0u64; 3];
        let n = (dims[0] * dims[1]) as usize;
        for k in 0..dims[2] {
            let mut a = vec![0f32; n];
            let mut b = vec![0f32; n];
            for j in 0..dims[1] {
                for i in 0..dims[0] {
                    let (x, y, z) = (
                        f64::from(i) - 12.0,
                        f64::from(j) - 18.0,
                        (f64::from(k) - 12.0) * 1.2 / 0.9,
                    );
                    let at = (j * dims[0] + i) as usize;
                    if x * x + y * y + z * z <= 64.0 {
                        a[at] = 1.0;
                        counts[0] += 1;
                    } else if (26..36).contains(&i) && (4..30).contains(&j) && (3..9).contains(&k) {
                        a[at] = 2.0;
                        counts[1] += 1;
                    }
                    if (26..36).contains(&i) && (4..30).contains(&j) && (14..20).contains(&k) {
                        b[at] = 60000.0;
                        counts[2] += 1;
                    }
                }
            }
            packed.put(k as usize, &a, dims[0], dims[1]).unwrap();
            frac.put(k as usize, &b, dims[0], dims[1]).unwrap();
        }
        let seg = |number, label: &str, voxels| Segment {
            number,
            label: label.into(),
            colour: [200, 40, 40],
            algorithm: "MANUAL".into(),
            fractional: false,
            ml: 0.0,
            voxels,
        };
        Labels {
            source: LabelSource::Seg,
            segments: vec![
                seg(1, "Liver/left lobe", counts[0]),
                seg(2, "", counts[1]),
                seg(3, "Liver/left lobe", counts[2]),
            ],
            layers: vec![
                Layer {
                    volume: packed,
                    fraction: None,
                },
                Layer {
                    volume: frac,
                    fraction: Some(2),
                },
            ],
            frame_of_reference: String::new(),
            referenced_series: Vec::new(),
            title: String::new(),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-scratch")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn every_segment_is_written_closed_with_its_volume() {
        let l = labels();
        let jobs = jobs_of_labels(&[&l]);
        assert_eq!(jobs.len(), 3);
        let voxel = 0.9 * 0.9 * 1.2;
        for (j, seg) in jobs.iter().zip(&l.segments) {
            let m = surface(j, true, Some(4000));
            assert!(m.is_closed(), "{} not closed", j.name);
            assert!(m.triangles.len() <= 4000);
            let want = seg.voxels as f64 * voxel;
            assert!(
                (m.volume() - want).abs() / want < 0.06,
                "{}: {} vs {want}",
                j.name,
                m.volume()
            );
        }
        let dir = scratch("segments");
        let r = export(&jobs, &dir, "case", true, Some(4000)).unwrap();
        assert_eq!((r.written, r.empty, r.open), (3, 0, 0));
        assert!(r.three_mf);
        assert_eq!(r.dir, dir.join("case-segments"));
        let mut files: Vec<String> = std::fs::read_dir(&r.dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(
            files,
            [
                "Liver_left lobe (2).stl",
                "Liver_left lobe.stl",
                "segment-2.stl",
                "segments.3mf"
            ]
        );
        // The in-tree mesh readers read every file back: as many triangles, the same volume.
        let lim = specular_mesh::Limits::default();
        let stl = specular_mesh::read_path(&r.dir.join("segment-2.stl"), &lim).unwrap();
        let tris = stl.indices.len() / 3;
        let m2 = surface(&jobs[1], true, Some(4000));
        assert_eq!(tris, m2.triangles.len());
        let three = specular_mesh::read_path(&r.dir.join("segments.3mf"), &lim).unwrap();
        let tris3 = three.indices.len() / 3;
        assert_eq!(tris3, r.triangles);
        // A second export goes to a folder of its own.
        let r2 = export(&jobs[..1], &dir, "case", false, None).unwrap();
        assert_eq!(r2.dir, dir.join("case-segments-2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_label_volume_gives_one_job_per_label() {
        let l = labels();
        let vol = Arc::new(l.layers[0].volume.clone());
        let jobs = jobs_of_label_volume(vol).unwrap();
        let names: Vec<_> = jobs.iter().map(|j| j.name.as_str()).collect();
        assert_eq!(names, ["label-1", "label-2"]);
        assert_eq!(jobs[0].voxels, l.segments[0].voxels);
        // Fractions are not labels.
        let mut v = l.layers[0].volume.clone();
        let mut buf = vec![0.5f32; 40 * 36];
        buf[0] = 1.0;
        v.put(0, &buf, 40, 36).unwrap();
        let f32v = {
            let mut g = Volume::new(v.geometry.clone(), VoxelKind::F32, 1).unwrap();
            g.put(0, &buf, 40, 36).unwrap();
            g
        };
        assert!(jobs_of_label_volume(Arc::new(f32v)).is_err());
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_stem("a/b\\c:d"), "a_b_c_d");
        assert_eq!(file_stem("  .. "), "segment");
        assert_eq!(file_stem("CON"), "segment CON");
        assert_eq!(file_stem("Niere links"), "Niere links");
        assert_eq!(file_stem(&"x".repeat(200)).chars().count(), 80);
    }
}
