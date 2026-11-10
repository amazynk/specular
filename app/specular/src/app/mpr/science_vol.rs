//! Scientific images as MPR volumes: which axis of a [`ScienceSummary`] is z (a microscopy
//! z-stack, an EM tomogram, a FITS, HDF5 or Zarr cube), the voxel size from the file's pixel
//! calibration, the z axis' step or a linear WCS (cubic voxels with a note when it gives
//! none), and the other axes' positions the planes are read at (the channel and time point
//! shown).

use specular_decode::science::{AxisKind, ScienceSummary};
use specular_i18n::t;

use super::load::Length;

/// A stack MPR reads plane by plane through the session.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stack {
    pub width: u32,
    pub height: u32,
    /// The z axis (an index into [`ScienceSummary::axes`]) and its length.
    pub z: usize,
    pub depth: usize,
    /// The position on every axis the planes are read at (z's is replaced).
    pub index: Vec<u32>,
    /// Voxel edges along x, y and z, in `length`.
    pub spacing: [f64; 3],
    pub length: Length,
    /// The values' unit.
    pub unit: Option<String>,
    /// The display range the science view uses for the shown plane.
    pub limits: Option<(f32, f32)>,
    /// The automatic stretch's midtones balance for the shown plane (0.5: none).
    pub mid: f32,
    /// The panel's lines: what the other axes show, the voxel size assumed.
    pub notes: Vec<String>,
}

impl Stack {
    pub(crate) fn slices(&self) -> usize {
        self.depth
    }

    /// The plane index of slice `k`.
    pub(crate) fn index_at(&self, k: usize) -> Vec<u32> {
        let mut i = self.index.clone();
        if let Some(z) = i.get_mut(self.z) {
            *z = k as u32;
        }
        i
    }
}

/// Fewer planes than this along z make no volume.
pub(crate) const MIN_DEPTH: u32 = 3;

/// Whether an axis' name is time or a spectrum's wavelength rather than space.
fn not_space(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    matches!(
        n.as_str(),
        "t" | "time" | "frame" | "frames" | "\u{3bb}" | "lambda" | "wavelength"
    )
}

/// The axis MPR stacks: one named Z, else the first data cube axis that is not time or
/// wavelength. `None` for images that are not volumes (slides shown from their raster,
/// seismic sections with their own volume, videos, meshes, too few planes or pixels).
pub(crate) fn z_axis(s: &ScienceSummary) -> Option<usize> {
    if s.raster_only
        || s.seismic.is_some()
        || s.video.is_some()
        || s.mesh.is_some()
        || s.width < 2
        || s.height < 2
    {
        return None;
    }
    let ok = |a: &specular_decode::science::NdAxis| a.len >= MIN_DEPTH;
    if let Some(z) = s
        .axes
        .iter()
        .position(|a| a.name.trim().eq_ignore_ascii_case("z"))
    {
        return ok(&s.axes[z]).then_some(z);
    }
    s.axes
        .iter()
        .position(|a| a.kind == AxisKind::Cube && ok(a) && !not_space(&a.name))
}

/// Metres per `unit` of length, `None` for anything else (degrees, hertz, seconds).
pub(crate) fn metres(unit: &str) -> Option<f64> {
    Some(match unit.trim() {
        "m" | "metre" | "meter" | "metres" | "meters" => 1.0,
        "cm" => 1e-2,
        "mm" => 1e-3,
        "\u{b5}m" | "\u{3bc}m" | "um" | "micron" | "microns" | "micrometer" | "micrometre" => 1e-6,
        "nm" | "nanometer" | "nanometre" => 1e-9,
        "\u{c5}" | "\u{212b}" | "A" | "angstrom" | "Angstrom" => 1e-10,
        "pm" => 1e-12,
        _ => return None,
    })
}

/// The unit lengths show in for voxels `edge` metres across.
fn length_for(edge: f64) -> Length {
    if edge >= 1e-4 {
        Length::Mm
    } else if edge >= 1e-7 {
        Length::Micron
    } else if edge >= 5e-10 {
        Length::Nanometre
    } else {
        Length::Angstrom
    }
}

fn per_unit(l: Length) -> f64 {
    match l {
        Length::Mm => 1e-3,
        Length::Micron => 1e-6,
        Length::Nanometre => 1e-9,
        Length::Angstrom => 1e-10,
        Length::Voxel => 1.0,
    }
}

