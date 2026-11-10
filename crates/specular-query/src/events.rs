//! Events: runs of photos taken close together in time and place.
//!
//! 1. **Time.** A gap longer than [`GAP`] between consecutive capture times starts a new event
//!    ([`clusters`]).
//! 2. **Place.** Within such a run, a move of more than [`JUMP_KM`] between geotagged photos
//!    starts a new event too ([`events`]), when the move is believable:
//!    - it is *confirmed*: the next geotagged photo agrees with the new place (within
//!      [`JUMP_KM`] of it, and also more than [`JUMP_KM`] from the old one). A single photo
//!      somewhere else between two that agree with each other is a GPS outlier and is ignored,
//!      so one bad fix never splits an event in two;
//!    - at either end of the run, where no photo can confirm it, the move must be possible:
//!      no faster than [`MAX_SPEED_KMH`] (a stray first or last fix thousands of kilometres off
//!      seconds apart is a glitch; one photo at home an hour after the last one at the airport
//!      is a move).
//!
//!    Positions are followed photo by photo, so a road trip that keeps shooting along the way
//!    never jumps, however far it goes in total.
//! 3. **Where the cut goes.** Photos without a position (another camera, an older cache entry,
//!    a fix the phone never got) never cause a split; they stay with their neighbours. A split
//!    between an old and a new place is placed at the longest pause in capture times between
//!    the last photo that could still have been at the old place and the first one at the new
//!    one: "could" meaning the move from it is no faster than [`MAX_SPEED_KMH`]. That keeps a
//!    flight's pause as the boundary even when the first photos after landing still carry the
//!    phone's stale fix from before take-off.
//!
//! Distances are great-circle (haversine) on a sphere of the mean Earth radius.

use std::ops::Range;

/// The gap that separates two events.
pub const GAP: i64 = 3 * 3600;

/// A move between consecutive geotagged photos longer than this starts a new event (a wedding's
/// church and its reception, or a city and its suburbs, stay together).
pub const JUMP_KM: f64 = 50.0;

/// The fastest believable travel between two photos, km/h: an airliner with a tail wind.
pub const MAX_SPEED_KMH: f64 = 1200.0;

/// Mean Earth radius (IUGG), km.
const EARTH_KM: f64 = 6371.0088;

/// One photo: capture time (seconds) and where it was taken, if known. Latitude and longitude
/// in degrees, south and west negative.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shot {
    pub time: i64,
    pub pos: Option<(f64, f64)>,
}

/// One event: an index range of the shots, and a representative position (one of its photos'
/// positions, near the middle of them), if any of them has one.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub range: Range<usize>,
    pub place: Option<(f64, f64)>,
}

/// Splits times sorted ascending into events: the index ranges of each run.
pub fn clusters(sorted: &[i64], gap: i64) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for k in 1..sorted.len() {
        if sorted[k].saturating_sub(sorted[k - 1]) > gap {
            out.push(start..k);
            start = k;
        }
    }
    if start < sorted.len() {
        out.push(start..sorted.len());
    }
    out
}

/// Splits shots sorted by time into events, on time gaps and on location jumps (see the module
/// documentation).
pub fn events(shots: &[Shot]) -> Vec<Event> {
    let times: Vec<i64> = shots.iter().map(|s| s.time).collect();
    let mut out = Vec::new();
    for run in clusters(&times, GAP) {
        let mut accepted = Vec::new();
        let cuts = jumps(shots, run.clone(), &mut accepted);
        let mut start = run.start;
        for end in cuts.into_iter().chain([run.end]) {
            out.push(Event {
                range: start..end,
                place: place(shots, &accepted, start..end),
            });
            start = end;
        }
    }
    out
}

/// Great-circle distance, km.
pub fn distance_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1) = (a.0.to_radians(), a.1.to_radians());
    let (la2, lo2) = (b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2)
        + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * EARTH_KM * h.clamp(0.0, 1.0).sqrt().asin()
}

/// A usable position: finite, in range, and not exactly 0° 0° (what a camera writes when it
/// has no fix).
fn usable(p: Option<(f64, f64)>) -> Option<(f64, f64)> {
    p.filter(|&(lat, lon)| {
        lat.is_finite()
            && lon.is_finite()
            && (-90.0..=90.0).contains(&lat)
            && (-180.0..=180.0).contains(&lon)
            && (lat != 0.0 || lon != 0.0)
    })
}

