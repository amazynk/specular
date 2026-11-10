//! Offline reverse geocoding with the `reverse_geocoder` crate (MIT/Apache-2.0)
//! and its bundled GeoNames city list (CC BY 4.0).
//!
//! The crate builds a k-d tree over ~144,000 places the first time it is used.
//! That takes a noticeable fraction of a second, so the index is built lazily
//! once per process ([`OnceLock`]) and can be warmed up ahead of time on a
//! background thread with [`warm_up_geocoder`]. Lookups are thread-safe.

use std::sync::OnceLock;

use reverse_geocoder::ReverseGeocoder;

use crate::{Localizer, Place};

static GEOCODER: OnceLock<ReverseGeocoder> = OnceLock::new();

/// Mean Earth radius (IUGG), km.
const EARTH_RADIUS_KM: f64 = 6371.0088;

fn geocoder() -> &'static ReverseGeocoder {
    GEOCODER.get_or_init(ReverseGeocoder::new)
}

/// True once the place index has been built (lookups will not block).
pub fn geocoder_ready() -> bool {
    GEOCODER.get().is_some()
}

/// Starts building the place index on a background thread, if it is not built
/// yet. Returns immediately. Safe to call any number of times.
pub fn warm_up_geocoder() {
    if geocoder_ready() {
        return;
    }
    let _ = specular_thread::builder("meta-geocoder").spawn(|| {
        geocoder();
    });
}

/// Finds the nearest named place to a coordinate. `country` is in English;
/// [`crate::attach_place`] replaces it with the report language's name.
/// Blocks to build the index on
/// first use if [`warm_up_geocoder`] has not finished yet.
///
/// Returns `None` for coordinates that are not finite or out of range.
pub fn geocode(lat: f64, lon: f64) -> Option<Place> {
    if !lat.is_finite()
        || !lon.is_finite()
        || !(-90.0..=90.0).contains(&lat)
        || !(-180.0..=180.0).contains(&lon)
    {
        return None;
    }
    let result = geocoder().search((lat, lon));
    let record = result.record;
    Some(Place {
        city: record.name.clone(),
        admin1: record.admin1.clone(),
        admin2: record.admin2.clone(),
        country_code: record.cc.clone(),
        country: Localizer::english_shared().country_name(&record.cc),
        distance_km: chord_sq_to_km(result.distance),
        lat: record.lat,
        lon: record.lon,
    })
}

/// `reverse_geocoder` reports the squared straight-line (chord) distance between
/// points on the unit sphere. Convert that to a great-circle distance in km.
fn chord_sq_to_km(chord_sq: f64) -> f64 {
    let chord = chord_sq.max(0.0).sqrt().min(2.0);
    2.0 * EARTH_RADIUS_KM * (chord / 2.0).asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_conversion() {
        assert_eq!(chord_sq_to_km(0.0), 0.0);
        // Antipodes: chord 2 → half circumference.
        let half = chord_sq_to_km(4.0);
        assert!((half - std::f64::consts::PI * EARTH_RADIUS_KM).abs() < 1e-6);
    }
}
