//! Screenshot tests of drawings: a synthetic floor plan (DXF, generated here) drawn by vello
//! over the view as the viewer draws it, with the layers panel: dark and light paper at 1×
//! and 1.5×, a layer turned off, a corner zoomed 200× with lineweights (still sharp), and
//! right to left in Hebrew. Text uses the bundled fonts only, so every machine draws the
//! same glyphs. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular cad::shots`, then review and
//! commit the rewritten PNGs.

use specular_core::Orientation;
use specular_decode::drawing::specular_cad;
use specular_decode::drawing::specular_vector::fonts::Fonts;
use specular_decode::drawing::specular_vector::prep::{PrepLimits, prepare};
use specular_render::{PxRect, View};
use specular_shots::{Goldens, Harness};

use super::*;

const SIZE: (f32, f32) = (480.0, 320.0);

/// Group code and value pairs as DXF text.
fn pairs(out: &mut String, items: &[(i32, &str)]) {
    for (c, v) in items {
        out.push_str(&format!("{c}\n{v}\n"));
    }
}

/// A small floor plan in millimetres: walls (with lineweight), a door swing, a bay window
/// (a bulged polyline), a column, a filled solid, a dashed hidden line, a dimension line,
/// text and multiline text, chairs as an arrayed block (one on a layer that is off), a
/// point and a true-colour line.
pub(super) fn plan() -> Vec<u8> {
    let mut s = String::new();
    pairs(
        &mut s,
        &[
            (0, "SECTION"),
            (2, "HEADER"),
            (9, "$ACADVER"),
            (1, "AC1015"),
            (9, "$INSUNITS"),
            (70, "4"),
            (9, "$LWDISPLAY"),
            (290, "0"),
            (0, "ENDSEC"),
            (0, "SECTION"),
            (2, "TABLES"),
            (0, "TABLE"),
            (2, "LTYPE"),
            (70, "2"),
            (0, "LTYPE"),
            (2, "CONTINUOUS"),
            (70, "0"),
            (3, "Solid line"),
            (72, "65"),
            (73, "0"),
            (40, "0.0"),
            (0, "LTYPE"),
            (2, "DASHED"),
            (70, "0"),
            (3, "Dashed __ __ __"),
            (72, "65"),
            (73, "2"),
            (40, "300.0"),
            (49, "200.0"),
            (49, "-100.0"),
            (0, "ENDTAB"),
            (0, "TABLE"),
            (2, "LAYER"),
            (70, "7"),
        ],
    );
    for (name, colour, ltype, lw) in [
        ("0", "7", "CONTINUOUS", "-3"),
        ("WALLS", "7", "CONTINUOUS", "70"),
        ("DOORS", "1", "CONTINUOUS", "25"),
        ("DIMENSIONS", "3", "CONTINUOUS", "18"),
        ("TEXT", "5", "CONTINUOUS", "-3"),
        ("HIDDEN", "30", "DASHED", "-3"),
        ("FURNITURE", "-6", "CONTINUOUS", "-3"),
    ] {
        pairs(
            &mut s,
            &[
                (0, "LAYER"),
                (2, name),
                (70, "0"),
                (62, colour),
                (6, ltype),
                (370, lw),
            ],
        );
    }
    pairs(
        &mut s,
        &[
            (0, "ENDTAB"),
            (0, "TABLE"),
            (2, "STYLE"),
            (70, "1"),
            (0, "STYLE"),
            (2, "STANDARD"),
            (70, "0"),
            (40, "0.0"),
            (41, "1.0"),
            (50, "0.0"),
            (71, "0"),
            (42, "250.0"),
            (3, "arial.ttf"),
            (0, "ENDTAB"),
            (0, "ENDSEC"),
            (0, "SECTION"),
            (2, "BLOCKS"),
            (0, "BLOCK"),
            (8, "0"),
            (2, "CHAIR"),
            (70, "0"),
            (10, "0.0"),
            (20, "0.0"),
            (30, "0.0"),
            (3, "CHAIR"),
            (0, "LWPOLYLINE"),
            (8, "0"),
            (90, "4"),
            (70, "1"),
            (10, "-250"),
            (20, "-250"),
            (10, "250"),
            (20, "-250"),
            (10, "250"),
            (20, "250"),
            (10, "-250"),
            (20, "250"),
            (0, "CIRCLE"),
            (8, "0"),
            (10, "0"),
            (20, "0"),
            (40, "150"),
            (0, "ENDBLK"),
            (8, "0"),
            (0, "ENDSEC"),
            (0, "SECTION"),
            (2, "ENTITIES"),
        ],
    );
    // Outer walls, and an inner wall with a door opening.
    pairs(
        &mut s,
        &[
            (0, "LWPOLYLINE"),
            (8, "WALLS"),
            (90, "4"),
            (70, "1"),
            (10, "0"),
            (20, "0"),
            (10, "10000"),
            (20, "0"),
            (10, "10000"),
            (20, "6000"),
            (10, "0"),
            (20, "6000"),
        ],
    );
    for (x0, y0, x1, y1, layer) in [
        ("6000", "0", "6000", "2000", "WALLS"),
        ("6000", "3000", "6000", "6000", "WALLS"),
        ("6000", "2000", "7000", "2000", "DOORS"),
        ("0", "0", "10000", "6000", "HIDDEN"),
        ("0", "-800", "10000", "-800", "DIMENSIONS"),
        ("0", "-1000", "0", "-600", "DIMENSIONS"),
        ("10000", "-1000", "10000", "-600", "DIMENSIONS"),
    ] {
        pairs(
            &mut s,
            &[
                (0, "LINE"),
                (8, layer),
                (10, x0),
                (20, y0),
                (11, x1),
                (21, y1),
            ],
        );
    }
    pairs(
        &mut s,
        &[
            // The door's swing.
            (0, "ARC"),
            (8, "DOORS"),
            (10, "6000"),
            (20, "2000"),
            (40, "1000"),
            (50, "0"),
            (51, "90"),
            // A bay window: a bulged segment out of the top wall.
            (0, "LWPOLYLINE"),
            (8, "WALLS"),
            (90, "2"),
            (70, "0"),
            (10, "4000"),
            (20, "6000"),
            (42, "0.5"),
            (10, "2000"),
            (20, "6000"),
            // A column and a filled pier.
            (0, "CIRCLE"),
            (8, "WALLS"),
            (10, "3000"),
            (20, "3000"),
            (40, "300"),
            (0, "SOLID"),
            (8, "WALLS"),
            (62, "8"),
            (10, "8600"),
            (20, "400"),
            (11, "9400"),
            (21, "400"),
            (12, "8600"),
            (22, "1200"),
            (13, "9400"),
            (23, "1200"),
            // The dimension's value, and room names.
            (0, "TEXT"),
            (8, "DIMENSIONS"),
            (10, "5000"),
            (20, "-700"),
            (40, "250"),
            (1, "10000"),
            (72, "1"),
            (11, "5000"),
            (21, "-700"),
            (0, "TEXT"),
            (8, "TEXT"),
            (10, "1200"),
            (20, "1200"),
            (40, "400"),
            (1, "LIVING"),
            (0, "TEXT"),
            (8, "TEXT"),
            (10, "7200"),
            (20, "4600"),
            (40, "400"),
            (1, "KITCHEN"),
            (0, "MTEXT"),
            (8, "TEXT"),
            (10, "0"),
            (20, "7200"),
            (40, "300"),
            (71, "7"),
            (1, "Ground floor\\PScale 1:50"),
            // Chairs: an array of two on TEXT, one on FURNITURE (off).
            (0, "INSERT"),
            (8, "TEXT"),
            (2, "CHAIR"),
            (10, "7600"),
            (20, "3000"),
            (70, "2"),
            (71, "1"),
            (44, "800"),
            (0, "INSERT"),
            (8, "FURNITURE"),
            (2, "CHAIR"),
            (10, "2000"),
            (20, "4000"),
            (0, "POINT"),
            (8, "0"),
            (10, "5000"),
            (20, "3000"),
            (0, "LINE"),
            (8, "0"),
            (420, "16746496"),
            (10, "500"),
            (20, "500"),
            (11, "1500"),
            (21, "500"),
            (0, "ENDSEC"),
            (0, "EOF"),
        ],
    );
    s.into_bytes()
}

