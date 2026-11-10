//! Frames built in the test by a small encoder that follows the arrangement algorithm of
//! SMPTE 314M 5.6 literally (three passes), with blocks long enough to overflow into the
//! macro block and the segment, then decoded and compared with the expected samples.

use crate::bits::Bits;
use crate::tables::{idct, inverse_weights, scans, step};
use crate::video::{AREA_BITS, AREA_START, block_places};
use crate::vlc::CODES;
use crate::{Chroma, Layout, SEQ, segment_places};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// One block: header fields, the code words, and the coefficients they give.
struct Blk {
    bits: Bits,
    coef: [f32; 64],
    mode248: bool,
}

fn code_of(run: u8, amp: u8) -> Option<(u32, usize)> {
    CODES
        .iter()
        .find(|c| c.1 == run && c.2 == amp)
        .map(|c| (u32::from_str_radix(c.0, 2).unwrap(), c.0.len()))
}

fn make_block(rng: &mut Rng, qno: u8, budget: usize) -> Blk {
    let mode248 = rng.below(3) == 0;
    let class = rng.below(4) as u8;
    let dc = rng.below(400) as i32 - 200;
    let mut b = Bits::default();
    b.push((dc & 0x1FF) as u32, 9);
    b.push(u32::from(mode248), 1);
    b.push(u32::from(class), 2);
    let scan = &scans()[usize::from(mode248)];
    let iw = &inverse_weights()[usize::from(mode248)];
    let mut coef = [0f32; 64];
    coef[0] = dc as f32 * iw[0];
    let mut pos = 1;
    while b.len() < budget {
        let (run, amp) = if rng.below(8) == 0 {
            (0u8, 23 + rng.below(233) as u8)
        } else {
            let c = CODES[rng.below(CODES.len() as u32) as usize];
            (c.1, c.2)
        };
        let at = pos + usize::from(run);
        if at >= 64 {
            break;
        }
        let neg = rng.below(2) == 1;
        if amp >= 23 && run == 0 && code_of(0, amp).is_none() {
            b.push(0b1111111, 7);
            b.push(u32::from(amp), 8);
            b.push(u32::from(neg), 1);
        } else {
            let (bits, len) = code_of(run, amp).unwrap();
            b.push(bits, len);
            if amp != 0 {
                b.push(u32::from(neg), 1);
            }
        }
        if amp != 0 {
            let r = usize::from(scan.raster[at]);
            let q = i32::from(amp) * step(class, scan.area[at], qno);
            coef[r] = (if neg { -q } else { q }) as f32 * iw[r];
        }
        pos = at + 1;
    }
    b.push(0b0110, 4);
    Blk {
        bits: b,
        coef,
        mode248,
    }
}

fn set_bit(buf: &mut [u8], p: usize, v: bool) {
    if v {
        buf[p / 8] |= 0x80 >> (p % 8);
    } else {
        buf[p / 8] &= !(0x80 >> (p % 8));
    }
}

/// Free space of one area: next free bit and end.
#[derive(Clone, Copy)]
struct Area {
    at: usize,
    end: usize,
}

/// Distributes `data` from bit `from` into `areas` in order; returns where it stopped.
fn distribute(buf: &mut [u8], areas: &mut [Area], data: &Bits, mut from: usize) -> usize {
    for a in areas.iter_mut() {
        while a.at < a.end && from < data.len() {
            set_bit(buf, a.at, data.read(from, 1) != 0);
            a.at += 1;
            from += 1;
        }
    }
    from
}

