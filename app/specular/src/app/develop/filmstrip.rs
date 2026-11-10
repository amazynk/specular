//! Develop's filmstrip: the folder's images around the one open, as small thumbnails along
//! the bottom (mirrored in right-to-left languages). A click opens another image; the
//! keyboard steps with the viewer's keys. The wheel scrolls the strip without opening
//! anything; opening another image centres it again.
//!
//! Thumbnails are made in the background from embedded previews where the file has one
//! (`specular_thumbs::generate`), a few at a time, and only for the images near the one open.

use std::collections::HashMap;
use std::path::PathBuf;

use crossbeam_channel::{Receiver, Sender, unbounded};
use specular_core::CancelToken;
use specular_gfx::{DrawList, Gpu, Rect, TexId, UiRenderer, wgpu};
use specular_i18n::t;
use specular_theme::{Color, Palette, metrics};
use specular_thumbs::generate::{self, GenOptions};
use specular_widgets::accesskit;

use crate::loader::Loader;

/// Height of the strip (logical px at the standard text size).
pub(in crate::app) const HEIGHT: f32 = 76.0;
/// Images on each side of the open one that get thumbnails.
const REACH: usize = 10;
/// Thumbnails made at once.
const AT_ONCE: usize = 3;
/// Thumbnails kept on the GPU.
const KEEP: usize = 64;

enum Thumb {
    Pending,
    Failed,
    Ready(u32, u32, Vec<u8>),
    Uploaded {
        _tex: wgpu::Texture,
        id: TexId,
        w: u32,
        h: u32,
    },
}

/// A thumbnail made in the background: its file and its pixels (`None`: none could be made).
type Made = (PathBuf, Option<(u32, u32, Vec<u8>)>);

/// The strip's state.
pub(in crate::app) struct Strip {
    thumbs: HashMap<PathBuf, Thumb>,
    tx: Sender<Made>,
    rx: Receiver<Made>,
    in_flight: usize,
    retired: Vec<TexId>,
    /// Cells as drawn: where, and which entry.
    pub hits: Vec<(Rect, usize)>,
    pub rect: Rect,
    /// Scrolled with the wheel: the entry open then, and the (fractional) entry centred.
    view: Option<(usize, f32)>,
}

impl Default for Strip {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Strip {
            thumbs: HashMap::new(),
            tx,
            rx,
            in_flight: 0,
            retired: Vec::new(),
            hits: Vec::new(),
            rect: Rect::default(),
            view: None,
        }
    }
}

impl std::fmt::Debug for Strip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Strip")
            .field("thumbs", &self.thumbs.len())
            .finish_non_exhaustive()
    }
}

impl Strip {
    /// The (fractional) entry in the middle of the strip: where it was scrolled to while
    /// `current` stays open, else `current`.
    fn centre(&self, current: usize, len: usize) -> f32 {
        let last = len.saturating_sub(1) as f32;
        match self.view {
            Some((open, c)) if open == current => c.clamp(0.0, last),
            _ => current as f32,
        }
    }

    /// A wheel turn over the strip (`dy` lines, up positive; `dx` lines, as the content
    /// should move): scrolls it by about one image a line. Whether it moved.
    pub fn scroll(
        &mut self,
        dx: f32,
        dy: f32,
        current: Option<usize>,
        len: usize,
        rtl: bool,
    ) -> bool {
        let Some(cur) = current else { return false };
        if len < 2 {
            return false;
        }
        let from = self.centre(cur, len);
        // Down and towards the reading direction's end show the next images.
        let by = -dy + if rtl { dx } else { -dx };
        let to = (from + by).clamp(0.0, (len - 1) as f32);
        self.view = Some((cur, to));
        to != from
    }

    /// The images near `current` of `entries` that need thumbnails, nearest first.
    fn near(entries: &[PathBuf], current: usize) -> Vec<usize> {
        let mut v = vec![current];
        for d in 1..=REACH {
            if current + d < entries.len() {
                v.push(current + d);
            }
            if let Some(i) = current.checked_sub(d) {
                v.push(i);
            }
        }
        v.retain(|i| *i < entries.len());
        v
    }

