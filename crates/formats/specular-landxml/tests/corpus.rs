//! LandXML samples (tools/corpus/fetch-eng2.sh) against the independent numbers of
//! tools/corpus/landxml-oracle.py:
//!
//!   python3 -I tools/corpus/landxml-oracle.py $CORPUS/landxml > $CORPUS/landxml-oracle.json
//!   SPECULAR_ENG2_CORPUS=$CORPUS cargo test -p specular-landxml --test corpus -- --ignored --nocapture

use std::path::PathBuf;

/// With `SPECULAR_ENG2_RENDERS` set: every sheet as `<dir>/<name>-<sheet>.png`.
fn render(name: &str, scene: specular_vector::VectorScene) {
    use specular_vector::draw::{View, fit_transform};
    let Some(dir) = std::env::var_os("SPECULAR_ENG2_RENDERS") else {
        return;
    };
    let fonts = specular_vector::fonts::Fonts::bundled_only();
    let p = specular_vector::prep::prepare(
        std::sync::Arc::new(scene),
        &fonts,
        specular_vector::prep::PrepLimits::default(),
    );
    for (i, sh) in p.sheets.iter().enumerate() {
        let Some(fit) = sh.fit else { continue };
        let size = (1600, 1200);
        let layers = vec![true; p.scene.layers.len()];
        let view = View {
            transform: fit_transform(fit, size, 8.0),
            size,
            dark: false,
            background: [250, 250, 250],
            lineweights: true,
            dpi_scale: 1.0,
            layers: &layers,
        };
        if let Some((rgba, _)) = specular_vector::raster::render(sh, &view) {
            let png =
                specular_png::encode(size.0, size.1, specular_png::ColorType::Rgba, &rgba).unwrap();
            let out = std::path::Path::new(&dir).join(format!("{name}-{i}.png"));
            std::fs::write(out, png).unwrap();
        }
    }
}

