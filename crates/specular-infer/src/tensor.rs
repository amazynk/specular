//! Host tensors: a shape and f32 or f16 data, row-major and contiguous.
//!
//! f16 is a storage type: it halves the size of weights on disk and in VRAM, and every op
//! reads it as f32 (exactly: every f16 is an f32) and computes in f32. Op outputs are f32
//! unless the op is [`Op::CastF16`](crate::graph::Op::CastF16).

use half::f16;

/// Element type of a tensor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DType {
    F32,
    F16,
}

impl DType {
    pub fn size(self) -> usize {
        match self {
            DType::F32 => 4,
            DType::F16 => 2,
        }
    }
}

/// Tensor data.
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    F32(Vec<f32>),
    /// f16 bit patterns.
    F16(Vec<u16>),
}

/// A host tensor.
#[derive(Clone, Debug, PartialEq)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub data: Data,
}

/// Number of elements of a shape (1 for a scalar), or `None` on overflow.
pub fn numel(shape: &[usize]) -> Option<usize> {
    shape.iter().try_fold(1usize, |a, &d| a.checked_mul(d))
}

/// Row-major strides of a shape.
pub fn strides(shape: &[usize]) -> Vec<usize> {
    let mut s = vec![1usize; shape.len()];
    for i in (0..shape.len().saturating_sub(1)).rev() {
        s[i] = s[i + 1] * shape[i + 1];
    }
    s
}

impl Tensor {
    /// Panics if the data length does not match the shape.
    pub fn f32(shape: &[usize], data: Vec<f32>) -> Tensor {
        assert_eq!(numel(shape), Some(data.len()), "tensor data length");
        Tensor {
            shape: shape.to_vec(),
            data: Data::F32(data),
        }
    }

    /// From f16 bit patterns. Panics if the data length does not match the shape.
    pub fn f16_bits(shape: &[usize], data: Vec<u16>) -> Tensor {
        assert_eq!(numel(shape), Some(data.len()), "tensor data length");
        Tensor {
            shape: shape.to_vec(),
            data: Data::F16(data),
        }
    }

    pub fn zeros(shape: &[usize]) -> Tensor {
        Tensor::f32(shape, vec![0.0; numel(shape).expect("shape overflow")])
    }

    pub fn dtype(&self) -> DType {
        match self.data {
            Data::F32(_) => DType::F32,
            Data::F16(_) => DType::F16,
        }
    }

    pub fn len(&self) -> usize {
        match &self.data {
            Data::F32(v) => v.len(),
            Data::F16(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The values as f32 (a copy for f16).
    pub fn to_f32(&self) -> Vec<f32> {
        match &self.data {
            Data::F32(v) => v.clone(),
            Data::F16(v) => v.iter().map(|&h| f16::from_bits(h).to_f32()).collect(),
        }
    }

    /// The values as f32, borrowed when they already are.
    pub fn as_f32(&self) -> std::borrow::Cow<'_, [f32]> {
        match &self.data {
            Data::F32(v) => std::borrow::Cow::Borrowed(v),
            Data::F16(_) => std::borrow::Cow::Owned(self.to_f32()),
        }
    }

    /// The same values stored as f16 (round to nearest even).
    pub fn to_f16(&self) -> Tensor {
        let bits = match &self.data {
            Data::F32(v) => v.iter().map(|&x| f16::from_f32(x).to_bits()).collect(),
            Data::F16(v) => v.clone(),
        };
        Tensor {
            shape: self.shape.clone(),
            data: Data::F16(bits),
        }
    }

    /// The same data with another shape of the same size.
    pub fn reshaped(mut self, shape: &[usize]) -> Tensor {
        assert_eq!(numel(shape), Some(self.len()), "reshape size");
        self.shape = shape.to_vec();
        self
    }

    /// Raw little-endian bytes as stored on the GPU (f16 padded to a whole word).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b: Vec<u8> = match &self.data {
            Data::F32(v) => bytemuck::cast_slice(v).to_vec(),
            Data::F16(v) => bytemuck::cast_slice(v).to_vec(),
        };
        while !b.len().is_multiple_of(4) || b.is_empty() {
            b.push(0);
        }
        b
    }
}

/// A small deterministic generator (xorshift64*) for test and benchmark weights.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in [-a, a).
    pub fn sym(&mut self, a: f32) -> f32 {
        (self.unit() * 2.0 - 1.0) * a
    }

    /// Uniform below `n` (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// A tensor of uniform values in [-a, a).
    pub fn tensor(&mut self, shape: &[usize], a: f32) -> Tensor {
        let n = numel(shape).expect("shape overflow");
        Tensor::f32(shape, (0..n).map(|_| self.sym(a)).collect())
    }
}

/// How far a result is from a reference.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Diff {
    /// Largest |got − want|.
    pub max_abs: f32,
    /// Largest |got − want| / (atol + rtol·|want|) for the tolerance it was checked with:
    /// ≤ 1 passes.
    pub worst: f32,
    /// Where the worst element is.
    pub at: usize,
    /// Cosine similarity of the two as vectors.
    pub cosine: f32,
}

/// Compares `got` with `want` elementwise: passes when every element has
/// |got − want| ≤ atol + rtol·|want| (and NaN/inf match exactly).
pub fn compare(got: &[f32], want: &[f32], atol: f32, rtol: f32) -> Diff {
    assert_eq!(got.len(), want.len(), "compared lengths");
    let mut d = Diff::default();
    let (mut ab, mut aa, mut bb) = (0f64, 0f64, 0f64);
    for (i, (&g, &w)) in got.iter().zip(want).enumerate() {
        let e = if g.is_finite() && w.is_finite() {
            (g - w).abs()
        } else if g.to_bits() == w.to_bits() || (g.is_nan() && w.is_nan()) {
            0.0
        } else {
            f32::INFINITY
        };
        let r = if e == 0.0 {
            0.0
        } else {
            e / (atol + rtol * w.abs())
        };
        if e > d.max_abs {
            d.max_abs = e;
        }
        if r.is_nan() || r > d.worst {
            d.worst = r;
            d.at = i;
        }
        if g.is_finite() && w.is_finite() {
            ab += g as f64 * w as f64;
            aa += g as f64 * g as f64;
            bb += w as f64 * w as f64;
        }
    }
    d.cosine = if aa == 0.0 && bb == 0.0 {
        1.0
    } else {
        (ab / (aa.sqrt() * bb.sqrt()).max(f64::MIN_POSITIVE)) as f32
    };
    d
}