/// The indices in `run` where a new event starts on a location jump (ascending), and the
/// geotagged shots believed (outliers left out) appended to `accepted`.
fn jumps(shots: &[Shot], run: Range<usize>, accepted: &mut Vec<usize>) -> Vec<usize> {
    let fixes: Vec<(usize, (f64, f64))> = run
        .clone()
        .filter_map(|i| usable(shots[i].pos).map(|p| (i, p)))
        .collect();
    let at = |k: usize| fixes[k].1;
    let km = |a: usize, b: usize| distance_km(at(a), at(b));
    // Whether the move from fix a to fix b is no faster than MAX_SPEED_KMH.
    let possible = |a: usize, b: usize| {
        let hours = shots[fixes[a].0].time.abs_diff(shots[fixes[b].0].time) as f64 / 3600.0;
        km(a, b) <= MAX_SPEED_KMH * hours
    };
    let mut cuts = Vec::new();
    // The first fix starts the walk unless it is a stray: far from the next one, at an
    // impossible speed, while that one is not a stray itself (it agrees with the one after,
    // or is the last).
    let mut first = 0;
    while first + 1 < fixes.len()
        && km(first, first + 1) > JUMP_KM
        && !possible(first, first + 1)
        && fixes
            .get(first + 2)
            .is_none_or(|_| km(first + 1, first + 2) <= JUMP_KM)
    {
        first += 1;
    }
    if first >= fixes.len() {
        return cuts;
    }
    let mut anchor = first;
    // Where the current event's accepted fixes start in `accepted`.
    let mut event_from = accepted.len();
    accepted.push(fixes[first].0);
    // Fix indices of the accepted ones, parallel to `accepted` from `base`.
    let base = accepted.len() - 1;
    let mut kept = vec![first];
    for j in first + 1..fixes.len() {
        if km(anchor, j) <= JUMP_KM {
            anchor = j;
            accepted.push(fixes[j].0);
            kept.push(j);
            continue;
        }
        let moved = match fixes.get(j + 1) {
            Some(_) => km(j, j + 1) <= JUMP_KM && km(anchor, j + 1) > JUMP_KM,
            None => possible(anchor, j),
        };
        if !moved {
            // An outlier: as if it had no position.
            continue;
        }
        // The cut: at the longest pause after the last fix of this event that could still
        // have been at the old place (the anchor when none could).
        let from = kept[event_from - base..]
            .iter()
            .rev()
            .find(|&&k| possible(k, j))
            .copied()
            .unwrap_or(anchor);
        let (lo, hi) = (fixes[from].0, fixes[j].0);
        let mut cut = hi;
        let mut longest = i64::MIN;
        for i in lo + 1..=hi {
            let gap = shots[i].time.saturating_sub(shots[i - 1].time);
            if gap >= longest {
                longest = gap;
                cut = i;
            }
        }
        cuts.push(cut);
        anchor = j;
        accepted.push(fixes[j].0);
        kept.push(j);
        // Accepted fixes from the cut on belong to the new event (a stale fix after a flight).
        event_from = base + kept.partition_point(|&k| fixes[k].0 < cut);
    }
    cuts
}

