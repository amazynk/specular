//! The crop and straighten tool: [`Action::DevelopCrop`]
//! (R) shows the whole straightened photo with the crop over it.
//!
//! - **Pointer:** drag a corner or an edge to resize, inside to move; Ctrl+drag a line
//!   along something that should be level (or plumb) to straighten.
//! - **Keys:** arrows move (Shift more), Ctrl+arrows resize, `,` and `.` straighten (Shift
//!   more), A next aspect, X swaps it, O next guide, `[` and `]` turn, H and V flip,
//!   Delete resets, Enter or R keeps, Esc goes back to where the tool started.
//! - The crop never shows past the photo: it shrinks to fit when the photo turns or its
//!   perspective changes (the region that shows the image: [`crop::Frame`]).
//! - **Perspective:** Alt+arrows change the vertical and horizontal keystone. Shift+R
//!   switches to **guides**: drag along something that should be plumb or level (up to
//!   four lines; a loupe shows the end), drag an end to move it, Delete removes the last
//!   one touched (asked first). The outline of the warped photo shows while the tool is
//!   open.
//!
//! Crop values are fractions of the straightened frame
//! ([`specular_develop::ops::geometry`]); the screen only maps them.
//!
//! [`Action::DevelopCrop`]: specular_input::Action::DevelopCrop

use specular_develop::model::params::CropOrientation;
use specular_develop::ops::geometry::crop::{self, ASPECTS, Frame, Grab, Guide};
use specular_develop::ops::geometry::persp::{self, M3, MAX_GUIDES};
use specular_develop::ops::geometry::{Plan, aspect_ratio, total_orient};
use specular_develop::preview::Orient;
use specular_gfx::{Align, DrawList, Rect, TexId, Text, TextStyle};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::{Color, Palette, metrics};

/// Where the straightened frame sits on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Layout {
    /// The whole straightened frame (logical px).
    pub frame: Rect,
    /// Its size in sensor pixels.
    pub w: f64,
    pub h: f64,
    /// Perspective: the map from the frame before the straighten turn to the oriented
    /// frame, in the plan's square pixels, with the plan's frame size
    /// ([`Layout::with_plan`]).
    pub keystone: Option<(M3, (f64, f64))>,
}

impl Layout {
    /// The frame of `p` for an image the viewer shows in `pic` (logical px), `stored`
    /// pixels as stored, turned by `base`.
    pub fn new(pic: Rect, stored: (u32, u32), base: Orient, p: &CropOrientation) -> Layout {
        let (bw, _) = base.size(stored.0, stored.1);
        let o = total_orient(base, p);
        let (w, h) = o.size(stored.0, stored.1);
        let k = pic.w / bw.max(1) as f32;
        let (cx, cy) = pic.center();
        let (fw, fh) = (w as f32 * k, h as f32 * k);
        Layout {
            frame: Rect::new(cx - fw / 2.0, cy - fh / 2.0, fw, fh),
            w: f64::from(w),
            h: f64::from(h),
            keystone: None,
        }
    }

    /// With the perspective of `plan` (the picture's warp), when it has one.
    pub fn with_plan(mut self, plan: Option<&Plan>) -> Layout {
        self.keystone = plan.and_then(|pl| {
            let g = pl.persp?;
            Some((persp::mul(&g, &persp::turn(pl.angle)), pl.oriented))
        });
        self
    }

    /// The region of the frame that shows the photo with `p`'s straighten angle.
    pub fn frame_of(&self, p: &CropOrientation) -> Frame {
        frame_for(self.w, self.h, self.keystone, Self::angle(p))
    }

    /// The screen rectangle of crop fractions `c`.
    pub fn rect(&self, c: [f32; 4]) -> Rect {
        let f = self.frame;
        Rect::from_min_max(
            f.x + c[0] * f.w,
            f.y + c[1] * f.h,
            f.x + c[2] * f.w,
            f.y + c[3] * f.h,
        )
    }

    fn angle(p: &CropOrientation) -> f64 {
        f64::from(p.angle_deg).to_radians()
    }
}

/// The region of a `w × h` straightened frame that shows the photo, turned by `angle`
/// (radians) after the keystone of [`Layout::keystone`].
pub(super) fn frame_for(w: f64, h: f64, keystone: Option<(M3, (f64, f64))>, angle: f64) -> Frame {
    let Some((k, (wo, ho))) = keystone else {
        return Frame::turned(w, h, angle);
    };
    // The plan's pixels (square) and these may differ in scale and stretch.
    let (sx, sy) = (wo / w.max(1e-9), ho / h.max(1e-9));
    let g = persp::mul(&k, &persp::turn(-angle));
    let d = [[sx, 0.0, 0.0], [0.0, sy, 0.0], [0.0, 0.0, 1.0]];
    let di = [[1.0 / sx, 0.0, 0.0], [0.0, 1.0 / sy, 0.0], [0.0, 0.0, 1.0]];
    Frame::warped(w, h, persp::mul(&di, &persp::mul(&g, &d)))
}

/// Guides on screen: the picture's warp (whole frame) and where the frame is drawn.
#[derive(Clone, Debug)]
pub(super) struct GuideMap {
    plan: Plan,
    frame: Rect,
    /// The edit's guides (source fractions).
    pub guides: Vec<[f32; 4]>,
}