/// Builds a 525/60 4:1:1 frame (consumer DV) and the samples it should decode to.
fn build(seed: u64, fill: usize) -> (Vec<u8>, [Vec<u8>; 3]) {
    let l = Layout {
        chroma: Chroma::C411,
        seqs: 10,
        channels: 1,
        width: 720,
        height: 480,
    };
    let mut rng = Rng(seed);
    let mut frame = vec![0xFFu8; 10 * SEQ];
    let mut planes = [
        vec![128u8; 720 * 480],
        vec![128u8; 180 * 480],
        vec![128u8; 180 * 480],
    ];
    let widths = [720usize, 180, 180];
    for s in 0..10 {
        let seq = &mut frame[s * SEQ..(s + 1) * SEQ];
        seq[0] = 0x1F;
        seq[1] = (s as u8) << 4 | 7;
        seq[2] = 0;
        seq[3] = 0x3F;
        seq[4] = 0xF8;
        for b in 1..3 {
            seq[b * 80] = 0x3F;
        }
        for b in 3..6 {
            seq[b * 80] = 0x56;
        }
        for k in 0..27u32 {
            let places = segment_places(&l, s, 0, k);
            // Five macro blocks: header byte, six blocks each.
            let mut blocks: Vec<Vec<Blk>> = Vec::new();
            let mut qnos = [0u8; 5];
            for q in 0..5 {
                qnos[q] = rng.below(16) as u8;
                let budget = |r: &mut Rng| match r.below(4) {
                    0 => 20,
                    1 => fill,
                    _ => 60,
                };
                blocks.push(
                    (0..6)
                        .map(|_| {
                            let b = budget(&mut rng);
                            make_block(&mut rng, qnos[q], b)
                        })
                        .collect(),
                );
            }
            // Keep the segment within its capacity (5 × 608 bits) so nothing is dropped.
            loop {
                let total: usize = blocks.iter().flatten().map(|b| b.bits.len()).sum();
                if total <= 5 * 608 {
                    break;
                }
                let (q, i) = (rng.below(5) as usize, rng.below(6) as usize);
                let b = budget_reduce(&mut rng, qnos[q]);
                blocks[q][i] = b;
            }
            // Pass 1, 2, 3.
            let dif = |q: usize| {
                let b = k as usize * 5 + q;
                s * SEQ + (7 + (b / 15) * 16 + b % 15) * 80
            };
            let mut areas: Vec<Vec<Area>> = Vec::new();
            let mut mr: Vec<Bits> = Vec::new();
            for q in 0..5 {
                let base = dif(q);
                frame[base] = 0x96;
                frame[base + 1] = (s as u8) << 4 | 7;
                frame[base + 2] = (k as usize * 5 + q) as u8;
                frame[base + 3] = qnos[q];
                let mut ar: Vec<Area> = (0..6)
                    .map(|a| Area {
                        at: (base + AREA_START[a]) * 8,
                        end: (base + AREA_START[a]) * 8 + AREA_BITS[a],
                    })
                    .collect();
                let mut rest = Bits::default();
                for (a, blk) in blocks[q].iter().enumerate() {
                    let used = distribute(&mut frame, &mut ar[a..a + 1], &blk.bits, 0);
                    rest.push_bits(&blk.bits, used);
                }
                areas.push(ar);
                mr.push(rest);
            }
            let mut vr = Bits::default();
            for q in 0..5 {
                let used = distribute(&mut frame, &mut areas[q], &mr[q], 0);
                vr.push_bits(&mr[q], used);
            }
            let mut from = 0;
            for ar in areas.iter_mut() {
                from = distribute(&mut frame, ar, &vr, from);
            }
            assert_eq!(from, vr.len(), "segment over capacity");
            // Expected samples.
            for q in 0..5 {
                let (wh, _) = block_places(Chroma::C411, places[q]);
                for (bi, blk) in blocks[q].iter().enumerate() {
                    let (plane, x, y, folded) = wh[bi];
                    let mut px = [0u8; 64];
                    idct(&blk.coef, blk.mode248, &mut px);
                    let w = widths[usize::from(plane)];
                    for yy in 0..8usize {
                        for xx in 0..8usize {
                            let (dx, dy) = if folded && xx >= 4 {
                                (xx - 4, yy + 8)
                            } else {
                                (xx, yy)
                            };
                            planes[usize::from(plane)][(y as usize + dy) * w + x as usize + dx] =
                                px[yy * 8 + xx];
                        }
                    }
                }
            }
        }
    }
    (frame, planes)
}

fn budget_reduce(rng: &mut Rng, qno: u8) -> Blk {
    make_block(rng, qno, 12)
}

#[test]
fn three_pass_distribution_round_trips() {
    for (seed, fill) in [(1u64, 200usize), (7, 400), (99, 120)] {
        let (frame, want) = build(seed, fill);
        let info = crate::probe(&frame).unwrap();
        assert_eq!(
            (info.width, info.height, info.chroma),
            (720, 480, Chroma::C411)
        );
        let p = crate::decode(&frame).unwrap();
        assert_eq!(p.damaged_blocks, 0);
        for (i, (a, b)) in p.planes.iter().zip(&want).enumerate() {
            let off = a.iter().zip(b).filter(|(x, y)| x != y).count();
            assert_eq!(off, 0, "plane {i}, seed {seed}");
        }
    }
}

#[test]
fn damaged_frames_do_not_panic() {
    let (frame, _) = build(5, 300);
    let mut rng = Rng(42);
    for _ in 0..20 {
        let mut f = frame.clone();
        for _ in 0..2000 {
            let i = rng.below(f.len() as u32) as usize;
            f[i] = rng.next() as u8;
        }
        if let Ok(p) = crate::decode(&f) {
            assert_eq!(p.planes[0].len(), 720 * 480);
        }
        let _ = crate::audio(&f);
        let _ = crate::decode(&f[..f.len() / 3]);
    }
}
