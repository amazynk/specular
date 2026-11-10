//! Monkey's Audio's prediction (file versions 3950 and later): up to three cascaded
//! sign-LMS ("NN") filters by compression level, then a fourth-order adaptive predictor
//! over the channel's own past and a fifth-order one over the other channel, then a scaled
//! first-order filter. Below 32 bits the arithmetic is 32-bit (wrapping) with 16-bit filter
//! taps; 32-bit audio uses 64-bit sums with 32-bit taps. Both are reproduced exactly,
//! including where the reference tests bit 31 of a 64-bit value for its sign.

/// The predictor's integer type: `i32`, or `i64` for 32-bit audio.
pub(crate) trait Int: Copy + Default + PartialOrd {
    const WIDE: bool;
    fn from_i64(v: i64) -> Self;
    fn i64(self) -> i64;
    fn add(self, o: Self) -> Self;
    fn sub(self, o: Self) -> Self;
    fn mul(self, o: Self) -> Self;
    fn shr(self, n: u32) -> Self;
    fn div(self, n: i64) -> Self;
    fn abs(self) -> Self;
    /// Bit 31 (the sign of a 32-bit value).
    fn bit31(self) -> bool {
        (self.i64() >> 31) & 1 == 1
    }
}

impl Int for i32 {
    const WIDE: bool = false;
    fn from_i64(v: i64) -> i32 {
        v as i32
    }
    fn i64(self) -> i64 {
        i64::from(self)
    }
    fn add(self, o: i32) -> i32 {
        self.wrapping_add(o)
    }
    fn sub(self, o: i32) -> i32 {
        self.wrapping_sub(o)
    }
    fn mul(self, o: i32) -> i32 {
        self.wrapping_mul(o)
    }
    fn shr(self, n: u32) -> i32 {
        self >> n
    }
    fn div(self, n: i64) -> i32 {
        self.wrapping_div(n as i32)
    }
    fn abs(self) -> i32 {
        self.wrapping_abs()
    }
}

impl Int for i64 {
    const WIDE: bool = true;
    fn from_i64(v: i64) -> i64 {
        v
    }
    fn i64(self) -> i64 {
        self
    }
    fn add(self, o: i64) -> i64 {
        self.wrapping_add(o)
    }
    fn sub(self, o: i64) -> i64 {
        self.wrapping_sub(o)
    }
    fn mul(self, o: i64) -> i64 {
        self.wrapping_mul(o)
    }
    fn shr(self, n: u32) -> i64 {
        self >> n
    }
    fn div(self, n: i64) -> i64 {
        self.wrapping_div(n)
    }
    fn abs(self) -> i64 {
        self.wrapping_abs()
    }
}

/// A filter tap: `i16` (below 32 bits) or `i32`.
pub(crate) trait Tap: Copy + Default {
    fn from_i32(v: i32) -> Self;
    /// The dot product as the reference accumulates it.
    fn dot(a: &[Self], b: &[Self]) -> i64;
    fn add(self, o: Self) -> Self;
    fn sub(self, o: Self) -> Self;
    fn half(self) -> Self;
}

impl Tap for i16 {
    fn from_i32(v: i32) -> i16 {
        v as i16
    }
    fn dot(a: &[i16], b: &[i16]) -> i64 {
        let mut s = 0i32;
        for (x, y) in a.iter().zip(b) {
            s = s.wrapping_add(i32::from(*x) * i32::from(*y));
        }
        i64::from(s)
    }
    fn add(self, o: i16) -> i16 {
        self.wrapping_add(o)
    }
    fn sub(self, o: i16) -> i16 {
        self.wrapping_sub(o)
    }
    fn half(self) -> i16 {
        self >> 1
    }
}

impl Tap for i32 {
    fn from_i32(v: i32) -> i32 {
        v
    }
    fn dot(a: &[i32], b: &[i32]) -> i64 {
        let mut s = 0i64;
        for (x, y) in a.iter().zip(b) {
            s = s.wrapping_add(i64::from(x.wrapping_mul(*y)));
        }
        s
    }
    fn add(self, o: i32) -> i32 {
        self.wrapping_add(o)
    }
    fn sub(self, o: i32) -> i32 {
        self.wrapping_sub(o)
    }
    fn half(self) -> i32 {
        self >> 1
    }
}