/// The plan prepared with the bundled fonts, as a page and its stored size.
pub(super) fn page() -> ((u32, u32), DrawingPage) {
    let scene = specular_cad::parse(&plan(), &specular_cad::Limits::default()).expect("parses");
    let prepared = prepare(
        Arc::new(scene),
        &Fonts::bundled_only(),
        PrepLimits::default(),
    );
    DrawingPage::new(Arc::new(prepared), 0, 0.0)
}

/// A multi-pen HP-GL/2 plot on A5 landscape: a frame, a circle, hatched and shaded
/// rectangles, a solid wedge, a dashed line, a shaded polygon with its edge, and labels in
/// two directions, with pens of their own colours and widths.
fn plot() -> Vec<u8> {
    concat!(
        "BP;IN;PS8400,5920;NP8;PC2,200,0,0;PC5,0,90,200;PW0.6,1;PW0.35,2;",
        "SP1;PA200,200;PD8200,200,8200,5720,200,5720,200,200;PU;",
        "SP2;PA1400,1600;CI800;",
        "SP3;FT3,120,45;PA2800,800;RA4600,2400;EP;",
        "SP5;FT10,35;PA5000,800;RA7600,2400;FT1;EP;",
        "SP4;PA2400,4000;WG900,10,100;EW900,10,100;",
        "SP6;LT2,3;PA400,3000;PD7800,3000;LT;PU;",
        "SP2;FT10,20;PM0;PA4800,3400;PD7600,3400,6800,5200,5400,4800;PM2;PU;FP;EP;",
        "SP1;SD1,21,2,1,4,14,5,0,6,3,7,4148;PA600,5200;LBSpecular HP-GL/2\x03;",
        "SP5;SD1,21,2,0,3,10,4,10,5,1,6,0,7,4099;DI0,1;PA7900,600;LBPen 5, upwards\x03;",
    )
    .as_bytes()
    .to_vec()
}

