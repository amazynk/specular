//! Layered items (LASzip compressor 3, item version 3) for point formats 6–10: each field
//! group is a separate arithmetic stream, so the streams of fields not shown (flags, scan
//! angle, user data, point source, GPS time, NIR, extra bytes, wave packets) are skipped
//! without decoding. Models are kept per scanner channel (four contexts), a context being
//! set up from the previous point the first time its channel appears in a chunk.

use super::arith::{Decoder, IntCoder, Median5, Model};
use super::tables::{RETURN_MAP_6, return_level_8};
use super::v2::Rgb;
use super::{Item, Layout};
use crate::las::Rec;
use crate::{Result, malformed};

/// The fields of the last point a context remembers.
#[derive(Clone, Copy, Default)]
struct Last {
    x: i32,
    y: i32,
    z: i32,
    intensity: u16,
    /// Return number and number of returns (4 bits each).
    r: u32,
    n: u32,
    class: u8,
    gps_change: bool,
}

impl Last {
    fn of(raw: &[u8]) -> Last {
        let i = |o: usize| i32::from_le_bytes([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]);
        Last {
            x: i(0),
            y: i(4),
            z: i(8),
            intensity: u16::from_le_bytes([raw[12], raw[13]]),
            r: u32::from(raw[14] & 0x0F),
            n: u32::from(raw[14] >> 4),
            class: raw[16],
            gps_change: false,
        }
    }

    fn put(&self, rec: &mut Rec) {
        rec.x = self.x;
        rec.y = self.y;
        rec.z = self.z;
        rec.intensity = self.intensity;
        rec.class = self.class;
        rec.returns = (self.r as u8) | ((self.n as u8) << 4);
    }
}

/// A scanner channel's models.
struct Ctx {
    changed: Vec<Model>,
    scanner: Model,
    n_models: Vec<Option<Model>>,
    r_models: Vec<Option<Model>>,
    r_same: Model,
    dx: IntCoder,
    dy: IntCoder,
    z: IntCoder,
    class: Vec<Option<Model>>,
    intensity: IntCoder,
    med_x: [Median5; 12],
    med_y: [Median5; 12],
    last_z: [i32; 8],
    last_intensity: [u16; 8],
    last: Last,
}

impl Ctx {
    fn new(last: Last) -> Box<Ctx> {
        Box::new(Ctx {
            changed: (0..8).map(|_| Model::new(128)).collect(),
            scanner: Model::new(3),
            n_models: (0..16).map(|_| None).collect(),
            r_models: (0..16).map(|_| None).collect(),
            r_same: Model::new(13),
            dx: IntCoder::new(32, 2),
            dy: IntCoder::new(32, 22),
            z: IntCoder::new(32, 20),
            class: (0..64).map(|_| None).collect(),
            intensity: IntCoder::new(16, 4),
            med_x: [Median5::default(); 12],
            med_y: [Median5::default(); 12],
            last_z: [last.z; 8],
            last_intensity: [last.intensity; 8],
            last: Last {
                gps_change: false,
                ..last
            },
        })
    }
}

/// The RGB item's per-channel models and last colours, kept as LASzip and laz-rs keep them:
/// the item sees the Point14 item's new scanner channel only on the point that switches to
/// it (every other point it is handed context 0), and switching to a channel it has seen
/// before keeps predicting from, and updating, the previous channel's last colour for that
/// one point.
struct RgbCtxs {
    ctx: [Option<Rgb>; 4],
    cur: usize,
}

impl RgbCtxs {
    fn new(channel: usize, first: [u16; 3]) -> RgbCtxs {
        let mut ctx = [None, None, None, None];
        ctx[channel] = Some(Rgb::new(&first.map(u16::to_le_bytes).concat()));
        RgbCtxs { ctx, cur: channel }
    }