impl GuideMap {
    pub fn new(plan: Plan, frame: Rect, guides: Vec<[f32; 4]>) -> GuideMap {
        GuideMap {
            plan: Plan {
                crop: [0.0, 0.0, 1.0, 1.0],
                ..plan
            },
            frame,
            guides,
        }
    }

    /// Where source fraction `g` shows on screen.
    pub fn to_screen(&self, g: [f32; 2]) -> Option<(f32, f32)> {
        let [fx, fy, fw, fh] = self.plan.frame;
        let s = (fx + f64::from(g[0]) * fw, fy + f64::from(g[1]) * fh);
        let o = self.plan.to_output(s, 1.0)?;
        let (wo, ho) = self.plan.oriented;
        let f = self.frame;
        Some((f.x + (o.0 / wo) as f32 * f.w, f.y + (o.1 / ho) as f32 * f.h))
    }

    /// The source fraction shown at screen point `p`.
    pub fn to_source(&self, p: (f32, f32)) -> Option<[f32; 2]> {
        let f = self.frame;
        let (u, v) = (
            f64::from((p.0 - f.x) / f.w.max(1.0)),
            f64::from((p.1 - f.y) / f.h.max(1.0)),
        );
        let (wo, ho) = self.plan.oriented;
        let (s, _) = self.plan.frame_to_source(1, (u * wo, v * ho));
        let [fx, fy, fw, fh] = self.plan.frame;
        let g = [((s.0 - fx) / fw) as f32, ((s.1 - fy) / fh) as f32];
        g.iter()
            .all(|v| v.is_finite() && v.abs() < 4.0)
            .then_some(g)
    }

    /// The guides' ends on screen.
    fn ends(&self) -> Vec<Option<[(f32, f32); 2]>> {
        self.guides
            .iter()
            .map(|g| Some([self.to_screen([g[0], g[1]])?, self.to_screen([g[2], g[3]])?]))
            .collect()
    }
}

