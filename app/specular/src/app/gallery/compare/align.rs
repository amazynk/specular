//! How compare lines up images of different sizes (DESIGN.md §6.3): each image gets a
//! [`Place`] in one frame, which the overlay, the split, the difference view, the metrics
//! and the probe all share.
//!
//! - **Top-left**: pixel for pixel from the top-left corner, in a frame of the largest width
//!   and height (the first behaviour, kept as the default).
//! - **Centred**: pixel for pixel from the centre, in the same frame.
//! - **Scaled to A**: every further image resized to fit the first (an export against its
//!   original), centred; the frame is the first image.
//! - **Auto**: the second image (the one the keys act on) aligned onto the first by a
//!   similarity found from the pictures (`specular_markup::align::auto` on grey copies of
//!   at most [`GREY_SIDE`] px, off the UI thread). It is kept only when it fits well; else
//!   compare falls back on top-left and says so.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use specular_core::{CancelToken, Orientation, Raster};
use specular_markup::align::{Grey, Similarity};
use specular_render::Place;

/// The grey copies' longest side.
pub(super) const GREY_SIDE: usize = 2048;
/// What a found alignment must meet: matched patches, their spread (grey px) and the scale.
const MIN_MATCHES: usize = 4;
const MAX_RMS: f64 = 3.0;
const SCALE_RANGE: (f64, f64) = (0.25, 4.0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(in crate::app) enum AlignMode {
    #[default]
    TopLeft,
    Centre,
    ScaleToA,
    Auto,
}

impl AlignMode {
    /// The next in the `a` cycle.
    pub(super) fn next(self) -> AlignMode {
        match self {
            AlignMode::TopLeft => AlignMode::Centre,
            AlignMode::Centre => AlignMode::ScaleToA,
            AlignMode::ScaleToA => AlignMode::Auto,
            AlignMode::Auto => AlignMode::TopLeft,
        }
    }
}

/// A similarity found for image `b`: its place in the first image's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Found {
    pub b: usize,
    pub place: Place,
}

/// The automatic alignment's state.
#[derive(Debug, Default)]
pub(super) enum Auto {
    #[default]
    None,
    /// Being found for image `b`.
    Running {
        b: usize,
        rx: Receiver<Option<Place>>,
        token: CancelToken,
    },
    Found(Found),
}

impl Auto {
    /// Stops a search in flight.
    pub(super) fn cancel(&mut self) {
        if let Auto::Running { token, .. } = self {
            token.cancel();
        }
        *self = Auto::None;
    }

    pub(super) fn found(&self, b: usize) -> Option<Place> {
        match self {
            Auto::Found(f) if f.b == b => Some(f.place),
            _ => None,
        }
    }

    pub(super) fn running(&self) -> bool {
        matches!(self, Auto::Running { .. })
    }
}

/// The frame and every image's place in it under `mode`; `b` is the image the automatic
/// alignment is for and `found` its place once found (until then it is scaled to A).
pub(super) fn places(
    mode: AlignMode,
    sizes: &[Option<(f32, f32)>],
    b: usize,
    found: Option<Place>,
) -> ((f32, f32), Vec<Place>) {
    let largest = super::frame_of(sizes);
    let first = sizes.first().copied().flatten();
    let centred = |frame: (f32, f32), s: (f32, f32), k: f32| Place {
        scale: k,
        rotation: 0.0,
        offset: ((frame.0 - s.0 * k) * 0.5, (frame.1 - s.1 * k) * 0.5),
    };
    match (mode, first) {
        (AlignMode::TopLeft, _) | (_, None) => (largest, vec![Place::IDENTITY; sizes.len()]),
        (AlignMode::Centre, _) => (
            largest,
            sizes
                .iter()
                .map(|s| s.map_or(Place::IDENTITY, |s| centred(largest, s, 1.0)))
                .collect(),
        ),
        (AlignMode::ScaleToA | AlignMode::Auto, Some(a)) => (
            a,
            sizes
                .iter()
                .enumerate()
                .map(|(k, s)| match (k, s) {
                    (0, _) | (_, None) => Place::IDENTITY,
                    (k, _) if k == b && mode == AlignMode::Auto && found.is_some() => {
                        found.unwrap_or(Place::IDENTITY)
                    }
                    (_, Some(s)) => {
                        let k = (a.0 / s.0.max(1.0)).min(a.1 / s.1.max(1.0));
                        centred(a, *s, k)
                    }
                })
                .collect(),
        ),
    }
}

