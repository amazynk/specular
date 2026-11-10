//! Ophthalmic tomography in the viewer: an OCT volume is a stack of B-scans shown frame by
//! frame in medical mode. A slider under the image steps the stack, and the en-face localiser
//! (the Ophthalmic Photography image the B-scans are located on, found in the same folder by
//! its SOP Instance UID) is drawn in a corner of the image area with every B-scan's line on
//! it, the current one highlighted.

use std::path::PathBuf;
use std::sync::Arc;

use specular_dicom::oct::Oct;
use specular_dicom::sr::Jump;
use specular_gfx::{DrawList, Rect};
use specular_i18n::t;
use specular_log::{FOLDER, debug};
use specular_render::{PxRect, RenderOptions, View};
use specular_theme::Color;

use super::App;
use super::science::{Overlay, SliderRow, slider_value};
use crate::loader::{Event, ImageId};

/// The localiser's share of the image area's shorter side.
const INSET_SHARE: f32 = 0.34;
/// Margin of the inset from the image area's corner (logical px).
const INSET_MARGIN: f32 = 12.0;
/// Most B-scan lines drawn faintly besides the current one.
const MAX_LINES: usize = 1024;

/// The current OCT volume and its localiser.
#[derive(Default)]
pub(super) struct OctState {
    cur: Option<OctView>,
    /// The file last looked at (each file is read once while it is shown).
    seen: Option<PathBuf>,
    /// The slider's track as last drawn (logical px).
    track: Option<Rect>,
    drag: bool,
    /// The file's header is being read.
    reading: bool,
}

struct OctView {
    file: PathBuf,
    oct: Arc<Oct>,
    /// The localiser's image, when it is in the folder.
    localiser: Option<ImageId>,
}

/// The localiser as drawn: the image's view in its rectangle, and the B-scans' lines on it
/// (physical px).
pub(super) struct OctInset {
    pub id: ImageId,
    pub view: View,
    pub rect: PxRect,
    pub lines: Vec<Vec<(f32, f32)>>,
    /// Index into `lines` of the shown B-scan's line.
    pub current: Option<usize>,
}

impl OctState {
    /// The slider's track as drawn (logical px).
    pub(super) fn set_track(&mut self, track: Option<Rect>) {
        self.track = track;
    }
}

impl App {
    /// The current file is known: an OCT volume's frame locations are read from its header
    /// and its localiser looked for beside it.
    pub(super) fn oct_opened(&mut self) {
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        if self.report.oct.seen.as_ref() == Some(&path) || !super::series::is_dicom_name(&path) {
            return;
        }
        self.report.oct.seen = Some(path.clone());
        self.report.oct.reading = true;
        self.loader.background("oct-read", move |l| {
            let oct = match Oct::read_file(&path) {
                Ok(Some(o)) => o,
                Ok(None) => {
                    l.post(Event::Oct {
                        file: path,
                        oct: None,
                        localiser: None,
                    });
                    return;
                }
                Err(e) => {
                    debug!(target: FOLDER, "OCT frame locations not read: {e}");
                    l.post(Event::Oct {
                        file: path,
                        oct: None,
                        localiser: None,
                    });
                    return;
                }
            };
            let localiser = oct.localiser().and_then(|uid| {
                let jump = Jump::Image {
                    sop_instance: uid.to_owned(),
                    frame: None,
                    point: None,
                };
                super::report::find_jump(path.parent()?, &jump).map(|(p, _)| p)
            });
            l.post(Event::Oct {
                file: path,
                oct: Some(Arc::new(oct)),
                localiser,
            });
        });
    }

    /// The viewer moved to another file.
    pub(super) fn oct_left(&mut self) {
        let o = &mut self.report.oct;
        o.seen = None;
        o.cur = None;
        o.track = None;
        o.drag = false;
        o.reading = false;
    }

    /// An OCT volume's frame locations, read.
    pub(super) fn on_oct(
        &mut self,
        file: PathBuf,
        oct: Option<Arc<Oct>>,
        localiser: Option<PathBuf>,
    ) {
        if self.current.and_then(|i| self.entries.get(i)) != Some(&file) {
            return;
        }
        self.report.oct.reading = false;
        let Some(oct) = oct else {
            return;
        };
        // The localiser is usually another series (outside the listing while the volume's
        // series is shown): it is opened by its path.
        let localiser = localiser.map(|p| self.id_for_path(&p));
        self.report.oct.cur = Some(OctView {
            file,
            oct,
            localiser,
        });
        if localiser.is_some() {
            self.schedule();
        }
        self.redraw = true;
    }

    /// The localiser's image while an OCT volume is shown (kept open and on the GPU).
    pub(super) fn oct_localiser(&self) -> Option<ImageId> {
        let v = self.report.oct.cur.as_ref()?;
        (self.current.and_then(|i| self.entries.get(i)) == Some(&v.file))
            .then_some(v.localiser)
            .flatten()
    }

