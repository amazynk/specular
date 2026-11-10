//! Shared by the targets: a few localizers (built once) and the report checks.

use specular_metadata::{read_at, Localizer, MetadataInput};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

/// English, German, Arabic (right-to-left) and Japanese.
pub fn localizer(pick: u8) -> &'static Localizer {
    static ALL: OnceLock<Vec<Localizer>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        ["en", "de", "ar", "ja"]
            .iter()
            .map(|l| Localizer::new(l.parse().unwrap()))
            .collect()
    });
    &all[usize::from(pick) % all.len()]
}

pub fn report(input: &MetadataInput, l: &Localizer) {
    // A fixed "now" keeps runs reproducible.
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let mut r = read_at(input, l, now);
    let _ = specular_metadata::one_line_summary(&r);
    r.strip_location(l);
    assert!(r.gps.is_none());
    std::hint::black_box(r);
}
