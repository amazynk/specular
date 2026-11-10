//! Printing from Develop (print layouts): the print thread's images are developed
//! photos, rendered on the GPU like an export (`specular_develop::export::print`), at the
//! size their area prints at (full size for the PDF at its resolution, small for the
//! dialog's preview), one at a time to stay inside the memory budget.
//!
//! The photo open in Develop prints with its edit as it is on screen; other photos (the
//! gallery's selection or the picks) with their saved edit, else the edit Develop would
//! start from. A style chosen at print time is applied to a copy of each edit: the edits
//! themselves never change.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::{ApplyMode, Edit, Registry, Style};
use specular_develop::ops::geometry::{Context, Plan};
use specular_develop::preview::Orient;
use specular_develop::proof::{ProofIntent, ProofSettings, Proofer};
use specular_gfx::{Gpu, wgpu};
use specular_print::{ImageSource, Loaded, PreviewFilter};

use super::super::App;
use super::aids;
use crate::app::print::DevelopPrint;

/// The edit a photo prints with: `edit`, with `style` applied to a copy when one is chosen.
pub(in crate::app) fn print_edit(edit: &Edit, style: Option<&Style>) -> Edit {
    match style {
        Some(s) => s.applied(edit, Registry::builtin(), ApplyMode::Append),
        None => edit.clone(),
    }
}

/// A photo's saved edit (its sidecar), else the edit Develop starts it with.
pub(in crate::app) fn saved_edit(path: &Path) -> Edit {
    match specular_develop::sidecar::read(path, Registry::builtin()) {
        Ok(Some(l)) => l.edit,
        _ => super::controls::new_edit(path),
    }
}

/// The developed size of a photo shown at `shown` (pixels, orientation applied) with
/// `edit`: its crop, straightening and turns (lens warps aside).
pub(in crate::app) fn developed_size(shown: (u32, u32), edit: &Edit) -> (u32, u32) {
    let plan = Plan::new(
        edit,
        &Context::image(shown.0.max(1), shown.1.max(1), Orient::default()),
        true,
    );
    plan.output_size(1.0)
}

/// Developed photos as a print source.
pub(in crate::app) struct DevelopSource {
    device: wgpu::Device,
    queue: wgpu::Queue,
    integrated: bool,
    registry: Arc<specular_decode::Registry>,
    /// The photos in print order.
    paths: Vec<PathBuf>,
    /// The photo open in Develop and its edit as it is now.
    open: (PathBuf, Edit),
    /// The style applied to each printed copy.
    style: Option<Style>,
}

impl DevelopSource {
    pub fn new(
        gpu: &Gpu,
        registry: Arc<specular_decode::Registry>,
        paths: Vec<PathBuf>,
        open: (PathBuf, Edit),
        style: Option<Style>,
    ) -> DevelopSource {
        DevelopSource {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            integrated: gpu.info.device_type == wgpu::DeviceType::IntegratedGpu
                || gpu.is_software(),
            registry,
            paths,
            open,
            style,
        }
    }

    /// The edit photo `index` prints with.
    fn edit(&self, index: usize) -> Option<(PathBuf, Edit)> {
        let path = self.paths.get(index)?;
        let edit = if *path == self.open.0 {
            self.open.1.clone()
        } else {
            saved_edit(path)
        };
        Some((path.clone(), print_edit(&edit, self.style.as_ref())))
    }
}

impl ImageSource for DevelopSource {
    fn key(&self, index: usize) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        let path = self.paths.get(index);
        std::hash::Hash::hash(&path, &mut h);
        if path == Some(&self.open.0) {
            let e = specular_develop::model::hash::edit_hash(&self.open.1);
            std::hash::Hash::hash(&e, &mut h);
        }
        if let Some(s) = &self.style {
            std::hash::Hash::hash(&s.name, &mut h);
            std::hash::Hash::hash(&format!("{:?}", s.items), &mut h);
        }
        std::hash::Hasher::finish(&h)
    }

    fn load(&self, index: usize, long_px: u32) -> Result<Loaded, String> {
        let (path, edit) = self.edit(index).ok_or("no such photo")?;
        let p = specular_develop::export::print::develop_rgb8(
            &self.device,
            &self.queue,
            Budget::for_device(self.integrated, None, None),
            &path,
            &edit,
            &self.registry,
            Some(long_px.max(16)),
        )?;
        let mut data = Vec::with_capacity(p.rgb.len() / 3 * 4);
        for c in p.rgb.chunks_exact(3) {
            data.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
        Ok(Loaded {
            raster: Raster {
                width: p.width,
                height: p.height,
                origin: (0, 0),
                scale: 1,
                layout: PixelLayout::Rgba,
                planes: vec![RasterPlane {
                    width: p.width,
                    height: p.height,
                    stride: p.width as usize * 4,
                    data,
                    h_div: 1,
                    v_div: 1,
                }],
                truncated: false,
            },
            stored: (p.width, p.height),
            orientation: Orientation::NORMAL,
            lut: None,
        })
    }

    /// One develop at a time: each holds a full-size render on the GPU.
    fn parallel(&self) -> usize {
        1
    }
}