/// The frame rectangle both `a` and `b` (display sizes, as placed) cover: their overlap.
pub(super) fn overlap(
    a: (f32, f32),
    pa: &Place,
    b: (f32, f32),
    pb: &Place,
) -> Option<(f32, f32, f32, f32)> {
    let (ra, rb) = (pa.inner_rect(a), pb.inner_rect(b));
    let r = (
        ra.0.max(rb.0),
        ra.1.max(rb.1),
        ra.2.min(rb.2),
        ra.3.min(rb.3),
    );
    (r.2 - r.0 >= 0.5 && r.3 - r.1 >= 0.5).then_some(r)
}

/// A raster (stored orientation) as a grey copy in display orientation of at most
/// [`GREY_SIDE`] px.
pub(super) fn grey_of(r: &Raster, orient: Orientation) -> Grey {
    let rgba = crate::clipboard::to_rgba(r);
    let (w, h) = (r.width as usize, r.height as usize);
    let g = Grey::from_rgba(w, h, &rgba, w * 4);
    let (g, _) = g.reduce(GREY_SIDE);
    oriented(&g, orient)
}

/// `g` (stored orientation) turned and flipped into display orientation.
fn oriented(g: &Grey, o: Orientation) -> Grey {
    if o == Orientation::NORMAL {
        return g.clone();
    }
    let (w, h) = (g.w as f32, g.h as f32);
    let inv = specular_render::view::display_from_stored(o, w, h).invert();
    let (dw, dh) = if o.swaps_axes() {
        (g.h, g.w)
    } else {
        (g.w, g.h)
    };
    let mut px = vec![255u8; dw * dh];
    for y in 0..dh {
        for x in 0..dw {
            let s = inv.apply((x as f32 + 0.5, y as f32 + 0.5));
            let (sx, sy) = (s.0.floor() as isize, s.1.floor() as isize);
            if sx >= 0 && sy >= 0 && (sx as usize) < g.w && (sy as usize) < g.h {
                px[y * dw + x] = g.px[sy as usize * g.w + sx as usize];
            }
        }
    }
    Grey::new(dw, dh, px)
}

/// The place of image B (display size `b`) in image A's frame (display size `a`) from the
/// similarity `t` found between their grey copies (`ga` and `gb` px wide), if it is one to
/// trust.
pub(super) fn place_of(
    t: &specular_markup::align::Aligned,
    a: (f32, f32),
    ga: usize,
    b: (f32, f32),
    gb: usize,
) -> Option<Place> {
    let s = t.t.scale();
    let fits = t.matches >= MIN_MATCHES
        && t.rms.is_finite()
        && t.rms <= MAX_RMS
        && (SCALE_RANGE.0..=SCALE_RANGE.1).contains(&s);
    if !fits {
        return None;
    }
    // grey A = ka · display A, grey B = kb · display B; frame = display A.
    let ka = ga as f64 / f64::from(a.0.max(1.0));
    let kb = gb as f64 / f64::from(b.0.max(1.0));
    let k = kb / ka;
    let sim = Similarity {
        a: t.t.a * k,
        b: t.t.b * k,
        tx: t.t.tx / ka,
        ty: t.t.ty / ka,
    };
    Some(Place {
        scale: sim.scale() as f32,
        rotation: sim.b.atan2(sim.a) as f32,
        offset: (sim.tx as f32, sim.ty as f32),
    })
}

