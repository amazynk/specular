//! How compare's measurements read (DESIGN.md §6.3): the error line (MAE, MSE, PSNR), the
//! quality line (the region, SSIM, ΔE2000) and the metric space's explanation. The numbers
//! themselves come from `specular_render::Comparer`; their definitions are
//! `specular_inspect::diff` and `specular_inspect::quality`.

use specular_i18n::t;
use specular_inspect::{MetricSpace, diff};
use specular_render::{METRIC_SIZE, MetricResult};

/// MAE, MSE and PSNR, and the sample grid when the region was larger than [`METRIC_SIZE`].
pub(super) fn error_line(m: &MetricResult) -> String {
    let s = &m.stats;
    let mut line = t!(
        "compare-metrics",
        mae = diff::metric_text(s.mae),
        mse = diff::metric_text(s.mse),
        psnr = s.psnr_text()
    );
    if m.density < 0.999 {
        line.push_str("  ·  ");
        line.push_str(&t!(
            "compare-sampled",
            width = i64::from(m.samples.0),
            height = i64::from(m.samples.1)
        ));
    }
    line
}

/// "In view" or "Whole image".
pub(super) fn region_text(whole: bool) -> String {
    if whole {
        t!("compare-region-whole")
    } else {
        t!("compare-region-view")
    }
}

/// SSIM as shown: four decimals ("1" for identical images).
pub(super) fn ssim_text(v: f64) -> String {
    if v >= 1.0 - 5e-5 {
        "1".into()
    } else {
        format!("{v:.4}")
    }
}

/// The region, SSIM and ΔE2000 (mean, 95th percentile, largest); a note when HDR values
/// above SDR white were clipped for ΔE.
pub(super) fn quality_line(m: &MetricResult, whole: bool, hdr: bool) -> String {
    let mut parts = vec![region_text(whole)];
    parts.push(match m.ssim {
        Some(v) => t!("compare-ssim", ssim = ssim_text(v)),
        None => t!("compare-ssim-small"),
    });
    let de = &m.delta_e;
    parts.push(t!(
        "compare-delta-e",
        mean = diff::metric_text(de.mean),
        p95 = diff::metric_text(de.p95),
        max = diff::metric_text(f64::from(de.max))
    ));
    if hdr {
        parts.push(t!("compare-delta-e-hdr"));
    }
    specular_i18n::I18n::global().join(&parts, "  ·  ")
}

/// The difference view's gain as shown: whole numbers plainly ("16"), else three digits.
pub(super) fn gain_text(g: f32) -> String {
    if g >= 1.0 && g.fract() == 0.0 {
        format!("{g:.0}")
    } else {
        diff::metric_text(f64::from(g))
    }
}

/// A scale factor as shown ("1.002").
pub(super) fn factor_text(k: f32) -> String {
    let s = format!("{k:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A turn in degrees as shown, with a real minus sign ("−0.4").
pub(super) fn angle_text(deg: f32) -> String {
    let digits = format!("{:.2}", deg.abs());
    let digits = digits.trim_end_matches('0').trim_end_matches('.');
    let negative = deg < 0.0 && digits != "0";
    specular_i18n::signed_number(digits, negative, false)
}

/// What the metric space means, for the space button's tooltip and screen readers.
pub(super) fn space_help(space: MetricSpace) -> String {
    let mut s = match space {
        MetricSpace::Linear => t!("compare-space-help-linear"),
        MetricSpace::Srgb => t!("compare-space-help-srgb"),
    };
    s.push(' ');
    s.push_str(&t!(
        "compare-space-help-common",
        size = i64::from(METRIC_SIZE)
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_plainly() {
        assert_eq!(factor_text(1.0), "1");
        assert_eq!(factor_text(1.002), "1.002");
        assert_eq!(factor_text(0.5), "0.5");
        assert_eq!(angle_text(-0.4), "\u{2212}0.4");
        assert_eq!(angle_text(0.4), "0.4");
        assert_eq!(angle_text(-0.001), "0");
        assert_eq!(ssim_text(1.0), "1");
        assert_eq!(ssim_text(0.98213), "0.9821");
        assert_eq!(gain_text(16.0), "16");
        assert_eq!(gain_text(0.125), "0.125");
    }
}