/// HP-GL/2 inside a PCL 5 job with PJL, as a driver prints it.
fn pcl_job() -> Vec<u8> {
    let mut b = b"\x1b%-12345X@PJL JOB NAME=\"shot\"\r\n@PJL ENTER LANGUAGE=PCL\r\n".to_vec();
    b.extend_from_slice(b"\x1bE\x1b&l1O\x1b&l26A\x1b%0B");
    b.extend_from_slice(
        concat!(
            "IN;SP1;PW0.5;PA1000,1000;PD9000,1000,9000,7000,1000,7000,1000,1000;PU;",
            "SP2;PA5000,4000;CI2400;SP3;PA1000,1000;PD9000,7000;PU;PA1000,7000;PD9000,1000;PU;",
            "SP1;SI0.4,0.6;PA1400,6200;LBPCL 5 job\x03;",
        )
        .as_bytes(),
    );
    b.extend_from_slice(b"\x1b%0A\x1bE\x1b%-12345X@PJL EOJ\r\n\x1b%-12345X");
    b
}

/// Which drawing a case shows.
#[derive(Clone, Copy, PartialEq)]
enum Src {
    Plan,
    Plot,
    Pcl,
    /// A GDSII fixture of specular-gds (tests/data), by file name.
    Gds(&'static str),
    /// The board fixture of specular-pcb (tests/data/board): a sheet of its views.
    Board(usize),
    /// A KiCad schematic fixture of specular-kicad, by file name: its first sheet.
    Schematic(&'static str),
    /// A spectrum: a JCAMP-DX fixture of specular-jcamp, or a Bruker folder of
    /// specular-nmr, by name; its first page.
    Jcamp(&'static str),
    Nmr(&'static str),
    /// The ODB++ job made by [`odb_job`]: a sheet of its views.
    Odb(usize),
    /// The LandXML file [`SURVEY`]: its plan.
    Survey,
}

/// An ODB++ job of two steps: a board with text (the built-in stroke font: the job has no
/// fonts), a negative plane with clearances, and a panel of four boards.
fn odb_job() -> Vec<(String, Vec<u8>)> {
    let layer = |row: u32, ty: &str, name: &str, pol: &str| {
        format!(
            "LAYER {{\n ROW={row}\n CONTEXT=BOARD\n TYPE={ty}\n NAME={name}\n POLARITY={pol}\n}}\n"
        )
    };
    let matrix = String::from("STEP {\n COL=1\n NAME=panel\n}\nSTEP {\n COL=2\n NAME=pcb\n}\n")
        + &layer(1, "SILK_SCREEN", "silk", "POSITIVE")
        + &layer(2, "SIGNAL", "top", "POSITIVE")
        + &layer(3, "POWER_GROUND", "gnd", "NEGATIVE");
    let profile = |w: u32, h: u32| {
        format!("UNITS=MM\n#\nS P 0\nOB 0 0 I\nOS {w} 0\nOS {w} {h}\nOS 0 {h}\nOS 0 0\nOE\nSE\n")
    };
    let silk = "UNITS=MM\n#\nT 2 24 std P 0 2 2.6 1 'SPECULAR 1.0' 1\n\
T 3 4 std P 8 30 1.4 1.8 0.6 'R1 C2 U3' 1\nT 38 4 std P 9 0 1.4 1.8 0.6 'BOTTOM' 1\n";
    let top = "UNITS=MM\n$0 r1600\n$1 r400\n$2 rect2400x1600\n#\nP 10 15 0 P 0 0\nP 30 15 0 P 0 0\n\
L 10 15 30 15 1 P 0\nP 20 8 2 P 0 1\nT 14 18 std P 0 1.2 1.6 0.8 'NET A' 1\n";
    let gnd = "UNITS=MM\n$0 r2400\n$1 r1200\n#\nP 10 15 0 P 0 0\nP 30 15 0 P 0 0\nP 20 8 1 P 0 0\n\
T 6 26 std P 0 2 2 1 'GND' 1\n";
    let rep = "UNITS=MM\nSTEP-REPEAT {\n NAME=pcb\n X=5\n Y=5\n DX=45\n DY=35\n NX=2\n NY=2\n \
ANGLE=0\n MIRROR=NO\n}\n";
    [
        ("matrix/matrix", matrix.as_str()),
        ("steps/pcb/stephdr", "UNITS=MM\n"),
        ("steps/pcb/profile", &profile(40, 30)),
        ("steps/pcb/layers/silk/features", silk),
        ("steps/pcb/layers/top/features", top),
        ("steps/pcb/layers/gnd/features", gnd),
        ("steps/panel/stephdr", rep),
        ("steps/panel/profile", &profile(100, 80)),
        (
            "steps/panel/layers/top/features",
            "UNITS=MM\n$0 r3000\n#\nP 2.5 2.5 0 P 0 0\nP 97.5 77.5 0 P 0 0\n",
        ),
    ]
    .iter()
    .map(|(p, t)| (format!("job/{p}"), t.as_bytes().to_vec()))
    .collect()
}

fn odb_page(sheet: usize) -> ((u32, u32), DrawingPage) {
    use specular_decode::drawing::specular_pcb;
    let mut scene =
        specular_pcb::odb::scene(&odb_job(), &specular_pcb::Limits::default()).expect("a job");
    specular_decode::drawing::name_board_views(&mut scene);
    prepared_page(scene, sheet)
}

/// A LandXML survey: a small TIN hill, an alignment over it, two parcels and survey points.
pub(in crate::app) const SURVEY: &str = r#"<?xml version="1.0"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
<Units><Metric linearUnit="meter" areaUnit="squareMeter" volumeUnit="cubicMeter"/></Units>
<CgPoints><CgPoint name="BM1" code="BM">1010 2010 52.1</CgPoint><CgPoint name="CP2">1090 2085 50.4</CgPoint>
<CgPoint name="CP3">1012 2090 49.8</CgPoint></CgPoints>
<Surfaces><Surface name="Hill"><Definition surfType="TIN"><Pnts>
<P id="1">1000 2000 50</P><P id="2">1000 2050 51</P><P id="3">1000 2100 50</P>
<P id="4">1050 2000 52</P><P id="5">1050 2050 58</P><P id="6">1050 2100 53</P>
<P id="7">1100 2000 49</P><P id="8">1100 2050 51</P><P id="9">1100 2100 50</P></Pnts>
<Faces><F>1 2 5</F><F>1 5 4</F><F>2 3 6</F><F>2 6 5</F><F>4 5 8</F><F>4 8 7</F><F>5 6 9</F><F>5 9 8</F></Faces>
</Definition></Surface></Surfaces>
<Parcels><Parcel name="Lot 1" area="2500"><Center>1025 2025</Center><CoordGeom>
<Line><Start>1000 2000</Start><End>1050 2000</End></Line><Line><Start>1050 2000</Start><End>1050 2050</End></Line>
<Line><Start>1050 2050</Start><End>1000 2050</End></Line><Line><Start>1000 2050</Start><End>1000 2000</End></Line>
</CoordGeom></Parcel><Parcel name="Lot 2" area="2304.6"><Center>1075 2075</Center><CoordGeom>
<Line><Start>1050 2050</Start><End>1100 2050</End></Line><Line><Start>1100 2050</Start><End>1100 2100</End></Line>
<Curve rot="ccw" radius="25"><Start>1100 2100</Start><Center>1075 2100</Center><End>1050 2100</End></Curve>
<Line><Start>1050 2100</Start><End>1050 2050</End></Line></CoordGeom></Parcel></Parcels>
<Alignments><Alignment name="Road" length="100" staStart="0"><CoordGeom>
<Line><Start>1005 2050</Start><End>1095 2050</End></Line></CoordGeom></Alignment></Alignments>
</LandXML>"#;

fn survey_page() -> ((u32, u32), DrawingPage) {
    use specular_decode::drawing::specular_landxml;
    let x = specular_landxml::parse(SURVEY.as_bytes()).expect("parses");
    prepared_page(specular_landxml::scene(&x), 0)
}

fn fixture(krate: &str, name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats")
        .join(krate)
        .join("tests/data")
        .join(name)
}

fn prepared_page(
    scene: specular_decode::drawing::specular_vector::VectorScene,
    sheet: usize,
) -> ((u32, u32), DrawingPage) {
    let prepared = prepare(
        Arc::new(scene),
        &Fonts::bundled_only(),
        PrepLimits::default(),
    );
    DrawingPage::new(Arc::new(prepared), sheet, 0.0)
}

/// The specular-pcb board fixture (a KiCad-named Gerber set) as the decoder makes it.
fn board_page(sheet: usize) -> ((u32, u32), DrawingPage) {
    use specular_decode::drawing::specular_pcb;
    let dir = fixture("specular-pcb", "board");
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .expect("board fixture")
        .flatten()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name, std::fs::read(e.path()).expect("reads"))
        })
        .collect();
    files.sort();
    let mut scene = specular_pcb::stack::board_scene(&files, &specular_pcb::Limits::default())
        .expect("a board");
    specular_decode::drawing::name_board_views(&mut scene);
    prepared_page(scene, sheet)
}

fn schematic_page(name: &str) -> ((u32, u32), DrawingPage) {
    use specular_decode::drawing::specular_kicad;
    let bytes = std::fs::read(fixture("specular-kicad", name)).expect("fixture");
    let resolve = |n: &str| std::fs::read(fixture("specular-kicad", n)).ok();
    let scene = specular_kicad::sch::scene(&bytes, &resolve, &specular_kicad::Limits::default())
        .expect("a schematic");
    prepared_page(scene, 0)
}

fn spectrum_page(
    file: &dyn specular_decode::drawing::specular_spectra::SpectraFile,
) -> ((u32, u32), DrawingPage) {
    let scene = specular_decode::drawing::specular_spectra::chart::scene(file, [0]);
    prepared_page(scene, 0)
}

/// A GDSII layout prepared with the bundled fonts, as its first top cell's page.
fn gds_page(name: &str) -> ((u32, u32), DrawingPage) {
    use specular_decode::drawing::specular_gds;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-gds/tests/data")
        .join(name);
    let bytes = std::fs::read(path).expect("fixture");
    let lib = specular_gds::parse(&bytes, specular_gds::Limits::default()).expect("parses");
    let prepared = prepare(
        Arc::new(specular_gds::scene::to_scene(&lib)),
        &Fonts::bundled_only(),
        PrepLimits::default(),
    );
    DrawingPage::new(Arc::new(prepared), 0, 0.0)
}

/// A plot prepared with the bundled fonts, as a page and its stored size.
fn plot_page(bytes: &[u8]) -> ((u32, u32), DrawingPage) {
    let scene = specular_cad::hpgl::parse(bytes, &specular_cad::Limits::default()).expect("parses");
    let prepared = prepare(
        Arc::new(scene),
        &Fonts::bundled_only(),
        PrepLimits::default(),
    );
    DrawingPage::new(Arc::new(prepared), 0, 0.0)
}

#[derive(Clone, Copy)]
struct Case {
    src: Src,
    name: &'static str,
    dark: bool,
    rtl: bool,
    scale: f32,
    panel: bool,
    /// Layers turned off by name.
    off: &'static [&'static str],
    lineweights: bool,
    /// Zoom (screen px per stored px at 1×) and the stored point at the centre; `None` fits.
    zoom: Option<(f32, (f32, f32))>,
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, renderer: &mut Option<VectorRenderer>, c: &Case) {
    let ((w, hh), page) = match c.src {
        Src::Plan => page(),
        Src::Plot => plot_page(&plot()),
        Src::Pcl => plot_page(&pcl_job()),
        Src::Gds(name) => gds_page(name),
        Src::Board(sheet) => board_page(sheet),
        Src::Schematic(name) => schematic_page(name),
        Src::Jcamp(name) => {
            let bytes = std::fs::read(fixture("specular-jcamp", name)).expect("fixture");
            let file =
                specular_decode::drawing::specular_jcamp::jcamp::parse(&bytes).expect("parses");
            spectrum_page(&file)
        }
        Src::Nmr(name) => {
            let file = specular_decode::drawing::specular_nmr::open(&fixture("specular-nmr", name))
                .expect("opens");
            spectrum_page(&file)
        }
        Src::Odb(sheet) => odb_page(sheet),
        Src::Survey => survey_page(),
    };
    let scene = page.prepared.scene.clone();
    let chart_page = page.clone();
    let layers: Vec<bool> = scene
        .layers
        .iter()
        .map(|l| l.visible() && !c.off.contains(&l.name.as_str()))
        .collect();
    let mut cad = Cad {
        id: Some(1),
        prepared: Some(page.prepared.clone()),
        page: Some(page),
        layers: layers.clone(),
        lineweights: c.lineweights,
        renderer: renderer.take(),
        ..Default::default()
    };
    let view_of = |size: (u32, u32)| {
        let vp = PxRect {
            x: 0.0,
            y: 0.0,
            w: size.0 as f32,
            h: size.1 as f32,
        };
        let (s, centre) = match c.zoom {
            Some((z, p)) => (z * c.scale, p),
            None => (
                (size.0 as f32 / w as f32).min(size.1 as f32 / hh as f32),
                (w as f32 / 2.0, hh as f32 / 2.0),
            ),
        };
        View::fixed((w, hh), Orientation::NORMAL, vp, s, centre)
    };
    let frame = CadFrame {
        id: 1,
        dark: c.dark,
        key: 0,
    };
    let pal = if c.dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    let panel = c.panel.then(|| {
        let rows: Vec<LayerRow> = scene
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| LayerRow {
                index: i,
                name: short_name(&super::shown_name(&l.name, l.generated)),
                color: l.color.on(c.dark, paper_colour(c.dark)),
                on: layers[i],
            })
            .collect();
        LayersPanel {
            shown: rows.iter().filter(|r| r.on).count(),
            total: rows.len(),
            rows,
            scroll: 0,
        }
    });
    let rtl = c.rtl;
    let dpi = f64::from(c.scale);
    let mut drawn = None;
    let shot = h.render_gpu(
        SIZE,
        c.scale,
        |gpu, enc, target, size| {
            if cad.renderer.is_none() {
                cad.renderer = Some(VectorRenderer::new(gpu).expect("vello"));
            }
            let view = view_of(size);
            render(&mut cad, gpu, enc, target, size, &view, &frame, dpi);
            assert!(!cad.failed, "the vector renderer failed");
        },
        |dl, text| {
            let area = (0.0, 0.0, SIZE.0, SIZE.1);
            let size = (
                (SIZE.0 * c.scale).round() as u32,
                (SIZE.1 * c.scale).round() as u32,
            );
            let chart =
                crate::app::chart::view_of(&chart_page, &view_of(size), c.scale, None, c.dark);
            if let Some(v) = &chart {
                crate::app::chart::draw(dl, text, &pal, v, area, 1.0);
            }
            if let Some(p) = &panel {
                drawn = Some(draw_panel(dl, text, &pal, p, area, rtl, 1.0));
            }
        },
    );
    *renderer = cad.renderer.take();
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    if let (Some(p), Some((r, rows, _))) = (&panel, drawn) {
        super::super::a11y_audit::audit_screen(c.name, SIZE, rtl, |t| {
            a11y(t, p, r, &rows, 1.0);
        });
    }
}