/// What the tool asks of the session.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outcome {
    /// New params (a history step); announce the text when there is one.
    Set(CropOrientation, Option<String>),
    /// Keep the crop and close the tool.
    Done,
    /// Go back to where the tool started and close it.
    Cancel,
    /// Only redraw (the guide changed).
    Redraw(Option<String>),
    /// New perspective guides (source fractions): a history step, mode Guided.
    Guides(Vec<[f32; 4]>, Option<String>),
    /// Ask before removing guide `i`.
    RemoveGuide(usize),
    /// Change the vertical and horizontal keystone by these slider steps.
    Keystone(f32, f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DragKind {
    Crop(Grab),
    Line,
    /// A perspective guide: a new one, or guide `i`'s end `end` moved.
    Guide(Option<(usize, usize)>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Drag {
    kind: DragKind,
    start: (f32, f32),
    now: (f32, f32),
    crop0: [f64; 4],
}

/// The tool's state while it is open.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CropTool {
    pub guide: Guide,
    /// The history position when the tool opened (Esc goes back to it).
    pub start: usize,
    drag: Option<Drag>,
    /// Drawing perspective guides instead of cropping.
    pub guides_on: bool,
    /// The guide last touched (Delete removes it).
    selected: Option<usize>,
}

/// Handle size and grab distance (logical px at text scale 1).
const HANDLE: f32 = 10.0;
const GRAB: f32 = 14.0;

impl CropTool {
    pub fn new(start: usize) -> CropTool {
        CropTool {
            guide: Guide::Thirds,
            start,
            drag: None,
            guides_on: false,
            selected: None,
        }
    }

    /// Opens on the guides.
    pub fn with_guides(mut self) -> CropTool {
        self.guides_on = true;
        self
    }

    /// A press while drawing guides: whether the tool took it, and what to say.
    pub fn guide_press(&mut self, lp: (f32, f32), map: &GuideMap) -> (bool, Option<Outcome>) {
        if !self.guides_on || !map.frame.inset(-GRAB).contains(lp) {
            return (false, None);
        }
        let near = |a: (f32, f32)| (a.0 - lp.0).hypot(a.1 - lp.1) <= GRAB;
        for (i, e) in map.ends().iter().enumerate() {
            let Some(e) = e else { continue };
            for end in 0..2 {
                if near(e[end]) {
                    self.selected = Some(i);
                    self.drag = Some(Drag {
                        kind: DragKind::Guide(Some((i, end))),
                        start: e[1 - end],
                        now: lp,
                        crop0: [0.0; 4],
                    });
                    return (true, None);
                }
            }
        }
        if map.guides.len() >= MAX_GUIDES {
            let say = t!("develop-persp-guides-full", n = MAX_GUIDES);
            return (true, Some(Outcome::Redraw(Some(say))));
        }
        self.drag = Some(Drag {
            kind: DragKind::Guide(None),
            start: lp,
            now: lp,
            crop0: [0.0; 4],
        });
        (true, None)
    }

    /// The guide being drawn or moved (screen ends), if any.
    fn guide_drag(&self) -> Option<((f32, f32), (f32, f32))> {
        self.drag
            .filter(|d| matches!(d.kind, DragKind::Guide(_)))
            .map(|d| (d.start, d.now))
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// A press at `lp`; `ctrl` draws a straighten line. Whether the tool took it.
    pub fn press(&mut self, lp: (f32, f32), ctrl: bool, lay: &Layout, p: &CropOrientation) -> bool {
        if self.guides_on && !ctrl {
            return false;
        }
        if !lay.frame.contains(lp) && !lay.frame.inset(-GRAB).contains(lp) {
            return false;
        }
        let kind = if ctrl {
            DragKind::Line
        } else {
            let r = lay.rect(p.crop);
            let u = f64::from((lp.0 - r.x) / r.w.max(1.0));
            let v = f64::from((lp.1 - r.y) / r.h.max(1.0));
            let e = (
                f64::from(GRAB / r.w.max(1.0)),
                f64::from(GRAB / r.h.max(1.0)),
            );
            match crop::grab(u, v, e) {
                Some(g) => DragKind::Crop(g),
                None => return true,
            }
        };
        self.drag = Some(Drag {
            kind,
            start: lp,
            now: lp,
            crop0: p.crop.map(f64::from),
        });
        true
    }

    /// The pointer moved during a drag: the crop's new params (a line only redraws).
    pub fn moved(
        &mut self,
        lp: (f32, f32),
        lay: &Layout,
        p: &CropOrientation,
    ) -> Option<CropOrientation> {
        let d = self.drag.as_mut()?;
        d.now = lp;
        let DragKind::Crop(g) = d.kind else {
            return None;
        };
        let dx = f64::from((lp.0 - d.start.0) / lay.frame.w.max(1.0));
        let dy = f64::from((lp.1 - d.start.1) / lay.frame.h.max(1.0));
        let ratio = aspect_ratio(&p.aspect, lay.w, lay.h);
        let c = crop::drag_in(d.crop0, g, dx, dy, ratio, &lay.frame_of(p));
        let mut n = p.clone();
        n.crop = c.map(|v| v as f32);
        (n != *p).then_some(n)
    }

    /// The press ended: a drawn line straightens; a guide is added or moved.
    pub fn release(
        &mut self,
        lay: &Layout,
        p: &CropOrientation,
        map: Option<&GuideMap>,
    ) -> Option<Outcome> {
        let d = self.drag.take()?;
        if let DragKind::Guide(which) = d.kind {
            let map = map?;
            let (a, b) = (d.start, d.now);
            if (a.0 - b.0).hypot(a.1 - b.1) < 12.0 {
                return None;
            }
            let (sa, sb) = (map.to_source(a)?, map.to_source(b)?);
            let mut gs = map.guides.clone();
            // The fixed end first, then the one dragged.
            let g = match which {
                Some((_, 0)) => [sb[0], sb[1], sa[0], sa[1]],
                _ => [sa[0], sa[1], sb[0], sb[1]],
            };
            let say = match which {
                Some((i, _)) if i < gs.len() => {
                    gs[i] = g;
                    t!("develop-persp-guide-moved")
                }
                _ => {
                    gs.push(g);
                    self.selected = Some(gs.len() - 1);
                    t!("develop-persp-guide-added", n = gs.len())
                }
            };
            return Some(Outcome::Guides(gs, Some(say)));
        }
        if d.kind != DragKind::Line {
            return None;
        }
        let a = (f64::from(d.start.0), f64::from(d.start.1));
        let b = (f64::from(d.now.0), f64::from(d.now.1));
        let delta = crop::level_angle(a, b)?;
        let mut n = p.clone();
        n.angle_deg = (f64::from(p.angle_deg) + delta).clamp(-45.0, 45.0) as f32;
        refit(&mut n, lay);
        let say = t!(
            "develop-crop-straightened",
            angle = specular_i18n::fixed(n.angle_deg, 1)
        );
        Some(Outcome::Set(n, Some(say)))
    }

    /// A key while the tool is open.
    pub fn key(&mut self, combo: KeyCombo, lay: &Layout, p: &CropOrientation) -> Option<Outcome> {
        let m = combo.mods;
        let mut n = p.clone();
        let ratio = aspect_ratio(&p.aspect, lay.w, lay.h);
        let fr = lay.frame_of(p);
        let c = p.crop.map(f64::from);
        let step = if m.shift { 0.05 } else { 0.005 };
        let arrow = |k: &NamedKey| -> Option<(f64, f64)> {
            Some(match k {
                NamedKey::ArrowLeft => (-1.0, 0.0),
                NamedKey::ArrowRight => (1.0, 0.0),
                NamedKey::ArrowUp => (0.0, -1.0),
                NamedKey::ArrowDown => (0.0, 1.0),
                _ => return None,
            })
        };
        match &combo.key {
            Key::Named(NamedKey::Enter) => return Some(Outcome::Done),
            Key::Named(NamedKey::Escape) => return Some(Outcome::Cancel),
            Key::Named(NamedKey::Delete | NamedKey::Backspace) if self.guides_on => {
                return self.selected.map(Outcome::RemoveGuide);
            }
            Key::Named(k) if m.alt && arrow(k).is_some() => {
                let (x, y) = arrow(k)?;
                let d = if m.shift { 5.0 } else { 1.0 };
                // Up widens the top (vertical), right the right side (horizontal).
                return Some(Outcome::Keystone(-y as f32 * d, x as f32 * d));
            }
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => {
                n.crop = [0.0, 0.0, 1.0, 1.0];
                n.angle_deg = 0.0;
                n.aspect = "original".into();
                return Some(Outcome::Set(n, Some(t!("develop-crop-reset"))));
            }
            Key::Named(k) => {
                let (x, y) = arrow(k)?;
                let moved = if m.ctrl {
                    // Resize from the right and bottom edges.
                    let g = Grab::Handle(if x != 0.0 { 4 } else { 8 });
                    crop::drag_in(c, g, x * step, y * step, ratio, &fr)
                } else {
                    crop::drag_in(c, Grab::Move, x * step, y * step, None, &fr)
                };
                n.crop = moved.map(|v| v as f32);
            }
            Key::Char(ch) => match ch.to_ascii_lowercase() {
                'r' if m.shift => {
                    self.guides_on = !self.guides_on;
                    self.drag = None;
                    let say = if self.guides_on {
                        t!("develop-persp-guides-on")
                    } else {
                        t!("develop-persp-guides-off")
                    };
                    return Some(Outcome::Redraw(Some(say)));
                }
                'r' => return Some(Outcome::Done),
                'o' => {
                    self.guide = self.guide.next();
                    return Some(Outcome::Redraw(Some(guide_name(self.guide))));
                }
                ',' | '.' | '<' | '>' => {
                    let d = if m.shift { 1.0 } else { 0.1 };
                    let d = if matches!(*ch, ',' | '<') { -d } else { d };
                    n.angle_deg = (p.angle_deg + d).clamp(-45.0, 45.0);
                    n.angle_deg = (n.angle_deg * 10.0).round() / 10.0;
                    refit(&mut n, lay);
                }
                'a' => {
                    let i = ASPECTS.iter().position(|a| *a == p.aspect);
                    let next = match i {
                        Some(i) => ASPECTS[(i + 1) % ASPECTS.len()],
                        // A custom ratio: back to free.
                        None => "free",
                    };
                    let (n, say) = with_aspect(p, lay, next);
                    return Some(Outcome::Set(n, Some(say)));
                }
                'x' => {
                    let (n, say) = swapped(p, lay);
                    return Some(Outcome::Set(n, Some(say)));
                }
                '[' | ']' => {
                    let (n, say) = turned_by(p, lay, *ch == ']');
                    return Some(Outcome::Set(n, Some(say)));
                }
                'h' | 'v' => {
                    let (n, say) = flipped(p, ch.eq_ignore_ascii_case(&'h'));
                    return Some(Outcome::Set(n, Some(say)));
                }
                _ => return None,
            },
        }
        (n != *p).then_some(Outcome::Set(n, None))
    }

    /// The overlay: the area outside the crop dimmed, the crop's edges, guides and handles,
    /// a line being drawn, and a label naming the aspect and the keys. With a perspective,
    /// the warped photo's outline; drawing guides, the guides, a grid while one is dragged
    /// and a loupe on `tex` (the picture shown) at the dragged end.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        lay: &Layout,
        p: &CropOrientation,
        rtl: bool,
        ts: f32,
        map: Option<&GuideMap>,
        tex: Option<TexId>,
    ) {
        let g = ts.max(1.0);
        let f = lay.frame;
        let r = lay.rect(p.crop);
        let dim = Color::BLACK.with_alpha(0.55);
        for o in [
            Rect::from_min_max(f.x, f.y, f.right(), r.y),
            Rect::from_min_max(f.x, r.bottom(), f.right(), f.bottom()),
            Rect::from_min_max(f.x, r.y, r.x, r.bottom()),
            Rect::from_min_max(r.right(), r.y, f.right(), r.bottom()),
        ] {
            if o.w > 0.0 && o.h > 0.0 {
                dl.rect(o, 0.0, dim);
            }
        }
        let line = Color::WHITE.with_alpha(0.55);
        let aspect = f64::from(r.w / r.h.max(1.0));
        for seg in self.guide.lines(aspect) {
            let a = (r.x + seg[0].0 as f32 * r.w, r.y + seg[0].1 as f32 * r.h);
            let b = (r.x + seg[1].0 as f32 * r.w, r.y + seg[1].1 as f32 * r.h);
            dl.line(a, b, g, line);
        }
        dl.stroke_rect(r, 0.0, Color::WHITE, 1.5 * g);
        let h = HANDLE * g;
        for (x, y) in [
            (r.x, r.y),
            (r.center().0, r.y),
            (r.right(), r.y),
            (r.right(), r.center().1),
            (r.right(), r.bottom()),
            (r.center().0, r.bottom()),
            (r.x, r.bottom()),
            (r.x, r.center().1),
        ] {
            let q = Rect::new(x - h / 2.0, y - h / 2.0, h, h);
            dl.rect_border(q, 2.0 * g, Color::WHITE, pal.accent, 1.5 * g);
        }
        if let Some(d) = self.drag.filter(|d| d.kind == DragKind::Line) {
            dl.line(d.start, d.now, 2.0 * g, pal.accent);
        }
        // The warped photo's outline (what shows of it).
        if lay.keystone.is_some()
            && let Some(q) = lay.frame_of(p).outline()
        {
            let at = |c: (f64, f64)| {
                (
                    f.x + (c.0 / lay.w) as f32 * f.w,
                    f.y + (c.1 / lay.h) as f32 * f.h,
                )
            };
            for i in 0..4 {
                dl.line(at(q[i]), at(q[(i + 1) % 4]), 1.5 * g, pal.accent);
            }
        }
        if self.guides_on {
            self.draw_guides(dl, pal, f, g, map, tex);
        }
        // The labels at the bottom, clear of the top handles: aspect and guide, then the keys.
        let lines = if self.guides_on {
            [
                t!("develop-persp-guides-keys"),
                t!(
                    "develop-persp-guides-label",
                    n = map.map_or(0, |m| m.guides.len()),
                    max = MAX_GUIDES
                ),
            ]
        } else {
            [
                t!("develop-crop-keys"),
                t!(
                    "develop-crop-label",
                    aspect = aspect_name(&p.aspect),
                    guide = guide_name(self.guide)
                ),
            ]
        };
        let style = TextStyle::new(metrics::FONT_SMALL * ts).weight(600.0);
        let mut y = f.bottom() - 8.0 * g;
        for s in lines {
            let b = text.layout(&s, style, None, Align::Start);
            let w = b.width + 16.0 * g;
            let x = if rtl {
                f.right() - 8.0 * g - w
            } else {
                f.x + 8.0 * g
            };
            let h = b.height + 8.0 * g;
            y -= h;
            let bg = Rect::new(x, y, w, h);
            dl.rect(bg, 6.0 * g, pal.glass);
            text.draw(dl, &b, x + 8.0 * g, y + 4.0 * g, pal.text);
            y -= 4.0 * g;
        }
    }
}

impl CropTool {
    /// The guides, the one dragged, a grid over the frame and the loupe.
    fn draw_guides(
        &self,
        dl: &mut DrawList,
        pal: &Palette,
        f: Rect,
        g: f32,
        map: Option<&GuideMap>,
        tex: Option<TexId>,
    ) {
        let drag = self.guide_drag();
        if drag.is_some() {
            let line = Color::WHITE.with_alpha(0.3);
            for seg in Guide::Grid.lines(f64::from(f.w / f.h.max(1.0))) {
                let a = (f.x + seg[0].0 as f32 * f.w, f.y + seg[0].1 as f32 * f.h);
                let b = (f.x + seg[1].0 as f32 * f.w, f.y + seg[1].1 as f32 * f.h);
                dl.line(a, b, g, line);
            }
        }
        let moving = self.drag.and_then(|d| match d.kind {
            DragKind::Guide(Some((i, _))) => Some(i),
            _ => None,
        });
        if let Some(m) = map {
            for (i, e) in m.ends().iter().enumerate() {
                let Some([a, b]) = *e else { continue };
                if moving == Some(i) {
                    continue;
                }
                let c = if self.selected == Some(i) {
                    pal.accent
                } else {
                    Color::WHITE
                };
                dl.line(a, b, 2.0 * g, Color::BLACK.with_alpha(0.6));
                dl.line(a, b, 1.25 * g, c);
                for e in [a, b] {
                    dl.circle(e, 4.0 * g, c);
                }
            }
        }
        let Some((a, b)) = drag else { return };
        dl.line(a, b, 2.0 * g, pal.accent);
        dl.circle(a, 4.0 * g, pal.accent);
        dl.circle(b, 4.0 * g, pal.accent);
        // The loupe: the picture around the dragged end, four times larger.
        let Some(tex) = tex else { return };
        let size = 112.0 * g;
        let zoom = 4.0;
        let mut x = b.0 + 24.0 * g;
        let mut y = b.1 - 24.0 * g - size;
        if x + size > f.right() {
            x = b.0 - 24.0 * g - size;
        }
        if y < f.y {
            y = b.1 + 24.0 * g;
        }
        let r = Rect::new(x, y, size, size);
        let (u, v) = ((b.0 - f.x) / f.w.max(1.0), (b.1 - f.y) / f.h.max(1.0));
        let (hu, hv) = (
            size / 2.0 / zoom / f.w.max(1.0),
            size / 2.0 / zoom / f.h.max(1.0),
        );
        dl.rect(r, size / 2.0, Color::BLACK);
        dl.image(r, [u - hu, v - hv, u + hu, v + hv], tex, size / 2.0);
        dl.stroke_rect(r, size / 2.0, Color::WHITE, 2.0 * g);
        let (cx, cy) = r.center();
        dl.line((cx - 8.0 * g, cy), (cx + 8.0 * g, cy), g, pal.accent);
        dl.line((cx, cy - 8.0 * g), (cx, cy + 8.0 * g), g, pal.accent);
    }
}

/// The crop kept inside the photo after the angle changed.
fn refit(p: &mut CropOrientation, lay: &Layout) {
    let c = crop::fit_inside_in(p.crop.map(f64::from), &lay.frame_of(p));
    p.crop = c.map(|v| v as f32);
}

/// The crop given `p`'s aspect.
fn apply_ratio(p: &mut CropOrientation, lay: &Layout) {
    let fr = lay.frame_of(p);
    let c = p.crop.map(f64::from);
    let c = match aspect_ratio(&p.aspect, lay.w, lay.h) {
        Some(r) => crop::with_ratio_in(c, r, &fr),
        None => crop::fit_inside_in(c, &fr),
    };
    p.crop = c.map(|v| v as f32);
}

/// The layout after a quarter turn from `before` to `after`.
fn turned(lay: &Layout, before: &CropOrientation, after: &CropOrientation) -> Layout {
    if (before.quarter_turns - after.quarter_turns).rem_euclid(2) == 1 {
        let (cx, cy) = lay.frame.center();
        Layout {
            frame: Rect::new(
                cx - lay.frame.h / 2.0,
                cy - lay.frame.w / 2.0,
                lay.frame.h,
                lay.frame.w,
            ),
            w: lay.h,
            h: lay.w,
            // The keystone follows the turn once the picture is drawn again.
            keystone: None,
        }
    } else {
        *lay
    }
}

/// The current crop's own ratio as `W:H` (rounded to whole pixels).
fn custom_ratio(c: [f64; 4], lay: &Layout) -> String {
    let w = ((c[2] - c[0]) * lay.w).round().max(1.0);
    let h = ((c[3] - c[1]) * lay.h).round().max(1.0);
    format!("{}:{}", fmt_num(w), fmt_num(h))
}

fn ratio_parts(s: &str) -> Option<(f64, f64)> {
    let (a, b) = s.split_once(':')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.3}")
    }
}

