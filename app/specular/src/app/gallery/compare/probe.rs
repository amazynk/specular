//! Compare's pixel probe (DESIGN.md §6.3): under the pointer, the same image pixel of the
//! first image and the one the keys act on (A and B), each with its position, its file value
//! (from a full-resolution overview, when there is one) and what is displayed (one texel of
//! its pane's render, read back from the GPU as the viewer's probe is), and the ΔE2000 of the
//! two displayed colours. The pointer's frame point is mapped through each image's place, so
//! it is the same pixel at any zoom, orientation and alignment. Alt holds it still.

use specular_gfx::Rect;
use specular_inspect::DisplayValue;
use specular_render::Place;
use specular_widgets::{PixelInfo, SourceState};

use super::super::quick::QuickPane;

/// Where one image is under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Hit {
    /// Its pane in this frame's plan.
    pub pane: usize,
    /// The texel of the pane's render (physical px), when inside it.
    pub texel: Option<(u32, u32)>,
    /// The stored pixel, when inside the image.
    pub stored: Option<(u32, u32)>,
}

/// A's and B's texels to read: their pane in the plan and the physical px in it.
pub(super) type Wanted = [Option<(usize, (u32, u32))>; 2];

/// The probe's state.
#[derive(Debug, Default)]
pub(in crate::app) struct Probe {
    pub on: bool,
    /// A's and B's pixels (None: the pointer is off that image).
    pub px: [Option<PixelInfo>; 2],
    /// The texels to read next (pane, physical px), once the readback is free.
    pub want: Option<Wanted>,
    /// The displayed texels last read (premultiplied linear).
    pub texels: [Option<[f32; 4]>; 2],
    /// Which of A and B the readback in flight reads.
    pub want_read: Option<[bool; 2]>,
    /// `--show`: put the probe at the middle of the first pane once it is laid out.
    pub show_at: bool,
}

impl Probe {
    /// The ΔE2000 of the two displayed colours, once both are in.
    pub(super) fn delta_e(&self) -> Option<f32> {
        match self.texels {
            [Some(a), Some(b)] => Some(specular_inspect::quality::pixel_delta_e(a, b)),
            _ => None,
        }
    }

