//! Reflectance transformation images (PTM, HSH RTI, Relight folders) in the viewer: the
//! coefficients the decoder read ([`specular_decode::Session::rti`]) go to the GPU with the
//! image ([`ImageRenderer::set_relight`]) and every change of the light redraws the overview
//! from them ([`ImageRenderer::relight`]), so moving the light needs no decode.
//!
//! The light is moved by Shift-dragging over the image (the image area stands for the
//! hemisphere seen from above: its centre is the light straight above, its edge the
//! horizon) or by dragging in the light widget, a small hemisphere in the bottom corner
//! opposite the readouts with the light as a dot. The keymap's relight mode
//! ([`Mode::Relight`], `[relight]` in `keymap.toml`, over the viewer's keys) is on while one
//! is shown: by default Shift+arrows move the light, `s` steps the mode (lit, specular
//! enhancement, normals, diffuse gain), `0` puts the light back above and `l` shows or hides
//! the widget. Every relight action is in [`Group::Relight`].
//!
//! The mode and the widget stay across files; the light starts above for each file.
//!
//! [`ImageRenderer::set_relight`]: specular_render::ImageRenderer::set_relight
//! [`ImageRenderer::relight`]: specular_render::ImageRenderer::relight
//! [`Mode::Relight`]: specular_input::Mode::Relight
//! [`Group::Relight`]: specular_input::Group::Relight

#[cfg(test)]
mod shots;

use std::sync::Arc;

use specular_decode::specular_rti::{DEFAULT_LIGHT, Look, Mode, Rti};
use specular_gfx::{DrawList, Rect};
use specular_i18n::t;
use specular_input::Action;
use specular_theme::Color;
use specular_theme::metrics::{RADIUS_MD, TOOLBAR_MARGIN};
use specular_widgets::accesskit;

use super::App;
use crate::loader::ImageId;

/// AccessKit id of the light widget (anonymisation uses 0x5A00…, measuring 0x5C00…, the
/// 3D view's readout 0x5E00…).
const A11Y_ID: u64 = 0x5F00_0000_0000_0000;

/// The light widget's side (logical px), backdrop included.
pub(super) const WIDGET: f32 = 104.0;

/// Inset of the hemisphere's rim from the widget's edge (logical px).
const WIDGET_PAD: f32 = 10.0;

/// A nudge key moves the light this far across the unit disk.
const NUDGE: f32 = 0.1;

/// What the primary button is dragging.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// The dot in the light widget.
    Widget,
    /// Shift-drag over the image area.
    Image,
}

/// The relightable image of the current file and how it is lit.
pub(super) struct RelightView {
    /// The file the state below belongs to.
    id: Option<ImageId>,
    rti: Option<Arc<Rti>>,
    /// The light and mode (the mode stays across files).
    pub(super) look: Look,
    /// The light widget is shown (kept across files).
    pub(super) widget: bool,
    drag: Option<Drag>,
    /// The image area and the widget as last drawn (logical px).
    area: Option<Rect>,
    rect: Option<Rect>,
    /// The file and look the GPU image was last relit for.
    applied: Option<(ImageId, Look)>,
    /// The drag hint has been shown (once a run).
    hinted: bool,
}

impl Default for RelightView {
    fn default() -> Self {
        RelightView {
            id: None,
            rti: None,
            look: Look::default(),
            widget: true,
            drag: None,
            area: None,
            rect: None,
            applied: None,
            hinted: false,
        }
    }
}

impl RelightView {
    /// The look for file `id`: the current one for the current file, else the light above
    /// in the current mode.
    pub(super) fn look_for(&self, id: ImageId) -> Look {
        if self.id == Some(id) {
            self.look
        } else {
            Look {
                light: DEFAULT_LIGHT,
                ..self.look
            }
        }
    }

    /// Notes that file `id` was relit for `look`.
    pub(super) fn applied(&mut self, id: ImageId, look: Look) {
        self.applied = Some((id, look));
    }