/// The voxel edges along x, y and z in the unit they show in, from the pixel calibration
/// and the z axis' step, else a linear WCS whose three axes are lengths; cubic voxels
/// (`true`: assumed) when the file gives no size for some axis.
pub(crate) fn spacing(s: &ScienceSummary, z: usize) -> ([f64; 3], Length, bool) {
    let step = |step: f64, unit: &str| {
        metres(unit)
            .map(|m| (step * m).abs())
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    let mut xyz = [None; 3];
    if let Some(p) = &s.pixel {
        xyz[0] = step(p.x.step, &p.unit_x);
        xyz[1] = step(p.y.step, &p.unit_y);
    }
    let a = &s.axes[z];
    xyz[2] = a.linear.and_then(|l| step(l.step, &a.unit));
    // A linear WCS of lengths (not sky angles or a spectrum).
    if let Some(w) = s.wcs.as_ref().filter(|w| w.celestial.is_none()) {
        let world = |axis: usize| {
            let d = w.axis_value(axis, 1)? - w.axis_value(axis, 0)?;
            step(d, &w.axes.get(axis)?.cunit)
        };
        xyz[0] = xyz[0].or_else(|| world(0));
        xyz[1] = xyz[1].or_else(|| world(1));
        xyz[2] = xyz[2].or_else(|| a.wcs_axis.and_then(world));
    }
    match xyz {
        [Some(x), Some(y), Some(z)] => {
            let l = length_for(x.min(y));
            let u = per_unit(l);
            ([x / u, y / u, z / u], l, false)
        }
        // Known in plane only: the slices one pixel apart.
        [Some(x), Some(y), None] => {
            let l = length_for(x.min(y));
            let u = per_unit(l);
            ([x / u, y / u, x.min(y) / u], l, true)
        }
        _ => ([1.0; 3], Length::Voxel, true),
    }
}

/// Values for the volume: NaN (undefined) as 0, so slabs and the 3D view stay finite.
pub(crate) fn finite(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .map(|&v| if v.is_finite() { v } else { 0.0 })
        .collect()
}

/// The stack of `s` at the positions `index` (the science view's), `limits` its display
/// range; `composite` says the view composites channels (MPR shows the channel at `index`).
pub(crate) fn stack(
    s: &ScienceSummary,
    index: &[u32],
    limits: Option<(f32, f32)>,
    composite: bool,
) -> Option<Stack> {
    let z = z_axis(s)?;
    let (spacing, length, assumed) = spacing(s, z);
    let mut index: Vec<u32> = (0..s.axes.len())
        .map(|a| index.get(a).copied().unwrap_or(0))
        .collect();
    for (i, a) in index.iter_mut().zip(&s.axes) {
        *i = (*i).min(a.len.saturating_sub(1));
    }
    let mut notes = Vec::new();
    for (a, axis) in s.axes.iter().enumerate() {
        if a == z || axis.len < 2 {
            continue;
        }
        let name = match axis.name.trim() {
            "" if axis.kind == AxisKind::Channel => t!("mpr-axis-channel"),
            "" => t!("mpr-axis-unnamed", axis = (a + 1) as i64),
            n => n.to_owned(),
        };
        let at = index[a] as usize;
        notes.push(match axis.labels.get(at).filter(|l| !l.is_empty()) {
            Some(l) => t!("mpr-axis-label", axis = name, label = l.clone()),
            None => t!(
                "mpr-axis-position",
                axis = name,
                position = (at + 1) as i64,
                count = i64::from(axis.len)
            ),
        });
    }
    if composite && s.channel_axis.is_some_and(|c| c != z) {
        notes.push(t!("mpr-one-channel"));
    }
    if assumed {
        notes.push(match length {
            Length::Voxel => t!("mpr-isotropic"),
            _ => t!("mpr-isotropic-z"),
        });
    }
    let unit = s
        .units_by
        .as_ref()
        .and_then(|(a, units)| units.get(index.get(*a).copied()? as usize).cloned())
        .or_else(|| s.unit.clone())
        .filter(|u| !u.trim().is_empty());
    Some(Stack {
        width: s.width,
        height: s.height,
        z,
        depth: s.axes[z].len as usize,
        index,
        spacing,
        length,
        unit,
        limits,
        mid: 0.5,
        notes,
    })
}