#[test]
fn drawings_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-drawing",
    );
    let mut renderer = None;
    let base = Case {
        src: Src::Plan,
        name: "",
        dark: true,
        rtl: false,
        scale: 1.0,
        panel: true,
        off: &[],
        lineweights: false,
        zoom: None,
    };
    // Points of the plan in stored pixels (it is fitted into 4096 px with a margin): the
    // door's hinge and the first chair's centre.
    let ((w, hh), page) = page();
    let stored = |x: f64, y: f64| {
        let p = page.to_stored * specular_decode::drawing::specular_vector::kurbo::Point::new(x, y);
        assert!(p.x > 0.0 && p.x < f64::from(w) && p.y > 0.0 && p.y < f64::from(hh));
        (p.x as f32, p.y as f32)
    };
    let hinge = stored(6000.0, 2000.0);
    let chair = stored(7600.0, 3000.0);
    let cases = [
        Case {
            name: "drawing-plan-dark",
            ..base
        },
        Case {
            name: "drawing-plan-light",
            dark: false,
            ..base
        },
        Case {
            name: "drawing-plan-light",
            dark: false,
            scale: 1.5,
            ..base
        },
        Case {
            name: "drawing-layers-off-dark",
            off: &["HIDDEN", "DIMENSIONS"],
            ..base
        },
        Case {
            name: "drawing-door-200x-lineweights-light",
            dark: false,
            panel: false,
            lineweights: true,
            zoom: Some((200.0, hinge)),
            ..base
        },
        Case {
            name: "drawing-chair-2.5x-dark",
            panel: false,
            zoom: Some((2.5, chair)),
            ..base
        },
        Case {
            src: Src::Plot,
            name: "plot-pens-dark",
            ..base
        },
        Case {
            src: Src::Plot,
            name: "plot-pens-light",
            dark: false,
            panel: false,
            lineweights: true,
            ..base
        },
        Case {
            src: Src::Pcl,
            name: "plot-pcl-light",
            dark: false,
            panel: false,
            ..base
        },
    ];
    for c in &cases {
        shoot(&mut h, &mut goldens, &mut renderer, c);
    }
    shoot_layouts(&mut h, &mut goldens, &mut renderer, base);
    shoot_boards_and_spectra(&mut h, &mut goldens, &mut renderer, base);
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Case {
            name: "drawing-plan-he",
            rtl: true,
            ..base
        };
        shoot(&mut h, &mut goldens, &mut renderer, &c);
    });
    goldens.finish();
}

