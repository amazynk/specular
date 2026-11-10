//! Electron Microscopy Dataset conventions on top of HDF5: Berkeley/NCEM EMD (groups marked
//! `emd_group_type`, a `data` dataset and `dim1..N` axis vectors) and Velox EMD (FEI/Thermo
//! Fisher: `/Data/Image/<id>/Data` with JSON metadata per frame).

use crate::{File, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Convention {
    Berkeley,
    Velox,
}

/// One axis of an EMD image dataset.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Axis {
    pub name: String,
    pub unit: String,
    /// Coordinate of the first sample and the step between samples, when the axis vector has
    /// at least two.
    pub start: f64,
    pub step: Option<f64>,
}

/// An image dataset found through an EMD convention.
#[derive(Clone, Debug, Default)]
pub struct Image {
    /// Index into [`File::objects`].
    pub index: usize,
    pub path: String,
    pub title: String,
    /// One per dataset dimension (Berkeley), empty when unknown.
    pub axes: Vec<Axis>,
    /// The dimensions that run along x and y, and the frame dimension when there is one.
    pub x_dim: usize,
    pub y_dim: usize,
    /// Physical size of one pixel, x then y, and its unit.
    pub pixel_size: Option<(f64, f64, String)>,
    /// Physical position of the first pixel, x then y, in the unit of `pixel_size` (Velox
    /// `BinaryResult.Offset`).
    pub pixel_offset: Option<(f64, f64)>,
    pub detector: Option<String>,
    pub instrument: Option<String>,
    pub date: Option<String>,
    /// Flattened metadata (Velox JSON keys, or Berkeley attributes).
    pub metadata: Vec<(String, String)>,
}

/// Which EMD convention a file follows, if any.
pub fn detect(file: &File) -> Option<Convention> {
    let root = file.get("/")?;
    if root.attr("version_major").is_some() && file.objects.iter().any(is_berkeley_group) {
        return Some(Convention::Berkeley);
    }
    if file.get("/Data/Image").is_some() || velox_version(file) {
        return Some(Convention::Velox);
    }
    None
}

fn velox_version(file: &File) -> bool {
    file.get("/Version").is_some_and(|o| o.dataset().is_some()) && file.get("/Data").is_some()
}

fn is_berkeley_group(o: &crate::Object) -> bool {
    match o.attr("emd_group_type") {
        Some(Value::Str(s)) => s
            .first()
            .is_some_and(|s| s.trim() == "array" || s.trim() == "1"),
        Some(v) => v.as_f64() == Some(1.0),
        None => false,
    }
}

/// "[n_m]" → "nm", "[1/n_m]" → "1/nm".
fn clean_unit(s: &str) -> String {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    s.replace('_', "")
}

fn str_attr(o: &crate::Object, name: &str) -> Option<String> {
    match o.attr(name)? {
        Value::Str(s) => s.first().cloned().filter(|s| !s.is_empty()),
        v => Some(v.display(8)),
    }
}

/// The image datasets of a file in a known EMD convention.
pub fn images(file: &File, data: &[u8]) -> Vec<Image> {
    match detect(file) {
        Some(Convention::Berkeley) => berkeley(file, data),
        Some(Convention::Velox) => velox(file, data),
        None => Vec::new(),
    }
}