    /// Nothing of an OCT volume is still on its way: its header is read and its localiser,
    /// when it has one, is on the GPU (for screenshots).
    pub(super) fn oct_settled(&self) -> bool {
        if self.report.oct.reading {
            return false;
        }
        self.oct_localiser().is_none_or(|id| {
            (self.images.get(&id)).is_none_or(|i| i.error.is_some() || i.gpu_rank.is_some())
        })
    }

    /// Whether the OCT slider is shown (a stack in medical mode, outside MPR and the
    /// gallery).
    pub(super) fn oct_shown(&self) -> bool {
        self.oct_current()
            && !self.gallery_active()
            && !self.mpr_active()
            && self.medical_state().is_some_and(|n| n > 1)
    }

    fn oct_current(&self) -> bool {
        self.report
            .oct
            .cur
            .as_ref()
            .is_some_and(|v| self.current.and_then(|i| self.entries.get(i)) == Some(&v.file))
    }

    /// The B-scan slider, drawn like a science image's plane slider.
    pub(super) fn oct_overlay(&self) -> Option<Overlay> {
        if !self.oct_shown() {
            return None;
        }
        let frames = self.medical_state()?;
        let index = self.medical_index();
        let depth = self
            .report
            .oct
            .cur
            .as_ref()
            .and_then(|v| v.oct.location(index as usize))
            .and_then(|l| l.depth)
            .map(|d| t!("oct-depth", depth = specular_i18n::fixed(d, 0)));
        Some(Overlay {
            sliders: vec![SliderRow {
                axis: 0,
                name: t!("oct-bscan"),
                world: depth,
                index,
                len: frames,
                selected: false,
            }],
            ..Default::default()
        })
    }

    /// The frame under logical point `lp` on the track.
    fn oct_slider_at(&self, lp: (f32, f32)) -> Option<u32> {
        let r = self.report.oct.track?;
        let frames = self.medical_state()?;
        let hit = Rect::new(r.x - 8.0, r.y - 10.0, r.w + 16.0, r.h + 20.0);
        hit.contains(lp)
            .then(|| slider_value(r, lp.0, frames, self.rtl()))
    }

    /// A press: on the slider it shows that B-scan and starts a drag. True when taken.
    pub(super) fn oct_press(&mut self, lp: (f32, f32)) -> bool {
        if !self.oct_shown() {
            return false;
        }
        let Some(v) = self.oct_slider_at(lp) else {
            return false;
        };
        self.report.oct.drag = true;
        self.medical_jump_to(v);
        true
    }

    /// Ends a slider drag; true when there was one.
    pub(super) fn oct_release(&mut self) -> bool {
        std::mem::take(&mut self.report.oct.drag)
    }

    /// Pointer motion while dragging the slider. True while dragging.
    pub(super) fn oct_motion(&mut self, p: (f32, f32)) -> bool {
        if !self.report.oct.drag {
            return false;
        }
        let lp = self.logical(p);
        if let (Some(r), Some(frames)) = (self.report.oct.track, self.medical_state()) {
            let v = slider_value(r, lp.0, frames, self.rtl());
            if v != self.medical_index() && self.medical_jump_to(v) {
                self.redraw = true;
            }
        }
        true
    }

    /// The localiser in the image area's top corner (the leading one), with the B-scans'
    /// lines; `None` until its image has a size.
    pub(super) fn oct_inset(&self) -> Option<OctInset> {
        if !self.oct_shown() {
            return None;
        }
        let v = self.report.oct.cur.as_ref()?;
        let id = v.localiser?;
        let info = self.images.get(&id)?.info.as_ref()?;
        let stored = (info.width.max(1), info.height.max(1));
        let vp = self.view.viewport;
        let ls = self.dl.scale.max(0.1);
        let (rect, s) = inset_rect(vp, stored, ls, self.rtl())?;
        let center = (stored.0 as f32 / 2.0, stored.1 as f32 / 2.0);
        let view = View::fixed(stored, specular_core::Orientation::NORMAL, rect, s, center);
        let at = |p: &[f32; 2]| (rect.x + p[0] * s, rect.y + p[1] * s);
        let index = self.medical_index() as usize;
        let mut lines = Vec::new();
        let mut current = None;
        let uid = v.oct.localiser();
        for l in &v.oct.locations {
            if Some(l.image.as_str()) != uid || l.points.is_empty() {
                continue;
            }
            if l.frame == index {
                current = Some(lines.len());
            } else if lines.len() >= MAX_LINES {
                continue;
            }
            lines.push(l.points.iter().map(at).collect());
        }
        Some(OctInset {
            id,
            view,
            rect,
            lines,
            current,
        })
    }
}

/// How the localiser is drawn in its inset: smoothly at any scale (a 64-pixel localiser in a
/// 300-pixel corner is a picture to orient by, not pixels to inspect), in grey, without the
/// pixel grid or the clipping highlight.
pub(super) fn inset_options(opts: &RenderOptions) -> RenderOptions {
    RenderOptions {
        hq: true,
        grid: None,
        false_colour: None,
        hilo: None,
        nearest_from: f32::INFINITY,
        ..*opts
    }
}