/// `p` with aspect `aspect` (an [`ASPECTS`] name, or `W:H`; "custom" takes the crop's own
/// shape), the crop fitted to it, and what to announce.
pub(super) fn with_aspect(
    p: &CropOrientation,
    lay: &Layout,
    aspect: &str,
) -> (CropOrientation, String) {
    let mut n = p.clone();
    n.aspect = if aspect == "custom" {
        custom_ratio(p.crop.map(f64::from), lay)
    } else {
        aspect.into()
    };
    apply_ratio(&mut n, lay);
    let say = t!("develop-crop-aspect", aspect = aspect_name(&n.aspect));
    (n, say)
}

/// Portrait and landscape swapped: the aspect turned on its side (free: the crop's own
/// shape, turned).
pub(super) fn swapped(p: &CropOrientation, lay: &Layout) -> (CropOrientation, String) {
    let c = p.crop.map(f64::from);
    let (a, b) = match aspect_ratio(&p.aspect, lay.w, lay.h) {
        Some(r) => ratio_parts(&p.aspect).unwrap_or((r, 1.0)),
        None => {
            let w = ((c[2] - c[0]) * lay.w).round().max(1.0);
            let h = ((c[3] - c[1]) * lay.h).round().max(1.0);
            (w, h)
        }
    };
    with_aspect(p, lay, &format!("{}:{}", fmt_num(b), fmt_num(a)))
}

