//! Reading a channel's samples from its runs.

use crate::{Channel, DataType, Elem, Error, Result, Run, f128_to_f64, time_f64};

/// Samples scaled per pass, bounding the scaling's temporaries.
const BLOCK: usize = 1 << 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    /// Real values; complex ones as magnitude.
    Value,
    Re,
    Im,
}

/// |re + i·im| in single precision: the larger part times √(1 + q²), q the ratio of the
/// smaller to the larger (numpy's single-precision `abs`, bit for bit).
fn abs32(re: f32, im: f32) -> f32 {
    let (a, b) = (re.abs(), im.abs());
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    if a.is_nan() || b.is_nan() {
        return f32::NAN;
    }
    if hi.is_infinite() {
        return f32::INFINITY;
    }
    if hi == 0.0 {
        return 0.0;
    }
    let q = lo / hi;
    hi * q.mul_add(q, 1.0).sqrt()
}

fn half<const M: usize>(a: &[u8], at: usize, be: bool) -> [u8; M] {
    let mut h = [0u8; M];
    h.copy_from_slice(&a[at..at + M]);
    if be {
        h.reverse();
    }
    h
}

/// Fills `out` with the samples `stride` bytes apart from byte `off`, each `N` bytes made a
/// value by `f`; samples past the end of `data` are NaN.
#[inline(always)]
fn fill<const N: usize>(
    data: &[u8],
    off: u64,
    stride: u64,
    out: &mut [f64],
    f: impl Fn([u8; N]) -> f64,
) {
    let get = |b: &[u8]| {
        let mut a = [0u8; N];
        a.copy_from_slice(&b[..N]);
        f(a)
    };
    let n = out.len() as u64;
    let span = n
        .saturating_sub(1)
        .checked_mul(stride)
        .and_then(|s| s.checked_add(off))
        .and_then(|s| s.checked_add(N as u64));
    match (
        usize::try_from(off),
        span.and_then(|s| usize::try_from(s).ok()),
    ) {
        (Ok(o), Some(e)) if e <= data.len() && stride > 0 => {
            for (v, b) in out.iter_mut().zip(data[o..e].chunks(stride as usize)) {
                *v = get(b);
            }
        }
        _ => {
            for (q, v) in out.iter_mut().enumerate() {
                let p = (q as u64)
                    .checked_mul(stride)
                    .and_then(|s| s.checked_add(off))
                    .and_then(|p| usize::try_from(p).ok());
                *v = p
                    .and_then(|p| data.get(p..p.checked_add(N)?))
                    .map_or(f64::NAN, get);
            }
        }
    }
}

