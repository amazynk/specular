//! Grid assembly (`grid` derived items).
//!
//! Tiles of a chroma-subsampled colour grid are stitched into full-width YUV bands, one tile
//! row each, before conversion, so that bilinear chroma upsampling reads the neighbouring
//! tile's samples across every seam and the result equals converting the stitched image (as
//! libavif does), with no seams from clamping at tile edges. Vertical seams need nothing more
//! than the full-width band; for horizontal ones (4:2:0 only) each band carries a halo of two
//! luma rows and one chroma row above and below, copied from the neighbouring bands. A band
//! is converted once the next tile row has been decoded into the other of two band buffers,
//! so memory is two tile rows of YUV rather than the whole canvas, and each conversion still
//! runs rows in parallel across the full width.
//!
//! Grids whose tile size is odd along a subsampled axis (which MIAF forbids and libavif
//! rejects) cannot be stitched in chroma, so they, alpha, monochrome and 4:4:4 grids (which
//! have nothing to stitch) are converted tile by tile.

use crate::Error;
use crate::backend::{Cicp, Picture, Planes};
use crate::convert::{self, Conv, Dst, Region, Target};
use crate::heif::Grid;

/// Whether the tiles of a grid whose first tile is `p` are stitched (see the module docs).
pub(crate) fn stitches(grid: &Grid, p: &Planes<'_>, target: Target) -> bool {
    target == Target::Color
        && !p.mono
        && (p.ssx || p.ssy)
        && grid.rows * grid.cols > 1
        && (grid.cols == 1 || !p.ssx || p.width.is_multiple_of(2))
        && (grid.rows == 1 || !p.ssy || p.height.is_multiple_of(2))
}

/// Sample format shared by every tile.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Format {
    bits: u8,
    hbd: bool,
    ssx: bool,
    ssy: bool,
    cosited_x: bool,
    cosited_y: bool,
    mono: bool,
}

impl Format {
    fn of(p: &Planes<'_>) -> Format {
        Format {
            bits: p.bits,
            hbd: p.hbd,
            ssx: p.ssx,
            ssy: p.ssy,
            cosited_x: p.cosited_x,
            cosited_y: p.cosited_y,
            mono: p.mono,
        }
    }
}

/// Band geometry, in bytes and rows.
struct Geom {
    /// Canvas width (luma samples) and its chroma width.
    gw: usize,
    cw: usize,
    bps: usize,
    tile_w: usize,
    tile_h: usize,
    /// Halo rows above and below the core (luma: 2 for 4:2:0, else 0; chroma: half that).
    halo: usize,
    halo_c: usize,
    /// Chroma rows of a full tile.
    core_c: usize,
    ssx: usize,
    ssy: usize,
}

impl Geom {
    fn ystride(&self) -> usize {
        self.gw * self.bps
    }
    fn cstride(&self) -> usize {
        self.cw * self.bps
    }
}

/// One tile row of the canvas: luma rows `y0..y0 + h` plus `top`/`bot` halo rows in use.
struct Band {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
    y0: usize,
    h: usize,
    top: usize,
    bot: usize,
}

impl Band {
    fn new(g: &Geom) -> Band {
        let rows = g.tile_h + 2 * g.halo;
        let crows = g.core_c + 2 * g.halo_c;
        Band {
            y: vec![0; rows * g.ystride()],
            u: vec![0; crows * g.cstride()],
            v: vec![0; crows * g.cstride()],
            y0: 0,
            h: 0,
            top: 0,
            bot: 0,
        }
    }

    /// Chroma rows covering the `h` core rows.
    fn core_c(&self, g: &Geom) -> usize {
        (self.h + g.ssy) >> g.ssy
    }

    /// Copies the part of tile `p` at canvas column `x0` that lies on the canvas.
    fn put_tile(&mut self, g: &Geom, p: &Planes<'_>, x0: usize) -> Result<(), Error> {
        let bad = || Error::Decoder("decoded picture geometry is inconsistent".into());
        let w = g.tile_w.min(g.gw - x0);
        let copy = |dst: &mut [u8],
                    dstride: usize,
                    drow0: usize,
                    src: &[u8],
                    sstride: usize,
                    rows: usize,
                    dx: usize,
                    n: usize|
         -> Result<(), Error> {
            for r in 0..rows {
                let s = src
                    .get(r * sstride..r * sstride + n * g.bps)
                    .ok_or_else(bad)?;
                let d0 = (drow0 + r) * dstride + dx * g.bps;
                dst.get_mut(d0..d0 + n * g.bps)
                    .ok_or_else(bad)?
                    .copy_from_slice(s);
            }
            Ok(())
        };
        let ys = g.ystride();
        copy(
            &mut self.y,
            ys,
            g.halo,
            p.planes[0],
            p.strides[0],
            self.h,
            x0,
            w,
        )?;
        let (cx0, cx1) = (x0 >> g.ssx, (x0 + w + g.ssx) >> g.ssx);
        let crows = self.core_c(g);
        let cs = g.cstride();
        for k in 1..3 {
            let dst = if k == 1 { &mut self.u } else { &mut self.v };
            copy(
                dst,
                cs,
                g.halo_c,
                p.planes[k],
                p.strides[k],
                crows,
                cx0,
                cx1 - cx0,
            )?;
        }
        Ok(())
    }