    /// The readback of `texels` (in the order asked) came in.
    pub(super) fn read(&mut self, wanted: [bool; 2], texels: &[[f32; 4]]) {
        let mut it = texels.iter();
        for (k, w) in wanted.iter().enumerate() {
            if !w {
                self.texels[k] = None;
                continue;
            }
            let t = it.next().copied();
            self.texels[k] = t;
            if let (Some(p), Some(t)) = (self.px[k].as_mut(), t) {
                p.display = Some(DisplayValue::from_premultiplied(t, 0));
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.px = [None, None];
        self.want = None;
        self.texels = [None, None];
    }
}

/// The pane `geo` entry under logical point `p`, and images `targets`' hits under it: the
/// point mapped from the pane's image to the frame (`places`, by image) and back to each
/// target's pane. `scale`: physical px per logical px.
pub(super) fn locate(
    geo: &[(usize, QuickPane)],
    places: &[Place],
    p: (f32, f32),
    scale: f32,
    targets: [usize; 2],
) -> Option<[Option<Hit>; 2]> {
    let (k, under) = geo.iter().find(|(_, q)| q.placed().0.contains(p))?;
    let rect: Rect = under.placed().0;
    let phys = ((p.0 - rect.x) * scale, (p.1 - rect.y) * scale);
    let d = under.view().screen_to_display(phys);
    let place = |i: usize| places.get(i).copied().unwrap_or(Place::IDENTITY);
    let f = place(*k).to_frame(d);
    let hit = |i: usize| {
        let (pane, q) = geo.iter().enumerate().find(|(_, g)| g.0 == i)?;
        let view = q.1.view();
        let s = view.display_to_screen(place(i).from_frame(f));
        let (_, px) = q.1.placed();
        let texel = (s.0 >= 0.0 && s.1 >= 0.0 && s.0 < px.0 as f32 && s.1 < px.1 as f32)
            .then_some((s.0 as u32, s.1 as u32));
        let st = view.stored_from_screen().apply(s);
        let (w, h) = view.stored_size();
        let stored = (st.0 >= 0.0 && st.1 >= 0.0 && st.0 < w as f32 && st.1 < h as f32)
            .then_some((st.0 as u32, st.1 as u32));
        Some(Hit {
            pane,
            texel,
            stored,
        })
    };
    Some([hit(targets[0]), hit(targets[1])])
}

/// A new pixel info for `hit`'s stored pixel, keeping the displayed value of `old` until
/// the readback answers (so the panel does not jump).
pub(super) fn info(hit: &Hit, source: SourceState, old: Option<&PixelInfo>) -> Option<PixelInfo> {
    let (x, y) = hit.stored?;
    Some(PixelInfo {
        x,
        y,
        source,
        display: old.and_then(|o| o.display),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_core::Orientation;
    use std::path::PathBuf;

    fn pane(k: usize, stored: (u32, u32), o: Orientation, area: Rect, scale: f32) -> QuickPane {
        QuickPane::new(
            PathBuf::from(format!("/{k}.png")),
            k as u64 + 1,
            stored,
            o,
            area,
            scale,
        )
    }

    /// The pointer over one image finds the same image pixel in the other, at any zoom and
    /// orientation and through the alignment.
    #[test]
    fn the_probe_maps_to_the_same_image_pixel() {
        let area = Rect::new(0.0, 0.0, 400.0, 300.0);
        for scale in [1.0, 1.5] {
            for zoom in [1.0, 3.0] {
                // B: A turned on its side by its orientation (stored portrait, shown
                // landscape), at half A's size, scaled to A.
                let frame = (400.0, 300.0);
                let mut a = pane(0, (400, 300), Orientation::NORMAL, area, scale)
                    .framed(frame, area, scale);
                let mut b =
                    pane(1, (150, 200), Orientation(6), area, scale).framed(frame, area, scale);
                let places = vec![
                    Place::IDENTITY,
                    Place {
                        scale: 2.0,
                        ..Place::IDENTITY
                    },
                ];
                b.place = places[1];
                for p in [&mut a, &mut b] {
                    p.zoom = zoom;
                    p.at = (0.6, 0.4);
                }
                let geo = vec![(0, a), (1, b)];
                let ra = geo[0].1.placed().0;
                // A logical point in the pane, mapped to A's stored pixel, then B's.
                let pt = (ra.x + ra.w * 0.3, ra.y + ra.h * 0.7);
                let hits = locate(&geo, &places, pt, scale, [0, 1]).unwrap();
                let (ha, hb) = (hits[0].unwrap(), hits[1].unwrap());
                let sa = ha.stored.unwrap();
                let sb = hb.stored.unwrap();
                // B's display pixel is half A's; stored (orientation 6) x = display y,
                // y = height − 1 − display x.
                let (dax, day) = (sa.0 as f32 / 2.0, sa.1 as f32 / 2.0);
                assert!(
                    (sb.0 as f32 - day).abs() <= 1.0,
                    "{scale} {zoom}: {sa:?} {sb:?}"
                );
                assert!((sb.1 as f32 - (199.0 - dax)).abs() <= 1.0, "{sa:?} {sb:?}");
                // Both panes share the frame: the same texel.
                assert_eq!(ha.texel, hb.texel);
                assert_eq!((ha.pane, hb.pane), (0, 1));
            }
        }
        // Off every pane: nothing.
        let geo = vec![(0, pane(0, (40, 30), Orientation::NORMAL, area, 1.0))];
        assert!(locate(&geo, &[Place::IDENTITY], (-5.0, -5.0), 1.0, [0, 1]).is_none());
    }

    #[test]
    fn readbacks_fill_the_displayed_values() {
        let mut p = Probe {
            on: true,
            px: [
                Some(PixelInfo {
                    x: 1,
                    y: 2,
                    source: SourceState::Unavailable,
                    display: None,
                }),
                None,
            ],
            ..Probe::default()
        };
        assert_eq!(p.delta_e(), None);
        p.read([true, false], &[[0.5, 0.5, 0.5, 1.0]]);
        assert!(p.px[0].unwrap().display.is_some());
        assert_eq!(p.texels[1], None);
        p.read([true, true], &[[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]]);
        assert!((p.delta_e().unwrap() - 100.0).abs() < 1e-2);
        p.clear();
        assert_eq!(p.px, [None, None]);
    }
}