/// Samples of `run` from byte `off` into `out`, one conversion loop per element type.
fn gather(data: &[u8], off: u64, run: &Run, part: Part, out: &mut [f64]) {
    let (s, be) = (run.stride, run.be);
    // Fixed-size numbers: the bytes reversed when big-endian. A big-endian timestamp
    // (seconds, fraction) reversed whole is the little-endian (fraction, seconds).
    macro_rules! num {
        ($n:literal, $conv:expr) => {{
            let conv = $conv;
            if be {
                fill::<$n>(data, off, s, out, |mut a: [u8; $n]| {
                    a.reverse();
                    conv(a)
                })
            } else {
                fill::<$n>(data, off, s, out, conv)
            }
        }};
    }
    let complex = matches!(run.elem, Elem::C32 | Elem::C64);
    if part == Part::Im && !complex {
        out.fill(0.0);
        return;
    }
    match run.elem {
        Elem::I8 => num!(1, |a: [u8; 1]| f64::from(a[0] as i8)),
        Elem::U8 => num!(1, |a: [u8; 1]| f64::from(a[0])),
        Elem::Bool => num!(1, |a: [u8; 1]| f64::from(u8::from(a[0] != 0))),
        Elem::Bit(b) => num!(1, move |a: [u8; 1]| f64::from((a[0] >> b) & 1)),
        Elem::I16 => num!(2, |a| f64::from(i16::from_le_bytes(a))),
        Elem::U16 => num!(2, |a| f64::from(u16::from_le_bytes(a))),
        Elem::I32 => num!(4, |a| f64::from(i32::from_le_bytes(a))),
        Elem::U32 => num!(4, |a| f64::from(u32::from_le_bytes(a))),
        Elem::F32 => num!(4, |a| f64::from(f32::from_le_bytes(a))),
        Elem::I64 => num!(8, |a| i64::from_le_bytes(a) as f64),
        Elem::U64 => num!(8, |a| u64::from_le_bytes(a) as f64),
        Elem::F64 => num!(8, f64::from_le_bytes),
        Elem::Time => num!(16, |a: [u8; 16]| {
            time_f64(
                i64::from_le_bytes(half(&a, 8, false)),
                u64::from_le_bytes(half(&a, 0, false)),
            )
        }),
        Elem::Ext => num!(16, f128_to_f64),
        Elem::C32 => fill::<8>(data, off, s, out, move |a| {
            let re = f32::from_le_bytes(half(&a, 0, be));
            let im = f32::from_le_bytes(half(&a, 4, be));
            match part {
                // Single-precision magnitudes in single precision, as numpy takes them.
                Part::Value => f64::from(abs32(re, im)),
                Part::Re => f64::from(re),
                Part::Im => f64::from(im),
            }
        }),
        Elem::C64 => fill::<16>(data, off, s, out, move |a| {
            let re = f64::from_le_bytes(half(&a, 0, be));
            let im = f64::from_le_bytes(half(&a, 8, be));
            match part {
                Part::Value => re.hypot(im),
                Part::Re => re,
                Part::Im => im,
            }
        }),
        Elem::Str => out.fill(f64::NAN),
    }
}

fn missing() -> Error {
    Error::Malformed("channel data missing".into())
}

fn read_runs(runs: &[Run], data: &[u8], start: u64, out: &mut [f64], part: Part) -> Result<()> {
    let mut i = start;
    let mut o = 0usize;
    let mut r = runs.partition_point(|r| r.end() <= i);
    while o < out.len() {
        let run = runs.get(r).ok_or_else(missing)?;
        if i < run.first || run.count == 0 {
            return Err(missing());
        }
        let rel = i - run.first;
        let (k, j) = (rel / run.count, rel % run.count);
        let n = usize::try_from(run.count - j)
            .unwrap_or(usize::MAX)
            .min(out.len() - o);
        let off = run
            .offset
            .saturating_add(k.saturating_mul(run.step))
            .saturating_add(j.saturating_mul(run.stride));
        gather(data, off, run, part, &mut out[o..o + n]);
        o += n;
        i += n as u64;
        if i >= run.end() {
            r += 1;
        }
    }
    Ok(())
}

impl Channel {
    fn check(&self, start: u64, n: usize) -> Result<()> {
        if start.checked_add(n as u64).is_none_or(|e| e > self.len) {
            return Err(Error::Malformed(
                "read past the end of a TDMS channel".into(),
            ));
        }
        match self.dtype {
            DataType::String => Err(Error::Unsupported(
                "a TDMS text channel holds no numbers".into(),
            )),
            DataType::Extended => Err(Error::Unsupported(
                "TDMS extended-precision channels are not supported".into(),
            )),
            DataType::Other(_) if n > 0 => Err(missing()),
            _ => Ok(()),
        }
    }

