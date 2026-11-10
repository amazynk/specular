use super::*;
use specular_vector::{Item, Shape};

const MATRIX: &str = "STEP {\n   COL=1\n   NAME=panel\n}\nSTEP {\n   COL=2\n   NAME=pcb\n}\n\
LAYER {\n   ROW=1\n   CONTEXT=BOARD\n   TYPE=COMPONENT\n   NAME=comp_+_top\n   POLARITY=POSITIVE\n}\n\
LAYER {\n   ROW=2\n   CONTEXT=BOARD\n   TYPE=SOLDER_MASK\n   NAME=smt\n   POLARITY=POSITIVE\n}\n\
LAYER {\n   ROW=3\n   CONTEXT=BOARD\n   TYPE=SIGNAL\n   NAME=top\n   POLARITY=POSITIVE\n}\n\
LAYER {\n   ROW=4\n   CONTEXT=BOARD\n   TYPE=SIGNAL\n   NAME=bottom\n   POLARITY=POSITIVE\n}\n\
LAYER {\n   ROW=5\n   CONTEXT=BOARD\n   TYPE=SOLDER_MASK\n   NAME=smb\n   POLARITY=POSITIVE\n}\n\
LAYER {\n   ROW=6\n   CONTEXT=BOARD\n   TYPE=DRILL\n   NAME=drill\n   POLARITY=POSITIVE\n}\n\
LAYER {\n   ROW=7\n   CONTEXT=MISC\n   TYPE=DOCUMENT\n   NAME=notes\n   POLARITY=POSITIVE\n}\n";

const PROFILE: &str = "UNITS=MM\n#\nS P 0\nOB 0 0 I\nOS 50 0\nOS 50 30\nOS 0 30\nOS 0 0\nOE\n\
OB 40 10 H\nOC 40 10 40 12 N\nOE\nSE\n";

const TOP: &str = "UNITS=MM\n$0 r200\n$1 rect1000x500\n$2 donut_r1000x500\n$3 myfid\n\
#\nP 10 10 1 P 0 0 ;0=1\nL 10 10 20 10 0 P 0\nA 20 10 22 12 20 12 0 P 0 Y\n\
P 30 20 2 P 0 8 45\nP 35 20 1 P 0 1\nP 5 25 3 P 0 0\nT 1 1 standard P 0 0.1 0.1 1 'HI' 1\n\
S P 0\nOB 1 1 I\nOS 3 1\nOS 3 3\nOS 1 3\nOE\nSE\n";

const FID: &str = "UNITS=MM\n$0 r500\n#\nP 0 0 0 P 0 0\nP 0 0 0 N 0 0\n";

const BOTTOM: &str = "UNITS=INCH\n$0 r10\n#\nL 0.5 0.5 1.5 0.5 0 P 0\n";

const DRILL: &str = "UNITS=MM\n$0 r800\n$1 r300\n#\nP 10 10 0 P 0 0\nP 20 10 0 P 0 0\n\
P 30 10 1 P 0 0\nL 40 5 40 8 0 P 0\n";

const COMPS: &str = "UNITS=MM\n# CMP 0\nCMP 0 25 15 90 N R1 RES10K ;0=1\nPRP VALUE '10k'\n\
CMP 1 5 5 0 N C7 CAP\n";

const EDA: &str = "UNITS=MM\nPKG R0603 0.5 -0.8 -0.4 0.8 0.4\nPKG C0402 0.5 -0.5 -0.25 0.5 0.25\n";

fn job() -> Vec<(String, Vec<u8>)> {
    let f = |p: &str, t: &str| (format!("myjob/{p}"), t.as_bytes().to_vec());
    vec![
        f("matrix/matrix", MATRIX),
        f(
            "misc/info",
            "JOB_NAME=myjob\nODB_VERSION_MAJOR=8\nODB_VERSION_MINOR=1\nODB_SOURCE=Unit test\n",
        ),
        f("steps/panel/stephdr", "STEP-REPEAT {\nNAME=pcb\n}\n"),
        f("steps/panel/profile", PROFILE),
        f("steps/pcb/stephdr", "UNITS=MM\n"),
        f("steps/pcb/profile", PROFILE),
        f("steps/pcb/layers/top/features", TOP),
        f("steps/pcb/layers/bottom/features", BOTTOM),
        f(
            "steps/pcb/layers/smt/features",
            "UNITS=MM\n$0 r1200\n#\nP 10 10 0 P 0 0\n",
        ),
        f("steps/pcb/layers/drill/features", DRILL),
        f("steps/pcb/layers/comp_+_top/components", COMPS),
        f("steps/pcb/eda/data", EDA),
        f("symbols/myfid/features", FID),
    ]
}

