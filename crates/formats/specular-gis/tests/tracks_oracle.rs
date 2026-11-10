//! FIT, TCX, GPX extensions and KML tracks and ground overlays against the oracle
//! (fitdecode and ElementTree, `gen/tracks_oracle.py`): record and position counts, every
//! sampled position within 1e-8 degrees, session counts, summaries and overlay boxes.
//!
//! The corpus comes from `tools/corpus/fetch-gis-formats.sh`; it is read from
//! SPECULAR_GIS_FORMATS_CORPUS, else tools/corpus/gis-formats, and the test skips without
//! its `expected/tracks.jsonl`.

use serde_json::Value as J;
use specular_gis::model::{Geometry, Pt, Value};
use std::path::{Path, PathBuf};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GIS_FORMATS_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/gis-formats")
        });
    if dir.join("expected/tracks.jsonl").is_file() {
        Some(dir)
    } else {
        eprintln!("no tracks oracle in {}; skipping", dir.display());
        None
    }
}

fn pt(j: &J) -> Option<Pt> {
    let a = j.as_array()?;
    Some([a[0].as_f64()?, a[1].as_f64()?])
}

fn close(a: Pt, b: Pt) -> bool {
    (a[0] - b[0]).abs() <= 1e-8 && (a[1] - b[1]).abs() <= 1e-8
}

fn line_points(g: &Option<Geometry>) -> Vec<Pt> {
    let mut v = Vec::new();
    if let Some(g) = g {
        g.for_each_pt(&mut |p| v.push(p));
    }
    v
}

fn attr(l: &specular_gis::Layer, i: usize, k: &str) -> Value {
    l.attributes(i)
        .into_iter()
        .find(|(n, _)| n == k)
        .map_or(Value::Null, |(_, v)| v)
}

/// FIT files fitdecode and the viewer read differently by design.
const FIT_SKIP: &[(&str, &str)] = &[];

