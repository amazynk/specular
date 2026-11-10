//! QOI ("Quite OK Image"), <https://qoiformat.org/qoi-specification.pdf>.
//!
//! The format is strictly sequential, so a decode always runs from the first pixel; scaled and
//! region decodes stream rows into the box filter and stop after the last row they need.

use crate::scale::{Plan, RowScaler, fill_grey};
use crate::{Cancel, Decode, Error, Image, Info, Options, PixelFormat, Result, u32be};

pub const MAGIC: &[u8; 4] = b"qoif";
const HEADER: usize = 14;

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

#[derive(Clone, Debug)]
pub struct Qoi {
    info: Info,
}

impl Qoi {
    pub fn parse(data: &[u8]) -> Result<Qoi> {
        if data.len() < HEADER {
            return Err(Error::Truncated("QOI header".into()));
        }
        if !probe(data) {
            return Err(Error::Malformed("not a QOI file".into()));
        }
        let w = u32be(data, 4).unwrap_or(0);
        let h = u32be(data, 8).unwrap_or(0);
        let channels = data[12];
        let colorspace = data[13];
        if !matches!(channels, 3 | 4) || colorspace > 1 {
            return Err(Error::Malformed(format!(
                "channels {channels}, colour space {colorspace}"
            )));
        }
        crate::check_dims(w, h)?;
        Ok(Qoi {
            info: Info {
                width: w,
                height: h,
                has_alpha: channels == 4,
                alpha: crate::AlphaKind::Straight,
                gray: false,
                model: crate::Model::Rgb,
                bit_depth: 8,
                coding: format!(
                    "QOI {}{}",
                    if channels == 4 { "RGBA" } else { "RGB" },
                    if colorspace == 1 { ", linear" } else { "" }
                ),
                random_access: false,
            },
        })
    }
}

/// Decoder state between calls of [`State::fill`].
struct State {
    p: usize,
    px: [u8; 4],
    index: [[u8; 4]; 64],
    run: u32,
    truncated: bool,
}

#[inline(always)]
fn hash(p: [u8; 4]) -> usize {
    (p[0] as usize * 3 + p[1] as usize * 5 + p[2] as usize * 7 + p[3] as usize * 11) & 63
}

impl State {
    fn new() -> State {
        State {
            p: HEADER,
            px: [0, 0, 0, 255],
            index: [[0; 4]; 64],
            run: 0,
            truncated: false,
        }
    }

    /// Decode the next `out.len() / 4` pixels. After the data runs out the rest is grey.
    fn fill(&mut self, data: &[u8], out: &mut [u8]) {
        let mut px = self.px;
        let mut p = self.p;
        let mut run = self.run;
        let mut chunks = out.chunks_exact_mut(4);
        // The 8-byte end marker can't start a pixel op that matters; stopping at the end of
        // the data (not at the marker) keeps files with a missing marker decodable.
        let n = data.len();
        loop {
            if run > 0 {
                // Repeat the previous pixel.
                let Some(o) = chunks.next() else { break };
                o.copy_from_slice(&px);
                run -= 1;
                continue;
            }
            let Some(o) = chunks.next() else { break };
            if p >= n {
                self.truncated = true;
                fill_grey(o, 4);
                for o in chunks.by_ref() {
                    fill_grey(o, 4);
                }
                break;
            }
            let b = data[p];
            p += 1;
            match b {
                0xFE => {
                    if p + 3 > n {
                        p = n;
                        self.truncated = true;
                        fill_grey(o, 4);
                        continue;
                    }
                    px[0] = data[p];
                    px[1] = data[p + 1];
                    px[2] = data[p + 2];
                    p += 3;
                }
                0xFF => {
                    if p + 4 > n {
                        p = n;
                        self.truncated = true;
                        fill_grey(o, 4);
                        continue;
                    }
                    px.copy_from_slice(&data[p..p + 4]);
                    p += 4;
                }
                _ => match b >> 6 {
                    0 => px = self.index[b as usize],
                    1 => {
                        px[0] = px[0].wrapping_add(((b >> 4) & 3).wrapping_sub(2));
                        px[1] = px[1].wrapping_add(((b >> 2) & 3).wrapping_sub(2));
                        px[2] = px[2].wrapping_add((b & 3).wrapping_sub(2));
                    }
                    2 => {
                        let Some(&b2) = data.get(p) else {
                            p = n;
                            self.truncated = true;
                            fill_grey(o, 4);
                            continue;
                        };
                        p += 1;
                        let vg = (b & 0x3f).wrapping_sub(32);
                        px[0] = px[0].wrapping_add(vg.wrapping_sub(8).wrapping_add(b2 >> 4));
                        px[1] = px[1].wrapping_add(vg);
                        px[2] = px[2].wrapping_add(vg.wrapping_sub(8).wrapping_add(b2 & 0x0f));
                    }
                    _ => run = (b & 0x3f) as u32,
                },
            }
            self.index[hash(px)] = px;
            o.copy_from_slice(&px);
        }
        self.px = px;
        self.p = p;
        self.run = run;
    }
}

impl Decode for Qoi {
    fn info(&self) -> &Info {
        &self.info
    }

    fn decode(&self, data: &[u8], opts: &Options, cancel: Cancel) -> Result<Image> {
        let (w, h) = (self.info.width, self.info.height);
        let plan = Plan::new(w, h, opts)?;
        // A run op gives 62 pixels.
        crate::proven(data.len(), w, h, 62)?;
        let mut st = State::new();
        let row = w as usize * 4;
        let data_out = if plan.f == 1 && plan.rw == w && plan.rh == h {
            // Whole image: decode in place, 64-row bands between cancellation checks.
            let mut out = vec![0u8; row * h as usize];
            for band in out.chunks_mut(row * 64) {
                if cancel() {
                    return Err(Error::Cancelled);
                }
                st.fill(data, band);
            }
            out
        } else {
            let mut sc = RowScaler::new(plan, 4, !self.info.has_alpha, true);
            let mut buf = vec![0u8; row];
            let mut y = 0u32;
            while !sc.done() {
                if y.is_multiple_of(64) && cancel() {
                    return Err(Error::Cancelled);
                }
                st.fill(data, &mut buf);
                sc.push(&buf);
                y += 1;
            }
            sc.finish()
        };
        Ok(Image {
            width: plan.rw,
            height: plan.rh,
            format: PixelFormat::Rgba8,
            data: data_out,
            truncated: st.truncated,
        })
    }
}