    /// Records where the image area and the widget were drawn (logical px).
    pub(super) fn drawn(&mut self, area: Rect, widget: Option<Rect>) {
        self.area = Some(area);
        self.rect = widget;
    }

    /// Puts the light over the point `(x, y)` of the unit disk (x right, y up).
    fn set_disk(&mut self, x: f32, y: f32) {
        self.look.light = Look::light_from_disk(x, y);
    }

    /// Moves the light by `(dx, dy)` across the unit disk, stopping at the rim.
    fn nudge(&mut self, dx: f32, dy: f32) {
        // Past the rim, `light_from_disk` puts the light on the horizon.
        self.set_disk(self.look.light[0] + dx, self.look.light[1] + dy);
    }
}

/// The unit-disk point (x right, y up) under the logical point `p` for a disk of radius `r`
/// centred at `c`.
fn disk_at(p: (f32, f32), c: (f32, f32), r: f32) -> (f32, f32) {
    let r = r.max(1.0);
    ((p.0 - c.0) / r, -(p.1 - c.1) / r)
}

/// The light's compass angle (degrees, 0 from the top of the image, clockwise) and its
/// elevation above the image plane (degrees), rounded.
pub(super) fn angles(light: [f32; 3]) -> (i32, i32) {
    let az = light[0]
        .atan2(light[1])
        .to_degrees()
        .rem_euclid(360.0)
        .round() as i32
        % 360;
    let el = light[2].clamp(-1.0, 1.0).asin().to_degrees().round() as i32;
    (az, el)
}

/// The name of a relighting mode.
pub(super) fn mode_name(m: Mode) -> String {
    match m {
        Mode::Default => t!("relight-mode-default"),
        Mode::Specular => t!("relight-mode-specular"),
        Mode::Normals => t!("relight-mode-normals"),
        Mode::DiffuseGain => t!("relight-mode-diffuse-gain"),
    }
}

/// The widget's label: where the light is and the mode.
pub(super) fn label(look: &Look) -> String {
    let (az, el) = angles(look.light);
    t!(
        "relight-light",
        azimuth = az.to_string(),
        elevation = el.to_string(),
        mode = mode_name(look.mode)
    )
}

/// The widget's rectangle in the image area `area` (logical px): the bottom corner on the
/// start side (the readouts take the end side).
pub(super) fn widget_rect(area: Rect, rtl: bool) -> Rect {
    let m = TOOLBAR_MARGIN;
    let x = if rtl {
        area.x + area.w - m - WIDGET
    } else {
        area.x + m
    };
    Rect::new(x, area.y + area.h - m - WIDGET, WIDGET, WIDGET)
}

/// The hemisphere's centre and radius in the widget `r`.
fn widget_disk(r: Rect) -> ((f32, f32), f32) {
    (r.center(), 0.5 * r.w.min(r.h) - WIDGET_PAD)
}

/// Draws the light widget into `r` (logical px): the hemisphere seen from above with rings
/// at 30° and 60° of elevation, and the light as a dot joined to the zenith.
pub(super) fn draw_widget(dl: &mut DrawList, r: Rect, look: &Look) {
    use std::f32::consts::TAU;
    dl.rect(r, RADIUS_MD, Color::rgba(0.0, 0.0, 0.0, 0.6));
    let (c, rad) = widget_disk(r);
    let rim = Color::WHITE.with_alpha(0.82);
    let faint = Color::WHITE.with_alpha(0.3);
    dl.circle(c, rad, Color::WHITE.with_alpha(0.06));
    dl.arc(c, rad, 0.0, TAU, 1.5, rim);
    for el in [30f32, 60.0] {
        dl.arc(c, rad * el.to_radians().cos(), 0.0, TAU, 1.0, faint);
    }
    dl.line((c.0 - rad, c.1), (c.0 + rad, c.1), 1.0, faint);
    dl.line((c.0, c.1 - rad), (c.0, c.1 + rad), 1.0, faint);
    let [x, y, _] = look.light;
    let p = (c.0 + x * rad, c.1 - y * rad);
    let warm = Color::rgba(1.0, 0.82, 0.32, 1.0);
    dl.line(c, p, 1.5, warm.with_alpha(0.7));
    dl.circle(p, 6.0, Color::BLACK.with_alpha(0.5));
    dl.circle(p, 4.5, warm);
}