const WINDOW: usize = 512;

/// A sign-LMS filter of `order` taps.
pub(crate) struct NnFilter<I: Int, D: Tap> {
    order: usize,
    shift: u32,
    round: i64,
    new_delta: bool,
    m: Vec<D>,
    input: Vec<D>,
    delta: Vec<D>,
    pos: usize,
    avg: I,
}

impl<I: Int, D: Tap> NnFilter<I, D> {
    pub fn new(order: usize, shift: u32, version: u16) -> Self {
        NnFilter {
            order,
            shift,
            round: 1 << (shift - 1),
            new_delta: version >= 3980,
            m: vec![D::default(); order],
            input: vec![D::default(); order + WINDOW],
            delta: vec![D::default(); order + WINDOW],
            pos: order,
            avg: I::default(),
        }
    }

    pub fn flush(&mut self) {
        self.m.fill(D::default());
        self.input.fill(D::default());
        self.delta.fill(D::default());
        self.pos = self.order;
        self.avg = I::default();
    }

    pub fn decompress(&mut self, x: I) -> I {
        let (o, p) = (self.order, self.pos);
        let dot = D::dot(&self.input[p - o..p], &self.m);
        let out = x.add(
            I::from_i64(dot)
                .add(I::from_i64(self.round))
                .shr(self.shift),
        );
        // Adapt the taps against the input's sign.
        let xs = x.i64();
        if xs < 0 {
            for (m, d) in self.m.iter_mut().zip(&self.delta[p - o..p]) {
                *m = m.add(*d);
            }
        } else if xs > 0 {
            for (m, d) in self.m.iter_mut().zip(&self.delta[p - o..p]) {
                *m = m.sub(*d);
            }
        }
        let neg = out.bit31();
        if self.new_delta {
            let a = out.abs();
            let avg = self.avg;
            self.delta[p] = D::from_i32(if a > avg.mul(I::from_i64(3)) {
                if neg { 32 } else { -32 }
            } else if a > avg.mul(I::from_i64(4)).div(3) {
                if neg { 16 } else { -16 }
            } else if a.i64() > 0 {
                if neg { 8 } else { -8 }
            } else {
                0
            });
            self.avg = avg.add(a.sub(avg).div(16));
            self.delta[p - 1] = self.delta[p - 1].half();
            self.delta[p - 2] = self.delta[p - 2].half();
            self.delta[p - 8] = self.delta[p - 8].half();
        } else {
            self.delta[p] = D::from_i32(if out.i64() == 0 {
                0
            } else if neg {
                4
            } else {
                -4
            });
            self.delta[p - 4] = self.delta[p - 4].half();
            self.delta[p - 8] = self.delta[p - 8].half();
        }
        let v = out.i64();
        let sat = if v as i16 as i64 != v {
            ((v >> 63) as i32 ^ 0x7fff) as i16
        } else {
            v as i16
        };
        self.input[p] = D::from_i32(i32::from(sat));
        self.pos += 1;
        if self.pos == self.input.len() {
            let n = self.input.len();
            self.input.copy_within(n - o..n, 0);
            self.delta.copy_within(n - o..n, 0);
            self.pos = o;
        }
        out
    }
}

/// The scaled first-order filter (31/32).
#[derive(Default)]
struct FirstOrder {
    last: i32,
}

impl FirstOrder {
    fn compress<I: Int>(&mut self, x: i32) -> I {
        let r = I::from_i64(i64::from(x)).sub(
            I::from_i64(i64::from(self.last))
                .mul(I::from_i64(31))
                .shr(5),
        );
        self.last = x;
        r
    }
    fn decompress<I: Int>(&mut self, x: I) -> i32 {
        self.last = x
            .add(
                I::from_i64(i64::from(self.last))
                    .mul(I::from_i64(31))
                    .shr(5),
            )
            .i64() as i32;
        self.last
    }
}

