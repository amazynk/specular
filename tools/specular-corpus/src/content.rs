//! Procedural, photo-like image content.
//!
//! Flat or purely synthetic gradients compress far better than camera images and make decoders look
//! unrealistically fast, so the scene mixes: a sky gradient with soft clouds, a ridged mountain
//! horizon, multi-octave textured ground (foliage-like detail down to 2 px), buildings with window
//! grids (sharp edges), signs with bitmap text (very high frequency), vignetting and sensor noise.
//!
//! Every pixel is a pure function of `(x, y, seed)`, so rows can be rendered in parallel bands and
//! streamed straight into `cjpeg` without ever holding the whole image in memory.

#[inline]
fn hash3(x: i32, y: i32, s: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ s.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    h
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

#[inline]
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    smooth(((x - a) / (b - a)).clamp(0.0, 1.0))
}

#[inline]
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Value noise in [-1, 1].
#[inline]
fn vnoise(x: f32, y: f32, s: u32) -> f32 {
    let xf = x.floor();
    let yf = y.floor();
    let fx = smooth(x - xf);
    let fy = smooth(y - yf);
    let (ix, iy) = (xf as i32, yf as i32);
    let a = unit(hash3(ix, iy, s));
    let b = unit(hash3(ix + 1, iy, s));
    let c = unit(hash3(ix, iy + 1, s));
    let d = unit(hash3(ix + 1, iy + 1, s));
    let top = a + (b - a) * fx;
    let bot = c + (d - c) * fx;
    (top + (bot - top) * fy) * 2.0 - 1.0
}

/// Fractal Brownian motion: `octaves` layers starting at `period` pixels, halving each time.
#[inline]
fn fbm(x: f32, y: f32, period: f32, octaves: u32, gain: f32, s: u32) -> f32 {
    let mut p = period;
    let mut amp = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        // Rotate/offset each octave a little to hide the lattice.
        sum += vnoise(
            x / p + i as f32 * 17.3,
            y / p - i as f32 * 9.1,
            s.wrapping_add(i),
        ) * amp;
        norm += amp;
        amp *= gain;
        p *= 0.5;
        if p < 1.0 {
            break;
        }
    }
    sum / norm
}

/// Small deterministic RNG for scene layout.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    fn f(&mut self) -> f32 {
        unit(self.next())
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

struct Building {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    color: [f32; 3],
    cell_w: f32,
    cell_h: f32,
    win_w: f32,
    win_h: f32,
    seed: u32,
}

struct Sign {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    dot: f32,
    cols: usize,
    text: Vec<u8>,
    fg: [f32; 3],
    bg: [f32; 3],
}

pub struct Scene {
    pub w: u32,
    pub h: u32,
    seed: u32,
    horizon: Vec<f32>,
    buildings: Vec<Building>,
    signs: Vec<Sign>,
    /// Sensor noise standard deviation (8-bit levels) for luma.
    pub noise: f32,
    sky_top: [f32; 3],
    sky_hz: [f32; 3],
}

const GLYPHS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