/// A quarter turn right (`cw`) or left; the crop moves with the photo.
pub(super) fn turned_by(p: &CropOrientation, lay: &Layout, cw: bool) -> (CropOrientation, String) {
    let mut n = p.clone();
    n.quarter_turns = (p.quarter_turns + if cw { 1 } else { 3 }).rem_euclid(4);
    let [l, t, r, b] = p.crop;
    n.crop = if cw {
        [1.0 - b, l, 1.0 - t, r]
    } else {
        [t, 1.0 - r, b, 1.0 - l]
    };
    let say = if cw {
        t!("develop-crop-turned-right")
    } else {
        t!("develop-crop-turned-left")
    };
    let lay = turned(lay, p, &n);
    apply_ratio(&mut n, &lay);
    (n, say)
}

/// Flipped left to right (`horizontal`) or top to bottom; the crop moves with the photo.
pub(super) fn flipped(p: &CropOrientation, horizontal: bool) -> (CropOrientation, String) {
    let mut n = p.clone();
    let [l, t, r, b] = p.crop;
    if horizontal {
        n.flip_horizontal = !p.flip_horizontal;
        n.crop = [1.0 - r, t, 1.0 - l, b];
    } else {
        n.flip_vertical = !p.flip_vertical;
        n.crop = [l, 1.0 - b, r, 1.0 - t];
    }
    n.angle_deg = -p.angle_deg;
    let say = if horizontal {
        t!("develop-crop-flipped-horizontal")
    } else {
        t!("develop-crop-flipped-vertical")
    };
    (n, say)
}

