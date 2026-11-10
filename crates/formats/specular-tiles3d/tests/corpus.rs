//! The downloaded corpus (tools/corpus/fetch-3dtiles.sh): every tileset is read and fully
//! expanded without a panic, every content is read (or refused with a reason), and for a set
//! of views per tileset the selection is written out for the independent Python traversal
//! (tools/corpus/tiles3d-oracle.py). When the oracle's answers are there
//! (`oracle.json` beside `views.json`), tile counts and selections must agree.
//!
//! Reads SPECULAR_TILES3D_CORPUS, else tools/corpus/tiles3d; skips when neither exists.
//! Writes to SPECULAR_TILES3D_OUT, else the corpus folder.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Value, json};
use specular_mesh::Limits;
use specular_tiles3d::math::{self, V3};
use specular_tiles3d::{DirFetch, MAX_SSE, Tileset, View};

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_TILES3D_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/tiles3d")
        });
    dir.is_dir().then_some(dir)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("tileset") && n.ends_with(".json"))
        {
            out.push(p);
        }
    }
}

/// Views of a tileset, in the drawing frame: the iso view fitted, then closer, a top view
/// and an orthographic one.
fn views(ts: &Tileset) -> Vec<View> {
    let (mn, mx) = ts.local_bounds();
    let c = math::scale(math::add(mn, mx), 0.5);
    let r = (math::len(math::sub(mx, mn)) / 2.0).max(1e-3);
    let fov = 35f64.to_radians();
    let fit = r / (fov / 2.0).sin();
    let iso = math::unit([1.0, -1.0, 0.8]);
    let mut out = Vec::new();
    let mk = |eye: V3, target: V3, ortho: Option<f64>, sse: f64| {
        let dir = math::unit(math::sub(target, eye));
        let side = if dir[2].abs() > 0.99 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        let right = math::unit(math::cross(dir, side));
        View {
            eye,
            dir,
            up: math::cross(right, dir),
            fov_y: fov,
            ortho_height: ortho,
            width_px: 1200.0,
            height_px: 800.0,
            max_sse: sse,
            cull: true,
        }
    };
    for k in [200.0, 40.0, 8.0, 2.0, 1.0, 0.5, 0.25, 0.1, 0.03] {
        out.push(mk(
            math::add(c, math::scale(iso, fit * k)),
            c,
            None,
            MAX_SSE,
        ));
    }
    out.push(mk(math::add(c, math::scale(iso, fit)), c, None, 256.0));
    // Close to a corner, looking along the ground.
    let corner = [
        mn[0] + 0.2 * (mx[0] - mn[0]),
        mn[1] + 0.2 * (mx[1] - mn[1]),
        c[2],
    ];
    out.push(mk(
        math::add(corner, [-0.1 * r, -0.1 * r, 0.05 * r]),
        math::add(corner, [r, r, 0.0]),
        None,
        MAX_SSE,
    ));
    out.push(mk(math::add(c, [0.0, 0.0, fit * 0.3]), c, None, 4.0));
    out.push(mk(
        math::add(c, math::scale(iso, fit)),
        c,
        Some(r * 0.5),
        MAX_SSE,
    ));
    // In the tileset's coordinates.
    out.into_iter()
        .map(|v| View {
            eye: math::point(&ts.frame, v.eye),
            dir: math::vector(&ts.frame, v.dir),
            up: math::vector(&ts.frame, v.up),
            ..v
        })
        .collect()
}

fn keys(ts: &Tileset, tiles: &[u32]) -> Vec<String> {
    let mut out: Vec<String> = tiles
        .iter()
        .flat_map(|&i| ts.tiles[i as usize].contents.iter().cloned())
        .map(|k| {
            if k.starts_with("data:") {
                format!("data:{}", k.len())
            } else {
                k
            }
        })
        .collect();
    out.sort();
    out
}

