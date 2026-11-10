//! The planes' export: the three views' planes through the crosshair, each over the whole
//! volume at its finest voxel edge (square pixels), with the slab and window shown, as 8-bit
//! grey PNG images (`<file>-axial.png`… or `<file>-xy.png`… without patient orientation).

use std::path::Path;

use specular_dicom::volume::Volume;
use specular_render::Window as WlWindow;
use specular_render_volume::{Plane, reference};

use super::views::{V3, Views, add, dot, sub};

/// The longest side an exported plane gets (pixels): a larger volume is sampled coarser.
pub(crate) const MAX_SIDE: f64 = 8192.0;

/// View `v`'s plane through the crosshair over the whole volume as grey pixels: width,
/// height and rows top-down.
pub(crate) fn plane_image(
    vol: &Volume,
    views: &Views,
    v: usize,
    window: WlWindow,
) -> (u32, u32, Vec<u8>) {
    let f = &views.frames[v];
    let n = views.normal(v);
    // The plane's point nearest the volume's centre, and the volume's extent along the
    // view's right and down.
    let centre = add(views.vc, n, dot(sub(views.c, views.vc), n));
    let (mut half_r, mut half_d) = (0.0f64, 0.0f64);
    for k in &views.corners {
        let d: V3 = sub(*k, centre);
        half_r = half_r.max(dot(d, f.right).abs());
        half_d = half_d.max(dot(d, f.down).abs());
    }
    let mut mm = views.step.max(1e-9);
    let longest = 2.0 * half_r.max(half_d);
    if longest / mm > MAX_SIDE {
        mm = longest / MAX_SIDE;
    }
    let w = ((2.0 * half_r / mm).round() as u32).max(1);
    let h = ((2.0 * half_d / mm).round() as u32).max(1);
    let mut plane = Plane::centred([0, 0, w, h], centre, f.right, f.down, mm, window);
    plane.thickness = views.thickness;
    plane.slab = if views.thickness > 0.0 {
        views.slab
    } else {
        specular_render_volume::Slab::Average
    };
    let mut px = Vec::with_capacity(w as usize * h as usize);
    for y in 0..h {
        for x in 0..w {
            let g = reference(vol, &plane, f64::from(x) + 0.5, f64::from(y) + 0.5)
                .map_or(0.0, |val| window.grey(f64::from(val)));
            px.push((g * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    (w, h, px)
}

/// The file names' plane parts.
fn plane_id(v: usize, oriented: bool) -> &'static str {
    match (v, oriented) {
        (0, true) => "axial",
        (1, true) => "coronal",
        (2, true) => "sagittal",
        (0, false) => "xy",
        (1, false) => "xz",
        _ => "yz",
    }
}

/// Writes the three planes into `dir` as `<stem>-<plane>.png` (each through a temporary
/// file beside it): how many, and the folder's name.
pub(crate) fn export(
    vol: &Volume,
    views: &Views,
    window: WlWindow,
    oriented: bool,
    dir: &Path,
    stem: &str,
) -> Result<(usize, String), String> {
    for v in 0..3 {
        let (w, h, px) = plane_image(vol, views, v, window);
        let png = specular_png::encode(w, h, specular_png::ColorType::Gray, &px)
            .map_err(|e| e.to_string())?;
        let path = dir.join(format!("{stem}-{}.png", plane_id(v, oriented)));
        let tmp = path.with_extension("png.part");
        std::fs::write(&tmp, &png)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                crate::error_text::io_reason(&e)
            })?;
    }
    let folder = dir.file_name().map_or_else(
        || dir.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    Ok((3, folder))
}