/// A typed aspect as `W:H`: "5:4", "5x4", "5×4", "5/4" or one number ("1.85" is
/// 1.85:1). `None` when it is not a ratio of two positive numbers (at most 100:1 either
/// way).
pub(super) fn parse_ratio(typed: &str) -> Option<String> {
    let s = typed.trim();
    let (a, b) = match s.split_once([':', 'x', 'X', '×', '/']) {
        Some((a, b)) => (a.trim(), b.trim()),
        None => (s, "1"),
    };
    // A decimal comma, as many locales type it.
    let num = |v: &str| v.replace(',', ".").parse::<f64>().ok();
    let (a, b) = (num(a)?, num(b)?);
    let ok = a.is_finite() && b.is_finite() && a > 0.0 && b > 0.0;
    (ok && (0.01..=100.0).contains(&(a / b))).then(|| format!("{}:{}", fmt_num(a), fmt_num(b)))
}

/// An aspect's name as shown.
pub(super) fn aspect_name(a: &str) -> String {
    match a {
        "free" => t!("develop-crop-aspect-free"),
        "original" => t!("develop-crop-aspect-original"),
        s => s.to_owned(),
    }
}

fn guide_name(g: Guide) -> String {
    match g {
        Guide::Thirds => t!("develop-crop-guide-thirds"),
        Guide::Golden => t!("develop-crop-guide-golden"),
        Guide::Diagonal => t!("develop-crop-guide-diagonal"),
        Guide::Grid => t!("develop-crop-guide-grid"),
    }
}