/// Engineering formats: an ODB++ job's layers (text in the stroke font, a negative plane)
/// and its panel step in board colours, and a LandXML plan with parcels and survey points;
/// English dark at 1×, Hebrew light at 1.5×.
#[test]
fn engineering_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-drawing",
    );
    let mut renderer = None;
    let base = Case {
        src: Src::Odb(0),
        name: "",
        dark: true,
        rtl: false,
        scale: 1.0,
        panel: true,
        off: &[],
        lineweights: false,
        zoom: None,
    };
    let cases = [
        Case {
            name: "odb-text-negative-dark",
            ..base
        },
        Case {
            src: Src::Odb(4),
            name: "odb-panel-top-dark",
            panel: false,
            ..base
        },
        Case {
            src: Src::Survey,
            name: "landxml-plan-parcels-dark",
            ..base
        },
    ];
    for c in &cases {
        shoot(&mut h, &mut goldens, &mut renderer, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        for c in &cases {
            let name = match c.src {
                Src::Odb(0) => "odb-text-negative-he-light",
                Src::Odb(_) => "odb-panel-top-he-light",
                _ => "landxml-plan-parcels-he-light",
            };
            let c = Case {
                name,
                dark: false,
                rtl: true,
                scale: 1.5,
                ..*c
            };
            shoot(&mut h, &mut goldens, &mut renderer, &c);
        }
    });
    goldens.finish();
}

