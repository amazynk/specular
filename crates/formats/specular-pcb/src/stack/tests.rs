use super::*;

fn gbr(function: &str, body: &str) -> Vec<u8> {
    format!(
        "%TF.GenerationSoftware,Test,Gen,1.0*%\n%TF.ProjectId,demo,00,A*%\n%TF.FileFunction,{function}*%\n\
         %FSLAX46Y46*%\n%MOMM*%\n{body}M02*\n"
    )
    .into_bytes()
}

fn set() -> Vec<(String, Vec<u8>)> {
    let outline = "%ADD10C,0.1*%\nD10*\nX0Y0D02*\nG01*\nX20000000Y0D01*\nX20000000Y10000000D01*\n\
                   X0Y10000000D01*\nX0Y0D01*\n";
    let copper = "%ADD11C,1.5*%\n%ADD12C,0.25*%\nD11*\nX5000000Y5000000D03*\nX15000000Y5000000D03*\n\
                  D12*\nX5000000Y5000000D02*\nX15000000Y5000000D01*\n";
    let mask = "%ADD13C,1.7*%\nD13*\nX5000000Y5000000D03*\nX15000000Y5000000D03*\n";
    let silk = "%ADD14C,0.15*%\nD14*\nX1000000Y1000000D02*\nX3000000Y1000000D01*\n";
    vec![
        ("demo-Edge_Cuts.gbr".into(), gbr("Profile,NP", outline)),
        ("demo-F_Cu.gbr".into(), gbr("Copper,L1,Top", copper)),
        ("demo-B_Cu.gbr".into(), gbr("Copper,L2,Bot", copper)),
        ("demo-F_Mask.gbr".into(), gbr("Soldermask,Top", mask)),
        ("demo-F_Silkscreen.gbr".into(), gbr("Legend,Top", silk)),
        (
            "demo.drl".into(),
            b"M48\nMETRIC\nT1C0.8\n%\nT1\nX5.0Y5.0\nX15.0Y5.0\nM30\n".to_vec(),
        ),
        ("README.txt".into(), b"made for a test".to_vec()),
    ]
}

#[test]
fn a_board_set_is_a_stack_with_board_views() {
    let s = board_scene(&set(), &Limits::default()).expect("builds");
    assert_eq!(s.sheets.len(), 3);
    assert_eq!(s.sheets[0].name, "Layers");
    // Six layer files (the readme is skipped), the outline's layer holds the substrate.
    assert_eq!(s.layers.len(), 6);
    assert!(
        s.layers[0].name.starts_with("Bottom copper"),
        "{}",
        s.layers[0].name
    );
    assert!(s.layers.last().unwrap().name.starts_with("Drill"));
    let info = s.pcb.as_ref().unwrap();
    assert_eq!(info.kind, PcbKind::GerberSet);
    assert_eq!(info.copper_layers, 2);
    assert_eq!(info.holes, 2);
    assert_eq!(info.drill_sizes, 1);
    assert_eq!(info.generator.as_deref(), Some("Test Gen 1.0"));
    assert_eq!(info.project.as_deref(), Some("demo"));
    let (w, h) = info.size_mm.unwrap();
    assert!(
        (w - 20.0).abs() < 1e-6 && (h - 10.0).abs() < 1e-6,
        "{w} {h}"
    );
    assert!(
        info.layers
            .iter()
            .all(|l| l.from_x2 || l.name.ends_with(".drl"))
    );
    assert_eq!(s.layer_opacity.len(), s.layers.len());
    // The top view: substrate first, mask openings erased in the mask's group.
    let top = &s.sheets[1];
    let outline = s
        .layers
        .iter()
        .position(|l| l.name.contains("outline"))
        .unwrap() as u32;
    assert_eq!(top.items[0].layer, outline);
    assert!(top.items.iter().any(|i| i.color == Color::Clear));
    // The bottom view is mirrored about the board's centre: the same extents.
    let b = s.sheets[2].extents.unwrap();
    let t = top.extents.unwrap();
    assert!(
        (b.x0 - t.x0).abs() < 1e-6 && (b.x1 - t.x1).abs() < 1e-6,
        "{b:?} {t:?}"
    );
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
}

#[test]
fn a_single_file_is_one_layer() {
    let f = set();
    let s = single_scene(&f[1].0, &f[1].1, &Limits::default()).unwrap();
    assert_eq!(s.format, "Gerber");
    assert_eq!(s.version, "RS-274X X2");
    assert_eq!(s.layers.len(), 1);
    assert_eq!(s.layers[0].name, "Top copper (demo-F_Cu.gbr)");
    let info = s.pcb.as_ref().unwrap();
    assert_eq!(info.kind, PcbKind::Gerber);
    assert_eq!(info.apertures, 2);
    let d = single_scene(&f[5].0, &f[5].1, &Limits::default()).unwrap();
    assert_eq!(d.format, "Excellon");
    assert_eq!(d.pcb.as_ref().unwrap().holes, 2);
}

#[test]
fn broken_files_are_warnings_unless_nothing_reads() {
    let mut f = set();
    f.push((
        "broken.GTL".into(),
        b"G04 RS-274-D*\nD10*\nX1Y1D03*\nM02*\n".to_vec(),
    ));
    let s = board_scene(&f, &Limits::default()).unwrap();
    assert!(
        s.warnings.iter().any(|w| w.contains("RS-274-D")),
        "{:?}",
        s.warnings
    );
    let bad = vec![("x.gbr".to_string(), b"hello".to_vec())];
    assert!(board_scene(&bad, &Limits::default()).is_err());
}
