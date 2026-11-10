//! The readers against the reference readers' output (`gen/oracle.py`: liffile, cross-checked
//! with readlif, and nd2), on the synthetic fixtures in `tests/data` and, when fetched, the
//! public samples in `tools/corpus/vendormicro` (`tools/corpus/fetch-vendormicro.sh`).

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value as J;
use specular_micro::{lif, nd2};

fn fnv(values: &[f32], float: bool) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &v in values {
        let w = if float { v.to_bits() } else { v as u32 };
        for b in w.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1e-30)
}

/// Compare one file; returns (planes compared, mismatches, notes).
fn check(path: &Path, exp: &J) -> (usize, usize, Vec<String>) {
    let data = std::fs::read(path).unwrap();
    let mut notes = Vec::new();
    let (mut planes, mut bad) = (0, 0);
    let miss = |what: String, notes: &mut Vec<String>| {
        notes.push(what);
    };
    match exp["kind"].as_str() {
        Some("lif") => {
            let stem = path.file_stem().unwrap().to_string_lossy();
            let f = lif::parse(&data, &stem).unwrap();
            assert_eq!(f.lof, exp["lof"].as_bool().unwrap(), "{path:?} lof");
            let images = exp["images"].as_array().unwrap();
            if f.images.len() != images.len() {
                miss(
                    format!("{} images, expected {}", f.images.len(), images.len()),
                    &mut notes,
                );
            }
            for (img, e) in f.images.iter().zip(images) {
                if img.path != e["path"].as_str().unwrap()
                    || img.name != e["name"].as_str().unwrap()
                {
                    miss(format!("path {} vs {}", img.path, e["path"]), &mut notes);
                }
                let edims = e["dims"].as_array().unwrap();
                let mut dims: Vec<&lif::Dim> = img.dims.iter().collect();
                dims.sort_by_key(|d| d.bytes_inc);
                for (d, ed) in dims.iter().zip(edims) {
                    let ok = d.kind.code() == ed["code"].as_str().unwrap()
                        && d.count as u64 == ed["count"].as_u64().unwrap()
                        && close(d.origin, ed["origin"].as_f64().unwrap())
                        && close(d.length, ed["length"].as_f64().unwrap())
                        && d.unit == ed["unit"].as_str().unwrap();
                    if !ok {
                        miss(format!("{}: dim {d:?} vs {ed}", img.path), &mut notes);
                    }
                }
                if dims.len() != edims.len() {
                    miss(format!("{}: {} dims", img.path, dims.len()), &mut notes);
                }
                let echans = e["channels"].as_array().unwrap();
                for (c, ec) in img.channels.iter().zip(echans) {
                    let ok = c.lut == ec["lut"].as_str().unwrap()
                        && u64::from(c.bits) == ec["bits"].as_u64().unwrap()
                        && c.sample.is_float() == ec["float"].as_bool().unwrap()
                        && c.name == ec["name"].as_str().unwrap();
                    if !ok {
                        miss(format!("{}: channel {c:?} vs {ec}", img.path), &mut notes);
                    }
                }
                if img.channels.len() != echans.len() {
                    miss(
                        format!("{}: {} channels", img.path, img.channels.len()),
                        &mut notes,
                    );
                }
                // Plane keys: channel, then indices over the oracle's non-XY dims (by byte
                // increment), mapped onto ours by code.
                let ecodes: Vec<&str> = edims
                    .iter()
                    .map(|d| d["code"].as_str().unwrap())
                    .filter(|c| !matches!(*c, "X" | "Y"))
                    .collect();
                let ours: Vec<&str> = img.extra_dims().map(|d| d.kind.code()).collect();
                let mut buf = vec![0f32; img.width() * img.height()];
                for (key, hash) in e["planes"].as_object().unwrap() {
                    let (c, rest) = key.split_once(':').unwrap();
                    let c: usize = c.parse().unwrap();
                    let idx: Vec<usize> = rest
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.parse().unwrap())
                        .collect();
                    let pos: Vec<usize> = ours
                        .iter()
                        .map(|code| ecodes.iter().position(|e| e == code).map_or(0, |k| idx[k]))
                        .collect();
                    planes += 1;
                    let float = img.channels[c].sample.is_float();
                    match img.read_plane(&data, c, &pos, &mut buf) {
                        Ok(_) if fnv(&buf, float) == hash.as_str().unwrap() => {}
                        r => {
                            bad += 1;
                            miss(format!("{} plane {key}: {r:?}", img.path), &mut notes);
                        }
                    }
                }
            }
        }
        Some("nd2") => {
            let f = nd2::parse(&data).unwrap();
            let ev: Vec<u32> = exp["version"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect();
            let scalar = [
                ("version", f.version == (ev[0], ev[1])),
                ("width", f.width as u64 == exp["width"].as_u64().unwrap()),
                ("height", f.height as u64 == exp["height"].as_u64().unwrap()),
                (
                    "components",
                    f.components as u64 == exp["components"].as_u64().unwrap(),
                ),
                ("bits", u64::from(f.bits) == exp["bits"].as_u64().unwrap()),
                ("frames", f.frames as u64 == exp["frames"].as_u64().unwrap()),
                (
                    "calibration",
                    close(
                        f.calibration.unwrap_or(1.0),
                        exp["calibration"].as_f64().unwrap(),
                    ),
                ),
            ];
            for (k, ok) in scalar {
                if !ok {
                    miss(format!("{k} differs"), &mut notes);
                }
            }
            let eloops = exp["loops"].as_array().unwrap();
            for (l, el) in f.loops.iter().zip(eloops) {
                let kind = match l.kind {
                    nd2::LoopKind::Time => "time",
                    nd2::LoopKind::Position => "position",
                    nd2::LoopKind::Z => "z",
                    nd2::LoopKind::Custom => "custom",
                };
                let mut ok = kind == el["kind"].as_str().unwrap()
                    && l.count as u64 == el["count"].as_u64().unwrap();
                // Unnamed positions: nd2 lists empty names, the reader none.
                if let Some(labels) = el.get("labels").and_then(J::as_array)
                    && !(l.labels.is_empty() && labels.iter().all(|s| s.as_str() == Some("")))
                {
                    ok &= labels
                        .iter()
                        .map(|s| s.as_str().unwrap())
                        .eq(l.labels.iter().map(String::as_str));
                }
                if let Some(step) = el.get("step").and_then(J::as_f64)
                    && !f.is_legacy()
                {
                    ok &= close(l.step.unwrap_or(0.0), step);
                }
                if !ok {
                    miss(format!("loop {l:?} vs {el}"), &mut notes);
                }
            }
            if f.loops.len() != eloops.len() {
                miss(
                    format!("{} loops, expected {}", f.loops.len(), eloops.len()),
                    &mut notes,
                );
            }
            let echans = exp["channels"].as_array().unwrap();
            for (c, ec) in f.channels.iter().zip(echans) {
                let col: Vec<u64> = ec["colour"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap())
                    .collect();
                let mut ok = c.name == ec["name"].as_str().unwrap()
                    && c.colour.map(|k| k.map(u64::from).to_vec()) == Some(col)
                    && c.components as u64 == ec["components"].as_u64().unwrap();
                if c.components == 1 {
                    ok &= c.emission == ec["emission"].as_f64();
                }
                if !ok {
                    miss(format!("channel {c:?} vs {ec}"), &mut notes);
                }
            }
            if f.channels.len() != echans.len() {
                miss(format!("{} channels", f.channels.len()), &mut notes);
            }
            for (k, v) in exp["text"].as_object().unwrap() {
                let name = match k.as_str() {
                    "description" => "Description",
                    "capturing" => "Capturing",
                    "date" => "Date",
                    "optics" => "Optics",
                    "appVersion" => "Application version",
                    "author" => "Author",
                    "sampleId" => "Sample ID",
                    "location" => "Location",
                    "conclusion" => "Conclusion",
                    "info1" => "Info 1",
                    "info2" => "Info 2",
                    "group" => "Group",
                    "imageId" => "Image ID",
                    "type" => "Type",
                    "sampling" => "Sampling",
                    o => o,
                };
                let want = v.as_str().unwrap().replace("\r\n", "\n");
                if f.text(name).map(str::trim) != Some(want.trim()) {
                    miss(
                        format!("text {name}: {:?} vs {want:?}", f.text(name)),
                        &mut notes,
                    );
                }
            }
            let mut buf = vec![0f32; f.width * f.height];
            let float = f.sample.is_float();
            for (key, hash) in exp["planes"].as_object().unwrap() {
                let (fr, comp) = key.split_once(':').unwrap();
                let (fr, comp): (usize, usize) = (fr.parse().unwrap(), comp.parse().unwrap());
                planes += 1;
                match f.read_plane(&data, fr, comp, &mut buf) {
                    Ok(_) if fnv(&buf, float) == hash.as_str().unwrap() => {}
                    r => {
                        bad += 1;
                        miss(format!("plane {key}: {r:?}"), &mut notes);
                    }
                }
            }
        }
        k => panic!("{path:?}: kind {k:?}"),
    }
    (planes, bad, notes)
}

