//! Packages built in memory: structure, drawing, text, resources, refusals and bounds.

use super::*;

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// A stored (uncompressed) ZIP of `files`.
pub(crate) fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    zip_flags(files, 0)
}

fn zip_flags(files: &[(&str, &[u8])], flags: u16) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let off = out.len() as u32;
        let crc = crc32(data);
        let h = |v: &mut Vec<u8>, central: bool| {
            v.extend_from_slice(if central {
                b"PK\x01\x02"
            } else {
                b"PK\x03\x04"
            });
            if central {
                v.extend_from_slice(&20u16.to_le_bytes());
            }
            v.extend_from_slice(&20u16.to_le_bytes());
            v.extend_from_slice(&flags.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes()); // stored
            v.extend_from_slice(&0u32.to_le_bytes()); // time, date
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            if central {
                v.extend_from_slice(&[0; 6]); // comment length, disk, internal attributes
                v.extend_from_slice(&0u32.to_le_bytes());
                v.extend_from_slice(&off.to_le_bytes());
            }
            v.extend_from_slice(name.as_bytes());
        };
        h(&mut out, false);
        out.extend_from_slice(data);
        h(&mut central, true);
    }
    let cd = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

const NS: &str = "http://schemas.microsoft.com/xps/2005/06";
const FIXED_REL: &str = "http://schemas.microsoft.com/xps/2005/06/fixedrepresentation";

fn rels(kind: &str, target: &str) -> String {
    format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="R1" Type="{kind}" Target="{target}"/><Relationship Id="R2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="/docProps/core.xml"/></Relationships>"#
    )
}

const CORE: &str = r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/"><dc:title>Test title</dc:title><dc:creator>Someone</dc:creator><dcterms:created>2024-01-02T03:04:05Z</dcterms:created></cp:coreProperties>"#;