fn berkeley(file: &File, data: &[u8]) -> Vec<Image> {
    let mut out = Vec::new();
    for g in file.objects.iter().filter(|o| is_berkeley_group(o)) {
        let base = if g.path == "/" {
            String::new()
        } else {
            g.path.clone()
        };
        let path = format!("{base}/data");
        let Some(index) = file.index_of(&path) else {
            continue;
        };
        let Some(ds) = file.objects[index].dataset() else {
            continue;
        };
        let rank = ds.shape().len();
        if rank < 2 {
            continue;
        }
        // Axis vectors: dim1..dimN (some writers start at dim0).
        let first = if file.get(&format!("{base}/dim0")).is_some()
            && file.get(&format!("{base}/dim{rank}")).is_none()
        {
            0
        } else {
            1
        };
        let mut axes = Vec::with_capacity(rank);
        for k in 0..rank {
            let dpath = format!("{base}/dim{}", k + first);
            let mut axis = Axis::default();
            if let (Some(i), Some(o)) = (file.index_of(&dpath), file.get(&dpath)) {
                axis.name = str_attr(o, "name")
                    .map(|s| clean_unit(&s))
                    .unwrap_or_default();
                axis.unit = str_attr(o, "units")
                    .map(|s| clean_unit(&s))
                    .unwrap_or_default();
                if let Ok(v) = file.read_all::<f64>(data, i, 1 << 24)
                    && let Some(&a) = v.first()
                {
                    axis.start = a;
                    axis.step = v
                        .get(1)
                        .map(|b| b - a)
                        .filter(|s| s.is_finite() && *s != 0.0);
                }
            }
            axes.push(axis);
        }
        // EMD stores images as (..., y, x) unless the axis names say otherwise.
        let (mut x_dim, mut y_dim) = (rank - 1, rank - 2);
        let named = |want: &[&str]| {
            axes.iter()
                .position(|a| want.iter().any(|w| a.name.eq_ignore_ascii_case(w)))
        };
        if let (Some(x), Some(y)) = (
            named(&["x", "rx", "kx", "qx"]),
            named(&["y", "ry", "ky", "qy"]),
        ) && x != y
        {
            x_dim = x;
            y_dim = y;
        }
        let pixel_size = match (&axes[x_dim].step, &axes[y_dim].step) {
            (Some(sx), Some(sy))
                if axes[x_dim].unit == axes[y_dim].unit && !axes[x_dim].unit.is_empty() =>
            {
                Some((sx.abs(), sy.abs(), axes[x_dim].unit.clone()))
            }
            _ => None,
        };
        let title = str_attr(g, "title")
            .or_else(|| str_attr(&file.objects[index], "title"))
            .unwrap_or_else(|| g.name().to_string());
        let mut metadata: Vec<(String, String)> = Vec::new();
        for o in [file.get("/"), Some(g)].into_iter().flatten() {
            for a in &o.attrs {
                metadata.push((a.name.clone(), a.value.display(8)));
            }
        }
        // Microscope and user groups carry the acquisition details.
        for grp in ["/microscope", "/user", "/sample", "/comments"] {
            if let Some(o) = file.get(grp) {
                for a in &o.attrs {
                    metadata.push((format!("{}.{}", &grp[1..], a.name), a.value.display(8)));
                }
            }
        }
        let instrument = file
            .get("/microscope")
            .and_then(|m| str_attr(m, "name").or_else(|| str_attr(m, "microscope")));
        out.push(Image {
            index,
            path,
            title,
            axes,
            x_dim,
            y_dim,
            pixel_size,
            pixel_offset: None,
            detector: None,
            instrument,
            date: None,
            metadata,
        });
    }
    out
}

/// Flattens JSON into dotted keys (at most `max` entries).
fn flatten(v: &serde_json::Value, prefix: &str, out: &mut Vec<(String, String)>, max: usize) {
    if out.len() >= max {
        return;
    }
    match v {
        serde_json::Value::Object(m) => {
            for (k, v) in m {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(v, &key, out, max);
            }
        }
        serde_json::Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                flatten(v, &format!("{prefix}[{i}]"), out, max);
            }
        }
        serde_json::Value::String(s) => out.push((prefix.to_string(), s.clone())),
        serde_json::Value::Null => {}
        other => out.push((prefix.to_string(), other.to_string())),
    }
}

