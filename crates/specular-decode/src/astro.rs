//! Astronomy subframes (FITS, XISF): the header facts they are sorted by and their star
//! metrics ([`specular_astro`]), from an open session.
//!
//! The metrics are measured on the first image plane: a colour camera's raw frame
//! (`BAYERPAT`) 2 × 2 binned so the mosaic does not cut stars up (sizes scaled back), an RGB
//! image as the sum of its three channels. The plate scale comes from the WCS, else from
//! `FOCALLEN` and `XPIXSZ`.

use specular_core::CancelToken;

pub use specular_astro::{
    FrameMetrics, Metric, Options, Star, SubframeHeader, Suggestion, suggest,
};

use crate::{DecodeError, Format, Session};

/// Frames larger than this are not measured (a mosaic or a survey plate is not a subframe).
const MAX_PIXELS: u64 = 200_000_000;

/// Whether `s` is a FITS or XISF image whose header and planes the metrics read.
fn astro_image(s: &dyn Session) -> Option<&crate::science::ScienceSummary> {
    matches!(s.info().format, Format::Fits | Format::Xisf)
        .then(|| s.science())
        .flatten()
        .filter(|sum| !sum.raster_only && sum.width > 0 && sum.height > 0)
}

/// The header facts of a FITS or XISF image; `None` for other images or when the header
/// has none of them.
pub fn subframe_header(s: &dyn Session) -> Option<SubframeHeader> {
    let sum = astro_image(s)?;
    let get = |k: &str| {
        sum.keywords
            .iter()
            .find(|c| c.0.eq_ignore_ascii_case(k))
            .map(|c| c.1.clone())
    };
    let mut h = SubframeHeader::from_keywords(get);
    // What the summary read from elsewhere (XISF properties, an inherited primary header).
    h.object = h.object.or_else(|| sum.object.clone());
    h.filter = h.filter.or_else(|| sum.filter.clone());
    h.exposure = h.exposure.or(sum.exposure);
    h.date_obs = h.date_obs.or_else(|| sum.date_obs.clone());
    (!h.is_empty()).then_some(h)
}

/// The plate scale in arc seconds per pixel: from the WCS, else the optics in the header.
pub fn plate_scale(s: &dyn Session) -> Option<f64> {
    let sum = astro_image(s)?;
    if let Some(v) = sum.wcs.as_ref().and_then(|w| w.arcsec_per_pixel()) {
        return Some(v);
    }
    let get = |k: &str| {
        sum.keywords
            .iter()
            .find(|c| c.0.eq_ignore_ascii_case(k))
            .map(|c| c.1.clone())
    };
    SubframeHeader::from_keywords(get)
        .optics_scale()
        .or_else(|| specular_astro::header::wcs_scale(get))
}

/// Measures the star metrics of a FITS or XISF image's first plane; `None` for other images
/// (and frames too large to be subframes).
pub fn measure(s: &dyn Session, cancel: &CancelToken) -> Option<Result<FrameMetrics, DecodeError>> {
    Some(measure_stars(s, cancel)?.map(|r| r.0))
}

/// [`measure`] with the stars measured (positions and sizes in the image's pixels).
pub fn measure_stars(
    s: &dyn Session,
    cancel: &CancelToken,
) -> Option<Result<(FrameMetrics, Vec<Star>), DecodeError>> {
    let sum = astro_image(s)?;
    if u64::from(sum.width) * u64::from(sum.height) > MAX_PIXELS {
        return None;
    }
    let base: Vec<u32> = if sum.start.len() == sum.axes.len() {
        sum.start.clone()
    } else {
        vec![0; sum.axes.len()]
    };
    let plane = match s.nd_plane(&base, cancel)? {
        Ok(p) => p,
        Err(e) => return Some(Err(e)),
    };
    let (w, h) = (plane.width as usize, plane.height as usize);
    // RGB: the three channels summed.
    let summed;
    let values: &[f32] = match sum.channel_axis {
        Some(axis) if sum.axes.get(axis).is_some_and(|a| a.len >= 3) => {
            let mut acc = plane.values.clone();
            for c in 1..3u32 {
                let mut idx = base.clone();
                idx[axis] = c;
                let p = match s.nd_plane(&idx, cancel)? {
                    Ok(p) => p,
                    Err(e) => return Some(Err(e)),
                };
                if p.values.len() != acc.len() {
                    return Some(Err(DecodeError::Malformed(
                        "channels differ in size".into(),
                    )));
                }
                for (a, v) in acc.iter_mut().zip(&p.values) {
                    *a += v;
                }
            }
            summed = acc;
            &summed
        }
        _ => &plane.values,
    };
    if cancel.is_cancelled() {
        return Some(Err(DecodeError::Cancelled));
    }
    let img = specular_astro::Image {
        values,
        width: w,
        height: h,
    };
    let opts = Options::default();
    let (mut m, stars) = if sum.cfa.is_some() {
        let (b, bw, bh) = specular_astro::bin2(&img);
        let binned = specular_astro::Image {
            values: &b,
            width: bw,
            height: bh,
        };
        let (m, stars) = specular_astro::measure_stars(&binned, &opts);
        // Back to the mosaic's pixels.
        let stars = stars
            .into_iter()
            .map(|mut st| {
                st.x = st.x * 2.0 + 0.5;
                st.y = st.y * 2.0 + 0.5;
                st.fwhm *= 2.0;
                st.fwhm_major *= 2.0;
                st.fwhm_minor *= 2.0;
                st.hfd *= 2.0;
                st
            })
            .collect();
        (m.scaled(2.0), stars)
    } else {
        specular_astro::measure_stars(&img, &opts)
    };
    m.scale = plate_scale(s).map(|v| v as f32);
    Some(Ok((m, stars)))
}