/// A one-document package with `pages` (FixedPage bodies) and extra parts.
fn package(ns: &str, rel: &str, pages: &[String], extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let fds = format!(
        r#"<FixedDocumentSequence xmlns="{ns}"><DocumentReference Source="Documents/1/FixedDocument.fdoc"/></FixedDocumentSequence>"#
    );
    let mut fdoc = format!(r#"<FixedDocument xmlns="{ns}">"#);
    for i in 0..pages.len() {
        fdoc.push_str(&format!(r#"<PageContent Source="Pages/{}.fpage"/>"#, i + 1));
    }
    fdoc.push_str("</FixedDocument>");
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="odttf" ContentType="application/vnd.ms-package.obfuscated-opentype"/></Types>"#.to_vec()),
        ("_rels/.rels".into(), rels(rel, "/FixedDocSeq.fdseq").into_bytes()),
        ("docProps/core.xml".into(), CORE.as_bytes().to_vec()),
        ("FixedDocSeq.fdseq".into(), fds.into_bytes()),
        ("Documents/1/FixedDocument.fdoc".into(), fdoc.into_bytes()),
    ];
    for (i, p) in pages.iter().enumerate() {
        files.push((
            format!("Documents/1/Pages/{}.fpage", i + 1),
            p.clone().into_bytes(),
        ));
    }
    for (n, d) in extra {
        files.push(((*n).to_owned(), d.clone()));
    }
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    zip_of(&refs)
}

fn page(body: &str) -> String {
    format!(
        r#"<FixedPage xmlns="{NS}" xmlns:x="http://schemas.microsoft.com/xps/2005/06/resourcedictionary-key" Width="96" Height="96" xml:lang="en">{body}</FixedPage>"#
    )
}

fn open(bytes: Vec<u8>) -> Document {
    Document::open(Arc::new(bytes), Some(test_decoder())).unwrap()
}

/// A decoder for test images: `IMG` + width + height bytes + RGBA pixels.
fn test_decoder() -> ImageDecoder {
    Arc::new(|d: &[u8]| {
        let rest = d.strip_prefix(b"IMG")?;
        let (w, h) = (u32::from(rest[0]), u32::from(rest[1]));
        Some((w, h, rest[2..2 + (w * h * 4) as usize].to_vec()))
    })
}

/// Renders page `p` at 96 dpi (1 px per page unit).
fn draw(doc: &Document, p: usize) -> (u32, u32, Vec<u8>) {
    let (w, h) = doc.page_size(p).unwrap();
    let (w, h) = ((w / 0.75).round() as u32, (h / 0.75).round() as u32);
    let px = doc
        .render(
            RenderRequest {
                page: p,
                scale_x: 1.0 / 0.75,
                scale_y: 1.0 / 0.75,
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            &|| false,
        )
        .unwrap();
    (w, h, px)
}

fn px(img: &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.0 + x) * 4) as usize;
    img.2[i..i + 4].try_into().unwrap()
}

fn near(a: [u8; 4], b: [u8; 3], tol: u8) -> bool {
    (0..3).all(|i| a[i].abs_diff(b[i]) <= tol)
}

#[test]
fn structure_and_facts() {
    let d = open(package(NS, FIXED_REL, &[page(""), page(""), page("")], &[]));
    assert_eq!(d.page_count(), 3);
    assert_eq!(d.page_size(0), Some((72.0, 72.0)));
    assert_eq!(d.page_size(3), None);
    let f = d.facts();
    assert!(!f.openxps);
    assert_eq!(f.pages, 3);
    assert_eq!(f.documents, 1);
    assert_eq!(f.title.as_deref(), Some("Test title"));
    assert_eq!(f.creator.as_deref(), Some("Someone"));
    assert_eq!(f.created.as_deref(), Some("2024-01-02T03:04:05Z"));
    // A blank page is white.
    let img = draw(&d, 0);
    assert!(img.2.iter().all(|&v| v == 255));
}

#[test]
fn openxps_namespace() {
    let ns = "http://schemas.openxps.org/oxps/v1.0";
    let body = format!(
        r##"<FixedPage xmlns="{ns}" Width="200" Height="100"><Path Data="M 0,0 H 200 V 100 H 0 Z" Fill="#FF00FF00"/></FixedPage>"##
    );
    let d = open(package(
        ns,
        "http://schemas.openxps.org/oxps/v1.0/fixedrepresentation",
        &[body],
        &[],
    ));
    assert!(d.facts().openxps);
    assert_eq!(d.page_size(0), Some((150.0, 75.0)));
    let img = draw(&d, 0);
    assert!(near(px(&img, 100, 50), [0, 255, 0], 1));
}

#[test]
fn paths_strokes_and_transforms() {
    let body = page(
        r##"<Path Data="F1 M 10,10 L 40,10 40,40 10,40 Z" Fill="#FF0000"/>
        <Canvas RenderTransform="1,0,0,1,50,0"><Path Data="M 0,10 L 30,10 30,40 0,40 Z" Fill="#0000FF" Opacity="0.5"/></Canvas>
        <Path Data="M 10,70 L 86,70" Stroke="#000000" StrokeThickness="6"/>
        <Path Data="M 10,85 L 86,85" Stroke="#000000" StrokeThickness="4" StrokeDashArray="1 1"/>"##,
    );
    let d = open(package(NS, FIXED_REL, &[body], &[]));
    let img = draw(&d, 0);
    assert!(near(px(&img, 25, 25), [255, 0, 0], 1));
    // Half-transparent blue over white.
    assert!(near(px(&img, 65, 25), [127, 127, 255], 2));
    assert!(near(px(&img, 50, 70), [0, 0, 0], 1));
    assert!(near(px(&img, 50, 66), [255, 255, 255], 1));
    // Dashes of 4 px on, 4 px off from x = 10.
    assert!(near(px(&img, 12, 85), [0, 0, 0], 1));
    assert!(near(px(&img, 16, 85), [255, 255, 255], 1));
}

#[test]
fn path_geometry_clip_and_resources() {
    let body = page(
        r##"<FixedPage.Resources><ResourceDictionary>
            <SolidColorBrush x:Key="green" Color="#00FF00"/>
            <PathGeometry x:Key="box" FillRule="NonZero"><PathFigure StartPoint="0,0" IsClosed="true"><PolyLineSegment Points="48,0 48,48 0,48"/></PathFigure></PathGeometry>
        </ResourceDictionary></FixedPage.Resources>
        <Canvas Clip="M 0,0 L 24,0 24,96 0,96 Z">
          <Path Data="{StaticResource box}" Fill="{StaticResource green}"/>
        </Canvas>
        <Path Fill="#0000FF"><Path.Data><PathGeometry><PathFigure StartPoint="60,60" IsClosed="true"><ArcSegment Point="90,60" Size="15,15" SweepDirection="Clockwise" RotationAngle="0" IsLargeArc="false"/></PathFigure></PathGeometry></Path.Data></Path>"##,
    );
    let d = open(package(NS, FIXED_REL, &[body], &[]));
    let img = draw(&d, 0);
    assert!(near(px(&img, 12, 24), [0, 255, 0], 1));
    // Clipped away.
    assert!(near(px(&img, 36, 24), [255, 255, 255], 1));
    // Clockwise from (60,60) to (90,60) in y-down space bulges upwards.
    assert!(near(px(&img, 75, 50), [0, 0, 255], 1));
    assert!(near(px(&img, 75, 70), [255, 255, 255], 1));
}

#[test]
fn gradients() {
    let body = page(
        r##"<Path Data="M 0,0 H 96 V 48 H 0 Z"><Path.Fill><LinearGradientBrush MappingMode="Absolute" StartPoint="0,0" EndPoint="96,0"><LinearGradientBrush.GradientStops><GradientStop Color="#000000" Offset="0"/><GradientStop Color="#FFFFFF" Offset="1"/></LinearGradientBrush.GradientStops></LinearGradientBrush></Path.Fill></Path>
        <Path Data="M 0,48 H 96 V 96 H 0 Z"><Path.Fill><RadialGradientBrush MappingMode="Absolute" Center="48,72" GradientOrigin="48,72" RadiusX="24" RadiusY="24"><RadialGradientBrush.GradientStops><GradientStop Color="#FF0000" Offset="0"/><GradientStop Color="#0000FF" Offset="1"/></RadialGradientBrush.GradientStops></RadialGradientBrush></Path.Fill></Path>"##,
    );
    let d = open(package(NS, FIXED_REL, &[body], &[]));
    let img = draw(&d, 0);
    let mid = px(&img, 48, 24);
    assert!((120..136).contains(&mid[0]), "{mid:?}");
    assert!(px(&img, 2, 24)[0] < 10);
    assert!(near(px(&img, 48, 72), [255, 0, 0], 12));
    assert!(near(px(&img, 2, 50), [0, 0, 255], 2));
}

fn test_image() -> Vec<u8> {
    // 2×2: red, green / blue, white.
    let mut v = b"IMG\x02\x02".to_vec();
    v.extend_from_slice(&[
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ]);
    v
}

#[test]
fn image_brush() {
    let body = page(
        r##"<Path Data="M 0,0 H 48 V 48 H 0 Z"><Path.Fill><ImageBrush ImageSource="/Resources/a.png" Viewbox="0,0,2,2" ViewboxUnits="Absolute" Viewport="0,0,48,48" ViewportUnits="Absolute"/></Path.Fill></Path>
        <Path Data="M 48,48 H 96 V 96 H 48 Z"><Path.Fill><ImageBrush ImageSource="/Resources/a.png" Viewbox="0,0,1,1" ViewboxUnits="Absolute" Viewport="48,48,8,8" ViewportUnits="Absolute" TileMode="Tile"/></Path.Fill></Path>"##,
    );
    let d = open(package(
        NS,
        FIXED_REL,
        &[body],
        &[("Resources/a.png", test_image())],
    ));
    let img = draw(&d, 0);
    assert!(near(px(&img, 6, 6), [255, 0, 0], 2));
    assert!(near(px(&img, 42, 6), [0, 255, 0], 2));
    assert!(near(px(&img, 6, 42), [0, 0, 255], 2));
    // The 1×1 viewbox (red) tiles across the second square.
    assert!(near(px(&img, 52, 52), [255, 0, 0], 2));
    assert!(near(px(&img, 90, 90), [255, 0, 0], 2));
    assert!(d.memory_bytes() > 0);
    assert!(d.trim_memory() > 0);
    // Without a decoder the image is not drawn.
    let d = Document::open(
        Arc::new(package(NS, FIXED_REL, &[page(r##"<Path Data="M 0,0 H 48 V 48 H 0 Z"><Path.Fill><ImageBrush ImageSource="/Resources/a.png" Viewbox="0,0,2,2" Viewport="0,0,48,48"/></Path.Fill></Path>"##)], &[("Resources/a.png", test_image())])),
        None,
    )
    .unwrap();
    assert!(near(px(&draw(&d, 0), 6, 6), [255, 255, 255], 0));
}

#[test]
fn visual_brush_and_opacity_mask() {
    let body = page(
        r##"<Path Data="M 0,0 H 48 V 48 H 0 Z"><Path.Fill><VisualBrush Viewbox="0,0,10,10" Viewport="0,0,12,12" TileMode="Tile"><VisualBrush.Visual><Path Data="M 0,0 H 5 V 10 H 0 Z" Fill="#FF0000"/></VisualBrush.Visual></VisualBrush></Path.Fill></Path>
        <Path Data="M 48,0 H 96 V 48 H 48 Z" Fill="#000000"><Path.OpacityMask><LinearGradientBrush MappingMode="Absolute" StartPoint="48,0" EndPoint="96,0"><LinearGradientBrush.GradientStops><GradientStop Color="#00000000" Offset="0"/><GradientStop Color="#FF000000" Offset="1"/></LinearGradientBrush.GradientStops></LinearGradientBrush></Path.OpacityMask></Path>
        <Path Data="M 0,48 H 96 V 96 H 0 Z"><Path.Fill><VisualBrush Viewbox="0,0,10,10" Viewport="0,48,96,48"><VisualBrush.Visual><Path Data="M 0,0 H 5 V 10 H 0 Z" Fill="#0000FF"/></VisualBrush.Visual></VisualBrush></Path.Fill></Path>"##,
    );
    let d = open(package(NS, FIXED_REL, &[body], &[]));
    let img = draw(&d, 0);
    // Tiles 12 px wide: red on the left half of each.
    assert!(near(px(&img, 2, 2), [255, 0, 0], 4));
    assert!(near(px(&img, 9, 2), [255, 255, 255], 4));
    assert!(near(px(&img, 14, 14), [255, 0, 0], 4));
    // The mask fades the black square in from the left.
    assert!(px(&img, 50, 24)[0] > 220);
    assert!(px(&img, 94, 24)[0] < 30);
    // An untiled visual drawn directly: blue on the left half.
    assert!(near(px(&img, 20, 72), [0, 0, 255], 1));
    assert!(near(px(&img, 70, 72), [255, 255, 255], 1));
}

/// The bundled font obfuscated the way XPS packages store it.
fn obfuscated_font(name: &str) -> Vec<u8> {
    let plain = specular_theme::fonts::INTER.to_vec();
    let mut f = plain.clone();
    font::xor_key(&mut f, &plain, &font::guid_key(name).unwrap());
    f
}

#[test]
fn glyphs_and_text() {
    let font = "Resources/0F1E2D3C-4B5A-6978-8796-A5B4C3D2E1F0.odttf";
    let body = page(&format!(
        r##"<Glyphs OriginX="4" OriginY="60" FontRenderingEmSize="60" FontUri="/{font}" UnicodeString="HI" Fill="#000000"/>
        <Glyphs OriginX="4" OriginY="90" FontRenderingEmSize="10" FontUri="/{font}" UnicodeString="{{}}{{x}}" Fill="#000000" StyleSimulations="BoldItalicSimulation"/>"##
    ));
    let d = open(package(
        NS,
        FIXED_REL,
        &[body],
        &[(font, obfuscated_font(font))],
    ));
    assert_eq!(d.facts().fonts, 1);
    let img = draw(&d, 0);
    // Inside the H's left stem (starts a few units right of the origin).
    let dark = (4..20).any(|x| px(&img, x, 40)[0] < 40);
    assert!(dark);
    assert_eq!(d.page_text(0).as_deref(), Some("HI\n{x}"));
}

#[test]
fn remote_dictionary_pieces_and_alternate_content() {
    let dict = format!(
        r##"<ResourceDictionary xmlns="{NS}" xmlns:x="http://schemas.microsoft.com/xps/2005/06/resourcedictionary-key"><SolidColorBrush x:Key="b" Color="#0000FF"/></ResourceDictionary>"##
    );
    let body = page(
        r##"<FixedPage.Resources><ResourceDictionary Source="../Resources/r.dict"/></FixedPage.Resources>
        <mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:v="http://example.com/unknown">
          <mc:Choice Requires="v"><Path Data="M 0,0 H 96 V 96 H 0 Z" Fill="#FF0000"/></mc:Choice>
          <mc:Fallback><Path Data="M 0,0 H 96 V 96 H 0 Z" Fill="{StaticResource b}"/></mc:Fallback>
        </mc:AlternateContent>"##,
    );
    // The page split into interleaved pieces.
    let bytes = body.into_bytes();
    let (a, b) = bytes.split_at(bytes.len() / 2);
    let fds = format!(
        r#"<FixedDocumentSequence xmlns="{NS}"><DocumentReference Source="Documents/1/FixedDocument.fdoc"/></FixedDocumentSequence>"#
    );
    let fdoc = format!(
        r#"<FixedDocument xmlns="{NS}"><PageContent Source="Pages/1.fpage" Width="96" Height="96"/></FixedDocument>"#
    );
    let rels = rels(FIXED_REL, "/FixedDocSeq.fdseq");
    let z = zip_of(&[
        ("_rels/.rels", rels.as_bytes()),
        ("FixedDocSeq.fdseq", fds.as_bytes()),
        ("Documents/1/FixedDocument.fdoc", fdoc.as_bytes()),
        ("Documents/1/Pages/1.fpage/[1].last.piece", b),
        ("Documents/1/Pages/1.fpage/[0].piece", a),
        ("Documents/1/Resources/r.dict", dict.as_bytes()),
    ]);
    let d = open(z);
    let img = draw(&d, 0);
    assert!(near(px(&img, 48, 48), [0, 0, 255], 1));
}

#[test]
fn regions_match_the_whole_page() {
    let body = page(
        r##"<Path Data="M 5,5 L 90,20 50,90 Z" Fill="#336699" Stroke="#000000" StrokeThickness="2"/>"##,
    );
    let d = open(package(NS, FIXED_REL, &[body], &[]));
    let s = 2.0 / 0.75;
    let full = d
        .render(
            RenderRequest {
                page: 0,
                scale_x: s,
                scale_y: s,
                x: 0,
                y: 0,
                width: 192,
                height: 192,
            },
            &|| false,
        )
        .unwrap();
    let part = d
        .render(
            RenderRequest {
                page: 0,
                scale_x: s,
                scale_y: s,
                x: 64,
                y: 32,
                width: 64,
                height: 96,
            },
            &|| false,
        )
        .unwrap();
    for y in 0..96 {
        for x in 0..64 {
            let a = &full[(((y + 32) * 192 + x + 64) * 4) as usize..][..4];
            let b = &part[((y * 64 + x) * 4) as usize..][..4];
            for c in 0..3 {
                assert!(a[c].abs_diff(b[c]) <= 1, "({x},{y})");
            }
        }
    }
}

#[test]
fn refusals() {
    let e = |b: Vec<u8>| Document::open(Arc::new(b), None).unwrap_err();
    let mut cfb = CFB.to_vec();
    cfb.extend_from_slice(&[0; 512]);
    assert_eq!(e(cfb), OpenError::Protected);
    assert_eq!(
        OpenError::Protected.to_string(),
        "rights-managed (protected) XPS document: not shown"
    );
    assert_eq!(e(b"hello".to_vec()), OpenError::NotXps);
    assert_eq!(e(zip_of(&[("a.txt", &b"x"[..])])), OpenError::NotXps);
    let enc = zip_flags(&[("_rels/.rels", &b"x"[..])], 1);
    assert_eq!(e(enc), OpenError::Encrypted);
    assert_eq!(e(package(NS, FIXED_REL, &[], &[])), OpenError::NoPages);
    let mut z = package(NS, FIXED_REL, &[page("")], &[]);
    let n = z.len();
    z.truncate(n - 30);
    assert!(matches!(e(z), OpenError::NotXps | OpenError::Damaged));
}

#[test]
fn hostile_input_is_bounded() {
    // Deep nesting, a huge repeat count of path commands and a bad page.
    // Nested past the drawing depth but within what the parser is given: drawn to the depth.
    let nest = |n: usize| {
        let mut deep = "<Canvas>".repeat(n);
        deep.push_str(r##"<Path Data="M 0,0 H 96 V 96 Z" Fill="#000000"/>"##);
        deep.push_str(&"</Canvas>".repeat(n));
        deep
    };
    let deep = nest(400);
    // Nested past what the parser is given: the page is refused, not a stack overflow.
    let deeper = nest(5000);
    let mut long = String::from("M 0,0");
    for i in 0..200_000 {
        long.push_str(&format!(" L {},{}", i % 96, (i * 7) % 96));
    }
    let pages = [
        page(&deep),
        page(&format!(
            r##"<Path Data="{long}" Stroke="#000000" StrokeThickness="0.1"/>"##
        )),
        "<FixedPage".to_owned(),
        page(
            r##"<Path Data="M 0,0 A 1e300,1e300 0 1 1 NaN,5 Z" Fill="#000000"/><Glyphs OriginX="0" OriginY="0" FontRenderingEmSize="1e308" UnicodeString="x" Indices="(9999999:0)" Fill="#000"/>"##,
        ),
        page(&deeper),
    ];
    let d = open(package(NS, FIXED_REL, &pages, &[]));
    let _ = draw(&d, 0);
    let _ = draw(&d, 1);
    assert!(matches!(
        d.render(
            RenderRequest {
                page: 2,
                scale_x: 1.0,
                scale_y: 1.0,
                x: 0,
                y: 0,
                width: 10,
                height: 10
            },
            &|| false
        ),
        Err(RenderError::Failed(_))
    ));
    let _ = draw(&d, 3);
    assert!(matches!(
        d.render(
            RenderRequest {
                page: 4,
                scale_x: 1.0,
                scale_y: 1.0,
                x: 0,
                y: 0,
                width: 10,
                height: 10
            },
            &|| false
        ),
        Err(RenderError::Failed(_))
    ));
    // Bounds of the request itself.
    assert!(matches!(
        d.render(
            RenderRequest {
                page: 0,
                scale_x: 1.0,
                scale_y: 1.0,
                x: 0,
                y: 0,
                width: 70_000,
                height: 1
            },
            &|| false
        ),
        Err(RenderError::TooLarge(..))
    ));
    assert_eq!(
        d.render(
            RenderRequest {
                page: 9,
                scale_x: 1.0,
                scale_y: 1.0,
                x: 0,
                y: 0,
                width: 1,
                height: 1
            },
            &|| false
        ),
        Err(RenderError::NoSuchPage(9))
    );
}

#[test]
fn cancellation() {
    let mut body = String::new();
    for i in 0..5000 {
        body.push_str(&format!(
            r##"<Path Data="M {x},0 H {x2} V 96 Z" Fill="#000000"/>"##,
            x = i % 90,
            x2 = i % 90 + 3
        ));
    }
    let d = open(package(NS, FIXED_REL, &[page(&body)], &[]));
    let r = d.render(
        RenderRequest {
            page: 0,
            scale_x: 1.0,
            scale_y: 1.0,
            x: 0,
            y: 0,
            width: 72,
            height: 72,
        },
        &|| true,
    );
    assert_eq!(r, Err(RenderError::Cancelled));
}

#[test]
fn root_size_scan() {
    assert_eq!(
        root_size(r#"<?xml version="1.0"?><FixedPage xmlns="x" Height = '20.5' Width="10">"#),
        Some((10.0, 20.5))
    );
    assert_eq!(
        root_size(r#"<FixedPage MaxWidth="3" Width="4" Height="5">"#),
        Some((4.0, 5.0))
    );
}

#[test]
fn fixture_packages_open() {
    for openxps in [false, true] {
        let p = crate::fixture::package(
            openxps,
            &[
                (
                    96.0,
                    48.0,
                    r##"<Path Data="M 0,0 H 96 V 48 H 0 Z" Fill="#FF0000"/>"##,
                ),
                (192.0, 96.0, ""),
            ],
        );
        let d = Document::open(Arc::new(p), None).unwrap();
        assert_eq!(d.page_count(), 2);
        assert_eq!(d.page_size(0), Some((72.0, 36.0)));
        assert_eq!(d.page_size(1), Some((144.0, 72.0)));
        let f = d.facts();
        assert_eq!(f.openxps, openxps);
        assert_eq!(f.title.as_deref(), Some(crate::fixture::TITLE));
        assert_eq!(f.creator.as_deref(), Some(crate::fixture::CREATOR));
        let img = draw(&d, 0);
        assert_eq!(px(&img, 10, 10), [255, 0, 0, 255]);
    }
}

/// Visual brushes 20 deep, each drawing three paths filled with the brush below: 3^20
/// draws, stopped at `MAX_DRAWS` (what was drawn is shown).
#[test]
fn visual_brush_fan_out_is_bounded() {
    let mut dict = String::from(r##"<SolidColorBrush x:Key="b0" Color="#00FF00"/>"##);
    for k in 1..=20 {
        let p = format!(
            r##"<Path Data="M 0,0 H 10 V 10 H 0 Z" Fill="{{StaticResource b{}}}"/>"##,
            k - 1
        );
        dict.push_str(&format!(
            r##"<VisualBrush x:Key="b{k}" Viewbox="0,0,10,10" Viewport="0,0,96,96"><VisualBrush.Visual><Canvas>{p}{p}{p}</Canvas></VisualBrush.Visual></VisualBrush>"##
        ));
    }
    let body = page(&format!(
        r##"<FixedPage.Resources><ResourceDictionary>{dict}</ResourceDictionary></FixedPage.Resources>
        <Path Data="M 0,0 H 96 V 96 H 0 Z" Fill="{{StaticResource b20}}"/>"##
    ));
    let t = std::time::Instant::now();
    let _ = draw(&open(package(NS, FIXED_REL, &[body], &[])), 0);
    assert!(t.elapsed().as_secs() < 30);
}

/// A remote resource dictionary naming itself, and a chain of 40 naming the next: each is
/// read once and at most `MAX_REMOTE_DEPTH` deep, where they used to recurse until the stack
/// overflowed.
#[test]
fn remote_dictionary_cycle_and_chain() {
    let dict = |next: &str| {
        format!(
            r##"<ResourceDictionary xmlns="{NS}" xmlns:x="http://schemas.microsoft.com/xps/2005/06/resourcedictionary-key">
            <SolidColorBrush x:Key="green" Color="#00FF00"/>
            <Canvas x:Key="c"><Canvas.Resources><ResourceDictionary Source="{next}"/></Canvas.Resources></Canvas>
            </ResourceDictionary>"##
        )
        .into_bytes()
    };
    let body = page(
        r##"<FixedPage.Resources><ResourceDictionary Source="/Resources/d0.dict"/></FixedPage.Resources>
        <Path Data="M 0,0 H 48 V 48 H 0 Z" Fill="{StaticResource green}"/>"##,
    );
    let cycle = package(
        NS,
        FIXED_REL,
        std::slice::from_ref(&body),
        &[("Resources/d0.dict", dict("/Resources/d0.dict"))],
    );
    let names: Vec<String> = (0..40).map(|i| format!("Resources/d{i}.dict")).collect();
    let parts: Vec<(&str, Vec<u8>)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), dict(&format!("/Resources/d{}.dict", i + 1))))
        .collect();
    let chain = package(NS, FIXED_REL, &[body], &parts);
    specular_thread::on_stack(1024 * 1024, || {
        for bytes in [cycle, chain] {
            let img = draw(&open(bytes), 0);
            assert!(near(px(&img, 12, 24), [0, 255, 0], 1));
        }
    });
}