#[test]
fn corpus_tilesets() {
    let Some(dir) = corpus() else {
        eprintln!("no 3D Tiles corpus; skipped");
        return;
    };
    let out_dir = std::env::var_os("SPECULAR_TILES3D_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.clone());
    let mut files = Vec::new();
    walk(&dir, &mut files);
    assert!(!files.is_empty());
    let limits = Limits::default();
    let mut report = Vec::new();
    let (mut contents, mut refused, mut failed) = (0, 0, 0);
    let mut slowest = (0.0f64, String::new());
    for f in &files {
        let rel = f.strip_prefix(&dir).unwrap().to_string_lossy().into_owned();
        let fetch = DirFetch::new(f.parent().unwrap());
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let t0 = Instant::now();
        let mut ts = match Tileset::open(&name, &fetch) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{rel}: refused: {e}");
                report.push(json!({"path": rel, "refused": e.to_string()}));
                continue;
            }
        };
        let fresh_views = views(&ts);
        let mut sels = Vec::new();
        for v in &fresh_views {
            // A fresh, lazily read tree per view.
            let mut lazy = Tileset::open(&name, &fetch).unwrap();
            let s = lazy.select(v, &fetch, 100_000);
            sels.push(keys(&lazy, &s.tiles));
        }
        ts.expand_all(&fetch);
        let n_tiles = ts.tiles.len();
        let n_contents: usize = ts.tiles.iter().map(|t| t.contents.len()).sum();
        let mut tris = 0usize;
        let mut points = 0usize;
        let mut reasons = Vec::new();
        for i in 0..ts.tiles.len() as u32 {
            if ts.tiles[i as usize].contents.is_empty() {
                continue;
            }
            let (cs, errs) = specular_tiles3d::load(&ts, i, &fetch, &limits);
            contents += cs.len();
            for c in &cs {
                tris += c.mesh.as_ref().map_or(0, |m| m.triangle_count());
                points += c.points.as_ref().map_or(0, |p| p.positions.len());
            }
            for e in errs {
                if e.contains("not supported") {
                    refused += 1;
                } else {
                    failed += 1;
                }
                reasons.push(e);
            }
        }
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        if ms > slowest.0 {
            slowest = (ms, rel.clone());
        }
        eprintln!(
            "{rel}: {n_tiles} tiles, {n_contents} contents, {tris} triangles, {points} points, {ms:.0} ms{}{}",
            if reasons.is_empty() { "" } else { "; " },
            reasons.first().map_or("", String::as_str)
        );
        let vs: Vec<Value> = fresh_views
            .iter()
            .map(|v| {
                json!({"eye": v.eye, "dir": v.dir, "up": v.up, "fov_y": v.fov_y,
                       "ortho_height": v.ortho_height, "width": v.width_px,
                       "height": v.height_px, "max_sse": v.max_sse})
            })
            .collect();
        report.push(json!({
            "path": rel, "tiles": n_tiles, "contents": n_contents, "notes": ts.notes,
            "errors": reasons, "views": vs, "selections": sels,
        }));
    }
    eprintln!(
        "{} tilesets, {contents} contents read, {refused} refused, {failed} failed; slowest {} ({:.0} ms)",
        files.len(),
        slowest.1,
        slowest.0
    );
    std::fs::write(
        out_dir.join("views.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    // The oracle's answers, when present.
    let Ok(oracle) = std::fs::read(out_dir.join("oracle.json")) else {
        eprintln!("no oracle.json yet: run tools/corpus/tiles3d-oracle.py");
        return;
    };
    let oracle: Vec<Value> = serde_json::from_slice(&oracle).unwrap();
    let (mut count_bad, mut sel_bad, mut sel_total) = (0, 0, 0);
    for o in &oracle {
        let Some(ours) = report.iter().find(|r| r["path"] == o["path"]) else {
            continue;
        };
        if ours.get("refused").is_some() || o.get("refused").is_some() {
            continue;
        }
        if ours["tiles"] != o["tiles"] || ours["contents"] != o["contents"] {
            count_bad += 1;
            eprintln!(
                "{}: tiles {} vs {}, contents {} vs {}",
                o["path"], ours["tiles"], o["tiles"], ours["contents"], o["contents"]
            );
        }
        let empty = Vec::new();
        let theirs = o["selections"].as_array().unwrap_or(&empty);
        for (k, s) in ours["selections"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .enumerate()
        {
            sel_total += 1;
            if theirs.get(k) != Some(s) {
                sel_bad += 1;
                eprintln!("{} view {k}: selections differ", o["path"]);
            }
        }
    }
    eprintln!(
        "oracle: {} tilesets, tile-count mismatches {count_bad}, selection mismatches {sel_bad} of {sel_total}",
        oracle.len()
    );
    assert_eq!(count_bad, 0);
    assert_eq!(sel_bad, 0);
}