/// The widget's node for assistive technology.
pub(super) fn a11y(tree: &mut accesskit::TreeUpdate, look: &Look, r: Rect, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let mut n = Node::new(Role::Status);
    n.set_label(format!("{}: {}", t!("keys-mode-relight"), label(look)));
    n.set_bounds(accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    });
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), n));
}

impl App {
    /// Follows the current file: its coefficients when it has some, relight mode's keys,
    /// and relights the GPU image when the look changed.
    pub(super) fn relight_sync(&mut self) {
        let cur = self.current_id();
        if cur != self.relight.id {
            let old = std::mem::take(&mut self.relight);
            self.relight = RelightView {
                id: cur,
                look: Look {
                    light: DEFAULT_LIGHT,
                    ..old.look
                },
                widget: old.widget,
                area: old.area,
                rect: old.rect,
                applied: old.applied,
                hinted: old.hinted,
                ..Default::default()
            };
        }
        if self.relight.rti.is_none()
            && let Some(id) = cur
            && let Some(r) = self
                .images
                .get(&id)
                .and_then(|i| i.session.as_ref())
                .and_then(|s| s.rti())
        {
            self.relight.rti = Some(r);
            self.redraw = true;
            if !self.relight.hinted {
                self.relight.hinted = true;
                let now = self.now();
                self.chrome.toast(t!("relight-hint"), None, now);
            }
        }
        let on = self.relight_active();
        if self.keys.relight != on {
            self.keys.relight = on;
            self.keys_sync_modes();
        }
        // Relight the GPU image for a new look (uploads relight it themselves).
        if let (Some(id), Some(rti)) = (self.relight.id, self.relight.rti.as_ref())
            && self.relight.applied != Some((id, self.relight.look))
            && let Some(gfx) = self.gfx.as_mut()
            && let Some(g) = self.gpu_images.get(id)
            && g.is_relightable()
        {
            let look = self.relight.look;
            if gfx.renderer.relight(&gfx.gpu, g, &rti.prepare(&look)) {
                self.relight.applied = Some((id, look));
                self.redraw = true;
            }
        }
    }

    /// A relightable image is shown in the viewer.
    pub(super) fn relight_active(&self) -> bool {
        self.relight.rti.is_some()
            && self.relight.id.is_some()
            && self.relight.id == self.current_id()
            && !self.gallery.active
            && !self.mpr_active()
            && !self.mesh_active()
    }

    /// What the relight actions depend on ([`specular_input::ActionState::relight`]).
    pub(super) fn relight_state(&self) -> bool {
        self.relight_active()
    }

    /// The look while a relightable image is shown, for the widget.
    pub(super) fn relight_widget(&self) -> Option<Look> {
        (self.relight_active() && self.relight.widget).then_some(self.relight.look)
    }

    /// Runs a relight action; true when taken.
    pub(super) fn relight_action(&mut self, a: Action) -> bool {
        let mine = matches!(
            a,
            Action::RelightLeft
                | Action::RelightRight
                | Action::RelightUp
                | Action::RelightDown
                | Action::RelightMode
                | Action::RelightReset
                | Action::RelightWidget
        );
        if !mine {
            return false;
        }
        if !self.relight_active() {
            return true;
        }
        let r = &mut self.relight;
        match a {
            Action::RelightLeft => r.nudge(-NUDGE, 0.0),
            Action::RelightRight => r.nudge(NUDGE, 0.0),
            Action::RelightUp => r.nudge(0.0, NUDGE),
            Action::RelightDown => r.nudge(0.0, -NUDGE),
            Action::RelightMode => {
                r.look.mode = r.look.mode.next();
                let msg = mode_name(r.look.mode);
                let now = self.now();
                self.chrome.toast(msg, None, now);
            }
            Action::RelightReset => r.look.light = DEFAULT_LIGHT,
            _ => r.widget = !r.widget,
        }
        self.redraw = true;
        true
    }

