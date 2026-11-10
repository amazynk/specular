//! Filled marks drawn from their owners' own path data: logos whose shape must not be redrawn
//! as strokes. A mark is rasterised once per size into the glyph atlas and drawn tinted, like
//! a glyph, so it takes the colour of the icons around it.
//!
//! [`DrawList::mark`](crate::DrawList::mark) records where a mark goes; the UI renderer has
//! the text system place it in the atlas ([`crate::Text::place_marks`]) before the frame is
//! uploaded.

/// A third-party mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    /// Discord's symbol ("Clyde"), black version, unchanged. Source and trademark terms:
    /// `mark.PROVENANCE.md`.
    Discord,
}

/// Discord's symbol: the `d` of `Discord-Symbol-Black.svg` in Discord's brand kit, verbatim
/// (viewBox `0 0 126.644 96`, non-zero fill). See `mark.PROVENANCE.md`.
const DISCORD_PATH: &str = "M81.15,0c-1.2376,2.1973-2.3489,4.4704-3.3591,6.794-9.5975-1.4396-19.3718-1.4396-28.9945,0-.985-2.3236-2.1216-4.5967-3.3591-6.794-9.0166,1.5407-17.8059,4.2431-26.1405,8.0568C2.779,32.5304-1.6914,56.3725.5312,79.8863c9.6732,7.1476,20.5083,12.603,32.0505,16.0884,2.6014-3.4854,4.8998-7.1981,6.8698-11.0623-3.738-1.3891-7.3497-3.1318-10.8098-5.1523.9092-.6567,1.7932-1.3386,2.6519-1.9953,20.281,9.547,43.7696,9.547,64.0758,0,.8587.7072,1.7427,1.3891,2.6519,1.9953-3.4601,2.0457-7.0718,3.7632-10.835,5.1776,1.97,3.8642,4.2683,7.5769,6.8698,11.0623,11.5419-3.4854,22.3769-8.9156,32.0509-16.0631,2.626-27.2771-4.496-50.9172-18.817-71.8548C98.9811,4.2684,90.1918,1.5659,81.1752.0505l-.0252-.0505ZM42.2802,65.4144c-6.2383,0-11.4159-5.6575-11.4159-12.6535s4.9755-12.6788,11.3907-12.6788,11.5169,5.708,11.4159,12.6788c-.101,6.9708-5.026,12.6535-11.3907,12.6535ZM84.3576,65.4144c-6.2637,0-11.3907-5.6575-11.3907-12.6535s4.9755-12.6788,11.3907-12.6788,11.4917,5.708,11.3906,12.6788c-.101,6.9708-5.026,12.6535-11.3906,12.6535Z";

impl Mark {
    /// The SVG path data, in the units of [`Mark::view_box`].
    pub fn path(self) -> &'static str {
        match self {
            Mark::Discord => DISCORD_PATH,
        }
    }

    /// The width and height of the mark's viewBox (its origin is 0, 0).
    pub fn view_box(self) -> (f32, f32) {
        match self {
            Mark::Discord => (126.644, 96.0),
        }
    }

    /// The mark's coverage at `k` physical px per path unit, `w`×`h` px, top left at the
    /// viewBox origin: one byte per pixel, rows top to bottom.
    pub fn raster(self, k: f32, w: u32, h: u32) -> Vec<u8> {
        use swash::zeno::{Fill, Format, Mask, Transform};
        let (data, _) = Mask::new(self.path())
            .style(Fill::NonZero)
            .format(Format::Alpha)
            .transform(Some(Transform::scale(k, k)))
            .size(w, h)
            .render();
        data
    }
}

/// A mark recorded in a draw list: its primitive (a glyph quad whose atlas position is still
/// to be filled in) and its scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkRef {
    /// Index of the primitive in [`crate::DrawList::prims`].
    pub prim: u32,
    pub mark: Mark,
    /// Physical px per path unit.
    pub k: f32,
    /// The quad's size in px.
    pub w: u32,
    pub h: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mark rasterises as the brand kit draws it: a solid head with two eye holes.
    #[test]
    fn discord_has_its_eyes() {
        let m = Mark::Discord;
        let (vw, vh) = m.view_box();
        let (w, h) = (vw.ceil() as u32, vh.ceil() as u32);
        let px = m.raster(1.0, w, h);
        assert_eq!(px.len(), (w * h) as usize);
        let at = |x: f32, y: f32| px[(y as u32 * w + x as u32) as usize];
        // The eyes' centres (42.3, 52.8) and (84.4, 52.8) are holes; the forehead between and
        // above them is filled; outside the head (the top corners) is empty.
        assert_eq!(at(42.3, 52.8), 0);
        assert_eq!(at(84.4, 52.8), 0);
        assert_eq!(at(63.3, 30.0), 255);
        assert_eq!(at(63.3, 60.0), 255);
        assert_eq!(at(2.0, 2.0), 0);
        assert_eq!(at(124.0, 2.0), 0);
        // Symmetric about the middle, as drawn.
        let left: u32 = (0..h)
            .flat_map(|y| (0..w / 2).map(move |x| (x, y)))
            .map(|(x, y)| px[(y * w + x) as usize] as u32)
            .sum();
        let total: u32 = px.iter().map(|&c| c as u32).sum();
        let ratio = left as f32 / total as f32;
        assert!((ratio - 0.5).abs() < 0.01, "left share {ratio}");
    }
}