impl Scene {
    pub fn new(w: u32, h: u32, seed: u32) -> Scene {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ ((seed as u64) << 17) ^ seed as u64);
        for _ in 0..4 {
            rng.next();
        }
        let wf = w as f32;
        let hf = h as f32;
        // Ridged mountain horizon; for very wide panoramas keep the ridge period tied to height.
        let base = rng.range(0.32, 0.42);
        let ridge_period = (wf / 3.0).min(hf * 2.5);
        let horizon = (0..w)
            .map(|x| {
                let r = fbm(x as f32, 0.5, ridge_period, 10, 0.55, seed ^ 0xabc);
                let ridge = 1.0 - r.abs();
                hf * (base - 0.16 * ridge * ridge
                    + 0.03 * fbm(x as f32, 3.5, ridge_period / 8.0, 6, 0.5, seed ^ 0x77))
            })
            .collect();
        // Buildings: roughly one per 600 px of width, at least a few.
        let n_buildings = ((wf / 600.0) as usize).clamp(5, 160);
        let mut buildings = Vec::new();
        for i in 0..n_buildings {
            let bw = rng.range(0.4, 1.4) * hf * 0.09;
            let cx = (i as f32 + rng.f()) / n_buildings as f32 * wf;
            let y1 = hf * rng.range(0.80, 1.02);
            let bh = hf * rng.range(0.12, 0.38);
            let tone = rng.range(0.35, 0.95);
            let tint = [
                rng.range(0.8, 1.1),
                rng.range(0.8, 1.05),
                rng.range(0.75, 1.0),
            ];
            let color = [
                190.0 * tone * tint[0],
                180.0 * tone * tint[1],
                165.0 * tone * tint[2],
            ];
            let cell_w = (hf / rng.range(140.0, 420.0)).max(4.0);
            let cell_h = cell_w * rng.range(1.1, 1.8);
            buildings.push(Building {
                x0: cx - bw / 2.0,
                x1: cx + bw / 2.0,
                y0: y1 - bh,
                y1,
                color,
                cell_w,
                cell_h,
                win_w: cell_w * rng.range(0.45, 0.75),
                win_h: cell_h * rng.range(0.45, 0.7),
                seed: rng.next(),
            });
        }
        // Signs with text (high-frequency content like labels, posters, licence plates).
        let n_signs = ((wf * hf / 1.5e6) as usize).clamp(4, 400);
        let mut signs = Vec::new();
        for _ in 0..n_signs {
            let dot = (hf / rng.range(900.0, 3000.0)).max(1.0).round();
            let cols = 8 + (rng.next() % 40) as usize;
            let rows = 1 + (rng.next() % 6) as usize;
            let x0 = rng.f() * wf;
            let y0 = hf * rng.range(0.45, 0.95);
            let text: Vec<u8> = (0..cols * rows)
                .map(|_| {
                    if rng.next().is_multiple_of(6) {
                        b' '
                    } else {
                        GLYPHS[(rng.next() % GLYPHS.len() as u32) as usize]
                    }
                })
                .collect();
            let light = rng.next().is_multiple_of(2);
            let bg = if light {
                [235.0, 232.0, 220.0]
            } else {
                [30.0 + rng.f() * 60.0, 40.0, 70.0 + rng.f() * 80.0]
            };
            let fg = if light {
                [20.0 + rng.f() * 120.0, 20.0, 25.0]
            } else {
                [240.0, 235.0, 200.0 * rng.f() + 40.0]
            };
            signs.push(Sign {
                x0,
                y0,
                x1: x0 + (cols as f32 * 6.0 + 2.0) * dot,
                y1: y0 + (rows as f32 * 9.0 + 1.0) * dot,
                dot,
                cols,
                text,
                fg,
                bg,
            });
        }
        let sky_top = [
            rng.range(20.0, 50.0),
            rng.range(70.0, 100.0),
            rng.range(150.0, 190.0),
        ];
        let sky_hz = [
            rng.range(180.0, 215.0),
            rng.range(200.0, 220.0),
            rng.range(215.0, 235.0),
        ];
        Scene {
            w,
            h,
            seed,
            horizon,
            buildings,
            signs,
            noise: 4.5,
            sky_top,
            sky_hz,
        }
    }

    /// Noise-free, unclamped colour at (x, y) in 8-bit units (fractional).
    fn base_pixel(&self, x: f32, y: f32, row_b: &[&Building], row_s: &[&Sign]) -> [f32; 3] {
        let s = self.seed;
        let wf = self.w as f32;
        let hf = self.h as f32;
        let xi = (x as usize).min(self.w as usize - 1);
        let hz = self.horizon[xi];
        let mut c = if y < hz {
            let t = (y / (hf * 0.42)).clamp(0.0, 1.0).powf(0.7);
            let base = lerp3(self.sky_top, self.sky_hz, t);
            let cl = fbm(x, y * 2.2, hf / 2.5, 9, 0.55, s ^ 0x1111);
            let cover = smoothstep(0.0, 0.5, cl);
            let shade = 0.85 + 0.15 * fbm(x, y, hf / 20.0, 6, 0.6, s ^ 0x2222);
            lerp3(
                base,
                [228.0 * shade, 230.0 * shade, 236.0 * shade],
                cover * 0.9,
            )
        } else {
            let depth = ((y - hz) / (hf - hz).max(1.0)).clamp(0.0, 1.0);
            let sel = fbm(x, y, hf / 3.0, 4, 0.5, s ^ 0x3333);
            let green = [62.0, 92.0, 40.0];
            let brown = [118.0, 92.0, 64.0];
            let rock = [128.0, 126.0, 120.0];
            let pal = if sel < 0.0 {
                lerp3(green, brown, smoothstep(-0.5, 0.0, sel))
            } else {
                lerp3(brown, rock, smoothstep(0.0, 0.5, sel))
            };
            // Coarse structure plus foliage-like detail; finer detail dominates in the distance.
            let coarse = fbm(x, y, 384.0, 5, 0.55, s ^ 0x4444);
            let detail = fbm(x, y, 48.0, 5, 0.75, s ^ 0x4545);
            let grain = vnoise(x / 1.7, y / 1.7, s ^ 0x5555) * 0.6
                + vnoise(x / 3.3, y / 3.3, s ^ 0x6666) * 0.4;
            let fine = 0.35 + 0.25 * (1.0 - depth);
            // Sharp-edged clumps (leaves, pebbles, shadows) give realistic mid-frequency edges.
            let clump = fbm(x, y, 20.0, 3, 0.6, s ^ 0x4646);
            let shadow = smoothstep(0.12, 0.2, clump) - 0.6 * smoothstep(-0.2, -0.12, -clump);
            let lum = (1.0 + 0.4 * coarse + 0.3 * detail + fine * grain - 0.3 * shadow).max(0.05);
            let c = [pal[0] * lum, pal[1] * lum, pal[2] * lum];
            // Aerial haze near the horizon.
            let haze = (1.0 - depth).powi(6) * 0.55;
            lerp3(c, self.sky_hz, haze)
        };
        for b in row_b {
            if x >= b.x0 && x < b.x1 {
                let lx = x - b.x0;
                let ly = y - b.y0;
                let cx = (lx / b.cell_w).floor();
                let cy = (ly / b.cell_h).floor();
                let fx = lx - cx * b.cell_w;
                let fy = ly - cy * b.cell_h;
                let grime = 1.0 + 0.18 * fbm(x, y, 48.0, 5, 0.6, b.seed);
                c = [b.color[0] * grime, b.color[1] * grime, b.color[2] * grime];
                let mx = (b.cell_w - b.win_w) * 0.5;
                let my = (b.cell_h - b.win_h) * 0.5;
                if fx > mx && fx < mx + b.win_w && fy > my && fy < my + b.win_h && ly > b.cell_h {
                    let hsh = hash3(cx as i32, cy as i32, b.seed);
                    let refl = 0.8 + 0.2 * vnoise(x / 6.0, y / 6.0, b.seed ^ 9);
                    c = if hsh.is_multiple_of(5) {
                        [235.0 * refl, 205.0 * refl, 120.0 * refl]
                    } else {
                        [
                            40.0 * refl + 30.0 * (hsh % 3) as f32,
                            55.0 * refl,
                            70.0 * refl,
                        ]
                    };
                }
                break;
            }
        }
        for sg in row_s {
            if x >= sg.x0 && x < sg.x1 {
                c = sg.bg;
                let gx = ((x - sg.x0) / sg.dot - 1.0).floor() as i32;
                let gy = ((y - sg.y0) / sg.dot - 1.0).floor() as i32;
                if gx >= 0 && gy >= 0 {
                    let (col, px) = ((gx / 6) as usize, gx % 6);
                    let (row, py) = ((gy / 9) as usize, gy % 9);
                    if col < sg.cols && px < 5 && py < 7 {
                        let ch = sg.text.get(row * sg.cols + col).copied().unwrap_or(b' ');
                        if glyph_bit(ch, px as u32, py as u32) {
                            c = sg.fg;
                        }
                    }
                }
                break;
            }
        }
        // Vignette.
        let dx = x / wf - 0.5;
        let dy = y / hf - 0.5;
        let v = 1.0 - 0.35 * (dx * dx + dy * dy).min(1.0);
        [c[0] * v, c[1] * v, c[2] * v]
    }

    fn row_objects(&self, y: f32) -> (Vec<&Building>, Vec<&Sign>) {
        let b = self
            .buildings
            .iter()
            .filter(|b| y >= b.y0 && y < b.y1)
            .collect();
        let s = self
            .signs
            .iter()
            .filter(|s| y >= s.y0 && y < s.y1)
            .collect();
        (b, s)
    }

    /// Render one 8-bit RGB row (with sensor noise) into `out` (len = 3 * w).
    pub fn render_row(&self, y: u32, out: &mut [u8]) {
        let yf = y as f32;
        let (rb, rs) = self.row_objects(yf);
        let n = self.noise * 2.449;
        for (x, px) in out.chunks_exact_mut(3).enumerate() {
            let c = self.base_pixel(x as f32, yf, &rb, &rs);
            let h1 = hash3(x as i32, y as i32, self.seed ^ 0xdead);
            let h2 = hash3(x as i32, y as i32, self.seed ^ 0xbeef);
            let ln = (unit(h1) + unit(h2) - 1.0) * n;
            let cn = (unit(h1 ^ h2.rotate_left(7)) - 0.5) * n * 0.6;
            px[0] = (c[0] + ln + cn).clamp(0.0, 255.0) as u8;
            px[1] = (c[1] + ln).clamp(0.0, 255.0) as u8;
            px[2] = (c[2] + ln - cn).clamp(0.0, 255.0) as u8;
        }
    }

    /// Render a high-precision row (`levels` = 4095 for 12-bit) with sensor noise.
    pub fn render_row_hp(&self, y: u32, levels: f32, out: &mut [[f32; 3]]) {
        let yf = y as f32;
        let (rb, rs) = self.row_objects(yf);
        let k = levels / 255.0;
        for (x, px) in out.iter_mut().enumerate() {
            let c = self.base_pixel(x as f32, yf, &rb, &rs);
            let h1 = hash3(x as i32, y as i32, self.seed ^ 0xdead);
            let ln = (unit(h1) - 0.5) * self.noise * 3.0;
            for i in 0..3 {
                px[i] = ((c[i] + ln) * k).clamp(0.0, levels);
            }
        }
    }

    /// Box-filtered thumbnail (e.g. 160x120 for an EXIF IFD1 thumbnail).
    pub fn thumbnail(&self, tw: u32, th: u32) -> Vec<u8> {
        let mut out = vec![0u8; (tw * th * 3) as usize];
        let ss = 4;
        for ty in 0..th {
            for tx in 0..tw {
                let mut acc = [0.0f32; 3];
                for sy in 0..ss {
                    let y = ((ty * ss + sy) as f32 + 0.5) / (th * ss) as f32 * self.h as f32;
                    let (rb, rs) = self.row_objects(y);
                    for sx in 0..ss {
                        let x = ((tx * ss + sx) as f32 + 0.5) / (tw * ss) as f32 * self.w as f32;
                        let c = self.base_pixel(x, y, &rb, &rs);
                        for i in 0..3 {
                            acc[i] += c[i];
                        }
                    }
                }
                let o = ((ty * tw + tx) * 3) as usize;
                for i in 0..3 {
                    out[o + i] = (acc[i] / (ss * ss) as f32).clamp(0.0, 255.0) as u8;
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// 5x7 bitmap font (rows top to bottom, bit 4 = leftmost column).

fn glyph(ch: u8) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        b'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        b'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        b'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        b'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        b'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        b'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        b'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        b'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        b'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        b'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        b'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        b'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        b'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        b'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        b'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        b'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        b'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        b'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        b'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        b'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        b'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        b'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        b'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        b'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        b'0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        b'1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        b'3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        b'4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        b'5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        b'6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        b'7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        b'9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        b'-' => [0, 0, 0, 0x1F, 0, 0, 0],
        b'.' => [0, 0, 0, 0, 0, 0x0C, 0x0C],
        b':' => [0, 0x0C, 0x0C, 0, 0x0C, 0x0C, 0],
        b'=' => [0, 0, 0x1F, 0, 0x1F, 0, 0],
        b'/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        _ => [0; 7],
    }
}

#[inline]
fn glyph_bit(ch: u8, px: u32, py: u32) -> bool {
    glyph(ch)[py as usize] & (0x10 >> px) != 0
}

/// Simple RGB canvas used for the orientation charts.
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Canvas {
    pub fn new(w: u32, h: u32, bg: [u8; 3]) -> Canvas {
        let mut px = Vec::with_capacity((w * h * 3) as usize);
        for _ in 0..w * h {
            px.extend_from_slice(&bg);
        }
        Canvas { w, h, px }
    }
    pub fn rect(&mut self, x0: u32, y0: u32, w: u32, h: u32, c: [u8; 3]) {
        for y in y0..(y0 + h).min(self.h) {
            for x in x0..(x0 + w).min(self.w) {
                let o = ((y * self.w + x) * 3) as usize;
                self.px[o..o + 3].copy_from_slice(&c);
            }
        }
    }
    pub fn text(&mut self, x0: u32, y0: u32, dot: u32, s: &str, c: [u8; 3]) {
        for (i, ch) in s.bytes().enumerate() {
            let gx = x0 + i as u32 * 6 * dot;
            for py in 0..7 {
                for pxl in 0..5 {
                    if glyph_bit(ch, pxl, py) {
                        self.rect(gx + pxl * dot, y0 + py * dot, dot, dot, c);
                    }
                }
            }
        }
    }
}

/// Upright orientation chart: a large asymmetric "F", coloured corners and a caption.
pub fn orientation_chart(o: u8) -> Canvas {
    let (w, h) = (800, 600);
    let mut c = Canvas::new(w, h, [236, 236, 230]);
    // Gentle gradient + texture so the JPEG isn't trivially flat.
    for y in 0..h {
        for x in 0..w {
            let n = vnoise(x as f32 / 9.0, y as f32 / 9.0, 42) * 6.0 + (y as f32 / h as f32) * 18.0;
            let o3 = ((y * w + x) * 3) as usize;
            for i in 0..3 {
                c.px[o3 + i] = (c.px[o3 + i] as f32 - n).clamp(0.0, 255.0) as u8;
            }
        }
    }
    c.rect(0, 0, 90, 90, [220, 30, 30]); // top-left red
    c.rect(w - 90, 0, 90, 90, [30, 170, 40]); // top-right green
    c.rect(0, h - 90, 90, 90, [30, 60, 220]); // bottom-left blue
    c.rect(w - 90, h - 90, 90, 90, [240, 200, 20]); // bottom-right yellow
    c.text(120, 24, 6, &format!("ORIENTATION {o}"), [20, 20, 20]);
    c.text(300, 130, 52, "F", [10, 10, 10]);
    c.text(110, 520, 3, "TOP-LEFT RED  TOP-RIGHT GREEN", [60, 60, 60]);
    c.text(110, 550, 3, "UPRIGHT F = CORRECT", [60, 60, 60]);
    c
}

/// Convert an upright (display) canvas into the stored pixel layout for EXIF orientation `o`,
/// so that a viewer that honours the tag displays the upright chart.
pub fn to_stored(disp: &Canvas, o: u8) -> Canvas {
    let (dw, dh) = (disp.w, disp.h);
    let swap = o >= 5;
    let (sw, sh) = if swap { (dh, dw) } else { (dw, dh) };
    let mut s = Canvas::new(sw, sh, [0, 0, 0]);
    for y in 0..dh {
        for x in 0..dw {
            // Display pixel (x, y) comes from stored pixel (sx, sy).
            let (sx, sy) = match o {
                1 => (x, y),
                2 => (dw - 1 - x, y),
                3 => (dw - 1 - x, dh - 1 - y),
                4 => (x, dh - 1 - y),
                5 => (y, x),
                6 => (y, dw - 1 - x),
                7 => (dh - 1 - y, dw - 1 - x),
                8 => (dh - 1 - y, x),
                _ => unreachable!(),
            };
            let di = ((y * dw + x) * 3) as usize;
            let si = ((sy * sw + sx) * 3) as usize;
            s.px[si..si + 3].copy_from_slice(&disp.px[di..di + 3]);
        }
    }
    s
}
