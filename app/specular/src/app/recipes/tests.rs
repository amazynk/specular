use std::time::Duration;

use specular_settings::Recipe;

use super::*;

/// A fresh folder under the target folder (tests write nothing elsewhere).
fn scratch(name: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base
        .join("recipe-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn recipe(program: &str, args: &[&str], output: &str, load: Load) -> UserCommand {
    UserCommand {
        name: "Test recipe".into(),
        program: program.into(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        key: None,
        reload: false,
        recipe: Some(Recipe {
            output: output.into(),
            load,
            timeout: 20,
            series: false,
            shell: false,
        }),
    }
}

/// Runs a command line to its end (or its time limit), as the app does.
#[cfg_attr(not(unix), allow(dead_code))] // only the Unix tests use it
fn run(prog: OsString, args: Vec<OsString>, timeout: Duration) -> Result<(), String> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    launch::spawn_with_timeout(
        &prog,
        &args,
        None,
        timeout,
        Box::new(move |r| {
            let _ = tx.send(match r {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(s.to_string()),
                Err(e) => Err(format!("{:?}", e.kind())),
            });
        }),
    )
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(60)).unwrap()
}

#[test]
fn fingerprints_follow_what_runs() {
    let a = recipe("seg", &["{file}"], "{out}/x.png", Load::Labels);
    let mut b = a.clone();
    b.name = "Renamed".into();
    assert_eq!(fingerprint(&a), fingerprint(&b), "the name does not matter");
    b.args.push("--fast".into());
    assert_ne!(fingerprint(&a), fingerprint(&b));
    let mut c = a.clone();
    c.recipe.as_mut().unwrap().shell = true;
    assert_ne!(fingerprint(&a), fingerprint(&c));
    let d = scratch("trust");
    let path = d.join("trusted.txt");
    assert!(read_trust(&path).is_empty());
    let all: HashSet<u64> = [fingerprint(&a), 7].into_iter().collect();
    write_trust(&path, &all).unwrap();
    assert_eq!(read_trust(&path), all);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn outlines_follow_pixel_edges() {
    // A 2×2 square of 1 with a ring of 2 around a hole, side by side.
    #[rustfmt::skip]
    let l: Vec<u32> = vec![
        1, 1, 0, 2, 2, 2,
        1, 1, 0, 2, 0, 2,
        0, 0, 0, 2, 2, 2,
    ];
    let shapes = label_outlines(6, 3, &l).unwrap();
    assert_eq!(
        shapes.len(),
        3,
        "the square, and the ring's outside and hole"
    );
    let mut sq = shapes[0].points.clone();
    sq.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(sq, vec![[0.0, 0.0], [0.0, 2.0], [2.0, 0.0], [2.0, 2.0]]);
    assert!(shapes.iter().all(|s| s.kind == ShapeKind::Polygon));
    assert_ne!(shapes[0].color, shapes[1].color);
    let hole_or_ring: Vec<usize> = shapes[1..].iter().map(|s| s.points.len()).collect();
    assert_eq!(hole_or_ring, vec![4, 4]);
    assert!(label_outlines(2, 2, &[1, 2, 3]).is_err());
    assert!(label_outlines(3, 1, &[0, 0, 0]).unwrap().is_empty());
}

#[test]
fn command_lines() {
    let file = Path::new("/data/a b.tif");
    let files = vec![file.to_path_buf()];
    let t = Target {
        file,
        files: &files,
        page: 1,
    };
    let out = Path::new("/run/x{1}");
    let c = recipe(
        "seg",
        &["{file}", "--out", "{out}/m.png"],
        "{out}/m.png",
        Load::Labels,
    );
    let (_, args) = command_line(&c, &t, out).unwrap();
    assert_eq!(
        args,
        ["/data/a b.tif", "--out", "/run/x{1}/m.png"].map(OsString::from)
    );
    assert_eq!(
        expand_one("{out}/{stem}.png", &t, out).unwrap(),
        PathBuf::from("/run/x{1}/a b.png")
    );
    assert!(expand_one("{files}", &t, out).is_ok());
    assert!(expand_one("{nope}", &t, out).is_err());
    let mut s = c.clone();
    s.program = "seg {file} > {out}/log.txt".into();
    s.recipe.as_mut().unwrap().shell = true;
    if cfg!(unix) {
        let (prog, args) = command_line(&s, &t, out).unwrap();
        assert_eq!(prog, "sh");
        assert_eq!(args[1], "seg '/data/a b.tif' > '/run/x{1}'/log.txt");
    }
}

/// The whole path of a labels recipe: a fake segmenter written into the target folder runs
/// with the file, writes a label PNG, and the output comes back through the decoders as
/// outlines. A program that runs too long is stopped.
#[cfg(unix)]
#[test]
fn a_fake_segmenter_runs_and_its_labels_come_back() {
    use std::os::unix::fs::PermissionsExt;
    let d = scratch("fake-seg");
    // The "segmentation": two labelled squares, as a grey PNG.
    let (w, h) = (16u32, 8u32);
    let mut px = vec![0u8; (w * h) as usize];
    for y in 1..5 {
        for x in 1..5 {
            px[(y * w + x) as usize] = 1;
            px[(y * w + x + 8) as usize] = 7;
        }
    }
    let png = specular_png::encode(w, h, specular_png::ColorType::Gray, &px).unwrap();
    let labels = d.join("labels-source.png");
    std::fs::write(&labels, png).unwrap();
    let script = d.join("fake-seg.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\n# fake-seg INPUT OUTPUT\ntest -f \"$1\" || exit 3\ncp '{}' \"$2\"\n",
            labels.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let input = d.join("input image.png");
    std::fs::copy(&labels, &input).unwrap();
    let out = d.join("run-1");
    std::fs::create_dir_all(&out).unwrap();
    let files = vec![input.clone()];
    let t = Target {
        file: &input,
        files: &files,
        page: 1,
    };
    let c = recipe(
        &script.to_string_lossy(),
        &["{file}", "{out}/{stem}-labels.png"],
        "{out}/{stem}-labels.png",
        Load::Labels,
    );
    let output = expand_one(&c.recipe.as_ref().unwrap().output, &t, &out).unwrap();
    let (prog, args) = command_line(&c, &t, &out).unwrap();
    run(prog, args, Duration::from_secs(20)).unwrap();
    assert!(output.is_file());
    let shapes = load_labels(&specular_decode::Registry::new(), &output).unwrap();
    assert_eq!(shapes.len(), 2);
    let mut first = shapes[0].points.clone();
    first.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(first, vec![[1.0, 1.0], [1.0, 5.0], [5.0, 1.0], [5.0, 5.0]]);
    // A file that is not an image is refused by the decoders, not read raw.
    let bogus = d.join("bogus.png");
    std::fs::write(&bogus, b"not a png").unwrap();
    assert!(load_labels(&specular_decode::Registry::new(), &bogus).is_err());

    // GeoJSON shapes (an overlay recipe), written by a shell recipe.
    let mut g = recipe(
        "printf '%s' '{{\"type\":\"Feature\",\"properties\":{{\"name\":\"cell\"}},\"geometry\":{{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[4,0],[4,4],[0,0]]]}}}}' > {out}/shapes.geojson",
        &[],
        "{out}/shapes.geojson",
        Load::Overlay,
    );
    g.recipe.as_mut().unwrap().shell = true;
    let (prog, args) = command_line(&g, &t, &out).unwrap();
    run(prog, args, Duration::from_secs(20)).unwrap();
    let shapes = load_geojson(&out.join("shapes.geojson")).unwrap();
    assert_eq!(shapes.len(), 1);
    assert_eq!(shapes[0].kind, ShapeKind::Polygon);
    assert_eq!(shapes[0].text, "cell");

    // Too slow: stopped at its limit.
    let slow = recipe("sleep", &["30"], "{out}/never", Load::Open);
    let (prog, args) = command_line(&slow, &t, &out).unwrap();
    let t0 = std::time::Instant::now();
    assert_eq!(
        run(prog, args, Duration::from_millis(300)),
        Err("TimedOut".into())
    );
    assert!(t0.elapsed() < Duration::from_secs(10));
    let _ = std::fs::remove_dir_all(d);
}

/// A series recipe's shapes go on the files they name (they all went on the current file).
#[test]
fn series_shapes_go_on_the_files_they_name() {
    let d = scratch("series-geojson");
    let feature = |file: Option<&str>, name: &str| {
        let mut props = serde_json::json!({ "name": name });
        if let Some(f) = file {
            props["file"] = serde_json::Value::from(f);
        }
        serde_json::json!({
            "type": "Feature",
            "properties": props,
            "geometry": {"type": "Point", "coordinates": [1, 2]}
        })
    };
    let a = d.join("a.fits");
    let b = d.join("b.fits");
    let fc = serde_json::json!({"type": "FeatureCollection", "features": [
        feature(Some("a.fits"), "star 1"),
        feature(Some(b.to_str().unwrap()), "star 2"),
        feature(None, "here"),
        feature(Some("a.fits"), "star 3"),
        feature(Some("elsewhere.fits"), "lost"),
    ]});
    let path = d.join("shapes.geojson");
    std::fs::write(&path, fc.to_string()).unwrap();
    let groups = load_geojson_by_file(&path).unwrap();
    let files = vec![a.clone(), b.clone()];
    let on = |f: &Path| -> Vec<String> {
        groups
            .iter()
            .filter(|(n, _)| {
                n.as_deref()
                    .and_then(|n| named_file(&files, n))
                    .map(PathBuf::as_path)
                    == Some(f)
            })
            .flat_map(|(_, s)| s.iter().map(|s| s.text.clone()))
            .collect()
    };
    assert_eq!(on(&a), ["star 1", "star 3"]);
    assert_eq!(on(&b), ["star 2"]);
    let unnamed: Vec<_> = groups.iter().filter(|g| g.0.is_none()).collect();
    assert_eq!(unnamed.len(), 1);
    assert_eq!(unnamed[0].1[0].text, "here");
    assert!(named_file(&files, "elsewhere.fits").is_none());
    let _ = std::fs::remove_dir_all(d);
}