    /// The colour of the next point in `context`, decoded from `d` (or repeated when the
    /// layer is empty).
    fn next(&mut self, context: usize, d: Option<&mut Decoder>) -> [u16; 3] {
        let mut slot = self.cur;
        if self.cur != context {
            self.cur = context;
            if self.ctx[context].is_none() {
                let last = self.ctx[slot].as_ref().map_or([0; 3], |r| r.last);
                self.ctx[context] = Some(Rgb::new(&last.map(u16::to_le_bytes).concat()));
                slot = context;
            }
        }
        let last = self.ctx[slot].as_ref().map_or([0; 3], |r| r.last);
        let (Some(d), Some(m)) = (d, self.ctx[context].as_mut()) else {
            return last;
        };
        let own = m.last;
        m.last = last;
        m.read(d);
        let v = m.last;
        if slot != context {
            m.last = own;
            if let Some(o) = self.ctx[slot].as_mut() {
                o.last = v;
            }
        }
        v
    }
}

/// A layer's decoder, or `None` when it is empty (the field is the same for every point of
/// the chunk) or not wanted.
type Layer<'a> = Option<Decoder<'a>>;

/// Decodes a chunk of `count` points into `out`.
pub(crate) fn decode(
    layout: &Layout,
    chunk: &[u8],
    count: usize,
    out: &mut Vec<Rec>,
) -> Result<()> {
    let len = layout.record_len;
    if chunk.len() < len + 4 {
        return malformed("LAZ chunk shorter than its first point");
    }
    let raw = &chunk[..len];
    let mut first = Rec::default();
    let mut last = None;
    let mut rgb0 = None;
    let mut channel = 0usize;
    for &(item, off) in &layout.items {
        let b = &raw[off..];
        match item {
            Item::Point14 => {
                let l = Last::of(b);
                l.put(&mut first);
                channel = usize::from((b[15] >> 4) & 3);
                last = Some(l);
            }
            Item::Rgb14 | Item::RgbNir14 => {
                let c = [0, 2, 4].map(|o| u16::from_le_bytes([b[o], b[o + 1]]));
                first.rgb = c;
                rgb0 = Some(c);
            }
            _ => {}
        }
    }
    let Some(last) = last else {
        return malformed("layered LAZ without a Point14 item");
    };
    out.push(first);
    if count <= 1 {
        return Ok(());
    }

    // The sizes of all layers, then their bytes, in item order.
    let mut pos = len + 4;
    let mut sizes = Vec::new();
    for &(item, _) in &layout.items {
        let n = match item {
            Item::Point14 => 9,
            Item::Rgb14 | Item::Wave14 => 1,
            Item::RgbNir14 => 2,
            Item::Byte14(n) => usize::from(n),
            _ => return malformed("pointwise item in a layered chunk"),
        };
        for _ in 0..n {
            let Some(s) = chunk.get(pos..pos + 4) else {
                return malformed("LAZ chunk's layer sizes truncated");
            };
            sizes.push(u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize);
            pos += 4;
        }
    }
    let mut layers: Vec<&[u8]> = Vec::with_capacity(sizes.len());
    for &s in &sizes {
        let Some(b) = chunk.get(pos..pos.saturating_add(s)) else {
            return malformed("LAZ chunk's layers truncated");
        };
        layers.push(b);
        pos += s;
    }
    let layer = |i: usize| -> Layer<'_> {
        layers
            .get(i)
            .filter(|b| !b.is_empty())
            .map(|b| Decoder::new(b))
    };
    // Point14's layers: XY (with returns), Z, class, flags, intensity, scan angle, user data,
    // point source, GPS time. The RGB layer follows them.
    let Some(mut xy) = layer(0) else {
        return malformed("LAZ chunk without its XY layer");
    };
    let mut lz = layer(1);
    let mut lclass = layer(2);
    let mut lint = layer(4);
    let rgb_layer = layout
        .items
        .iter()
        .any(|(i, _)| matches!(i, Item::Rgb14 | Item::RgbNir14));
    let mut lrgb = if rgb_layer { layer(9) } else { None };

    let mut ctxs: [Option<Box<Ctx>>; 4] = [None, None, None, None];
    ctxs[channel] = Some(Ctx::new(last));
    let mut rgb = rgb0.map(|c| RgbCtxs::new(channel, c));
    let mut cur = channel;
    let mut rec = first;
    for _ in 1..count {
        let ctx = ctxs[cur].as_mut().expect("current context exists");
        let l = ctx.last;
        let lpr =
            usize::from(l.r == 1) + 2 * usize::from(l.r >= l.n) + 4 * usize::from(l.gps_change);
        let changed = xy.symbol(&mut ctx.changed[lpr]);
        let mut switched = false;
        if changed & (1 << 6) != 0 {
            let diff = xy.symbol(&mut ctx.scanner) as usize;
            let next = (cur + diff + 1) % 4;
            if ctxs[next].is_none() {
                let from = ctxs[cur].as_ref().expect("current context exists");
                ctxs[next] = Some(Ctx::new(from.last));
            }
            cur = next;
            switched = true;
        }
        let ctx = ctxs[cur].as_mut().expect("current context exists");
        let gps_change = changed & (1 << 4) != 0;
        let gps = usize::from(gps_change);
        let p = &mut ctx.last;
        let n = if changed & (1 << 2) != 0 {
            let m = ctx.n_models[p.n as usize].get_or_insert_with(|| Model::new(16));
            xy.symbol(m)
        } else {
            p.n
        };
        p.n = n;
        let r = match changed & 3 {
            0 => p.r,
            1 => (p.r + 1) % 16,
            2 => (p.r + 15) % 16,
            _ => {
                if gps_change {
                    let m = ctx.r_models[p.r as usize].get_or_insert_with(|| Model::new(16));
                    xy.symbol(m)
                } else {
                    (p.r + xy.symbol(&mut ctx.r_same) + 2) % 16
                }
            }
        };
        p.r = r;
        let m = usize::from(RETURN_MAP_6[n as usize & 15][r as usize & 15]);
        let lvl = return_level_8(n, r);
        let cpr = 2 * u32::from(r == 1) + u32::from(r >= n);
        let one = u32::from(n == 1);
        let mi = (m << 1) | gps;

        let diff = ctx.dx.decode(&mut xy, ctx.med_x[mi].get(), one);
        p.x = p.x.wrapping_add(diff);
        ctx.med_x[mi].add(diff);
        let k = ctx.dx.k;
        let diff = ctx.dy.decode(
            &mut xy,
            ctx.med_y[mi].get(),
            one + if k < 20 { k & !1 } else { 20 },
        );
        p.y = p.y.wrapping_add(diff);
        ctx.med_y[mi].add(diff);

        if let Some(dz) = lz.as_mut() {
            let k = (ctx.dx.k + ctx.dy.k) / 2;
            p.z = ctx
                .z
                .decode(dz, ctx.last_z[lvl], one + if k < 18 { k & !1 } else { 18 });
            ctx.last_z[lvl] = p.z;
        }
        if let Some(dc) = lclass.as_mut() {
            let ccc = ((usize::from(p.class) & 0x1F) << 1) + usize::from(cpr == 3);
            let model = ctx.class[ccc].get_or_insert_with(|| Model::new(256));
            p.class = dc.symbol(model) as u8;
        }
        if let Some(di) = lint.as_mut() {
            let slot = ((cpr as usize) << 1) | gps;
            let v = ctx
                .intensity
                .decode(di, i32::from(ctx.last_intensity[slot]), cpr) as u16;
            ctx.last_intensity[slot] = v;
            p.intensity = v;
        }
        p.put(&mut rec);
        p.gps_change = gps_change;
        if let Some(r) = rgb.as_mut() {
            rec.rgb = r.next(if switched { cur } else { 0 }, lrgb.as_mut());
        }
        out.push(rec);
    }
    let over = [
        Some(&xy),
        lz.as_ref(),
        lclass.as_ref(),
        lint.as_ref(),
        lrgb.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|d| d.overrun)
    .max()
    .unwrap_or(0);
    if over > super::OVERRUN_SLACK {
        return malformed("LAZ layer ended early");
    }
    Ok(())
}