/// A representative position of the accepted shots in `range`: the one nearest their
/// per-axis median (so always a real photo's position, inside the bulk of them).
fn place(shots: &[Shot], accepted: &[usize], range: Range<usize>) -> Option<(f64, f64)> {
    let from = accepted.partition_point(|&i| i < range.start);
    let to = accepted.partition_point(|&i| i < range.end);
    let pos: Vec<(f64, f64)> = accepted[from..to]
        .iter()
        .filter_map(|&i| shots[i].pos)
        .collect();
    if pos.is_empty() {
        return None;
    }
    let median = |f: fn(&(f64, f64)) -> f64| {
        let mut v: Vec<f64> = pos.iter().map(f).collect();
        v.sort_unstable_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let mid = (median(|p| p.0), median(|p| p.1));
    pos.iter()
        .copied()
        .min_by(|&a, &b| distance_km(a, mid).total_cmp(&distance_km(b, mid)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3600;
    const MIN: i64 = 60;
    // Made-up places, roughly: two cities 800 km apart, a suburb 20 km from the first, and a
    // spot far away. None is anyone's real location.
    const CITY: (f64, f64) = (45.0, 10.0);
    const SUBURB: (f64, f64) = (45.18, 10.0);
    const OTHER_CITY: (f64, f64) = (45.0, 20.2);
    const FAR: (f64, f64) = (-30.0, 140.0);

    fn at(time: i64, pos: (f64, f64)) -> Shot {
        Shot {
            time,
            pos: Some(pos),
        }
    }

    fn bare(time: i64) -> Shot {
        Shot { time, pos: None }
    }

    fn ranges(shots: &[Shot]) -> Vec<Range<usize>> {
        events(shots).into_iter().map(|e| e.range).collect()
    }

    /// `n` shots every `step` seconds from `t0`, at `pos`.
    fn burst(t0: i64, n: usize, step: i64, pos: (f64, f64)) -> Vec<Shot> {
        (0..n).map(|i| at(t0 + i as i64 * step, pos)).collect()
    }

    #[test]
    fn splits_on_gaps() {
        assert!(clusters(&[], GAP).is_empty());
        assert_eq!(clusters(&[5], GAP), vec![0..1]);
        let t = [0, H, 2 * H, 6 * H, 6 * H + 1, 20 * H];
        assert_eq!(clusters(&t, GAP), vec![0..3, 3..5, 5..6]);
        // Exactly the gap stays together.
        assert_eq!(clusters(&[0, GAP], GAP), vec![0..2]);
        assert_eq!(clusters(&[i64::MIN, i64::MAX], GAP), vec![0..1, 1..2]);
    }

    #[test]
    fn without_positions_only_time_splits() {
        assert!(events(&[]).is_empty());
        let t = [0, H, 2 * H, 6 * H, 6 * H + 1, 20 * H];
        let shots: Vec<Shot> = t.iter().map(|&t| bare(t)).collect();
        assert_eq!(ranges(&shots), clusters(&t, GAP));
        assert!(events(&shots).iter().all(|e| e.place.is_none()));
        assert_eq!(
            events(&[at(i64::MIN, CITY), at(i64::MAX, FAR)]).len(),
            2,
            "no overflow"
        );
    }

    #[test]
    fn distances() {
        assert!(distance_km(CITY, CITY) < 1e-9);
        assert!((distance_km(CITY, SUBURB) - 20.0).abs() < 0.2);
        let d = distance_km(CITY, OTHER_CITY);
        assert!((790.0..810.0).contains(&d), "{d}");
        // Across the antimeridian: 0.2° of longitude on the equator.
        assert!((distance_km((0.0, 179.9), (0.0, -179.9)) - 22.24).abs() < 0.05);
        assert!(
            (distance_km((90.0, 0.0), (-90.0, 0.0)) - EARTH_KM * std::f64::consts::PI).abs() < 1e-6
        );
    }

    #[test]
    fn a_trip_with_a_flight_splits_at_the_flight() {
        // Morning in one city, a 2 h 40 flight (under the time gap), afternoon in another.
        let mut s = burst(0, 6, 10 * MIN, CITY);
        let land = s.last().unwrap().time + 2 * H + 40 * MIN;
        s.extend(burst(land, 6, 10 * MIN, OTHER_CITY));
        let e = events(&s);
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].range.clone(), e[1].range.clone()), (0..6, 6..12));
        assert_eq!((e[0].place, e[1].place), (Some(CITY), Some(OTHER_CITY)));
    }

    #[test]
    fn short_moves_and_road_trips_stay_together() {
        // Church in the city, reception in the suburb 20 km away.
        let mut s = burst(0, 5, 5 * MIN, CITY);
        s.extend(burst(H, 5, 5 * MIN, SUBURB));
        assert_eq!(ranges(&s), vec![0..10]);
        // A road trip shooting every 20 minutes, 30 km apart: 600 km in all, no jump.
        let trip: Vec<Shot> = (0..21)
            .map(|i| at(i * 20 * MIN, (45.0, 10.0 + i as f64 * 0.38)))
            .collect();
        assert!(distance_km(trip[0].pos.unwrap(), trip[20].pos.unwrap()) > 550.0);
        assert_eq!(ranges(&trip), vec![0..21]);
    }

    #[test]
    fn a_single_outlier_never_splits() {
        // In the middle: one fix on another continent between two that agree.
        let mut s = burst(0, 8, MIN, CITY);
        s[4].pos = Some(FAR);
        assert_eq!(ranges(&s), vec![0..8]);
        assert_eq!(events(&s)[0].place, Some(CITY));
        // Also 800 km off, at a possible speed for its time gap.
        let mut s = burst(0, 8, H / 2, CITY);
        s[4].pos = Some(OTHER_CITY);
        assert_eq!(ranges(&s), vec![0..8]);
        // First or last, seconds from the others: impossible speeds.
        let mut s = burst(0, 8, MIN, CITY);
        s[0].pos = Some(FAR);
        assert_eq!(ranges(&s), vec![0..8]);
        assert_eq!(events(&s)[0].place, Some(CITY));
        let mut s = burst(0, 8, MIN, CITY);
        s[7].pos = Some(FAR);
        assert_eq!(ranges(&s), vec![0..8]);
        // An outlier second: the first fix is fine, the outlier is skipped.
        let mut s = burst(0, 8, MIN, CITY);
        s[1].pos = Some(OTHER_CITY);
        assert_eq!(ranges(&s), vec![0..8]);
        // Two strays that disagree with each other and with the rest.
        let mut s = burst(0, 8, MIN, CITY);
        s[3].pos = Some(FAR);
        s[4].pos = Some(OTHER_CITY);
        assert_eq!(ranges(&s), vec![0..8]);
        // A fix of 0° 0° means none.
        let mut s = burst(0, 4, MIN, CITY);
        s.extend(burst(4 * MIN, 4, MIN, (0.0, 0.0)));
        assert_eq!(ranges(&s), vec![0..8]);
    }

    #[test]
    fn a_lone_photo_at_either_end_splits_when_the_move_is_possible() {
        // Home an hour before the first photo in the other city, then the city.
        let mut s = vec![at(0, SUBURB)];
        s.extend(burst(H, 5, 5 * MIN, OTHER_CITY));
        assert_eq!(ranges(&s), vec![0..1, 1..6]);
        // The city, then one photo back home 1 h 30 later.
        let mut s = burst(0, 5, 5 * MIN, OTHER_CITY);
        s.push(at(20 * MIN + 90 * MIN, CITY));
        assert_eq!(ranges(&s), vec![0..5, 5..6]);
        // Two photos only.
        assert_eq!(
            ranges(&[at(0, CITY), at(2 * H, OTHER_CITY)]),
            vec![0..1, 1..2]
        );
        assert_eq!(ranges(&[at(0, CITY), at(MIN, OTHER_CITY)]), vec![0..2]);
    }

    #[test]
    fn photos_without_positions_go_with_their_neighbours() {
        // Phone shots (positions) and camera shots (none) interleaved, both on the same clock.
        let mut s = Vec::new();
        for i in 0..6 {
            s.push(at(i * 10 * MIN, CITY));
            s.push(bare(i * 10 * MIN + 3 * MIN));
        }
        // Camera shots on the way to the airport, then the flight, then the other city where
        // the camera shoots first.
        s.push(bare(70 * MIN));
        s.push(bare(80 * MIN));
        let land = 80 * MIN + 2 * H + 30 * MIN;
        s.push(bare(land));
        s.push(bare(land + MIN));
        for i in 0..4 {
            s.push(at(land + 5 * MIN + i * 10 * MIN, OTHER_CITY));
            s.push(bare(land + 8 * MIN + i * 10 * MIN));
        }
        let e = events(&s);
        assert_eq!(e.len(), 2, "{e:?}");
        assert_eq!(e[0].range, 0..14);
        assert_eq!(e[1].range, 14..s.len());
        // Without any positions the camera shots alone never split.
        let bare_only: Vec<Shot> = s.iter().map(|x| bare(x.time)).collect();
        assert_eq!(ranges(&bare_only), vec![0..s.len()]);
    }

    #[test]
    fn mixed_cameras_with_one_unknown_position_each() {
        // Two phones, one camera: positions where known agree; a body without GPS shoots
        // between them. Then a long time gap and a second day with only some positions.
        let mut s = Vec::new();
        for i in 0..12 {
            let t = i * 4 * MIN;
            s.push(match i % 3 {
                0 => at(t, CITY),
                1 => at(t, SUBURB),
                _ => bare(t),
            });
        }
        let day2 = 24 * H;
        for i in 0..6 {
            s.push(if i == 2 {
                at(day2 + i * 10 * MIN, OTHER_CITY)
            } else {
                bare(day2 + i * 10 * MIN)
            });
        }
        let e = events(&s);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].range, 0..12);
        assert_eq!(e[1].range, 12..18);
        assert_eq!(e[1].place, Some(OTHER_CITY));
        assert!(matches!(e[0].place, Some(p) if p == CITY || p == SUBURB));
    }

    #[test]
    fn a_stale_fix_after_landing_still_splits_at_the_flight() {
        // The first two photos after landing still carry the fix from before take-off.
        let mut s = burst(0, 4, 10 * MIN, CITY);
        let land = 30 * MIN + 2 * H + 30 * MIN;
        s.extend(burst(land, 2, 2 * MIN, CITY));
        s.extend(burst(land + 10 * MIN, 4, 5 * MIN, OTHER_CITY));
        let e = events(&s);
        assert_eq!(ranges(&s), vec![0..4, 4..10], "{e:?}");
        assert_eq!(e[1].place, Some(OTHER_CITY));
    }

    #[test]
    fn several_jumps_in_one_run() {
        // City → other city → city again, each confirmed, an hour apart: three events.
        let mut s = burst(0, 3, 5 * MIN, CITY);
        s.extend(burst(H + 10 * MIN, 3, 5 * MIN, OTHER_CITY));
        s.extend(burst(2 * H + 20 * MIN, 3, 5 * MIN, CITY));
        assert_eq!(ranges(&s), vec![0..3, 3..6, 6..9]);
    }
}