fn json_num(v: Option<&serde_json::Value>) -> Option<f64> {
    match v? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn json_str(v: Option<&serde_json::Value>) -> Option<String> {
    match v? {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The JSON metadata of the first frame of a Velox image.
fn velox_json(file: &File, data: &[u8], path: &str) -> Option<serde_json::Value> {
    let i = file.index_of(path)?;
    let ds = file.objects[i].dataset()?;
    let shape = ds.shape();
    if shape.len() != 2 || shape[0] == 0 || shape[1] == 0 || shape[0] > 1 << 24 {
        return None;
    }
    let bytes = file
        .read_box_bytes(data, i, &[0, 0], &[shape[0], 1], &|| false)
        .ok()?;
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    serde_json::from_slice(&bytes[..end]).ok()
}

/// The JSON of a one-element string dataset (Velox `Operations`, `Displays`, ...).
fn json_dataset(file: &File, data: &[u8], path: &str) -> Option<serde_json::Value> {
    let i = file.index_of(path)?;
    let ds = file.objects[i].dataset()?;
    if ds.shape().contains(&0) {
        return None;
    }
    let rank = ds.shape().len();
    let elem = file
        .read_box_bytes(data, i, &vec![0; rank], &vec![1; rank], &|| false)
        .ok()?;
    let text = match &ds.dtype {
        crate::Dtype::VlenStr => crate::gheap::vlen(data, &file.ctx, &elem).ok()?,
        crate::Dtype::Str { .. } => &elem[..],
        _ => return None,
    };
    let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
    serde_json::from_slice(&text[..end]).ok()
}

/// The JSON of the string datasets in a group, by name.
fn json_children(file: &File, data: &[u8], group: &str) -> Vec<serde_json::Value> {
    let mut paths: Vec<&str> = file
        .children(group)
        .filter(|o| o.dataset().is_some())
        .map(|o| o.path.as_str())
        .collect();
    paths.sort_unstable();
    paths
        .into_iter()
        .filter_map(|p| json_dataset(file, data, p))
        .collect()
}

fn json_has(v: &serde_json::Value, field: &str, key: &str) -> bool {
    v.get(field)
        .and_then(|p| p.as_str())
        .is_some_and(|p| p.contains(key))
}

/// The name Velox's processing gives image group `key`: the camera or STEM detector that
/// acquired it, or the iDPC/dDPC, difference, DPC, DCFI or FFT operation that computed it.
fn velox_operation_name(ops: &[(String, serde_json::Value)], key: &str) -> Option<String> {
    let of = |kind: &'static str| ops.iter().filter(move |(k, _)| k == kind).map(|(_, v)| v);
    let dpc = |v: &serde_json::Value| {
        if v.get("integrationMode").and_then(|m| m.as_str()) == Some("true") {
            "iDPC"
        } else {
            "dDPC"
        }
    };
    if let Some(v) = of("CameraInputOperation").find(|v| json_has(v, "dataPath", key)) {
        return json_str(v.get("cameraName"));
    }
    if let Some(v) = of("StemInputOperation").find(|v| json_has(v, "dataPath", key)) {
        return json_str(v.get("detector"));
    }
    for v in of("SurfaceReconstructionOperation") {
        if json_has(v, "unfilteredDataPath", key) {
            return Some(dpc(v).into());
        }
        if json_has(v, "dataPath", key) {
            return Some(
                if v.get("enableFilter").and_then(|f| f.as_str()) == Some("true") {
                    format!("Filtered {}", dpc(v))
                } else {
                    dpc(v).into()
                },
            );
        }
    }
    if let Some(v) = of("MathematicsOperation").find(|v| json_has(v, "dataPath", key)) {
        match v.pointer("/outputs/0/inputIndex").and_then(|i| i.as_str()) {
            Some("0") => return Some("A-C".into()),
            Some("1") => return Some("B-D".into()),
            _ => {}
        }
    }
    if of("DpcOperation").any(|v| json_has(v, "dataPath", key)) {
        return Some("DPC".into());
    }
    if of("IntegrationOperation").any(|v| json_has(v, "dataPath", key)) {
        return Some("DCFI".into());
    }
    if of("FftOperation").any(|v| json_has(v, "imageOutputPath", key)) {
        return Some("Half FFT".into());
    }
    None
}

/// Image group → display label, for the element maps of a spectrum image (Velox 11 keeps
/// displays in `/Displays`, older files in `/Presentation/Displays`).
fn velox_map_labels(file: &File, data: &[u8]) -> Vec<(String, String)> {
    let leaf = |p: &str| p.rsplit('/').next().unwrap_or("").to_string();
    let mut out = Vec::new();
    for v in json_children(file, data, "/Displays/ImageDisplay") {
        let (Some(r), Some(id)) = (json_str(v.get("data")), json_str(v.get("id"))) else {
            continue;
        };
        if let Some(p) = json_dataset(file, data, &r).and_then(|d| json_str(d.get("dataPath"))) {
            out.push((leaf(&p), id));
        }
    }
    for v in json_children(file, data, "/Presentation/Displays/ImageDisplay") {
        if let (Some(p), Some(label)) = (
            json_str(v.get("dataPath")),
            json_str(v.pointer("/display/label")),
        ) {
            out.push((leaf(&p), label));
        }
    }
    out
}

fn velox(file: &File, data: &[u8]) -> Vec<Image> {
    let mut out = Vec::new();
    let mut groups: Vec<&crate::Object> = file.children("/Data/Image").collect();
    groups.sort_by(|a, b| a.path.cmp(&b.path));
    let mut kinds: Vec<&crate::Object> = file
        .children("/Operations")
        .filter(|o| o.dataset().is_none())
        .collect();
    kinds.sort_by(|a, b| a.path.cmp(&b.path));
    let ops: Vec<(String, serde_json::Value)> = kinds
        .iter()
        .flat_map(|k| {
            json_children(file, data, &k.path)
                .into_iter()
                .map(|v| (k.name().to_string(), v))
        })
        .collect();
    // The element maps of a spectrum image are named as Velox displays them.
    let labels = if file.get("/Data/SpectrumImage").is_some() {
        velox_map_labels(file, data)
    } else {
        Vec::new()
    };
    for g in groups {
        let path = format!("{}/Data", g.path);
        let Some(index) = file.index_of(&path) else {
            continue;
        };
        let Some(ds) = file.objects[index].dataset() else {
            continue;
        };
        if ds.shape().len() < 2 {
            continue;
        }
        let json = velox_json(file, data, &format!("{}/Metadata", g.path));
        let mut metadata = Vec::new();
        let (mut pixel_size, mut pixel_offset) = (None, None);
        let (mut detector, mut instrument, mut date, mut detector_name) = (None, None, None, None);
        if let Some(j) = &json {
            flatten(j, "", &mut metadata, 2000);
            let br = j.get("BinaryResult");
            let ps = br.and_then(|b| b.get("PixelSize"));
            let w = json_num(ps.and_then(|p| p.get("width")));
            let h = json_num(ps.and_then(|p| p.get("height")));
            let unit = json_str(br.and_then(|b| b.get("PixelUnitX"))).unwrap_or_else(|| "m".into());
            if let Some(w) = w.filter(|w| w.is_finite() && *w > 0.0) {
                let h = h.filter(|h| h.is_finite() && *h > 0.0).unwrap_or(w);
                pixel_size = Some((w, h, unit));
                let off = br.and_then(|b| b.get("Offset"));
                pixel_offset = json_num(off.and_then(|o| o.get("x")))
                    .zip(json_num(off.and_then(|o| o.get("y"))))
                    .filter(|(x, y)| x.is_finite() && y.is_finite());
            }
            detector = json_str(br.and_then(|b| b.get("Detector")));
            detector_name = json_str(br.and_then(|b| b.get("DetectorIndex"))).and_then(|k| {
                json_str(
                    j.get("Detectors")
                        .and_then(|d| d.get(format!("Detector-{k}")))
                        .and_then(|d| d.get("DetectorName")),
                )
            });
            let ins = j.get("Instrument");
            instrument = json_str(ins.and_then(|i| i.get("InstrumentModel")))
                .or_else(|| json_str(ins.and_then(|i| i.get("InstrumentClass"))));
            date = json_num(
                j.get("Acquisition")
                    .and_then(|a| a.get("AcquisitionStartDatetime"))
                    .and_then(|d| d.get("DateTime")),
            )
            .and_then(unix_to_iso);
        }
        let key = g.name();
        let title = labels
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, l)| l.clone())
            .or_else(|| velox_operation_name(&ops, key))
            .or(detector_name)
            .or_else(|| detector.clone())
            .unwrap_or_else(|| key.to_string());
        // Velox stores (rows, columns, frames).
        out.push(Image {
            index,
            path,
            title,
            axes: Vec::new(),
            x_dim: 1,
            y_dim: 0,
            pixel_size,
            pixel_offset,
            detector,
            instrument,
            date,
            metadata,
        });
    }
    out
}

/// Seconds since 1970 as an ISO 8601 UTC date and time.
fn unix_to_iso(secs: f64) -> Option<String> {
    if !(0.0..1e11).contains(&secs) {
        return None;
    }
    let s = secs as i64;
    let days = s.div_euclid(86_400);
    let rem = s.rem_euclid(86_400);
    // Civil date from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_and_dates() {
        assert_eq!(clean_unit("[n_m]"), "nm");
        assert_eq!(clean_unit("[1/n_m]"), "1/nm");
        assert_eq!(unix_to_iso(0.0).as_deref(), Some("1970-01-01T00:00:00"));
        assert_eq!(
            unix_to_iso(1_700_000_000.0).as_deref(),
            Some("2023-11-14T22:13:20")
        );
    }
}
