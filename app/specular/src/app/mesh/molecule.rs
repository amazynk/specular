//! Molecular structures (PDB, mmCIF, CIF, SDF, XYZ; [`specular_decode::Session::molecule`])
//! in the 3D view, under the same camera, keys and grid as models: atoms and bonds drawn as
//! impostors (balls and capped cylinders) and the cartoon as a mesh, in one depth buffer.
//! `m` steps the style (cartoon, ball-and-stick, space-filling; the cartoon only when the
//! structure has a polymer) and Shift+C the colouring (element, chain, secondary structure,
//! B-factor, rainbow). A click (a press and release without dragging) picks the atom under
//! it, a second the distance between the two, a click on nothing clears them; picked atoms
//! show as magenta balls joined by a magenta stick. The ground grid is off for molecules
//! until Alt+g. Style and colouring are per file and
//! start at the structure's own defaults.

use std::time::Instant;

use specular_decode::MoleculeModel;
use specular_decode::specular_mol::scene::{self, Ball, Scene, Stick};
use specular_decode::specular_mol::{Colouring, Molecule, Style};
use specular_gfx::{Gpu, wgpu};
use specular_i18n::{I18n, t};
use specular_render_3d::{MeshGpu, MeshRenderer, MoleculeGpu, View};

use super::{grid_line, projection_name, size_line};
use specular_decode::specular_mesh::Bounds;
use specular_render_3d::Projection;

/// Picked atoms and the line between them (sRGB): magenta, which no element has.
const PICK_COLOUR: [u8; 4] = [255, 40, 220, 255];

/// The distance line's radius (Å).
const DISTANCE_RADIUS: f32 = 0.09;

/// A pick ball is this much larger than the atom it marks (so it shows over it), and at
/// least [`PICK_MIN`] Å.
const PICK_GROW: f32 = 1.15;
const PICK_MIN: f32 = 0.55;

/// A molecule shown in the 3D view, how it is drawn and what is picked.
pub(in crate::app) struct MolView {
    pub(super) model: MoleculeModel,
    pub(super) style: Style,
    pub(super) colouring: Colouring,
    /// The structure has a polymer the cartoon can draw.
    cartoon_ok: bool,
    pub(super) scene: Scene,
    /// On the GPU (made on the first frame that draws it, again after a change of look).
    gpu: Option<MoleculeGpu>,
    cartoon: Option<MeshGpu>,
    /// Picked atoms, at most two (the second gives the distance).
    pub(super) picks: Vec<usize>,
    /// The picks changed since the overlay was put on the GPU.
    overlay_stale: bool,
    /// The ground grid (off at first: it would dwarf the molecule).
    pub(super) grid: bool,
    /// Nothing has been drawn yet (for the first-frame log line).
    first_frame: bool,
}

impl MolView {
    /// The GPU device was replaced: uploaded again when next drawn.
    pub(super) fn gpu_reset(&mut self) {
        self.gpu = None;
        self.cartoon = None;
    }

    pub(super) fn new(model: MoleculeModel) -> MolView {
        let m = &*model.molecule;
        let style = m.default_style();
        let colouring = scene::default_colouring(m, style);
        let cartoon_ok = m.has_cartoon();
        let scene = build(m, style, colouring);
        MolView {
            model,
            style,
            colouring,
            cartoon_ok,
            scene,
            gpu: None,
            cartoon: None,
            picks: Vec::new(),
            overlay_stale: true,
            grid: false,
            first_frame: true,
        }
    }

    pub(super) fn molecule(&self) -> &Molecule {
        &self.model.molecule
    }

    /// Draws in `style` and `colouring` from the next frame on.
    pub(super) fn set_look(&mut self, style: Style, colouring: Colouring) {
        if (style, colouring) == (self.style, self.colouring) {
            return;
        }
        self.style = style;
        self.colouring = colouring;
        self.scene = build(&self.model.molecule, style, colouring);
        self.gpu = None;
        self.cartoon = None;
        self.overlay_stale = true;
        // Atoms the cartoon hides cannot stay picked.
        let targets = &self.scene.targets;
        self.picks
            .retain(|&i| targets.iter().any(|&(t, _)| t as usize == i));
    }

    pub(super) fn next_style(&mut self) {
        let s = self.style.next(self.cartoon_ok);
        self.set_look(s, self.colouring);
    }

    pub(super) fn next_colouring(&mut self) {
        let c = self.colouring.next();
        self.set_look(self.style, c);
    }

    /// A click hit atom `hit` (or nothing): picks it, unpicks it when it is the last
    /// picked, starts over after two; nothing clears the picks.
    pub(super) fn click(&mut self, hit: Option<usize>) {
        match hit {
            None => self.picks.clear(),
            Some(i) if self.picks.last() == Some(&i) => {
                self.picks.pop();
            }
            Some(i) => {
                if self.picks.len() >= 2 {
                    self.picks.clear();
                }
                self.picks.push(i);
            }
        }
        self.overlay_stale = true;
    }