/// The inset's rectangle in viewport `vp` (physical px) for a localiser of `stored` pixels,
/// and its scale; `None` when the area is too small to hold one.
fn inset_rect(vp: PxRect, stored: (u32, u32), ls: f32, rtl: bool) -> Option<(PxRect, f32)> {
    let side = vp.w.min(vp.h) * INSET_SHARE;
    if side < 48.0 * ls {
        return None;
    }
    let (w, h) = (stored.0 as f32, stored.1 as f32);
    let s = side / w.max(h);
    let (rw, rh) = ((w * s).round(), (h * s).round());
    let m = INSET_MARGIN * ls;
    let x = if rtl { vp.x + vp.w - m - rw } else { vp.x + m };
    Some((
        PxRect {
            x,
            y: vp.y + m,
            w: rw,
            h: rh,
        },
        s,
    ))
}

/// Draws the inset's frame and the B-scans' lines (the shown one bright) in logical px.
pub(super) fn draw_inset(dl: &mut DrawList, inset: &OctInset, scale: f32) {
    let l = |p: (f32, f32)| (p.0 / scale, p.1 / scale);
    let r = inset.rect;
    let frame = Rect::new(r.x / scale, r.y / scale, r.w / scale, r.h / scale);
    dl.push_clip(frame);
    let faint = Color::rgba(0.3, 0.85, 1.0, 0.35);
    for (i, line) in inset.lines.iter().enumerate() {
        if Some(i) == inset.current {
            continue;
        }
        for w in line.windows(2) {
            dl.line(l(w[0]), l(w[1]), 1.0, faint);
        }
    }
    if let Some(line) = inset.current.and_then(|i| inset.lines.get(i)) {
        let bright = Color::rgba(1.0, 0.85, 0.1, 1.0);
        for w in line.windows(2) {
            dl.line(l(w[0]), l(w[1]), 2.0, bright);
        }
        // A single point (a transverse scan's centre): a small cross.
        if let [p] = line.as_slice() {
            let (x, y) = l(*p);
            dl.line((x - 5.0, y), (x + 5.0, y), 2.0, bright);
            dl.line((x, y - 5.0), (x, y + 5.0), 2.0, bright);
        }
    }
    dl.pop_clip();
    let edge = Color::rgba(1.0, 1.0, 1.0, 0.7);
    let (x0, y0, x1, y1) = (frame.x, frame.y, frame.x + frame.w, frame.y + frame.h);
    dl.line((x0, y0), (x1, y0), 1.0, edge);
    dl.line((x1, y0), (x1, y1), 1.0, edge);
    dl.line((x1, y1), (x0, y1), 1.0, edge);
    dl.line((x0, y1), (x0, y0), 1.0, edge);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_sits_in_the_leading_corner_and_keeps_the_aspect() {
        let vp = PxRect {
            x: 0.0,
            y: 40.0,
            w: 1200.0,
            h: 800.0,
        };
        let (r, s) = inset_rect(vp, (400, 200), 1.0, false).unwrap();
        assert_eq!((r.x, r.y), (12.0, 52.0));
        assert!((s - 0.68).abs() < 1e-6);
        assert_eq!((r.w, r.h), (272.0, 136.0));
        let (r, _) = inset_rect(vp, (400, 200), 1.0, true).unwrap();
        assert_eq!(r.x, 1200.0 - 12.0 - 272.0);
        // Too small an area holds none.
        let tiny = PxRect {
            w: 100.0,
            h: 100.0,
            ..vp
        };
        assert!(inset_rect(tiny, (400, 200), 1.0, false).is_none());
    }

    /// The inset is filtered whatever the high-zoom smoothing setting: a small localiser
    /// enlarged into the corner is not drawn as blocks.
    #[test]
    fn inset_is_never_nearest_neighbour() {
        let opts = RenderOptions {
            nearest_from: 1.0001,
            grid: Some([1.0; 4]),
            ..Default::default()
        };
        let o = inset_options(&opts);
        assert!(o.nearest_from.is_infinite());
        assert!(o.hq && o.grid.is_none() && o.false_colour.is_none() && o.hilo.is_none());
    }

    /// The fixture volume's frame locations, read from its header, and its localiser found
    /// beside it as the jump search finds images.
    #[test]
    fn fixture_volume_finds_its_localiser() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/formats/specular-dicom/tests/data/dicomrt/oct");
        let oct = Oct::read_file(&dir.join("oct.dcm")).unwrap().unwrap();
        let jump = Jump::Image {
            sop_instance: oct.localiser().unwrap().to_owned(),
            frame: None,
            point: None,
        };
        let (p, f) = super::super::report::find_jump(&dir, &jump).unwrap();
        assert_eq!(p.file_name().unwrap(), "op.dcm");
        assert_eq!(f, 0);
        assert!(oct.location(7).is_some());
    }
}