/// GDSII layouts (specular-gds fixtures): a cell placed rotated, mirrored, magnified and
/// arrayed with every path type, a box and a label, with the layer/datatype panel, dark and
/// light at 1.5×, zoomed on its label; arrays with a layer off; and a deep hierarchy of
/// billions of flat shapes, its small cells drawn as tinted rectangles.
fn shoot_layouts(
    h: &mut Harness,
    goldens: &mut Goldens,
    renderer: &mut Option<VectorRenderer>,
    base: Case,
) {
    let ((w, hh), page) = gds_page("raw.gds");
    // The first CHILD's label at (10, 20) database units runs down from there (rotated 90°,
    // right-aligned, 250 units high): the middle of it, in stored pixels.
    let um = page.prepared.scene.layout.as_ref().unwrap().info.db_unit_m * 1e6;
    let p = page.to_stored
        * specular_decode::drawing::specular_vector::kurbo::Point::new(10.0 * um, -520.0 * um);
    assert!(p.x > 0.0 && p.x < f64::from(w) && p.y > 0.0 && p.y < f64::from(hh));
    let label = (p.x as f32, p.y as f32);
    let raw = Case {
        src: Src::Gds("raw.gds"),
        ..base
    };
    for c in [
        Case {
            name: "layout-raw-dark",
            ..raw
        },
        Case {
            name: "layout-raw-light",
            dark: false,
            scale: 1.5,
            panel: false,
            ..raw
        },
        Case {
            name: "layout-raw-label-4x-light",
            dark: false,
            panel: false,
            zoom: Some((4.0, label)),
            ..raw
        },
        Case {
            src: Src::Gds("arrays.gds"),
            name: "layout-arrays-layer-off-dark",
            off: &["2/5"],
            ..base
        },
        Case {
            src: Src::Gds("deep.gds"),
            name: "layout-deep-light",
            dark: false,
            ..base
        },
    ] {
        shoot(h, goldens, renderer, &c);
    }
}