/// The Crop card's rows below Straighten (the tool's keys, for the pointer and the screen
/// reader): the aspect, a typed ratio, portrait or landscape, turn and flip.
pub(super) mod card {
    use specular_develop::model::params::CropOrientation;
    use specular_develop::ops::geometry::crop::ASPECTS;
    use specular_i18n::t;
    use specular_widgets::choice::Choice;
    use specular_widgets::develop_panel::{Body, Control};

    use super::super::controls::id;

    /// Whether `control` is one of the card's rows (handled by the application, which knows
    /// where the photo is: [`super::with_aspect`] and the rest).
    pub fn is_ours(control: u32) -> bool {
        (id::CROP_ASPECT..=id::CROP_FLIP_V).contains(&control)
    }

    /// The rows, after Straighten.
    pub fn controls() -> Vec<Control> {
        let button = |id: u32, label: String| Control {
            id,
            body: Body::Choice(Choice::button(label)),
        };
        vec![
            Control {
                id: id::CROP_ASPECT,
                body: Body::Choice(Choice::new(
                    t!("develop-crop-aspect-row"),
                    ASPECTS.iter().map(|a| option_name(a)).collect(),
                )),
            },
            button(id::CROP_TYPE, t!("develop-crop-type-ratio")),
            button(id::CROP_SWAP, t!("develop-crop-swap")),
            button(id::CROP_TURN_LEFT, t!("develop-crop-turn-left")),
            button(id::CROP_TURN_RIGHT, t!("develop-crop-turn-right")),
            button(id::CROP_FLIP_H, t!("develop-crop-flip-horizontal")),
            button(id::CROP_FLIP_V, t!("develop-crop-flip-vertical")),
        ]
    }

    /// An aspect row option's name ("custom" alone; a typed ratio shows itself).
    fn option_name(a: &str) -> String {
        match a {
            "free" => t!("develop-crop-aspect-option-free"),
            "original" => t!("develop-crop-aspect-option-original"),
            "custom" => t!("develop-crop-aspect-option-custom"),
            s => s.to_owned(),
        }
    }

    /// Where the aspect row stands for `p`: a listed aspect, else Custom showing the ratio.
    pub fn read(p: &CropOrientation, ctl: &mut Control) -> bool {
        if !is_ours(ctl.id) {
            return false;
        }
        if let (id::CROP_ASPECT, Body::Choice(c)) = (ctl.id, &mut ctl.body) {
            let last = ASPECTS.len() - 1;
            let i = ASPECTS[..last].iter().position(|a| *a == p.aspect);
            c.options[last] = match i {
                Some(_) => option_name("custom"),
                None => p.aspect.clone(),
            };
            c.set(i.unwrap_or(last));
        }
        true
    }

