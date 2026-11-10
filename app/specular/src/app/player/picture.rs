//! A movie's picture while it plays: its turn (rotate and flip, over the turn the file asks
//! for), kept per file with its other player settings; the speed menu's speeds; the picture
//! adjustments (brightness, contrast, saturation, gamma, hue: drawn by the image shader for
//! this session, never written to a file); and where subtitles go while the picture is
//! zoomed or panned (where they would be at fit, so they stay put).

use std::time::Instant;

use specular_core::Orientation;
use specular_i18n::t;
use specular_render::PictureAdjust;
use specular_widgets::PICTURE;

use super::super::App;
use crate::anim;

/// `base` (the file's own orientation) with the user's `turn` on top: a turn is what the
/// viewer's rotate and flip actions did to the upright picture, mirror first.
pub(crate) fn apply(base: Orientation, turn: Orientation) -> Orientation {
    let (q, f) = turn.to_rotation_flip();
    let o = if f { base.flipped_h() } else { base };
    o.rotated(i32::from(q))
}

/// The adjustments from the menu's values ([`PICTURE`]'s order).
pub(crate) fn adjust(v: [f32; 5]) -> PictureAdjust {
    PictureAdjust {
        brightness: v[0],
        contrast: v[1],
        saturation: v[2],
        gamma: v[3],
        hue: v[4],
    }
    .clamped()
}

/// The menu's neutral values.
pub(crate) fn neutral() -> [f32; 5] {
    PICTURE.map(|k| k.neutral)
}

/// The picture adjustments' values ([`PICTURE`]'s order); neutral by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Values(pub [f32; 5]);

impl Default for Values {
    fn default() -> Self {
        Values(neutral())
    }
}

/// Where subtitles go over a picture of display size `d` in `viewport` (x, y, w, h) at
/// `fit` scale: the picture's place at fit, within the viewport.
pub(crate) fn fit_rect(
    d: (f32, f32),
    viewport: (f32, f32, f32, f32),
    fit: f32,
) -> (f32, f32, f32, f32) {
    let (vx, vy, vw, vh) = viewport;
    let (w, h) = ((d.0 * fit).min(vw), (d.1 * fit).min(vh));
    (
        vx + (vw - w) * 0.5,
        vy + (vh - h) * 0.5,
        w.max(1.0),
        h.max(1.0),
    )
}

impl App {
    /// A rotate or flip action on the movie on screen (`f` as it would change the
    /// orientation): the turn is remembered for the file. True when it was a movie's (a
    /// sound file's picture does not turn).
    pub(in crate::app) fn player_turn(&mut self, f: &dyn Fn(Orientation) -> Orientation) -> bool {
        let Some((id, _)) = self.player.movie.clone() else {
            return false;
        };
        if self.current_id() != Some(id) {
            return false;
        }
        if !self.video_picture_current() {
            return true;
        }
        self.player.turn = f(self.player.turn);
        self.player_apply_turn(id);
        self.player_save();
        true
    }

    /// The movie's orientation: its file's, with the remembered turn on top.
    pub(in crate::app) fn player_apply_turn(&mut self, id: crate::loader::ImageId) {
        let turn = self.player.turn;
        let Some(img) = self.images.get_mut(&id) else {
            return;
        };
        let Some(base) = img.info.as_ref().map(|i| i.orientation) else {
            return;
        };
        let o = apply(base, turn);
        if img.orient == Some(o) {
            return;
        }
        img.orient = Some(o);
        if self.current_id() == Some(id) {
            self.view.set_orientation(o);
            self.view.zoom_fit();
            self.view.settle();
            self.after_view_change();
            self.scene = None;
        }
    }

    /// A speed from the speed menu (kept within a movie's range).
    pub(in crate::app) fn player_speed(&mut self, s: f64) {
        let Some(a) = self.anim.as_mut().filter(|a| a.movie.is_some()) else {
            return;
        };
        let s = s.clamp(anim::VIDEO_MIN_SPEED, anim::VIDEO_MAX_SPEED);
        a.set_speed(s, Instant::now());
        let now = self.now();
        self.chrome.toast(
            t!(
                "toast-speed",
                speed = specular_i18n::I18n::global().localizer().number(s, 2)
            ),
            None,
            now,
        );
        self.tick_animation();
    }

    /// A picture adjustment stepped (0: back to neutral), or all of them reset (`None`).
    pub(in crate::app) fn player_picture(&mut self, step: Option<(usize, i64)>) {
        match step {
            Some((k, s)) if k < PICTURE.len() => {
                self.player.picture.0[k] = PICTURE[k].stepped(self.player.picture.0[k], s);
            }
            Some(_) => return,
            None => self.player.picture = Values::default(),
        }
        self.scene = None;
        self.redraw = true;
    }

    /// The adjustments to draw the picture with: the movie on screen's, when any is set.
    pub(in crate::app) fn player_render_picture(&self) -> Option<PictureAdjust> {
        self.player_movie()?;
        let p = adjust(self.player.picture.0);
        (!p.is_neutral()).then_some(p)
    }