    /// Values `start..start + out.len()`: scaled where the channel has a scaling, complex
    /// numbers as their magnitude, timestamps as seconds since 1904-01-01 UTC. Samples whose
    /// bytes lie past the end of a truncated file are NaN.
    pub fn read(&self, data: &[u8], start: u64, out: &mut [f64]) -> Result<()> {
        self.check(start, out.len())?;
        let s = match &self.scaling {
            Err(m) => return Err(Error::Unsupported(m.clone())),
            Ok(None) => return read_runs(&self.runs, data, start, out, Part::Value),
            Ok(Some(s)) => s,
        };
        for (b, chunk) in out.chunks_mut(BLOCK).enumerate() {
            let at = start + (b * BLOCK) as u64;
            let mut input = |src: Option<u32>, buf: &mut [f64]| match src {
                None => read_runs(&self.runs, data, at, buf, Part::Value),
                Some(id) => {
                    let runs = self
                        .scalers
                        .iter()
                        .find(|s| s.0 == id)
                        .map(|s| &s.1[..])
                        .ok_or_else(|| {
                            Error::Malformed(format!(
                                "a TDMS scale reads DAQmx scaler {id}, which the channel lacks"
                            ))
                        })?;
                    read_runs(runs, data, at, buf, Part::Value)
                }
            };
            let v = s.eval(s.last(), chunk.len(), &mut input)?;
            chunk.copy_from_slice(&v);
        }
        Ok(())
    }

    /// Values as stored, without the scaling (for DAQmx data, the first scaler's).
    pub fn read_raw(&self, data: &[u8], start: u64, out: &mut [f64]) -> Result<()> {
        self.check(start, out.len())?;
        read_runs(&self.runs, data, start, out, Part::Value)
    }

    /// The DAQmx scale ids this channel has raw data for.
    pub fn scaler_ids(&self) -> Vec<u32> {
        self.scalers.iter().map(|s| s.0).collect()
    }

    /// A DAQmx scaler's raw values.
    pub fn read_scaler(&self, id: u32, data: &[u8], start: u64, out: &mut [f64]) -> Result<()> {
        self.check(start, out.len())?;
        let runs = self
            .scalers
            .iter()
            .find(|s| s.0 == id)
            .ok_or_else(missing)?;
        read_runs(&runs.1, data, start, out, Part::Value)
    }

    /// Real and imaginary parts (real channels: an imaginary part of zero), unscaled.
    pub fn read_complex(
        &self,
        data: &[u8],
        start: u64,
        re: &mut [f64],
        im: &mut [f64],
    ) -> Result<()> {
        if re.len() != im.len() {
            return Err(Error::Malformed("real and imaginary buffers differ".into()));
        }
        self.check(start, re.len())?;
        read_runs(&self.runs, data, start, re, Part::Re)?;
        read_runs(&self.runs, data, start, im, Part::Im)
    }

    /// The first `max` strings of a text channel (lossy UTF-8).
    pub fn strings(&self, data: &[u8], max: usize) -> Result<Vec<String>> {
        if self.dtype != DataType::String {
            return Err(Error::Unsupported("not a TDMS text channel".into()));
        }
        let mut out = Vec::new();
        let u32_at = |p: u64, be: bool| -> Option<u32> {
            let p = usize::try_from(p).ok()?;
            let b: [u8; 4] = data.get(p..p.checked_add(4)?)?.try_into().ok()?;
            Some(if be {
                u32::from_be_bytes(b)
            } else {
                u32::from_le_bytes(b)
            })
        };
        for run in &self.runs {
            for k in 0..run.repeats {
                let base = run.offset.saturating_add(k.saturating_mul(run.step));
                let text = base.saturating_add(run.count.saturating_mul(4));
                let mut prev = 0u64;
                for j in 0..run.count {
                    if out.len() >= max {
                        return Ok(out);
                    }
                    let s = match u32_at(base.saturating_add(4 * j), run.be) {
                        Some(end) if u64::from(end) >= prev => {
                            let (a, b) = (
                                text.saturating_add(prev),
                                text.saturating_add(u64::from(end)),
                            );
                            prev = u64::from(end);
                            usize::try_from(a)
                                .ok()
                                .zip(usize::try_from(b).ok())
                                .and_then(|(a, b)| data.get(a..b))
                                .map(|s| String::from_utf8_lossy(s).into_owned())
                                .unwrap_or_default()
                        }
                        _ => String::new(),
                    };
                    out.push(s);
                }
            }
        }
        Ok(out)
    }
}
