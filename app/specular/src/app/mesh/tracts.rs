//! Tractograms (TrackVis .trk, MRtrix .tck; [`specular_decode::Session::tracts`]) in the
//! 3D view, under the same camera, keys and grid as models: each streamline is drawn as a
//! chain of capped cylinders with a ball at every point (so bends close), in RAS millimetres
//! (z up). By default each segment takes the DTI direction colour (|x| red, |y| green, |z|
//! blue); `c` steps through the file's per-point values, coloured over their robust range,
//! and back. `]` and `[` make the streamlines thicker and thinner; Alt+O steps their
//! opacity down from opaque (see [`OPACITIES`]): below it the renderer blends the sticks
//! with weighted, order-independent transparency and leaves out the joints, so a bundle's
//! inside shows through without sorting millions of segments per frame. Files above
//! [`specular_decode::neuro_dec::MAX_SHOWN_POINTS`] points arrive subsampled.

use std::time::Instant;

use specular_decode::TractsModel;
use specular_decode::neuro_dec::{auto_colours, direction_colour};
use specular_decode::specular_mesh::Bounds;
use specular_decode::specular_mol::scene::{Ball, Stick};
use specular_gfx::{Gpu, wgpu};
use specular_i18n::{I18n, t};
use specular_render_3d::{MeshRenderer, MoleculeGpu, Projection, View};

use super::{grid_line, projection_name, size_line};

/// The thickness steps `]` and `[` take, as factors of the default.
const WIDTHS: [f32; 9] = [0.25, 0.35, 0.5, 0.7, 1.0, 1.5, 2.0, 3.0, 4.0];

/// The opacity steps Alt+O takes, from opaque, round again.
pub(in crate::app) const OPACITIES: [f32; 6] = [1.0, 0.7, 0.5, 0.35, 0.2, 0.1];

/// The default thickness at [`WIDTHS`]`[4]`: this share of the box's diagonal.
const WIDTH_SHARE: f32 = 1.0 / 200.0;

/// How the streamlines are coloured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum TractColour {
    Direction,
    /// The file's per-point value at this index.
    Scalar(usize),
}

/// A tractogram shown in the 3D view and how it is drawn.
pub(in crate::app) struct TractView {
    model: TractsModel,
    pub(super) colour: TractColour,
    /// Index into [`WIDTHS`].
    width: usize,
    /// The default radius (mm).
    base: f32,
    pub(super) bounds: Bounds,
    /// On the GPU (made on the first frame that draws it, again after a change of look).
    gpu: Option<MoleculeGpu>,
    pub(super) grid: bool,
    /// Index into [`OPACITIES`].
    opacity: usize,
}

impl TractView {
    /// The GPU device was replaced: uploaded again when next drawn.
    pub(super) fn gpu_reset(&mut self) {
        self.gpu = None;
    }

    pub(super) fn new(model: TractsModel) -> TractView {
        let mut bounds = Bounds::EMPTY;
        for &p in &model.tracts.points {
            if p.iter().all(|v| v.is_finite()) {
                bounds.add(p);
            }
        }
        let s = bounds.size();
        let diag = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        let base = if diag > 0.0 && diag.is_finite() {
            diag * WIDTH_SHARE / 2.0
        } else {
            0.1
        };
        TractView {
            model,
            colour: TractColour::Direction,
            width: 4,
            base,
            bounds,
            gpu: None,
            grid: false,
            opacity: 0,
        }
    }

    fn radius(&self) -> f32 {
        self.base * WIDTHS[self.width]
    }

    /// The next colouring: direction, then each per-point value, then direction again.
    pub(super) fn next_colour(&mut self) {
        let n = self.model.tracts.scalars.len();
        self.colour = match self.colour {
            TractColour::Direction if n > 0 => TractColour::Scalar(0),
            TractColour::Scalar(i) if i + 1 < n => TractColour::Scalar(i + 1),
            _ => TractColour::Direction,
        };
        self.gpu = None;
    }

    /// The next opacity (round from the faintest to opaque again); returns it.
    pub(super) fn step_opacity(&mut self) -> f32 {
        self.opacity = (self.opacity + 1) % OPACITIES.len();
        self.opacity()
    }

    pub(super) fn opacity(&self) -> f32 {
        OPACITIES[self.opacity]
    }

    /// The opacity as a readout or toast line.
    pub(super) fn opacity_name(&self) -> String {
        t!(
            "tracts-opacity",
            percent = (self.opacity() * 100.0).round() as i64
        )
    }

    /// Thicker (or thinner); false at the end of the steps.
    pub(super) fn step_width(&mut self, up: bool) -> bool {
        let w = if up {
            (self.width + 1).min(WIDTHS.len() - 1)
        } else {
            self.width.saturating_sub(1)
        };
        let changed = w != self.width;
        self.width = w;
        if changed {
            self.gpu = None;
        }
        changed
    }

    pub(super) fn colour_name(&self) -> String {
        match self.colour {
            TractColour::Direction => t!("tracts-colour-direction"),
            TractColour::Scalar(i) => {
                let name = self
                    .model
                    .tracts
                    .scalars
                    .get(i)
                    .map_or("", |s| s.0.as_str());
                t!(
                    "tracts-colour-scalar",
                    name = specular_i18n::isolate_ltr(name)
                )
            }
        }
    }

