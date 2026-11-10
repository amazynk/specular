/// Converts the linear samples with the BT.709 transfer curve.
pub fn linear_to_bt709(samples: &mut [f32]) {
    for x in samples {
        let a = *x;
        *x = if a <= 0.018 {
            4.5 * a
        } else {
            crate::color::fastmath::fast_powf_generic(a, 0.45).mul_add(1.099, -0.099)
        };
    }
}

pub fn bt709_to_linear(samples: &mut [f32]) {
    for x in samples {
        let a = *x;
        *x = if a <= 0.081 {
            a / 4.5
        } else {
            crate::color::fastmath::fast_powf_generic(
                a.mul_add(1.0 / 1.099, 0.099 / 1.099),
                1.0 / 0.45,
            )
        };
    }
}