    /// The readout: size, atoms, the picks and the distance, the grid and the look.
    pub(super) fn readout(&self, bounds: &Bounds, p: Projection) -> Vec<String> {
        let m = self.molecule();
        let loc = I18n::global().localizer();
        let mut lines = Vec::new();
        if !bounds.is_empty() {
            lines.push(size_line(m.bounds().size().map(f64::from), "Å"));
        }
        lines.push(t!("molecule-counts", atoms = m.atoms.len() as i64));
        for &i in &self.picks {
            lines.push(t!(
                "molecule-picked",
                atom = specular_i18n::isolate_ltr(&m.atom_label(i))
            ));
        }
        if let [i, j] = self.picks[..] {
            lines.push(t!(
                "molecule-distance",
                distance = loc.number(m.distance(i, j), 2)
            ));
        }
        lines.extend(grid_line(bounds, self.grid, "Å"));
        lines.push(specular_i18n::dot_join(&[
            t!(
                "molecule-style-line",
                style = style_name(self.style),
                colour = colouring_name(self.colouring)
            ),
            projection_name(p),
        ]));
        lines
    }

    /// Puts the scene on the GPU when it is not there and draws it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render(
        &mut self,
        renderer: &mut MeshRenderer,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view: &View,
        bounds: Bounds,
    ) {
        let t0 = Instant::now();
        if self.gpu.is_none() {
            let s = &self.scene;
            let g = renderer.upload_molecule(gpu, &s.balls, &s.sticks, bounds);
            self.cartoon = s.cartoon.as_ref().map(|c| renderer.upload(gpu, c, &[]));
            let bytes = g.bytes + self.cartoon.as_ref().map_or(0, |c| c.bytes);
            specular_log::info!(
                target: specular_log::DECODE,
                "molecule: {} balls, {} sticks, {} cartoon triangles on the GPU in {:.0} ms ({:.1} MiB)",
                g.balls,
                g.sticks,
                self.cartoon.as_ref().map_or(0, |c| c.triangles),
                t0.elapsed().as_secs_f64() * 1e3,
                bytes as f64 / f64::from(1u32 << 20)
            );
            self.gpu = Some(g);
            self.overlay_stale = true;
        }
        let Some(g) = self.gpu.as_mut() else {
            return;
        };
        if self.overlay_stale {
            let (balls, sticks) = overlay_of(&self.model.molecule, &self.scene, &self.picks);
            renderer.set_overlay(gpu, g, &balls, &sticks);
            self.overlay_stale = false;
        }
        renderer.draw_scene(gpu, enc, target, size, self.cartoon.as_ref(), Some(g), view);
        if self.first_frame {
            self.first_frame = false;
            specular_log::info!(
                target: specular_log::DECODE,
                "molecule: first frame recorded {:.0} ms after the upload began",
                t0.elapsed().as_secs_f64() * 1e3
            );
        }
    }
}

/// The picked atoms as balls a little larger than the atoms and, for two, the line between
/// them.
fn overlay_of(m: &Molecule, scene: &Scene, picks: &[usize]) -> (Vec<Ball>, Vec<Stick>) {
    let balls = picks
        .iter()
        .filter_map(|&i| {
            let a = m.atoms.get(i)?;
            let r = scene
                .targets
                .iter()
                .find(|&&(t, _)| t as usize == i)
                .map_or(0.4, |&(_, r)| r);
            Some(Ball {
                centre: a.pos,
                radius: (r * PICK_GROW).max(PICK_MIN),
                colour: PICK_COLOUR,
            })
        })
        .collect();
    let sticks = match picks {
        &[i, j] => vec![Stick {
            a: m.atoms[i].pos,
            b: m.atoms[j].pos,
            radius: DISTANCE_RADIUS,
            colours: [PICK_COLOUR; 2],
        }],
        _ => Vec::new(),
    };
    (balls, sticks)
}
/// Builds the scene, logging how long it took.
fn build(m: &Molecule, style: Style, colouring: Colouring) -> Scene {
    let t0 = Instant::now();
    let s = scene::build(m, style, colouring);
    specular_log::info!(
        target: specular_log::DECODE,
        "molecule: {} scene of {} atoms built in {:.0} ms",
        style.id(),
        m.atoms.len(),
        t0.elapsed().as_secs_f64() * 1e3
    );
    s
}

pub(in crate::app) fn style_name(s: Style) -> String {
    match s {
        Style::Cartoon => t!("molecule-style-cartoon"),
        Style::BallStick => t!("molecule-style-ball-stick"),
        Style::Spacefill => t!("molecule-style-spacefill"),
    }
}

pub(in crate::app) fn colouring_name(c: Colouring) -> String {
    match c {
        Colouring::Element => t!("molecule-colour-element"),
        Colouring::Chain => t!("molecule-colour-chain"),
        Colouring::Ss => t!("molecule-colour-ss"),
        Colouring::BFactor => t!("molecule-colour-bfactor"),
        Colouring::Rainbow => t!("molecule-colour-rainbow"),
    }
}