/// Finds image B's place on image A from their overviews (on a worker).
pub(super) fn find(
    a: (Arc<Raster>, Orientation, (f32, f32)),
    b: (Arc<Raster>, Orientation, (f32, f32)),
    cancel: &CancelToken,
) -> Option<Place> {
    let ga = grey_of(&a.0, a.1);
    if cancel.is_cancelled() {
        return None;
    }
    let gb = grey_of(&b.0, b.1);
    if cancel.is_cancelled() {
        return None;
    }
    let t = specular_markup::align::auto(&ga, &gb, None)?;
    place_of(&t, a.2, ga.w, b.2, gb.w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_place_images_in_one_frame() {
        let sizes = [Some((400.0, 300.0)), Some((200.0, 150.0)), None];
        let (f, p) = places(AlignMode::TopLeft, &sizes, 1, None);
        assert_eq!(f, (400.0, 300.0));
        assert!(p.iter().all(|p| *p == Place::IDENTITY));
        // Centred: the smaller in the middle, pixel for pixel.
        let (f, p) = places(AlignMode::Centre, &sizes, 1, None);
        assert_eq!(f, (400.0, 300.0));
        assert_eq!(p[0], Place::IDENTITY);
        assert_eq!(p[1], Place::shifted((100.0, 75.0)));
        // Scaled to A: the export at twice its size, over the whole first image.
        let (f, p) = places(AlignMode::ScaleToA, &sizes, 1, None);
        assert_eq!(f, (400.0, 300.0));
        assert_eq!(p[1].scale, 2.0);
        assert_eq!(p[1].offset, (0.0, 0.0));
        let o = overlap((400.0, 300.0), &p[0], (200.0, 150.0), &p[1]).unwrap();
        assert_eq!(o, (0.0, 0.0, 400.0, 300.0));
        // Another aspect: fitted inside, centred, compared over what it covers.
        let sizes = [Some((400.0, 300.0)), Some((300.0, 300.0))];
        let (_, p) = places(AlignMode::ScaleToA, &sizes, 1, None);
        assert_eq!((p[1].scale, p[1].offset), (1.0, (50.0, 0.0)));
        let o = overlap((400.0, 300.0), &p[0], (300.0, 300.0), &p[1]).unwrap();
        assert_eq!(o, (50.0, 0.0, 350.0, 300.0));
        // Auto: scaled to A until found, then the found place.
        let found = Place {
            scale: 1.01,
            rotation: 0.01,
            offset: (3.0, -2.0),
        };
        let (_, p) = places(AlignMode::Auto, &sizes, 1, None);
        assert_eq!(p[1].offset, (50.0, 0.0));
        let (_, p) = places(AlignMode::Auto, &sizes, 1, Some(found));
        assert_eq!(p[1], found);
        assert_eq!(AlignMode::Auto.next(), AlignMode::TopLeft);
    }

    #[test]
    fn top_left_overlap_is_the_smaller_size() {
        let o = overlap(
            (4000.0, 3000.0),
            &Place::IDENTITY,
            (3000.0, 4000.0),
            &Place::IDENTITY,
        );
        assert_eq!(o, Some((0.0, 0.0, 3000.0, 3000.0)));
        // Nothing in common.
        let far = Place::shifted((5000.0, 0.0));
        assert_eq!(
            overlap((10.0, 10.0), &Place::IDENTITY, (10.0, 10.0), &far),
            None
        );
    }

    #[test]
    fn found_similarities_become_places_in_display_units() {
        let t = specular_markup::align::Aligned {
            // B's grey px onto A's: turned 0.4°, 1.002× and shifted.
            t: Similarity::new(1.002, 0.4f64.to_radians(), 20.0, -10.0),
            rms: 0.5,
            matches: 30,
        };
        // A shown 4000 px wide from a 2000 px grey copy, B 2000 px wide from 2000.
        let p = place_of(&t, (4000.0, 3000.0), 2000, (2000.0, 1500.0), 2000).unwrap();
        assert!((p.scale - 2.004).abs() < 1e-4, "{}", p.scale);
        assert!((p.rotation.to_degrees() - 0.4).abs() < 1e-4);
        assert_eq!(p.offset, (40.0, -20.0));
        // A poor fit, or one with a silly scale, is not trusted.
        let poor = specular_markup::align::Aligned { rms: 9.0, ..t };
        assert!(place_of(&poor, (1.0, 1.0), 1, (1.0, 1.0), 1).is_none());
        let few = specular_markup::align::Aligned { matches: 2, ..t };
        assert!(place_of(&few, (1.0, 1.0), 1, (1.0, 1.0), 1).is_none());
        let nan = specular_markup::align::Aligned { rms: f64::NAN, ..t };
        assert!(place_of(&nan, (1.0, 1.0), 1, (1.0, 1.0), 1).is_none());
    }

    #[test]
    fn grey_copies_follow_the_orientation() {
        // A 4 × 2 raster, dark at its stored left edge; turned 90° clockwise (6) the dark
        // column becomes the top row.
        let g = Grey::new(4, 2, vec![0, 255, 255, 255, 0, 255, 255, 255]);
        let o = oriented(&g, Orientation(6));
        assert_eq!((o.w, o.h), (2, 4));
        assert_eq!(&o.px[..2], &[0, 0]);
        assert!(o.px[2..].iter().all(|&v| v == 255));
        assert_eq!(oriented(&g, Orientation::NORMAL), g);
    }
}