    /// The band as a picture: core rows plus the halo rows in use.
    fn planes(&self, g: &Geom, f: Format, cicp: Cicp) -> Planes<'_> {
        let (ly, lc) = (g.halo - self.top, g.halo_c - (self.top >> 1));
        Planes {
            planes: [
                &self.y[ly * g.ystride()..],
                &self.u[lc * g.cstride()..],
                &self.v[lc * g.cstride()..],
            ],
            strides: [g.ystride(), g.cstride(), g.cstride()],
            width: g.gw as u32,
            height: (self.top + self.h + self.bot) as u32,
            bits: f.bits,
            hbd: f.hbd,
            ssx: f.ssx,
            ssy: f.ssy,
            cosited_x: f.cosited_x,
            cosited_y: f.cosited_y,
            mono: false,
            cicp,
        }
    }
}

/// Copies the halo rows between vertically adjacent bands `a` (above) and `b` (below).
fn link(g: &Geom, a: &mut Band, b: &mut Band) {
    if g.halo == 0 {
        return;
    }
    let (ys, cs) = (g.ystride(), g.cstride());
    // Below `a`: the first rows of `b`.
    a.bot = g.halo.min(b.h);
    let (src, dst) = (g.halo * ys, (g.halo + a.h) * ys);
    a.y[dst..dst + a.bot * ys].copy_from_slice(&b.y[src..src + a.bot * ys]);
    let (src, dst) = (g.halo_c * cs, (g.halo_c + a.core_c(g)) * cs);
    for (pa, pb) in [(&mut a.u, &b.u), (&mut a.v, &b.v)] {
        pa[dst..dst + g.halo_c * cs].copy_from_slice(&pb[src..src + g.halo_c * cs]);
    }
    // Above `b`: the last rows of `a` (a full tile, at least `halo` rows).
    b.top = g.halo;
    let src = (g.halo + a.h - g.halo) * ys;
    b.y[..g.halo * ys].copy_from_slice(&a.y[src..src + g.halo * ys]);
    let src = (g.halo_c + a.core_c(g) - g.halo_c) * cs;
    for (pa, pb) in [(&a.u, &mut b.u), (&a.v, &mut b.v)] {
        pb[..g.halo_c * cs].copy_from_slice(&pa[src..src + g.halo_c * cs]);
    }
}

/// Decodes the tiles of `grid` in order (tile 0 is `first`, the rest come from `decode`),
/// stitches them into bands and converts each band into `dst`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_stitched(
    grid: &Grid,
    first: Box<dyn Picture>,
    decode: &mut dyn FnMut(usize) -> Result<Box<dyn Picture>, Error>,
    conv_for: &dyn Fn(&Planes<'_>) -> Conv,
    scale: usize,
    dst: &mut Dst<'_>,
    parallel: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let (gw, gh) = (grid.width as usize, grid.height as usize);
    let (cols, rows) = (grid.cols as usize, grid.rows as usize);
    let (fmt, cicp, tile_w, tile_h) = {
        let p = first.planes();
        (Format::of(&p), p.cicp, p.width as usize, p.height as usize)
    };
    let (ssx, ssy) = (usize::from(fmt.ssx), usize::from(fmt.ssy));
    let halo = if fmt.ssy { 2 } else { 0 };
    // A band never holds more than the canvas height: a tall tile over a short canvas would
    // otherwise size each band buffer from the tile (up to 256 × the tile's area).
    let band_h = tile_h.min(gh);
    let g = Geom {
        gw,
        cw: (gw + ssx) >> ssx,
        bps: if fmt.hbd { 2 } else { 1 },
        tile_w,
        tile_h: band_h,
        halo,
        halo_c: halo >> 1,
        core_c: (band_h + ssy) >> ssy,
        ssx,
        ssy,
    };
    let mut first = Some(first);
    // `pending` waits for the band below it before conversion; `spare` is recycled.
    let mut pending: Option<Band> = None;
    let mut spare: Option<Band> = None;
    let convert_band = |band: &Band, dst: &mut Dst<'_>| -> Result<(), Error> {
        let p = band.planes(&g, fmt, cicp);
        let region = Region {
            x: 0,
            y: band.top,
            w: gw,
            h: band.h,
            scale,
        };
        let dy = dst.y;
        dst.y = dy + band.y0 / scale;
        let r = convert::convert(
            &p,
            &conv_for(&p),
            region,
            dst,
            Target::Color,
            parallel,
            cancel,
        );
        dst.y = dy;
        r
    };
    for r in 0..rows {
        let y0 = r * tile_h;
        if y0 >= gh {
            break;
        }
        let mut band = spare.take().unwrap_or_else(|| Band::new(&g));
        band.y0 = y0;
        band.h = tile_h.min(gh - y0);
        band.top = 0;
        band.bot = 0;
        for c in 0..cols {
            let x0 = c * tile_w;
            if x0 >= gw {
                break;
            }
            if cancel() {
                return Err(Error::Cancelled);
            }
            let pic = match first.take() {
                Some(p) => p,
                None => decode(r * cols + c)?,
            };
            let p = pic.planes();
            if p.width as usize != tile_w || p.height as usize != tile_h {
                return Err(Error::Malformed("grid tiles differ in size".into()));
            }
            if Format::of(&p) != fmt {
                return Err(Error::Malformed("grid tiles differ in format".into()));
            }
            band.put_tile(&g, &p, x0)?;
        }
        if let Some(mut above) = pending.take() {
            link(&g, &mut above, &mut band);
            convert_band(&above, dst)?;
            spare = Some(above);
        }
        pending = Some(band);
    }
    if let Some(band) = pending {
        convert_band(&band, dst)?;
    }
    Ok(())
}