    /// Takes finished thumbnails and starts the next ones; whether any arrived.
    pub fn tick(&mut self, entries: &[PathBuf], current: Option<usize>, loader: &Loader) -> bool {
        let mut arrived = false;
        while let Ok((p, r)) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            self.thumbs.insert(
                p,
                match r {
                    Some((w, h, px)) => Thumb::Ready(w, h, px),
                    None => Thumb::Failed,
                },
            );
            arrived = true;
        }
        let Some(cur) = current else {
            return arrived;
        };
        // Thumbnails for the images in view (scrolled to, else around the open one).
        let cur = self.centre(cur, entries.len()).round() as usize;
        for i in Strip::near(entries, cur) {
            if self.in_flight >= AT_ONCE {
                break;
            }
            let p = &entries[i];
            if self.thumbs.contains_key(p) {
                continue;
            }
            self.thumbs.insert(p.clone(), Thumb::Pending);
            self.in_flight += 1;
            let tx = self.tx.clone();
            let registry = loader.registry();
            let p = p.clone();
            loader.background("develop-thumb", move |l| {
                let r = registry.open_path(&p).ok().and_then(|s| {
                    generate::generate(
                        &*s,
                        128,
                        GenOptions::default(),
                        &CancelToken::new(),
                        &mut |_| {},
                    )
                    .ok()
                });
                let _ = tx.send((p, r.map(|px| (px.width, px.height, px.rgba))));
                l.waker()();
            });
        }
        // Far thumbnails go when there are too many.
        if self.thumbs.len() > KEEP {
            let keep: Vec<&PathBuf> = Strip::near(entries, cur)
                .into_iter()
                .map(|i| &entries[i])
                .collect();
            let far: Vec<PathBuf> = self
                .thumbs
                .keys()
                .filter(|k| !keep.contains(k))
                .cloned()
                .collect();
            for k in far {
                if let Some(Thumb::Uploaded { id, .. }) = self.thumbs.remove(&k) {
                    self.retired.push(id);
                }
            }
        }
        arrived
    }

    /// Uploads arrived thumbnails; frees retired ones.
    pub fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        for t in self.thumbs.values_mut() {
            if let Thumb::Ready(w, h, px) = t {
                let (w, h) = (*w, *h);
                let (tex, id) = super::preview_upload(gpu, ui, w, h, px);
                *t = Thumb::Uploaded {
                    _tex: tex,
                    id,
                    w,
                    h,
                };
            }
        }
    }

    /// Drops every thumbnail (leaving Develop).
    /// A new GPU device: the thumbnails' textures went with the old one (made again).
    pub fn gpu_reset(&mut self) {
        self.thumbs.clear();
        self.retired.clear();
        self.hits.clear();
    }

    pub fn clear(&mut self) {
        for (_, t) in self.thumbs.drain() {
            if let Thumb::Uploaded { id, .. } = t {
                self.retired.push(id);
            }
        }
        self.hits.clear();
        self.view = None;
    }

    /// Draws the strip in `r` (logical px): the open image centred (or where the wheel
    /// scrolled it), its neighbours on each side (the next one towards the reading
    /// direction's end). `focused`: the strip has the keyboard (F6), ringed.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        dl: &mut DrawList,
        pal: &Palette,
        r: Rect,
        entries: &[PathBuf],
        current: Option<usize>,
        rtl: bool,
        ts: f32,
        focused: bool,
    ) {
        self.rect = r;
        self.hits.clear();
        dl.rect(r, 0.0, pal.sidebar);
        dl.rect(Rect::new(r.x, r.y, r.w, 1.0), 0.0, pal.separator);
        let Some(cur) = current else { return };
        let g = ts.max(1.0);
        let pad = (6.0 * g).round();
        let cell_h = r.h - 2.0 * pad;
        let cell_w = (cell_h * 1.4).round();
        let gap = (6.0 * g).round();
        let cx = r.center().0;
        let n = ((r.w / (cell_w + gap)) as usize / 2 + 1).min(REACH);
        dl.push_clip(r);
        let centre = self.centre(cur, entries.len());
        let mid = centre.round() as usize;
        let lo = mid.saturating_sub(n);
        let hi = (mid + n).min(entries.len().saturating_sub(1));
        for (i, path) in entries.iter().enumerate().take(hi + 1).skip(lo) {
            let off = i as f32 - centre;
            let off = if rtl { -off } else { off };
            let x = cx - cell_w * 0.5 + off * (cell_w + gap);
            let cell = Rect::new(x, r.y + pad, cell_w, cell_h);
            if cell.right() < r.x || cell.x > r.right() {
                continue;
            }
            dl.rect(cell, metrics::RADIUS_SM, pal.surface);
            if let Some(Thumb::Uploaded { id, w, h, .. }) = self.thumbs.get(path) {
                let k = (cell.w / *w as f32).min(cell.h / *h as f32);
                let (tw, th) = (*w as f32 * k, *h as f32 * k);
                let img = Rect::new(
                    cell.x + (cell.w - tw) * 0.5,
                    cell.y + (cell.h - th) * 0.5,
                    tw,
                    th,
                );
                dl.image(img, [0.0, 0.0, 1.0, 1.0], *id, 2.0);
            }
            if i == cur {
                dl.rect_border(
                    cell.inset(-2.0),
                    metrics::RADIUS_SM + 2.0,
                    Color::TRANSPARENT,
                    pal.accent,
                    2.0,
                );
                // The keyboard's ring outside the open image's.
                if focused {
                    dl.rect_border(
                        cell.inset(-5.0),
                        metrics::RADIUS_SM + 5.0,
                        Color::TRANSPARENT,
                        pal.focus_ring,
                        2.0,
                    );
                }
            }
            self.hits.push((cell, i));
        }
        dl.pop_clip();
        // Reading order for screen readers and the audit.
        self.hits.sort_by(|a, b| {
            if rtl {
                b.0.x.total_cmp(&a.0.x)
            } else {
                a.0.x.total_cmp(&b.0.x)
            }
        });
    }

    /// The entry under `p`.
    pub fn hit(&self, p: (f32, f32)) -> Option<usize> {
        self.hits
            .iter()
            .find(|(r, _)| r.contains(p))
            .map(|(_, i)| *i)
    }

    /// The strip's screen-reader nodes: a list of the images shown, the open one selected
    /// (and focused while the strip has the keyboard).
    pub fn a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        base: u64,
        entries: &[PathBuf],
        current: Option<usize>,
        focused: bool,
        scale: f64,
    ) {
        use accesskit::{Action, Node, NodeId, Role};
        if self.hits.is_empty() {
            return;
        }
        let bounds = |r: Rect| accesskit::Rect {
            x0: f64::from(r.x) * scale,
            y0: f64::from(r.y) * scale,
            x1: f64::from(r.x + r.w) * scale,
            y1: f64::from(r.y + r.h) * scale,
        };
        let mut list = Node::new(Role::ListBox);
        list.set_label(t!("develop-filmstrip"));
        list.set_bounds(bounds(self.rect));
        let mut kids = Vec::new();
        for (r, i) in &self.hits {
            let id = NodeId(base + 1 + *i as u64);
            let mut n = Node::new(Role::ListBoxOption);
            let name = entries[*i]
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            n.set_label(name);
            n.set_selected(Some(*i) == current);
            n.add_action(Action::Click);
            n.set_bounds(bounds(*r));
            if focused && Some(*i) == current {
                tree.focus = id;
            }
            kids.push(id);
            tree.nodes.push((id, n));
        }
        list.set_children(kids);
        let win = tree.tree.as_ref().map_or(NodeId(1), |t| t.root);
        if let Some((_, w)) = tree.nodes.iter_mut().find(|(id, _)| *id == win) {
            w.push_child(NodeId(base));
        }
        tree.nodes.push((NodeId(base), list));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_scrolls_without_opening_and_reopening_recentres() {
        let mut s = Strip::default();
        // Down two lines: two images on, clamped at the ends.
        assert!(s.scroll(0.0, -2.0, Some(5), 10, false));
        assert_eq!(s.centre(5, 10), 7.0);
        assert!(s.scroll(0.0, -10.0, Some(5), 10, false));
        assert_eq!(s.centre(5, 10), 9.0);
        assert!(!s.scroll(0.0, -1.0, Some(5), 10, false));
        // Sideways: towards the reading direction's end is next; mirrored right to left.
        let mut s = Strip::default();
        s.scroll(-1.0, 0.0, Some(5), 10, false);
        assert_eq!(s.centre(5, 10), 6.0);
        let mut s = Strip::default();
        s.scroll(-1.0, 0.0, Some(5), 10, true);
        assert_eq!(s.centre(5, 10), 4.0);
        // Another image opened: centred on it again.
        assert_eq!(s.centre(8, 10), 8.0);
        // Nothing open, or nothing to scroll.
        assert!(!s.scroll(0.0, 1.0, None, 10, false));
        assert!(!s.scroll(0.0, 1.0, Some(0), 1, false));
    }
}