    /// The readout: size, streamlines and points, the grid and the look.
    pub(super) fn readout(&self, p: Projection) -> Vec<String> {
        let t = &self.model.tracts;
        let loc = I18n::global().localizer();
        let mut lines = Vec::new();
        if !self.bounds.is_empty() {
            lines.push(size_line(self.bounds.size().map(f64::from), "mm"));
        }
        lines.push(t!(
            "tracts-counts",
            streamlines = t.streamlines() as i64,
            points = t.points.len() as i64
        ));
        lines.extend(grid_line(&self.bounds, self.grid, "mm"));
        let width = f64::from(self.radius() * 2.0);
        let digits = if width >= 1.0 { 1 } else { 2 };
        let look = t!(
            "tracts-look-line",
            colour = self.colour_name(),
            width = loc.number(width, digits)
        );
        let mut parts = vec![look];
        if self.opacity() < 1.0 {
            parts.push(self.opacity_name());
        }
        parts.push(projection_name(p));
        lines.push(specular_i18n::dot_join(&parts));
        lines
    }

    /// The balls and sticks of the current look.
    pub(super) fn build(&self) -> (Vec<Ball>, Vec<Stick>) {
        build(&self.model, self.colour, self.radius())
    }

    /// Puts the streamlines on the GPU when they are not there and draws them.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render(
        &mut self,
        renderer: &mut MeshRenderer,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view: &View,
    ) {
        if self.gpu.is_none() {
            let t0 = Instant::now();
            let (balls, sticks) = self.build();
            let g = renderer.upload_molecule(gpu, &balls, &sticks, self.bounds);
            specular_log::info!(
                target: specular_log::DECODE,
                "tracts: {} segments, {} joints on the GPU in {:.0} ms ({:.1} MiB)",
                g.sticks,
                g.balls,
                t0.elapsed().as_secs_f64() * 1e3,
                g.bytes as f64 / f64::from(1u32 << 20)
            );
            self.gpu = Some(g);
        }
        let opacity = self.opacity();
        if let Some(g) = self.gpu.as_mut() {
            g.opacity = opacity;
            renderer.draw_scene(gpu, enc, target, size, None, Some(g), view);
        }
    }
}

/// Each streamline as sticks between its points and balls at them, coloured by `colour`.
fn build(model: &TractsModel, colour: TractColour, radius: f32) -> (Vec<Ball>, Vec<Stick>) {
    let t = &model.tracts;
    let scalar = match colour {
        TractColour::Scalar(i) => t
            .scalars
            .get(i)
            .filter(|s| s.1.len() == t.points.len())
            .map(|s| auto_colours(&s.1)),
        TractColour::Direction => None,
    };
    let finite = |p: &[f32; 3]| p.iter().all(|v| v.is_finite());
    let mut balls = Vec::with_capacity(t.points.len());
    let mut sticks = Vec::with_capacity(t.points.len());
    for s in 0..t.streamlines() {
        let start = t.offsets[s] as usize;
        let pts = t.streamline(s);
        for (k, &p) in pts.iter().enumerate() {
            if !finite(&p) {
                continue;
            }
            let at = start + k;
            // A point's direction is along its neighbours (the chord through it).
            let prev = pts[k.saturating_sub(1)];
            let next = pts[(k + 1).min(pts.len() - 1)];
            let c = match &scalar {
                Some(c) => c[at],
                None => direction_colour(prev, next),
            };
            balls.push(Ball {
                centre: p,
                radius,
                colour: c,
            });
            if let Some(&q) = pts.get(k + 1)
                && finite(&q)
            {
                let colours = match &scalar {
                    Some(c) => [c[at], c[at + 1]],
                    None => [direction_colour(p, q); 2],
                };
                sticks.push(Stick {
                    a: p,
                    b: q,
                    radius,
                    colours,
                });
            }
        }
    }
    (balls, sticks)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn model() -> TractsModel {
        let t = specular_decode::neuro_dec::Tractogram {
            offsets: vec![0, 3, 5],
            points: vec![
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 20.0],
            ],
            scalars: vec![("fa".into(), vec![0.1, 0.2, 0.3, 0.4, 0.5])],
            ..Default::default()
        };
        TractsModel {
            tracts: Arc::new(t),
        }
    }

    #[test]
    fn streamlines_become_sticks_and_joints() {
        let mut v = TractView::new(model());
        let (balls, sticks) = v.build();
        assert_eq!(balls.len(), 5);
        assert_eq!(sticks.len(), 3);
        assert_eq!(sticks[0].colours[0], [255, 0, 0, 255]);
        assert_eq!(sticks[1].colours[0], [0, 255, 0, 255]);
        assert_eq!(sticks[2].colours[0], [0, 0, 255, 255]);
        v.next_colour();
        assert_eq!(v.colour, TractColour::Scalar(0));
        let (_, sticks) = v.build();
        assert_ne!(sticks[0].colours[0], sticks[2].colours[1]);
        v.next_colour();
        assert_eq!(v.colour, TractColour::Direction);
        let r = v.radius();
        assert!(v.step_width(true));
        assert!(v.radius() > r);
        for _ in 0..20 {
            v.step_width(false);
        }
        assert!(!v.step_width(false));
    }

    #[test]
    fn opacity_steps_down_and_round_to_opaque() {
        let mut v = TractView::new(model());
        assert_eq!(v.opacity(), 1.0);
        let mut seen = vec![v.opacity()];
        for _ in 1..OPACITIES.len() {
            seen.push(v.step_opacity());
        }
        assert!(seen.windows(2).all(|w| w[1] < w[0]), "{seen:?}");
        assert_eq!(v.step_opacity(), 1.0);
        // The look does not change on the GPU: no rebuild.
        let (balls, _) = v.build();
        assert_eq!(balls.len(), 5);
    }
}