fn adapt_sign<I: Int>(v: I) -> I {
    I::from_i64(if v.i64() == 0 {
        0
    } else if v.bit31() {
        1
    } else {
        -1
    })
}

/// One channel's predictor.
pub(crate) struct Predictor<I: Int, D: Tap> {
    filters: Vec<NnFilter<I, D>>,
    pa: [I; 4],
    pb: [I; 4],
    ma: [I; 4],
    mb: [I; 5],
    stage_a: FirstOrder,
    stage_b: FirstOrder,
    last_a: I,
    bits: u16,
}

impl<I: Int, D: Tap> Predictor<I, D> {
    /// The filters of `level` (1000…5000), applied in this order.
    pub fn new(level: u16, version: u16, bits: u16) -> Option<Self> {
        let spec: &[(usize, u32)] = match level {
            1000 => &[],
            2000 => &[(16, 11)],
            3000 => &[(64, 11)],
            4000 => &[(32, 10), (256, 13)],
            5000 => &[(16, 11), (256, 13), (1280, 15)],
            _ => return None,
        };
        let mut p = Predictor {
            filters: spec
                .iter()
                .map(|&(o, s)| NnFilter::new(o, s, version))
                .collect(),
            pa: [I::default(); 4],
            pb: [I::default(); 4],
            ma: [I::default(); 4],
            mb: [I::default(); 5],
            stage_a: FirstOrder::default(),
            stage_b: FirstOrder::default(),
            last_a: I::default(),
            bits,
        };
        p.flush();
        Some(p)
    }

    pub fn flush(&mut self) {
        for f in self.filters.iter_mut() {
            f.flush();
        }
        self.pa = [I::default(); 4];
        self.pb = [I::default(); 4];
        self.ma = [360, 317, -109, 98].map(I::from_i64);
        self.mb = [I::default(); 5];
        self.stage_a = FirstOrder::default();
        self.stage_b = FirstOrder::default();
        self.last_a = I::default();
    }

    pub fn decompress(&mut self, a: i64, b: i32) -> i32 {
        let mut a = I::from_i64(a);
        for f in self.filters.iter_mut() {
            a = f.decompress(a);
        }
        let a0 = self.last_a;
        let a1 = a0.sub(self.pa[0]);
        let ta = [a0, a1, self.pa[1], self.pa[2]];
        let b0: I = self.stage_b.compress(b);
        let b1 = b0.sub(self.pb[0]);
        let tb = [b0, b1, self.pb[1], self.pb[2], self.pb[3]];
        let current = if self.bits <= 16 || I::WIDE {
            let pa = ta
                .iter()
                .zip(&self.ma)
                .fold(I::default(), |s, (t, m)| s.add(t.mul(*m)));
            let pb = tb
                .iter()
                .zip(&self.mb)
                .fold(I::default(), |s, (t, m)| s.add(t.mul(*m)));
            a.add(pa.add(pb.shr(1)).shr(10))
        } else {
            let wide = |t: &[I], m: &[I]| -> i64 {
                t.iter().zip(m).fold(0i64, |s, (t, m)| {
                    s.wrapping_add(t.i64().wrapping_mul(m.i64()))
                })
            };
            let pa = wide(&ta, &self.ma);
            let pb = wide(&tb, &self.mb);
            let s = (pa as i32).wrapping_add((pb as i32) >> 1) >> 10;
            a.add(I::from_i64(i64::from(s)))
        };
        let dir = I::from_i64(i64::from(a.i64() < 0) - i64::from(a.i64() > 0));
        for (m, t) in self.ma.iter_mut().zip(&ta) {
            *m = m.add(adapt_sign(*t).mul(dir));
        }
        for (m, t) in self.mb.iter_mut().zip(&tb) {
            *m = m.add(adapt_sign(*t).mul(dir));
        }
        let out = self.stage_a.decompress(current);
        self.last_a = current;
        self.pa = [a0, a1, self.pa[1], self.pa[2]];
        self.pb = [b0, b1, self.pb[1], self.pb[2]];
        out
    }
}