    /// A primary button press or release at physical point `p`; true when relighting took
    /// it (a press in the widget, or with Shift over the image area).
    pub(super) fn relight_button(&mut self, p: (f32, f32), down: bool) -> bool {
        if !down {
            return self.relight.drag.take().is_some();
        }
        if !self.relight_active() {
            return false;
        }
        let scale = self.dl.scale.max(0.1);
        let lp = (p.0 / scale, p.1 / scale);
        let r = &mut self.relight;
        r.drag = if r.widget && r.rect.is_some_and(|w| w.contains(lp)) {
            Some(Drag::Widget)
        } else if self.mods.shift_key() && r.area.is_some_and(|a| a.contains(lp)) {
            Some(Drag::Image)
        } else {
            None
        };
        if r.drag.is_none() {
            return false;
        }
        self.relight_motion(p)
    }

    /// Pointer motion at physical `p`: moves the light while dragging. True when it did.
    pub(super) fn relight_motion(&mut self, p: (f32, f32)) -> bool {
        let r = &mut self.relight;
        let (c, rad) = match r.drag {
            Some(Drag::Widget) => match r.rect {
                Some(w) => widget_disk(w),
                None => return false,
            },
            Some(Drag::Image) => match r.area {
                Some(a) => (a.center(), 0.5 * a.w.min(a.h)),
                None => return false,
            },
            None => return false,
        };
        let scale = self.dl.scale.max(0.1);
        let (x, y) = disk_at((p.0 / scale, p.1 / scale), c, rad);
        r.set_disk(x, y);
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nudges_stay_on_the_hemisphere() {
        let mut r = RelightView::default();
        for _ in 0..30 {
            r.nudge(NUDGE, 0.0);
        }
        let [x, y, z] = r.look.light;
        assert!((x - 1.0).abs() < 1e-6 && y.abs() < 1e-6 && z.abs() < 1e-6);
        r.nudge(0.0, NUDGE);
        let [x, y, z] = r.look.light;
        assert!((x * x + y * y + z * z - 1.0).abs() < 1e-5);
        assert!(y > 0.0 && z == 0.0);
    }

    #[test]
    fn disk_follows_the_screen() {
        // Up on screen is up on the disk; the rim is the horizon.
        assert_eq!(disk_at((50.0, 0.0), (50.0, 50.0), 50.0), (0.0, 1.0));
        assert_eq!(disk_at((100.0, 50.0), (50.0, 50.0), 50.0), (1.0, -0.0));
        assert_eq!(angles(DEFAULT_LIGHT), (0, 90));
        assert_eq!(angles(Look::light_from_disk(1.0, 0.0)), (90, 0));
        assert_eq!(angles(Look::light_from_disk(0.0, -0.5)), (180, 60));
        assert_eq!(angles(Look::light_from_disk(-0.5, 0.0)), (270, 60));
    }

    #[test]
    fn widget_sits_in_the_start_corner() {
        let area = Rect::new(0.0, 0.0, 800.0, 600.0);
        let l = widget_rect(area, false);
        let r = widget_rect(area, true);
        assert!(l.x < 100.0 && r.right() > 700.0);
        assert_eq!(l.bottom(), 600.0 - TOOLBAR_MARGIN);
        let ((cx, cy), rad) = widget_disk(l);
        assert!(rad > 30.0 && l.contains((cx + rad, cy)));
    }

    #[test]
    fn label_names_the_light_and_mode() {
        let look = Look {
            light: Look::light_from_disk(0.5, 0.0),
            mode: Mode::Specular,
            ..Default::default()
        };
        let s = label(&look);
        assert!(
            s.contains("90") && s.contains("60") && s.contains("Specular"),
            "{s}"
        );
    }
}