/// A Shapefile of parcels (polygons, every third with a hole, in UTM zone 33N; generated
/// here with its `.shx`, `.dbf` and `.prj`) drawn as the viewer draws a map, fitted, with the
/// readout at a parcel (map position, longitude and latitude, the parcel's name) and the
/// scale bar in metres: dark at 1× and light at 1.5×.
#[test]
fn shapefile_map_matches_golden() {
    use specular_decode::geo::specular_gis::fixtures;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-gis-map",
    );
    let dir = std::env::temp_dir().join(format!("specular-gis-shots-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let feats = fixtures::parcels(6, 100.0, 4, (500_000.0, 5_000_000.0));
    let p = fixtures::shapefile_polygons(&feats, fixtures::UTM33N_WKT);
    std::fs::write(dir.join("parcels.shp"), &p.shp).unwrap();
    std::fs::write(dir.join("parcels.shx"), &p.shx).unwrap();
    std::fs::write(dir.join("parcels.dbf"), &p.dbf).unwrap();
    std::fs::write(dir.join("parcels.prj"), &p.prj).unwrap();
    let s = specular_decode::Registry::default()
        .open_path(&dir.join("parcels.shp"))
        .expect("opens");
    let _ = std::fs::remove_dir_all(&dir);
    let page = s.drawing().expect("a map page").clone();
    let g = s.geo().expect("georeferenced");
    let (w, hh) = (s.info().width, s.info().height);
    let mut renderer: Option<VectorRenderer> = None;
    for (name, dark, scale) in [
        ("gis-shapefile-dark", true, 1.0f32),
        ("gis-shapefile-light", false, 1.5),
    ] {
        let scene = page.prepared.scene.clone();
        let mut cad = Cad {
            id: Some(1),
            prepared: Some(page.prepared.clone()),
            page: Some(page.clone()),
            layers: scene.layers.iter().map(|l| l.visible()).collect(),
            renderer: renderer.take(),
            ..Default::default()
        };
        let view_of = |size: (u32, u32)| {
            let vp = PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            };
            let s = (size.0 as f32 / w as f32).min(size.1 as f32 / hh as f32) * 0.9;
            View::fixed(
                (w, hh),
                Orientation::NORMAL,
                vp,
                s,
                (w as f32 / 2.0, hh as f32 / 2.0),
            )
        };
        let size_px = (
            (SIZE.0 * scale).round() as u32,
            (SIZE.1 * scale).round() as u32,
        );
        let view = view_of(size_px);
        // The readout at the middle of parcel 7 (the second row's second parcel).
        let inv = g.to_map.inverse().unwrap();
        let (px, py) = inv.apply(500_000.0 + 100.0 + 20.0, 5_000_000.0 + 100.0 + 20.0);
        let hit = g
            .features
            .as_ref()
            .unwrap()
            .hit(g.map_at(px, py).into(), 0.1)
            .expect("a parcel");
        let mut lines = super::super::geo::geo_position_lines(&g, px, py);
        lines.extend(super::super::geo::feature_name(&g, hit));
        let bar = super::super::geo::geo_scale_bar(
            &g,
            f64::from(w) / 2.0,
            f64::from(hh) / 2.0,
            f64::from(view.scale() / scale),
        );
        assert!(
            bar.as_ref().is_some_and(|b| b.label.contains("m\u{2069}")),
            "{bar:?}"
        );
        let frame = CadFrame {
            id: 1,
            dark,
            key: 0,
        };
        let pal = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let shot = h.render_gpu(
            SIZE,
            scale,
            |gpu, enc, target, size| {
                if cad.renderer.is_none() {
                    cad.renderer = Some(VectorRenderer::new(gpu).expect("vello"));
                }
                let view = view_of(size);
                render(
                    &mut cad,
                    gpu,
                    enc,
                    target,
                    size,
                    &view,
                    &frame,
                    f64::from(scale),
                );
                assert!(!cad.failed, "the vector renderer failed");
            },
            |dl, text| {
                let area = (0.0, 0.0, SIZE.0, SIZE.1);
                crate::app::medical::draw_readout(dl, text, &lines, area, false, 1.0);
                if let Some(bar) = &bar {
                    super::super::science::draw_scale(dl, text, &pal, bar, area, None, false, 1.0);
                }
            },
        );
        renderer = cad.renderer.take();
        goldens.check(&format!("{name}-{scale}x"), &shot);
    }
    goldens.finish();
}

/// Electronics and spectra: the board fixture's layers stacked (dark) and its top in board
/// colours (light), a KiCad schematic sheet on light paper, an IR spectrum (JCAMP-DX, its
/// wavenumber axis running right to left) and an NMR spectrum (Bruker, ppm axis, peak marks).
fn shoot_boards_and_spectra(
    h: &mut Harness,
    goldens: &mut Goldens,
    renderer: &mut Option<VectorRenderer>,
    base: Case,
) {
    for c in [
        Case {
            src: Src::Board(0),
            name: "board-layers-dark",
            ..base
        },
        Case {
            src: Src::Board(1),
            name: "board-top-light",
            dark: false,
            panel: false,
            ..base
        },
        Case {
            src: Src::Schematic("sch8.kicad_sch"),
            name: "schematic-sheet-light",
            dark: false,
            panel: false,
            ..base
        },
        Case {
            src: Src::Jcamp("ir_dif.jdx"),
            name: "spectrum-ir-light",
            dark: false,
            panel: false,
            ..base
        },
        Case {
            src: Src::Nmr("fid1d"),
            name: "spectrum-nmr-dark",
            panel: false,
            scale: 1.5,
            ..base
        },
    ] {
        shoot(h, goldens, renderer, &c);
    }
}