/// The soft proof of the print preview: Develop's proof chain for the printer profile with
/// the print's intent and black point compensation, applied to the page's sRGB pixels.
pub(in crate::app) fn proof_filter(
    icc: &[u8],
    relative: bool,
    bpc: bool,
    view: ProofSettings,
) -> Result<PreviewFilter, String> {
    let settings = ProofSettings {
        intent: if relative {
            ProofIntent::Relative
        } else {
            ProofIntent::Perceptual
        },
        bpc,
        ..view
    };
    let lut = Proofer::new(icc, settings)
        .map_err(|e| e.to_string())?
        .bake();
    let decode: Vec<f32> = (0..256)
        .map(|v| specular_color::math::srgb_to_linear(v as f32 / 255.0))
        .collect();
    Ok(Arc::new(move |rgba: &mut [u8]| {
        for p in rgba.chunks_exact_mut(4) {
            let lin = [
                decode[usize::from(p[0])],
                decode[usize::from(p[1])],
                decode[usize::from(p[2])],
            ];
            let out = lut.eval(lin);
            for k in 0..3 {
                p[k] = (specular_develop::scopes::encode(out[k]) * 255.0 + 0.5) as u8;
            }
        }
    }))
}

impl App {
    /// What printing from Develop needs, while Develop is open.
    pub(in crate::app) fn develop_print(&mut self) -> Option<DevelopPrint> {
        self.develop.batch.shelf.load();
        let styles = self
            .develop
            .batch
            .shelf
            .listed()
            .into_iter()
            .map(|l| (l.name, l.style))
            .collect();
        let s = self.develop.session.as_ref()?;
        let pic = self.develop.pic;
        // The developed photo's aspect from the picture, its long side from the crop of
        // the straightened frame once known.
        let long = s
            .frame
            .map(|(w, h)| {
                let (w, h) = developed_size(
                    (w.round().max(1.0) as u32, h.round().max(1.0) as u32),
                    &s.edit,
                );
                w.max(h) as f32
            })
            .unwrap_or(6000.0);
        let (w, h) = if pic.w >= pic.h && pic.w > 0.0 {
            (long, long * pic.h / pic.w)
        } else if pic.h > 0.0 {
            (long * pic.w / pic.h, long)
        } else {
            (long, long * 2.0 / 3.0)
        };
        let proof = &self.develop.aids.proof;
        let s_ = proof.settings;
        Some(DevelopPrint {
            path: s.path.clone(),
            edit: s.edit.clone(),
            size: (w.round().max(1.0) as u32, h.round().max(1.0) as u32),
            profile: proof
                .chosen
                .as_ref()
                .map(|c: &aids::Chosen| (c.name.clone(), c.icc.clone())),
            through: proof.on && proof.chosen.is_some(),
            relative: s_.intent == ProofIntent::Relative,
            // On by default for relative colorimetric (switchable in the dialog).
            bpc: true,
            proof: proof.on,
            view: ProofSettings {
                paper: s_.paper,
                ink: s_.ink,
                ..ProofSettings::default()
            },
            styles,
            style: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::model::ModuleKind;
    use specular_develop::model::style::shipped_styles;

    /// A style chosen at print changes the printed copy only.
    #[test]
    fn the_style_applies_to_the_print_only() {
        let path = Path::new("/nowhere/IMG_0001.jpg");
        let edit = super::super::controls::new_edit(path);
        let before = edit.clone();
        let reg = Registry::builtin();
        let style = shipped_styles(reg)
            .into_iter()
            .find(|s| !s.items.is_empty())
            .expect("a shipped style");
        let printed = print_edit(&edit, Some(&style));
        assert_eq!(edit, before, "the edit is unchanged");
        assert_ne!(printed, edit, "the print has the style");
        let kinds: Vec<ModuleKind> = style.kinds();
        assert!(
            kinds
                .iter()
                .all(|k| printed.modules.iter().any(|m| m.kind == *k)),
            "{kinds:?}"
        );
        assert_eq!(print_edit(&edit, None), edit);
        // The source keys the style, so a new style renders again.
        let Some(h) = specular_shots::Harness::new() else {
            return;
        };
        let reg = Arc::new(specular_decode::Registry::default());
        let paths = vec![path.to_path_buf()];
        let open = (paths[0].clone(), edit.clone());
        let a = DevelopSource::new(&h.gpu, reg.clone(), paths.clone(), open.clone(), None);
        let b = DevelopSource::new(&h.gpu, reg, paths, open, Some(style));
        assert_ne!(a.key(0), b.key(0));
        assert_eq!(a.edit(0).unwrap().1, edit);
        assert_eq!(a.parallel(), 1);
    }
}