#[test]
#[ignore = "needs the corpus (tools/corpus/fetch-eng2.sh)"]
fn samples_match_the_oracle() {
    let Some(root) = std::env::var_os("SPECULAR_ENG2_CORPUS").map(PathBuf::from) else {
        eprintln!("SPECULAR_ENG2_CORPUS not set");
        return;
    };
    let oracle: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("landxml-oracle.json")).expect("oracle"),
    )
    .unwrap();
    let (mut checked, mut bad) = (0, 0);
    for (name, want) in oracle.as_object().unwrap() {
        let bytes = std::fs::read(root.join("landxml").join(name)).unwrap();
        let t0 = std::time::Instant::now();
        let x = match specular_landxml::parse(&bytes) {
            Ok(x) => x,
            Err(e) => {
                let empty = want["surfaces"].as_array().unwrap().is_empty()
                    && want["alignments"].as_array().unwrap().is_empty();
                eprintln!("{name}: {e}{}", if empty { "" } else { "  MISMATCH" });
                bad += usize::from(!empty);
                continue;
            }
        };
        let s = specular_landxml::scene(&x);
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        render(name, s.clone());
        let mut notes = Vec::new();
        let ws = want["surfaces"].as_array().unwrap();
        let got: Vec<_> = x.surfaces.iter().filter(|s| !s.faces.is_empty()).collect();
        checked += 1;
        if ws.len() != got.len() {
            notes.push(format!("surfaces {} vs {}", got.len(), ws.len()));
        }
        for (g, w) in got.iter().zip(ws) {
            let m = specular_landxml::tin::measures(g).unwrap();
            for (k, v) in [
                ("area_2d", m.area_2d),
                ("area_3d", m.area_3d),
                ("volume", m.volume),
                ("z_min", m.z_min),
                ("z_max", m.z_max),
            ] {
                checked += 1;
                let wv = w[k].as_f64().unwrap();
                if (v - wv).abs() > 1e-6 * wv.abs().max(1.0) {
                    notes.push(format!("{} {k} {v} vs {wv}", g.name));
                }
            }
            checked += 1;
            if g.faces.len() as u64 != w["triangles"].as_u64().unwrap() {
                notes.push(format!("{} triangles {}", g.name, g.faces.len()));
            }
            // The 3D model drapes on the surface: its elevation at the probes.
            for p in w["probes"].as_array().unwrap() {
                let v: Vec<f64> = p
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap())
                    .collect();
                checked += 1;
                match specular_landxml::model::elevation_at(g, [v[0], v[1]]) {
                    Some(z) if (z - v[2]).abs() <= 1e-6 * v[2].abs().max(1.0) => {}
                    z => notes.push(format!("{} z at {v:?}: {z:?}", g.name)),
                }
            }
        }
        // Parcels and survey points.
        let wp = want["parcels"].as_array().unwrap();
        checked += 1;
        if wp.len() != x.parcels.len() {
            notes.push(format!("parcels {} vs {}", x.parcels.len(), wp.len()));
        }
        for (p, w) in x.parcels.iter().zip(wp) {
            checked += 1;
            let len: f64 = p.boundary.iter().map(|g| g.length()).sum();
            let wl = w["length"].as_f64().unwrap();
            if (len - wl).abs() > 1e-6 * wl.max(1.0) {
                notes.push(format!("parcel {} length {len} vs {wl}", p.name));
            }
        }
        let wq = &want["points"];
        let (se, sn) = x
            .points
            .iter()
            .fold((0.0, 0.0), |a, p| (a.0 + p.at[0], a.1 + p.at[1]));
        checked += 3;
        if x.points.len() as u64 != wq["count"].as_u64().unwrap()
            || (se - wq["sum_e"].as_f64().unwrap()).abs() > 1e-6 * se.abs().max(1.0)
            || (sn - wq["sum_n"].as_f64().unwrap()).abs() > 1e-6 * sn.abs().max(1.0)
        {
            notes.push(format!("points {} {se} {sn}", x.points.len()));
        }
        let t1 = std::time::Instant::now();
        let model = specular_landxml::model::model(&x);
        let model_ms = t1.elapsed().as_secs_f64() * 1e3;
        if let Some(m) = &model {
            eprintln!(
                "  3D: {} triangles drawn, {} contour and {} alignment segments, {} vertices, {model_ms:.0} ms",
                m.triangles,
                m.contour_segments,
                m.alignment_segments,
                m.positions.len()
            );
        }
        let wa = want["alignments"].as_array().unwrap();
        for (a, w) in x.alignments.iter().zip(wa) {
            checked += 1;
            let wl = w["length"].as_f64().unwrap();
            if (a.computed_length() - wl).abs() > 1e-6 * wl.max(1.0) {
                notes.push(format!("{} length {} vs {wl}", a.name, a.computed_length()));
            }
            // Where the file states element ends, ours agree.
            let worst = a
                .elements
                .iter()
                .filter_map(|e| {
                    let (c, s) = e.ends();
                    s.map(|s| (c[0] - s[0]).hypot(c[1] - s[1]))
                })
                .fold(0.0, f64::max);
            eprintln!("  {}: worst element end {worst:.6}", a.name);
        }
        bad += notes.len();
        eprintln!(
            "{name}: {} surfaces, {} alignments, {} parcels, {} points, {} sheets, {ms:.0} ms, crs {:?}, warnings {:?}{}",
            got.len(),
            x.alignments.len(),
            x.parcels.len(),
            x.points.len(),
            s.sheets.len(),
            x.crs.as_ref().and_then(|c| c.describe()),
            s.warnings,
            if notes.is_empty() {
                String::new()
            } else {
                format!("  MISMATCH {}", notes.join("; "))
            }
        );
    }
    eprintln!("{checked} values checked, {bad} mismatches");
    assert_eq!(bad, 0);
}