fn area(items: &[Item], layer: u32) -> f64 {
    items
        .iter()
        .filter(|i| i.layer == layer)
        .map(|i| match &i.shape {
            Shape::Fill { path, .. } => path.area(),
            _ => 0.0,
        })
        .sum()
}

#[test]
fn reads_a_job() {
    let files = job();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(looks_like_job(&names));
    let s = scene(&files, &Limits::default()).unwrap();
    assert_eq!(s.format, "ODB++");
    assert_eq!(s.version, "8.1");
    let info = s.pcb.as_ref().unwrap();
    assert_eq!(info.kind, PcbKind::Odb);
    assert_eq!(info.copper_layers, 2);
    assert_eq!(info.components, 2);
    assert_eq!(info.holes, 4);
    assert_eq!(info.generator.as_deref(), Some("Unit test"));
    let (w, h) = info.size_mm.unwrap();
    assert!((w - 50.0).abs() < 1e-9 && (h - 30.0).abs() < 1e-9);
    let roles: Vec<&str> = info.layers.iter().map(|l| l.role.as_str()).collect();
    // In stack order (bottom up); smb has no features file.
    assert_eq!(
        roles,
        vec![
            "Bottom copper",
            "Top copper",
            "Top solder mask",
            "Plated drill"
        ]
    );
    let layer_names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
    assert!(layer_names.contains(&"Top components"), "{layer_names:?}");
    // The board step first, then the panel (its one copy of the board), each named.
    let groups: Vec<&str> = s.sheets.iter().filter_map(|s| s.group.as_deref()).collect();
    assert_eq!(groups, ["pcb", "pcb", "pcb", "panel", "panel", "panel"]);
    // The text is drawn in the built-in font (the job has no fonts/standard).
    assert!(
        s.warnings
            .iter()
            .any(|w| w.contains("built-in font: standard")),
        "{:?}",
        s.warnings
    );
    let counts: std::collections::HashMap<&str, u64> =
        s.counts.iter().map(|(k, n)| (k.as_str(), *n)).collect();
    // Top 4, mask 1, drill 3, and the user symbol's 2 once.
    assert_eq!(counts["Pad"], 4 + 1 + 3 + 2);
    assert_eq!(counts["Component"], 2);
    // The bottom layer is in inches: 1 inch of a 10 mil round line.
    let bottom = s
        .layers
        .iter()
        .position(|l| l.name.contains("(bottom)"))
        .unwrap() as u32;
    let a = area(&s.sheets[0].items, bottom);
    let want = 25.4 * 0.254 + std::f64::consts::PI * 0.127 * 0.127;
    assert!((a - want).abs() < 1e-3, "{a} {want}");
    // Three sheets a step: layers, top, bottom; the top shows R1's reference.
    assert_eq!(s.sheets.len(), 6);
    let texts: Vec<String> = s.sheets[1]
        .items
        .iter()
        .filter_map(|i| match &i.shape {
            Shape::Text(t) => Some(t.plain()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["R1", "C7"]);
}

/// A small job of one step: `extra` files over a matrix of one signal layer `sig` (with
/// `polarity`) and a profile 50 × 30 mm with a 2 mm round cutout.
fn small_job(polarity: &str, extra: &[(&str, &str)]) -> Vec<(String, Vec<u8>)> {
    let matrix = format!(
        "STEP {{\n   COL=1\n   NAME=pcb\n}}\nLAYER {{\n   ROW=1\n   CONTEXT=BOARD\n   \
         TYPE=SIGNAL\n   NAME=sig\n   POLARITY={polarity}\n}}\n"
    );
    let mut v = vec![
        ("j/matrix/matrix".to_string(), matrix.into_bytes()),
        ("j/steps/pcb/stephdr".into(), b"UNITS=MM\n".to_vec()),
        ("j/steps/pcb/profile".into(), PROFILE.as_bytes().to_vec()),
    ];
    for (p, t) in extra {
        v.push((format!("j/{p}"), t.as_bytes().to_vec()));
    }
    v
}

/// The line segments (ends) of a layer's objects, rounded to 1 µm.
fn segments(objs: &[Object]) -> Vec<[(i64, i64); 2]> {
    let r = |p: Point| ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64);
    let mut v: Vec<_> = objs
        .iter()
        .filter_map(|o| match o.segment {
            Some(Segment::Line(a, b)) => Some([r(a), r(b)]),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

fn read_layer(files: &[(String, Vec<u8>)], step: &str, layer: &str) -> Vec<Object> {
    let job = Job::new(files);
    let mut r = Reader {
        job: &job,
        user: HashMap::new(),
        tally: Tally::default(),
        limits: Limits::default(),
        items: 0,
        warnings: Vec::new(),
        fonts: HashMap::new(),
        steps: HashMap::new(),
        copies: 0,
    };
    r.step_layer(step, layer, 0).as_ref().clone()
}

/// A square "H" font: 1 × 1 cells, half a cell between them.
const FONT: &str = "XSIZE 1\nYSIZE 1\nOFFSET 0.5\n\nCHAR H\nLINE 0 0 0 1 P R 0.01\n\
LINE 1 0 1 1 P R 0.01\nLINE 0 0.5 1 0.5 P S 0.01\nECHAR\n";

#[test]
fn text_in_the_job_font() {
    // "HH" at (10, 5) in 2 × 3 mm cells; then the same turned 90° clockwise and mirrored.
    let feats =
        "UNITS=MM\n#\nT 10 5 standard P 0 2 3 1 'HH' 1\nT 0 0 standard N 9 90 2 3 0.5 'H' 1\n";
    let files = small_job(
        "POSITIVE",
        &[
            ("fonts/standard", FONT),
            ("steps/pcb/layers/sig/features", feats),
        ],
    );
    let objs = read_layer(&files, "pcb", "sig");
    assert_eq!(objs.len(), 9);
    let segs = segments(&objs);
    // Stems of the first H at x 10 and 12, the second's at 13 and 15 (a 1 mm gap), 5 to 8.
    for x in [10_000, 12_000, 13_000, 15_000] {
        assert!(segs.contains(&[(x, 5000), (x, 8000)]), "{x} {segs:?}");
    }
    assert!(segs.contains(&[(10_000, 6500), (12_000, 6500)]));
    // 12 mil wide, round; the cross bar square.
    let w = objs[0].path.bounding_box().width();
    assert!((w - 0.3048).abs() < 1e-6, "{w}");
    // Mirrored, then turned 90° clockwise: (x, y) → (−x, y) → (y, x). The stems run along
    // y = 0 and y = 2.
    let last: Vec<_> = segments(&objs[6..]);
    assert!(last.contains(&[(0, 0), (3000, 0)]), "{last:?}");
    assert!(last.contains(&[(0, 2000), (3000, 2000)]), "{last:?}");
    assert!(objs[6..].iter().all(|o| !o.dark));
}

#[test]
fn text_without_a_font_uses_the_built_in_one() {
    let feats = "UNITS=MM\n#\nT 0 0 nosuch P 0 6 6 1 'I' 1\n";
    let files = small_job("POSITIVE", &[("steps/pcb/layers/sig/features", feats)]);
    let segs = segments(&read_layer(&files, "pcb", "sig"));
    // The built-in "I": a stem in the cell's middle, baseline to cap height.
    assert!(segs.contains(&[(3000, 6000), (3000, 0)]), "{segs:?}");
    let s = scene(&files, &Limits::default()).unwrap();
    assert!(
        s.warnings
            .iter()
            .any(|w| w.contains("built-in font: nosuch"))
    );
    assert!(s.counts.iter().any(|(k, n)| k == "Text" && *n == 1));
}

#[test]
fn negative_layers_are_drawn_negative() {
    let feats = "UNITS=MM\n$0 r2000\n#\nP 10 10 0 P 0 0\nP 20 10 0 N 0 0\n";
    let files = small_job("NEGATIVE", &[("steps/pcb/layers/sig/features", feats)]);
    let s = scene(&files, &Limits::default()).unwrap();
    assert!(
        !s.warnings.iter().any(|w| w.contains("negative")),
        "{:?}",
        s.warnings
    );
    let items: Vec<&Item> = s.sheets[0].items.iter().filter(|i| i.layer == 0).collect();
    // The profile dark (less its cutout), the positive pad clear, the negative pad dark.
    let Shape::Fill { path, .. } = &items[0].shape else {
        panic!()
    };
    let want = 50.0 * 30.0 - std::f64::consts::PI * 4.0;
    assert!((path.area().abs() - want).abs() < 0.05, "{}", path.area());
    assert_ne!(items[0].color, specular_vector::Color::Clear);
    assert_eq!(items[1].color, specular_vector::Color::Clear);
    assert_ne!(items[2].color, specular_vector::Color::Clear);
    // The top view: copper everywhere but the clearance.
    let top: Vec<&Item> = s.sheets[1].items.iter().filter(|i| i.layer == 0).collect();
    assert!(top.iter().any(|i| i.color == specular_vector::Color::Clear));
}

#[test]
fn step_and_repeat_copies() {
    let matrix = "STEP {\n COL=1\n NAME=panel\n}\nSTEP {\n COL=2\n NAME=one\n}\n\
LAYER {\n ROW=1\n CONTEXT=BOARD\n TYPE=SIGNAL\n NAME=sig\n POLARITY=POSITIVE\n}\n";
    let files: Vec<(String, Vec<u8>)> = [
        ("matrix/matrix", matrix),
        (
            "steps/panel/stephdr",
            "UNITS=MM\nSTEP-REPEAT {\n NAME=one\n X=100\n Y=50\n DX=20\n DY=10\n NX=3\n NY=2\n \
             ANGLE=0\n MIRROR=NO\n}\nSTEP-REPEAT {\n NAME=one\n X=0\n Y=0\n DX=0\n DY=0\n NX=1\n \
             NY=1\n ANGLE=90\n MIRROR=YES\n}\n",
        ),
        (
            "steps/panel/layers/sig/features",
            "UNITS=MM\n$0 r100\n#\nL 0 0 1 0 0 P 0\n",
        ),
        ("steps/one/stephdr", "UNITS=MM\nX_DATUM=1\nY_DATUM=2\n"),
        (
            "steps/one/profile",
            "UNITS=MM\n#\nS P 0\nOB 0 0 I\nOS 4 0\nOS 4 2\nOS 0 2\nOS 0 0\nOE\nSE\n",
        ),
        (
            "steps/one/layers/sig/features",
            "UNITS=MM\n$0 r0\n#\nL 1 2 4 2 0 P 0\n",
        ),
    ]
    .iter()
    .map(|(p, t)| (format!("j/{p}"), t.as_bytes().to_vec()))
    .collect();
    let objs = read_layer(&files, "panel", "sig");
    // The panel's own line, six copies, one more turned and mirrored.
    assert_eq!(objs.len(), 1 + 6 + 1);
    let segs = segments(&objs);
    for (i, j) in [(0, 0), (1, 0), (2, 0), (0, 1), (2, 1)] {
        let (x, y) = (100_000 + i * 20_000, 50_000 + j * 10_000);
        assert!(segs.contains(&[(x, y), (x + 3000, y)]), "{i} {j} {segs:?}");
    }
    // (3, 0) from the datum, mirrored (−3, 0), turned 90° clockwise: (0, 3).
    assert!(segs.contains(&[(0, 0), (0, 3000)]), "{segs:?}");
    // The chosen step is "one" (no step-and-repeat); the panel follows as its own pages.
    let s = scene(&files, &Limits::default()).unwrap();
    // Its board views outline each of the seven boards (one's 4 × 2 mm profile).
    let lines: Vec<_> = s.sheets[4]
        .items
        .iter()
        .filter(|i| i.color == specular_vector::Color::Rgb(70, 64, 30))
        .collect();
    assert_eq!(lines.len(), 7, "{}", s.sheets[4].items.len());
    let first = s.sheets[4]
        .items
        .iter()
        .filter(|i| i.color == specular_vector::Color::Rgb(70, 64, 30))
        .filter_map(|i| match &i.shape {
            specular_vector::Shape::Stroke { path, .. } => Some(path.bounding_box()),
            _ => None,
        })
        .any(|b| (b.x0 - 99.0).abs() < 1e-6 && (b.y0 - 48.0).abs() < 1e-6);
    assert!(
        first,
        "the first copy's outline at (100, 50) less the datum (1, 2)"
    );
    assert_eq!(s.sheets[0].group.as_deref(), Some("one"));
    assert_eq!(s.sheets[3].group.as_deref(), Some("panel"));
    assert_eq!(s.sheets.len(), 6);
}

#[test]
fn symbols() {
    let k = 0.001;
    assert_eq!(standard("r200", k), Some(Prim::Circle(0.2)));
    assert_eq!(standard("s100", k), Some(Prim::Rect(0.1, 0.1)));
    assert_eq!(standard("rect200x100", k), Some(Prim::Rect(0.2, 0.1)));
    assert_eq!(
        standard("rect200x100xr25", k),
        Some(Prim::RoundRect(0.2, 0.1, 0.025))
    );
    assert_eq!(
        standard("rect200x100xc25x13", k),
        Some(Prim::Chamfer(0.2, 0.1, 0.025))
    );
    assert_eq!(standard("oval300x100", k), Some(Prim::Oval(0.3, 0.1)));
    assert_eq!(standard("donut_r100x50", k), Some(Prim::Donut(0.1, 0.05)));
    assert_eq!(
        standard("thr100x60x45x4x10", k),
        Some(Prim::Thermal(0.1, 0.06, 45.0, 4, 0.01))
    );
    assert_eq!(standard("myfid", k), None);
    assert_eq!(orient(&["8", "30"]), (30.0, false));
    assert_eq!(orient(&["5"]), (90.0, true));
}

/// A `.Z` written by the system's `compress` (tests/data/odb/features.Z): 320 kB of text, so
/// codes grow to 16 bits.
#[test]
fn compress_files_unpack() {
    let z = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/odb/features.Z"
    ))
    .unwrap();
    let mut want = String::new();
    for i in 0u64..12000 {
        want += &format!(
            "P {} {} {} P 0 0 ;0={}\n",
            (i * 7919) % 100_003,
            (i * 104_729) % 7919,
            i % 12,
            (i * 31) % 40
        );
    }
    let (name, back) = unpack("features.Z", z.clone(), 1 << 24).unwrap();
    assert_eq!(name, "features");
    assert_eq!(back.len(), want.len());
    assert!(back == want.as_bytes());
    assert!(matches!(unpack("f.z", z, 1000), Err(Error::TooLarge)));
    assert!(lzw::decompress(&[0x1F, 0x9D, 0x90, 0xFF, 0xFF, 0xFF], 100).is_err());
}

#[test]
fn damaged_jobs_fail_cleanly() {
    let files = job();
    let mut x: u32 = 0x1234_5678;
    for _ in 0..200 {
        let mut f = files.clone();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        let i = x as usize % f.len();
        let b = &mut f[i].1;
        if !b.is_empty() {
            let at = (x >> 8) as usize % b.len();
            b[at] = (x >> 3) as u8;
            b.truncate(b.len() - (x as usize % b.len().max(1)) / 2);
        }
        let _ = scene(&f, &Limits::default());
    }
    assert!(scene(&files[1..], &Limits::default()).is_err());
}

/// A hand-made panel: board "one" (one component, U1 at (2, 1) turned 30° clockwise, datum
/// (1, 2)) twice side by side, and once more turned 90° clockwise and mirrored. The panel
/// lists and draws U1 once per board, each placed by its copy.
#[test]
fn panel_components_follow_each_repeat() {
    let matrix = "STEP {\n COL=1\n NAME=panel\n}\nSTEP {\n COL=2\n NAME=one\n}\n\
LAYER {\n ROW=1\n CONTEXT=BOARD\n TYPE=COMPONENT\n NAME=comp_+_top\n POLARITY=POSITIVE\n}\n\
LAYER {\n ROW=2\n CONTEXT=BOARD\n TYPE=SIGNAL\n NAME=sig\n POLARITY=POSITIVE\n}\n";
    let files: Vec<(String, Vec<u8>)> = [
        ("matrix/matrix", matrix),
        (
            "steps/panel/stephdr",
            "UNITS=MM\nSTEP-REPEAT {\n NAME=one\n X=100\n Y=50\n DX=20\n DY=0\n NX=2\n NY=1\n \
             ANGLE=0\n MIRROR=NO\n}\nSTEP-REPEAT {\n NAME=one\n X=0\n Y=0\n DX=0\n DY=0\n NX=1\n \
             NY=1\n ANGLE=90\n MIRROR=YES\n}\n",
        ),
        ("steps/one/stephdr", "UNITS=MM\nX_DATUM=1\nY_DATUM=2\n"),
        (
            "steps/one/profile",
            "UNITS=MM\n#\nS P 0\nOB 0 0 I\nOS 4 0\nOS 4 2\nOS 0 2\nOS 0 0\nOE\nSE\n",
        ),
        (
            "steps/one/layers/sig/features",
            "UNITS=MM\n$0 r100\n#\nL 1 2 4 2 0 P 0\n",
        ),
        (
            "steps/one/layers/comp_+_top/components",
            "UNITS=MM\nCMP 0 2 1 30 N U1 IC\n",
        ),
        (
            "steps/one/eda/data",
            "UNITS=MM\nPKG SOT 0.5 -0.5 -0.25 0.5 0.25\n",
        ),
    ]
    .iter()
    .map(|(p, t)| (format!("j/{p}"), t.as_bytes().to_vec()))
    .collect();
    let job = Job::new(&files);
    let mut r = Reader {
        job: &job,
        user: HashMap::new(),
        tally: Tally::default(),
        limits: Limits::default(),
        items: 0,
        warnings: Vec::new(),
        fonts: HashMap::new(),
        steps: HashMap::new(),
        copies: 0,
    };
    let layers = vec![
        MatLayer {
            row: 1,
            name: "comp_+_top".into(),
            ty: "COMPONENT".into(),
            negative: false,
        },
        MatLayer {
            row: 2,
            name: "sig".into(),
            ty: "SIGNAL".into(),
            negative: false,
        },
    ];
    let one = r.step_components("one", &layers, 0);
    assert_eq!(one.len(), 1);
    assert!((one[0].rotation - -30.0).abs() < 1e-9);
    let comps = r.step_components("panel", &layers, 0);
    assert_eq!(comps.len(), 3, "{comps:?}");
    let near = |c: &Component, x: f64, y: f64, rot: f64| {
        (c.at.x - x).abs() < 1e-9
            && (c.at.y - y).abs() < 1e-9
            && ((c.rotation - rot).rem_euclid(360.0) < 1e-9
                || (rot - c.rotation).rem_euclid(360.0) < 1e-9)
    };
    // (2, 1) less the datum: (1, −1) from each copy's place.
    assert!(near(&comps[0], 101.0, 49.0, 330.0), "{:?}", comps[0]);
    assert!(near(&comps[1], 121.0, 49.0, 330.0), "{:?}", comps[1]);
    // Mirrored (−1, −1), turned 90° clockwise (−1, 1); 30° clockwise mirrored is 210°,
    // turned 90° clockwise 120°.
    assert!(near(&comps[2], -1.0, 1.0, 120.0), "{:?}", comps[2]);
    assert!(
        comps
            .iter()
            .all(|c| c.refdes == "U1" && c.side == Side::Top)
    );
    // Each outline moved with its copy: the mirrored one's centre is its place.
    let b = comps[2].outline.as_ref().unwrap().bounding_box();
    assert!((b.center().x + 1.0).abs() < 1e-6 && (b.center().y - 1.0).abs() < 1e-6);
    // The panel's pages draw U1 three times.
    let s = scene(&files, &Limits::default()).unwrap();
    let panel = s
        .sheets
        .iter()
        .filter(|sh| sh.group.as_deref() == Some("panel"))
        .map(|sh| {
            sh.items
                .iter()
                .filter(|i| match &i.shape {
                    Shape::Text(t) => t
                        .paragraphs
                        .iter()
                        .any(|p| p.spans.iter().any(|s| s.text == "U1")),
                    _ => false,
                })
                .count()
        })
        .max()
        .unwrap();
    assert_eq!(panel, 3);
}