fn run(dir: &Path, expected: &Path) -> (usize, usize, usize, usize) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            matches!(
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("lif" | "lof" | "nd2")
            )
        })
        .collect();
    files.sort();
    let (mut nfiles, mut nplanes, mut nbad, mut nmeta) = (0, 0, 0, 0);
    for p in files {
        let json = expected.join(format!("{}.json", p.file_name().unwrap().to_string_lossy()));
        let Ok(text) = std::fs::read_to_string(&json) else {
            eprintln!("{}: no expected output", p.display());
            continue;
        };
        let exp: J = serde_json::from_str(&text).unwrap();
        let t = Instant::now();
        let (planes, bad, notes) = check(&p, &exp);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        eprintln!(
            "{:28} {planes:5} planes {bad:3} pixel mismatches {:3} metadata  {ms:8.1} ms (parse + every plane, debug build)",
            p.file_name().unwrap().to_string_lossy(),
            notes.len() - bad
        );
        for n in notes.iter().take(8) {
            eprintln!("    {n}");
        }
        nfiles += 1;
        nplanes += planes;
        nbad += bad;
        nmeta += notes.len() - bad;
    }
    (nfiles, nplanes, nbad, nmeta)
}

#[test]
fn fixtures_match_oracles() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let (files, planes, bad, meta) = run(&dir, &dir.join("expected"));
    eprintln!(
        "fixtures: {files} files, {planes} planes, {bad} pixel mismatches, {meta} metadata mismatches"
    );
    assert_eq!(files, 8);
    assert_eq!((bad, meta), (0, 0));
}