    /// The aspect chosen on the row (an [`ASPECTS`] name).
    pub fn chosen(ctl: &Control) -> Option<&'static str> {
        match &ctl.body {
            Body::Choice(c) if ctl.id == id::CROP_ASPECT => ASPECTS.get(c.selected).copied(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_input::Mods;
    use specular_widgets::develop_panel::Body;

    fn lay() -> Layout {
        Layout {
            frame: Rect::new(100.0, 50.0, 600.0, 400.0),
            w: 6000.0,
            h: 4000.0,
            keystone: None,
        }
    }

    fn key(c: &str) -> KeyCombo {
        KeyCombo {
            key: Key::Char(c.chars().next().expect("a key")),
            mods: Mods::NONE,
        }
    }

    #[test]
    fn layout_follows_quarter_turns() {
        let pic = Rect::new(0.0, 0.0, 300.0, 200.0);
        let p = CropOrientation {
            quarter_turns: 1,
            ..Default::default()
        };
        let l = Layout::new(pic, (3000, 2000), Orient::default(), &p);
        assert_eq!((l.w, l.h), (2000.0, 3000.0));
        assert_eq!(l.frame, Rect::new(50.0, -50.0, 200.0, 300.0));
    }

    #[test]
    fn keys_turn_aspect_and_straighten() {
        let mut t = CropTool::new(0);
        let p = CropOrientation::default();
        // A: free → original… cycle starts from "original".
        let Some(Outcome::Set(n, _)) = t.key(key("a"), &lay(), &p) else {
            panic!("aspect")
        };
        assert_eq!(n.aspect, "1:1");
        let r = lay().rect(n.crop);
        assert!((r.w - r.h).abs() < 0.01, "{r:?}");
        // X on 3:2 gives 2:3.
        let p32 = CropOrientation {
            aspect: "3:2".into(),
            ..Default::default()
        };
        let Some(Outcome::Set(n, _)) = t.key(key("x"), &lay(), &p32) else {
            panic!("swap")
        };
        assert_eq!(n.aspect, "2:3");
        // A turn moves the crop with the photo.
        let pc = CropOrientation {
            crop: [0.1, 0.2, 0.5, 0.6],
            aspect: "free".into(),
            ..Default::default()
        };
        let Some(Outcome::Set(n, _)) = t.key(key("]"), &lay(), &pc) else {
            panic!("turn")
        };
        assert_eq!(n.quarter_turns, 1);
        for (a, b) in n.crop.iter().zip([0.4, 0.1, 0.8, 0.5]) {
            assert!((a - b).abs() < 1e-6, "{:?}", n.crop);
        }
        // Straighten shrinks the crop to fit.
        let Some(Outcome::Set(n, _)) = t.key(
            KeyCombo {
                key: Key::Char('.'),
                mods: Mods {
                    shift: true,
                    ..Default::default()
                },
            },
            &lay(),
            &p,
        ) else {
            panic!("straighten")
        };
        assert_eq!(n.angle_deg, 1.0);
        assert!(n.crop[0] > 0.0 && n.crop[2] < 1.0);
        assert_eq!(
            t.key(key("o"), &lay(), &p),
            Some(Outcome::Redraw(Some(guide_name(Guide::Golden))))
        );
        assert_eq!(t.key(key("r"), &lay(), &p), Some(Outcome::Done));
    }

    #[test]
    fn typed_ratios_and_the_card_rows() {
        assert_eq!(parse_ratio("5:4").as_deref(), Some("5:4"));
        assert_eq!(parse_ratio(" 5 x 4 ").as_deref(), Some("5:4"));
        assert_eq!(parse_ratio("5×4").as_deref(), Some("5:4"));
        assert_eq!(parse_ratio("16/9").as_deref(), Some("16:9"));
        assert_eq!(parse_ratio("1.85").as_deref(), Some("1.850:1"));
        assert_eq!(parse_ratio("2,39:1").as_deref(), Some("2.390:1"));
        for bad in ["", "abc", "0:1", "4:0", "-4:3", "1000:1", "inf:1", "NaN"] {
            assert_eq!(parse_ratio(bad), None, "{bad}");
        }
        // A typed ratio sets the crop's shape.
        let p = CropOrientation::default();
        let (n, _) = with_aspect(&p, &lay(), "5:4");
        let r = lay().rect(n.crop);
        assert!((r.w / r.h - 1.25).abs() < 0.01, "{r:?}");
        // Free swaps the crop's own shape: 6000x4000 becomes 2:3-ish portrait.
        let free = CropOrientation {
            aspect: "free".into(),
            ..Default::default()
        };
        let (n, _) = swapped(&free, &lay());
        assert_eq!(n.aspect, "4000:6000");
        let r = lay().rect(n.crop);
        assert!(r.h > r.w, "{r:?}");
        // The aspect row: a listed aspect is chosen; a typed one is Custom showing itself.
        let mut rows = card::controls();
        let row = &mut rows[0];
        assert!(card::read(&n, row));
        let Body::Choice(c) = &row.body else {
            panic!("a choice")
        };
        assert_eq!(c.selected, ASPECTS.len() - 1);
        assert_eq!(c.value_text(), "4000:6000");
        assert_eq!(card::chosen(row), Some("custom"));
        let third = CropOrientation {
            aspect: "3:2".into(),
            ..Default::default()
        };
        assert!(card::read(&third, row));
        assert_eq!(card::chosen(row), Some("3:2"));
        // Turn and flip buttons do what the keys do.
        let mut t = CropTool::new(0);
        assert_eq!(
            t.key(key("]"), &lay(), &free),
            Some(Outcome::Set(
                turned_by(&free, &lay(), true).0,
                Some(turned_by(&free, &lay(), true).1)
            ))
        );
        assert_eq!(
            t.key(key("h"), &lay(), &free),
            Some(Outcome::Set(
                flipped(&free, true).0,
                Some(flipped(&free, true).1)
            ))
        );
    }

    #[test]
    fn a_drawn_line_levels_the_photo() {
        let mut t = CropTool::new(0);
        let p = CropOrientation::default();
        let l = lay();
        assert!(t.press((200.0, 200.0), true, &l, &p));
        let rise = 100.0 * 2f32.to_radians().tan();
        t.moved((300.0, 200.0 + rise), &l, &p);
        let Some(Outcome::Set(n, _)) = t.release(&l, &p, None) else {
            panic!("line")
        };
        assert!((n.angle_deg + 2.0).abs() < 1e-3, "{}", n.angle_deg);
    }

    #[test]
    fn dragging_a_corner_resizes() {
        let mut t = CropTool::new(0);
        let p = CropOrientation {
            aspect: "free".into(),
            ..Default::default()
        };
        let l = lay();
        assert!(t.press((700.0, 450.0), false, &l, &p));
        let n = t.moved((640.0, 410.0), &l, &p).expect("moved");
        assert!(
            (n.crop[2] - 0.9).abs() < 1e-4 && (n.crop[3] - 0.9).abs() < 1e-4,
            "{:?}",
            n.crop
        );
        assert_eq!(t.release(&l, &n, None), None);
    }
}
