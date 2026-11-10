//! The developed photo for printing: the same full-resolution render as an export,
//! as 8-bit sRGB rows (the print path's page bands are sRGB; a printer profile, when chosen,
//! converts them as the PDF is written).

use std::path::Path;

use specular_convert::Resize;

use super::batch::{read_input, set_as_shot};
use super::render::{Exporter, Samples};
use super::{Depth, Preset, Space};
use crate::gpu::pool::Budget;
use crate::model::Edit;
use crate::preview::{Input, Orient};

/// A developed photo: upright sRGB8 rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Printed {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

/// Develops `path` with `edit` on `device` at full size, or with its long side at most
/// `long_side` (never enlarged).
pub fn develop_rgb8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    budget: Budget,
    path: &Path,
    edit: &Edit,
    registry: &specular_decode::Registry,
    long_side: Option<u32>,
) -> Result<Printed, String> {
    let read = read_input(path, registry).map_err(|e| format!("{e:?}"))?;
    let mut edit = edit.clone();
    if let Some(xy) = read.as_shot {
        set_as_shot(&mut edit, xy);
    }
    let mut exporter = Exporter::new(device, queue, budget);
    if let Input::Raw(s) = &read.input {
        exporter.calibration =
            crate::ops::correct::calib::for_edit(&edit, s, Some((path, &read.bytes)))?;
    }
    let vars = crate::ops::effects::frame::Vars::of_file(path, &read.bytes);
    develop_framed(
        &mut exporter,
        read.input,
        &edit,
        Orient::from_exif(read.orientation),
        long_side,
        Some(&vars),
    )
}

/// [`develop_rgb8`] for an input already read (no borders or watermark).
pub fn develop_input(
    exporter: &mut Exporter,
    input: Input,
    edit: &Edit,
    orient: Orient,
    long_side: Option<u32>,
) -> Result<Printed, String> {
    develop_framed(exporter, input, edit, orient, long_side, None)
}

/// [`develop_input`] with the edit's borders and watermark when `vars` are given.
pub fn develop_framed(
    exporter: &mut Exporter,
    input: Input,
    edit: &Edit,
    orient: Orient,
    long_side: Option<u32>,
    vars: Option<&crate::ops::effects::frame::Vars>,
) -> Result<Printed, String> {
    let preset = Preset {
        depth: Depth::Eight,
        space: Space::Srgb,
        size: long_side.map_or(Resize::None, Resize::LongEdge),
        upscale: false,
        ..Preset::default()
    };
    let mut img = exporter
        .render(input, edit, orient, &preset, &mut |_| true)
        .map_err(|e| e.to_string())?;
    if let Some(v) = vars {
        // A watermark that cannot be drawn leaves the print without it.
        let _ = crate::ops::effects::frame::add_to(&mut img, edit, v, Space::Srgb, false);
    }
    match img.samples {
        Samples::Eight(rgb) => Ok(Printed {
            width: img.width,
            height: img.height,
            rgb,
        }),
        _ => Err("the render did not give 8-bit samples".into()),
    }
}