#[test]
fn corpus_matches_oracles() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/vendormicro");
    if !dir.join("expected").is_dir() {
        eprintln!("no corpus (run tools/corpus/fetch-vendormicro.sh)");
        return;
    }
    let (files, planes, bad, meta) = run(&dir, &dir.join("expected"));
    eprintln!(
        "corpus: {files} files, {planes} planes, {bad} pixel mismatches, {meta} metadata mismatches"
    );
    assert_eq!(bad, 0);
}

#[test]
fn malformed_input_never_panics() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    for name in [
        "multi.lif",
        "single.lof",
        "loops_v3.nd2",
        "blobs_v3.nd2",
        "zlib_v3.nd2",
        "rgb_v2.nd2",
        "legacy_v1.nd2",
    ] {
        let data = std::fs::read(dir.join(name)).unwrap();
        // Truncations and byte flips must give errors or partial images, never panics.
        for cut in (0..data.len()).step_by(data.len() / 97 + 1) {
            let d = &data[..cut];
            touch(d);
        }
        let mut d = data.clone();
        let mut x: u32 = 12345;
        for _ in 0..200 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = x as usize % d.len();
            d[i] ^= (x >> 8) as u8 | 1;
            touch(&d);
        }
    }
}

fn touch(d: &[u8]) {
    if let Ok(f) = lif::parse(d, "x") {
        for img in &f.images {
            let n = img.width().saturating_mul(img.height());
            if n < 1 << 22 {
                let mut buf = vec![0f32; n];
                let _ = img.read_plane(d, 0, &[], &mut buf);
            }
        }
    }
    if let Ok(f) = nd2::parse(d)
        && f.width.saturating_mul(f.height) < 1 << 22
    {
        let mut buf = vec![0f32; f.width * f.height];
        let _ = f.read_plane(d, 0, 0, &mut buf);
    }
}
