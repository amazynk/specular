//! Export recipes ([`specular_convert::recipe`]) on developed photos: each image with an
//! edit is rendered through the export path (the same graph as the preview, borders and
//! watermark of the edit included) as linear Rec.2020 floats at the size its recipes need,
//! and the recipes then size, colour, sharpen, watermark and write it. Images without an
//! edit go through the plain decoders, as Convert does.
//!
//! Renders take turns on the one device (a lock around the exporter) while the other
//! workers finish and encode, so GPU and CPU overlap.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use specular_convert::recipe::finish::Linear;
use specular_convert::recipe::space::Primaries;
use specular_convert::recipe::{Facts, Files, Item, Render, Rendered};
use specular_convert::{Failure, Resize};
use specular_core::CancelToken;
use specular_decode::Registry;

use super::batch::{self, Job};
use super::render::{Exporter, Samples};
use super::{Depth, FileFormat, HdrOutput, Preset, Space};
use crate::gpu::pool::Budget;
use crate::model::Edit;
use crate::preview::Orient;

/// Renders developed photos for recipes.
pub struct Developed {
    exporter: Mutex<Exporter>,
    plain: Files,
    /// The edit (and turn) of each image that has one.
    edits: HashMap<PathBuf, (Edit, Option<Orient>)>,
}

impl Developed {
    /// `edits`: the photos to develop, with their edit and turn (`None`: as the file says);
    /// any other image is decoded as it is.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        budget: Budget,
        registry: Arc<Registry>,
        edits: HashMap<PathBuf, (Edit, Option<Orient>)>,
    ) -> Developed {
        Developed {
            exporter: Mutex::new(Exporter::new(device, queue, budget)),
            plain: Files { registry },
            edits,
        }
    }
}

/// The preset that renders linear Rec.2020 floats at `percent` of full size.
fn float_preset(percent: u32) -> Preset {
    Preset {
        format: FileFormat::Tiff,
        depth: Depth::Float,
        space: Space::Rec2020,
        hdr: HdrOutput::Off,
        size: if percent >= 100 {
            Resize::None
        } else {
            Resize::Percent(percent.max(1))
        },
        ..Preset::default()
    }
}

impl Render for Developed {
    fn facts(&self, item: &Item) -> Result<Facts, Failure> {
        self.plain.facts(item)
    }

    fn render(
        &self,
        item: &Item,
        scale: f64,
        cancel: &CancelToken,
    ) -> Result<Option<Rendered>, Failure> {
        let Some((edit, orient)) = self.edits.get(&item.path) else {
            return self.plain.render(item, scale, cancel);
        };
        let read = batch::read_input(&item.path, &self.plain.registry)?;
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let noise = match &read.input {
            crate::preview::Input::Raw(s)
                if edit
                    .modules
                    .iter()
                    .any(|m| m.kind == crate::model::ModuleKind::DENOISE && m.enabled) =>
            {
                Some(crate::ops::detail::RawNoise::of_sensor(s))
            }
            _ => None,
        };
        let percent = ((scale * 100.0).ceil() as u32).clamp(1, 100);
        let preset = float_preset(percent);
        let job = Job {
            source: item.path.clone(),
            edit: edit.clone(),
            orient: *orient,
        };
        let mut part = |_f: f32| !cancel.is_cancelled();
        let mut ex = self.exporter.lock().unwrap_or_else(|e| e.into_inner());
        let res = batch::render_job(&mut ex, &job, read, noise, &preset, &mut part);
        drop(ex);
        let (img, bytes) = match res {
            Ok(r) => r,
            Err(_) if cancel.is_cancelled() => return Ok(None),
            Err(e) => return Err(e),
        };
        let Samples::Float(v) = &img.samples else {
            return Err(Failure::Decode("the render gave no float samples".into()));
        };
        let n = (img.width * img.height) as usize;
        let alpha = img.alpha.as_deref().filter(|a| a.len() == n);
        let mut rgba = Vec::with_capacity(n * 4);
        for (i, p) in v.chunks_exact(3).enumerate().take(n) {
            let a = alpha.map_or(1.0, |a| a[i].clamp(0.0, 1.0));
            rgba.extend([p[0] * a, p[1] * a, p[2] * a, a]);
        }
        let k = 100.0 / f64::from(percent);
        let full = (
            (f64::from(img.width) * k).round() as u32,
            (f64::from(img.height) * k).round() as u32,
        );
        Ok(Some(Rendered {
            image: Linear {
                width: img.width,
                height: img.height,
                rgba,
                primaries: Primaries::Rec2020,
            },
            full,
            meta: specular_convert::source_meta(&bytes),
        }))
    }
}
