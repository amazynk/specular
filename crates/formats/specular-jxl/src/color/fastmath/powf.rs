#![allow(clippy::excessive_precision)]

const POW2F_NUMER_COEFFS: [f32; 3] = [1.01749063e1, 4.88687798e1, 9.85506591e1];
const POW2F_DENOM_COEFFS: [f32; 4] = [2.10242958e-1, -2.22328856e-2, -1.94414990e1, 9.85506633e1];

#[inline]
fn fast_pow2f_generic(x: f32) -> f32 {
    let x_floor = x.floor();
    let exp = f32::from_bits(((x_floor as i32 + 127) as u32) << 23);
    let frac = x - x_floor;

    let num = frac + POW2F_NUMER_COEFFS[0];
    let num = num * frac + POW2F_NUMER_COEFFS[1];
    let num = num * frac + POW2F_NUMER_COEFFS[2];
    let num = num * exp;

    let den = POW2F_DENOM_COEFFS[0] * frac + POW2F_DENOM_COEFFS[1];
    let den = den * frac + POW2F_DENOM_COEFFS[2];
    let den = den * frac + POW2F_DENOM_COEFFS[3];

    num / den
}

const LOG2F_P: [f32; 3] = [
    -1.8503833400518310e-6,
    1.4287160470083755,
    7.4245873327820566e-1,
];
const LOG2F_Q: [f32; 3] = [
    9.9032814277590719e-1,
    1.0096718572241148,
    1.7409343003366853e-1,
];

#[inline]
fn fast_log2f_generic(x: f32) -> f32 {
    let x_bits = x.to_bits() as i32;
    let exp_bits = x_bits - 0x3f2aaaab;
    let exp_shifted = exp_bits >> 23;
    let mantissa = f32::from_bits((x_bits - (exp_shifted << 23)) as u32);
    let exp_val = exp_shifted as f32;

    let x = mantissa - 1.0;
    super::rational_poly::eval_generic(x, LOG2F_P, LOG2F_Q) + exp_val
}

#[inline]
pub fn fast_powf_generic(base: f32, exp: f32) -> f32 {
    fast_pow2f_generic(fast_log2f_generic(base) * exp)
}