#[test]
fn tracks_match_the_oracle() {
    let Some(dir) = corpus() else { return };
    let text = std::fs::read_to_string(dir.join("expected/tracks.jsonl")).unwrap();
    let mut mismatches: Vec<String> = Vec::new();
    let (mut files, mut records, mut positions, mut refused, mut partial) = (0, 0, 0, 0, 0);
    for line in text.lines() {
        let r: J = serde_json::from_str(line).unwrap();
        let file = r["file"].as_str().unwrap();
        let data = std::fs::read(dir.join(file)).unwrap();
        files += 1;
        match r["kind"].as_str().unwrap() {
            "fit" => {
                if FIT_SKIP.iter().any(|(f, _)| *f == file) {
                    continue;
                }
                let want_records = r["records"].as_u64().unwrap() as usize;
                let want_pos = r["positioned"].as_u64().unwrap() as usize;
                let (n, pts) = match specular_gis::fit::positions(&data) {
                    Ok(v) => v,
                    Err(e) => {
                        mismatches.push(format!("{file}: {e}"));
                        continue;
                    }
                };
                if !r["error"].is_null() {
                    // fitdecode stopped at damage; the viewer reads at least as far.
                    partial += 1;
                    if n < want_records || pts.len() < want_pos {
                        mismatches.push(format!(
                            "{file}: {n} records / {} positions, fitdecode (stopped early) {want_records} / {want_pos}",
                            pts.len()
                        ));
                    }
                    continue;
                }
                records += n;
                positions += pts.len();
                if n != want_records || pts.len() != want_pos {
                    mismatches.push(format!(
                        "{file}: {n} records / {} positions, fitdecode {want_records} / {want_pos}",
                        pts.len()
                    ));
                    continue;
                }
                let step = (want_pos / 50).max(1);
                for (k, s) in r["sample"].as_array().unwrap().iter().enumerate() {
                    let theirs = pt(s).unwrap();
                    if !close(pts[k * step], theirs) {
                        mismatches.push(format!(
                            "{file}: position {} {:?} vs {theirs:?}",
                            k * step,
                            pts[k * step]
                        ));
                        break;
                    }
                }
                let ds = specular_gis::fit::read(&data, "x");
                if want_pos == 0 {
                    match ds {
                        Err(e) if e.contains("no GPS positions") => refused += 1,
                        other => mismatches.push(format!(
                            "{file}: not refused: {:?}",
                            other.map(|d| d.feature_count())
                        )),
                    }
                    continue;
                }
                let ds = match ds {
                    Ok(ds) => ds,
                    Err(e) => {
                        mismatches.push(format!("{file}: {e}"));
                        continue;
                    }
                };
                let sessions = r["sessions"].as_array().unwrap();
                let tracks = &ds.layers[0];
                if tracks.features.len() != sessions.len().max(1) {
                    mismatches.push(format!(
                        "{file}: {} tracks, {} sessions",
                        tracks.features.len(),
                        sessions.len()
                    ));
                }
                for (k, s) in sessions.iter().enumerate().take(tracks.features.len()) {
                    if let (Some(d), Value::Float(ours)) =
                        (s["distance"].as_f64(), attr(tracks, k, "distance_m"))
                        && (d - ours).abs() > 0.006
                    {
                        mismatches.push(format!(
                            "{file} session {k}: distance {ours}, fitdecode {d}"
                        ));
                    }
                    if let (Some(t), Value::Float(ours)) =
                        (s["elapsed"].as_f64(), attr(tracks, k, "duration_s"))
                        && (t - ours).abs() > 0.0006
                    {
                        mismatches
                            .push(format!("{file} session {k}: elapsed {ours}, fitdecode {t}"));
                    }
                }
                if sessions.len() <= 1 {
                    let ours = line_points(&tracks.features[0].geometry);
                    let first = pt(&r["first"]).unwrap();
                    let last = pt(&r["last"]).unwrap();
                    let finite: Vec<Pt> = pts
                        .iter()
                        .copied()
                        .filter(|p| p[0].is_finite() && p[1].is_finite())
                        .collect();
                    if ours.len() != finite.len()
                        || !close(ours[0], first)
                        || !close(*ours.last().unwrap(), last)
                    {
                        mismatches.push(format!(
                            "{file}: track {} points, first/last differ",
                            ours.len()
                        ));
                    }
                }
            }
            "tcx" => {
                let ds = match specular_gis::tcx::read(&data, "x") {
                    Ok(d) => d,
                    Err(e) => {
                        mismatches.push(format!("{file}: {e}"));
                        continue;
                    }
                };
                let tracks = &ds.layers[0];
                let want = r["tracks"].as_array().unwrap();
                if tracks.features.len() != want.len() {
                    mismatches.push(format!(
                        "{file}: {} tracks, oracle {}",
                        tracks.features.len(),
                        want.len()
                    ));
                    continue;
                }
                for (k, w) in want.iter().enumerate() {
                    let ours = line_points(&tracks.features[k].geometry);
                    positions += ours.len();
                    records += w["trackpoints"].as_u64().unwrap() as usize;
                    let n = w["positioned"].as_u64().unwrap() as usize;
                    if ours.len() != n
                        || (n > 0
                            && (!close(ours[0], pt(&w["first"]).unwrap())
                                || !close(*ours.last().unwrap(), pt(&w["last"]).unwrap())))
                    {
                        mismatches.push(format!(
                            "{file} track {k}: {} points, oracle {n}",
                            ours.len()
                        ));
                    }
                }
            }
            "gpx" => {
                let ds = match specular_gis::gpx::read(&data) {
                    Ok(d) => d,
                    Err(e) => {
                        mismatches.push(format!("{file}: {e}"));
                        continue;
                    }
                };
                let tracks = ds.layers.iter().find(|l| l.name == "tracks").unwrap();
                for (k, w) in r["tracks"].as_array().unwrap().iter().enumerate() {
                    let Some(_) = tracks.features.get(k) else {
                        mismatches.push(format!("{file}: no track {k}"));
                        break;
                    };
                    for (key, tol) in [
                        ("avg_hr", 0.051),
                        ("max_hr", 0.0),
                        ("avg_cadence", 0.051),
                        ("min_temp", 0.0),
                        ("max_temp", 0.0),
                    ] {
                        let ours = attr(tracks, k, key);
                        let ok = match (w[key].as_f64(), &ours) {
                            (None, Value::Null) => true,
                            (Some(t), Value::Float(o)) => (t - o).abs() <= tol,
                            _ => false,
                        };
                        if !ok {
                            mismatches.push(format!(
                                "{file} track {k} {key}: {ours:?}, oracle {}",
                                w[key]
                            ));
                        }
                    }
                    let n = w["points"].as_i64().unwrap();
                    records += n as usize;
                    if attr(tracks, k, "points") != Value::Int(n) {
                        mismatches.push(format!("{file} track {k}: points differ"));
                    }
                }
            }
            "kml" => {
                let ds = match specular_gis::kml::read(&data, "x") {
                    Ok(d) => d,
                    // Not XML to the oracle either (junk after the document, truncated).
                    Err(_) if !r["error"].is_null() => continue,
                    Err(e) => {
                        mismatches.push(format!("{file}: {e}"));
                        continue;
                    }
                };
                if !r["error"].is_null() {
                    mismatches.push(format!("{file}: read, oracle failed: {}", r["error"]));
                    continue;
                }
                let coords: i64 = ds
                    .layers
                    .iter()
                    .flat_map(|l| (0..l.features.len()).map(move |i| attr(l, i, "track_points")))
                    .map(|v| if let Value::Int(n) = v { n } else { 0 })
                    .sum();
                if coords != r["track_coords"].as_i64().unwrap() {
                    mismatches.push(format!(
                        "{file}: {coords} gx:coord, oracle {}",
                        r["track_coords"]
                    ));
                }
                let want = r["overlays"].as_array().unwrap();
                if ds.overlays.len() != want.len() {
                    mismatches.push(format!(
                        "{file}: {} overlays, oracle {}",
                        ds.overlays.len(),
                        want.len()
                    ));
                    continue;
                }
                for (o, w) in ds.overlays.iter().zip(want) {
                    let mut ok = w["href"].as_str() == Some(o.href.as_str());
                    if let Some(b) = w["box"].as_object() {
                        let g = |k: &str| b.get(k).and_then(J::as_f64);
                        ok &= g("north") == Some(o.north)
                            && g("south") == Some(o.south)
                            && g("east") == Some(o.east)
                            && g("west") == Some(o.west)
                            && g("rotation").unwrap_or(0.0) == o.rotation;
                    }
                    if let Some(q) = w["quad"].as_array() {
                        ok &= o
                            .quad
                            .is_some_and(|oq| q.iter().zip(oq).all(|(a, b)| pt(a) == Some(b)));
                    }
                    if !ok {
                        mismatches.push(format!("{file}: overlay {} differs", o.href));
                    }
                }
            }
            k => panic!("unknown kind {k}"),
        }
    }
    eprintln!(
        "{files} files, {records} records/points, {positions} positions, {refused} refused without positions, {partial} damaged; {} mismatches",
        mismatches.len()
    );
    for m in &mismatches {
        eprintln!("  {m}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}
