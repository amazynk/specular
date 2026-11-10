//! Ophthalmic Tomography (PS3.3 A.67, C.8.17.10): an OCT volume is a multi-frame image of
//! B-scans, each located on an en-face Ophthalmic Photography image (the localiser) by its
//! Ophthalmic Frame Location: the referenced image and the B-scan's line on it as Reference
//! Coordinates (row, column pairs, in the image's pixel coordinates, the top left corner of
//! the top left pixel at 0, 0).

use std::path::Path;

use crate::charset::Charset;
use crate::parse::DataSet;
use crate::{Dicom, Result, Values};

/// Ophthalmic Tomography Image Storage.
pub const OPT_SOP_CLASS: &str = "1.2.840.10008.5.1.4.1.1.77.1.5.4";
/// Ophthalmic Photography 8 and 16 bit Image Storage.
pub const OP_SOP_CLASSES: [&str; 2] = [
    "1.2.840.10008.5.1.4.1.1.77.1.5.1",
    "1.2.840.10008.5.1.4.1.1.77.1.5.2",
];

const SOP_CLASS: u32 = 0x0008_0016;
const NUMBER_OF_FRAMES: u32 = 0x0028_0008;
const REFERENCED_SOP_INSTANCE: u32 = 0x0008_1155;
const REFERENCED_FRAME: u32 = 0x0008_1160;
const FRAME_LOCATION: u32 = 0x0022_0031;
const REFERENCE_COORDINATES: u32 = 0x0022_0032;
const DEPTH: u32 = 0x0022_0033;
const ORIENTATION: u32 = 0x0022_0039;
const SHARED: u32 = 0x5200_9229;
const PER_FRAME: u32 = 0x5200_9230;

/// Whether `sop_class` is Ophthalmic Tomography.
pub fn is_oct(sop_class: &str) -> bool {
    sop_class == OPT_SOP_CLASS
}

/// Whether `sop_class` is Ophthalmic Photography.
pub fn is_photography(sop_class: &str) -> bool {
    OP_SOP_CLASSES.contains(&sop_class)
}

/// Where one B-scan lies on the localiser.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameLocation {
    /// The B-scan (frame index, 0-based).
    pub frame: usize,
    /// SOP Instance UID of the localiser image.
    pub image: String,
    /// Frame of the localiser (1-based), when it has several.
    pub image_frame: Option<u32>,
    /// The B-scan's line on the localiser: (column, row) points.
    pub points: Vec<[f32; 2]>,
    /// LINEAR, NONLINEAR or TRANSVERSE.
    pub orientation: String,
    /// Depth of a transverse image (µm).
    pub depth: Option<f64>,
}

/// An OCT volume's frame locations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Oct {
    pub frames: usize,
    pub locations: Vec<FrameLocation>,
}

impl Oct {
    /// Reads the frame locations of OCT image `d` (parsed from `file`); None for other files.
    pub fn read(d: &Dicom, file: &[u8]) -> Option<Oct> {
        Oct::from_dataset(&d.dataset, &d.values(file))
    }

    /// Reads the frame locations of the OCT image at `path` from its header (the pixel data
    /// is not read); `Ok(None)` for other files.
    pub fn read_file(path: &Path) -> Result<Option<Oct>> {
        Ok(crate::series::read_header_with(path, &|ds, v, _| Oct::from_dataset(ds, v))?.flatten())
    }

    fn from_dataset(ds: &DataSet, v: &Values) -> Option<Oct> {
        let v = *v;
        let cs = Charset::of(ds, &v);
        let class = ds
            .get(SOP_CLASS)
            .map(|e| cs.decode(v.text_bytes(e), e.vr))
            .unwrap_or_default();
        if !is_oct(class.trim()) {
            return None;
        }
        let frames = ds
            .get(NUMBER_OF_FRAMES)
            .and_then(|e| v.number(e, 0))
            .map_or(1, |f| f.clamp(1.0, 1e9) as usize);
        let mut locations = Vec::new();
        let per = ds
            .get(PER_FRAME)
            .map(|e| e.items.as_slice())
            .unwrap_or_default();
        for (f, it) in per.iter().enumerate() {
            for loc in it
                .get(FRAME_LOCATION)
                .map(|e| e.items.as_slice())
                .unwrap_or_default()
            {
                if let Some(l) = location(loc, &v, &cs, f) {
                    locations.push(l);
                }
            }
        }
        // Without per-frame locations: shared, or top-level items, one per frame in order.
        if locations.is_empty() {
            let shared = ds.item(SHARED).and_then(|s| s.get(FRAME_LOCATION));
            let top = ds.get(FRAME_LOCATION);
            if let Some(e) = top.or(shared) {
                for (f, loc) in e.items.iter().enumerate() {
                    if let Some(l) = location(loc, &v, &cs, f) {
                        locations.push(l);
                    }
                }
            }
        }
        Some(Oct { frames, locations })
    }

    /// The localiser most frames are located on.
    pub fn localiser(&self) -> Option<&str> {
        let mut best: Option<(&str, usize)> = None;
        for l in &self.locations {
            let n = self.locations.iter().filter(|o| o.image == l.image).count();
            if best.is_none_or(|(_, b)| n > b) {
                best = Some((&l.image, n));
            }
        }
        best.map(|(s, _)| s)
    }

    /// The location of B-scan `frame` (0-based).
    pub fn location(&self, frame: usize) -> Option<&FrameLocation> {
        self.locations.iter().find(|l| l.frame == frame)
    }
}

fn location(it: &DataSet, v: &Values, cs: &Charset, frame: usize) -> Option<FrameLocation> {
    let cs = cs.within(it, v);
    let text = |tag| {
        it.get(tag)
            .map(|e| cs.decode(v.text_bytes(e), e.vr).trim().to_owned())
            .unwrap_or_default()
    };
    let image = text(REFERENCED_SOP_INSTANCE);
    let e = it.get(REFERENCE_COORDINATES)?;
    let n = match e.vr {
        crate::dict::Vr::FL => e.value.len() / 4,
        crate::dict::Vr::FD => e.value.len() / 8,
        _ => v.count(e),
    };
    let c: Vec<f64> = (0..n).filter_map(|i| v.number(e, i)).collect();
    let points: Vec<[f32; 2]> = c
        .chunks_exact(2)
        .map(|p| [p[1] as f32, p[0] as f32])
        .collect();
    if points.is_empty() {
        return None;
    }
    Some(FrameLocation {
        frame,
        image,
        image_frame: it
            .get(REFERENCED_FRAME)
            .and_then(|e| v.int(e))
            .map(|f| f as u32),
        points,
        orientation: text(ORIENTATION),
        depth: it.get(DEPTH).and_then(|e| v.number(e, 0)),
    })
}
