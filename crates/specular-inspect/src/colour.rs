//! sRGB transfer and the binning rule shared with `inspect.wgsl`.

/// sRGB-encodes a linear value (IEC 61966-2-1), clamped to 0‥1.
pub fn srgb_encode(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Linear value of an sRGB-encoded one, clamped to 0‥1.
pub fn srgb_decode(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// 0‥1 to the nearest 8-bit code.
pub fn to_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Histogram bin (0‥255) of an encoded 0‥1 value; the shader's `bin_of`.
pub fn bin(encoded: f32) -> usize {
    (encoded.clamp(0.0, 1.0) * 255.0 + 0.5).min(255.0) as usize
}

/// Relative luminance (Rec. 709 / sRGB primaries) of linear RGB.
pub fn luma(lin: [f32; 3]) -> f32 {
    0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]
}