    /// Where subtitles go (logical px): the picture's place at fit while it is zoomed or
    /// panned, so they stay put; the shown part of it when cropped to a shape.
    pub(in crate::app) fn player_subtitle_rect(&self) -> (f32, f32, f32, f32) {
        let v = self.view.viewport;
        if self.player.crop == 0 {
            return fit_rect(
                self.view.display_size(),
                (v.x, v.y, v.w, v.h),
                self.view.fit_scale(),
            );
        }
        let r = self.view.image_screen_rect();
        let (x0, y0) = (r.x.max(v.x), r.y.max(v.y));
        let (x1, y1) = ((r.x + r.w).min(v.x + v.w), (r.y + r.h).min(v.y + v.h));
        (x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where stored pixel `p` of a `w`×`h` picture lands on screen under EXIF orientation `o`.
    fn place(o: Orientation, w: i32, h: i32, p: (i32, i32)) -> (i32, i32) {
        let (q, f) = o.to_rotation_flip();
        let (mut x, mut y) = (if f { w - 1 - p.0 } else { p.0 }, p.1);
        let (mut cw, mut ch) = (w, h);
        for _ in 0..q {
            // A quarter turn clockwise.
            (x, y) = (ch - 1 - y, x);
            (cw, ch) = (ch, cw);
        }
        let _ = cw;
        (x, y)
    }

    type Act = fn(Orientation) -> Orientation;
    const ACTIONS: [(&str, Act); 4] = [
        ("left", |o| o.rotated(-1)),
        ("right", |o| o.rotated(1)),
        ("flip h", |o| o.flipped_h()),
        ("flip v", |o| o.flipped_h().rotated(2)),
    ];

    #[test]
    fn a_turn_on_top_of_any_file_orientation_is_the_actions_done_on_it() {
        // Every file orientation, every sequence of up to three actions: applying the turn
        // the actions make from upright equals doing the actions on the file's orientation.
        for base in 1..=8u8 {
            let base = Orientation(base);
            let mut seqs: Vec<Vec<usize>> = vec![vec![]];
            for _ in 0..3 {
                let next: Vec<Vec<usize>> = seqs
                    .iter()
                    .flat_map(|s| {
                        (0..ACTIONS.len()).map(move |a| {
                            let mut s = s.clone();
                            s.push(a);
                            s
                        })
                    })
                    .collect();
                seqs.extend(next);
            }
            for s in seqs {
                let (mut turn, mut direct) = (Orientation::NORMAL, base);
                for &a in &s {
                    turn = (ACTIONS[a].1)(turn);
                    direct = (ACTIONS[a].1)(direct);
                }
                assert_eq!(apply(base, turn), direct, "base {base:?} actions {s:?}");
            }
        }
    }

    #[test]
    fn turns_move_pixels_as_named() {
        // A 3×2 picture's top-left pixel.
        let p = (0, 0);
        let right = Orientation::NORMAL.rotated(1);
        // Turned right (clockwise) it is at the top right of the 2×3 picture.
        assert_eq!(place(right, 3, 2, p), (1, 0));
        let left = Orientation::NORMAL.rotated(-1);
        assert_eq!(place(left, 3, 2, p), (0, 2));
        let h = Orientation::NORMAL.flipped_h();
        assert_eq!(place(h, 3, 2, p), (2, 0));
        let v = Orientation::NORMAL.flipped_h().rotated(2);
        assert_eq!(place(v, 3, 2, p), (0, 1));
        // A phone movie stored sideways (the file turns it right): turned back left it is
        // as stored.
        let phone = Orientation(6);
        assert_eq!(apply(phone, left), Orientation::NORMAL);
        // Flipped, a sideways phone movie mirrors left to right on screen.
        let shown = apply(phone, h);
        assert_eq!(place(shown, 3, 2, (0, 0)), (0, 0));
        assert_eq!(place(phone, 3, 2, (0, 0)), (1, 0));
    }

    #[test]
    fn subtitles_stay_where_the_picture_fits() {
        // A 16:9 picture in a 1200×800 view: letterboxed, the same place at any zoom.
        let r = fit_rect((1920.0, 1080.0), (0.0, 0.0, 1200.0, 800.0), 1200.0 / 1920.0);
        assert!((r.0 - 0.0).abs() < 1e-3 && (r.2 - 1200.0).abs() < 1e-3);
        assert!((r.1 - 62.5).abs() < 1e-3 && (r.3 - 675.0).abs() < 1e-3);
        // Turned on its side: a pillar-boxed 9:16 picture.
        let r = fit_rect((1080.0, 1920.0), (0.0, 0.0, 1200.0, 800.0), 800.0 / 1920.0);
        assert!((r.2 - 450.0).abs() < 1e-3 && (r.0 - 375.0).abs() < 1e-3);
    }

    #[test]
    fn neutral_adjustments_draw_nothing() {
        assert!(adjust(neutral()).is_neutral());
        let mut v = neutral();
        v[4] = PICTURE[4].stepped(v[4], 3);
        let a = adjust(v);
        assert!(!a.is_neutral());
        assert_eq!(a.hue, 30.0);
    }
}
