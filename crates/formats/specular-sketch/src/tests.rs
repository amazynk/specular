use super::*;
use serde_json::json;
use specular_drawpage::{Op, RenderRequest};
use std::sync::Arc;

fn crc32(d: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xedb8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// A stored (uncompressed) ZIP.
pub(crate) fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for (name, data) in members {
        let off = out.len() as u32;
        let crc = crc32(data);
        let mut h = Vec::new();
        h.extend_from_slice(b"PK\x03\x04");
        h.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&h);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
        cd.extend_from_slice(&[0; 12]);
        cd.extend_from_slice(&off.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    let n = members.len() as u16;
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn rgb(r: f64, g: f64, b: f64) -> serde_json::Value {
    json!({"_class": "color", "red": r, "green": g, "blue": b, "alpha": 1.0})
}

fn frame(x: f64, y: f64, w: f64, h: f64) -> serde_json::Value {
    json!({"_class": "rect", "x": x, "y": y, "width": w, "height": h})
}

fn solid(c: serde_json::Value) -> serde_json::Value {
    json!({"style": {"fills": [{"_class": "fill", "isEnabled": true, "fillType": 0, "color": c}]}})
}

fn merge(mut a: serde_json::Value, b: serde_json::Value) -> serde_json::Value {
    if let (Some(a), serde_json::Value::Object(b)) = (a.as_object_mut(), b) {
        for (k, v) in b {
            a.insert(k, v);
        }
    }
    a
}

fn sketch_file(page_layers: serde_json::Value, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let doc = json!({"_class": "document", "pages": [
        {"_class": "MSJSONFileReference", "_ref_class": "MSImmutablePage", "_ref": "pages/P1"}
    ], "currentPageIndex": 0});
    let meta = json!({"appVersion": "70.3", "version": 136, "compatibilityVersion": 99,
        "app": "com.bohemiancoding.sketch3"});
    let page = json!({"_class": "page", "name": "Page 1", "layers": page_layers});
    let d = doc.to_string();
    let m = meta.to_string();
    let p = page.to_string();
    let mut members: Vec<(&str, &[u8])> = vec![
        ("document.json", d.as_bytes()),
        ("meta.json", m.as_bytes()),
        ("pages/P1.json", p.as_bytes()),
    ];
    members.extend_from_slice(extra);
    zip(&members)
}

fn render(p: &specular_drawpage::Page) -> Vec<u8> {
    let (w, h) = (p.size.0.ceil() as u32, p.size.1.ceil() as u32);
    specular_drawpage::render(
        p,
        RenderRequest {
            scale_x: 1.0,
            scale_y: 1.0,
            x: 0,
            y: 0,
            width: w,
            height: h,
        },
        &|| false,
    )
    .unwrap()
}

fn px(b: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [b[i], b[i + 1], b[i + 2], b[i + 3]]
}

fn sample() -> Vec<u8> {
    let rect = merge(
        json!({"_class": "rectangle", "do_objectID": "R", "frame": frame(10.0, 10.0, 40.0, 30.0),
            "isVisible": true}),
        solid(rgb(1.0, 0.0, 0.0)),
    );
    let oval = merge(
        json!({"_class": "oval", "frame": frame(60.0, 10.0, 30.0, 30.0)}),
        solid(rgb(0.0, 0.0, 1.0)),
    );
    let tri = merge(
        json!({"_class": "shapePath", "frame": frame(10.0, 50.0, 40.0, 40.0), "isClosed": true,
        "points": [
            {"point": "{0, 1}", "curveFrom": "{0, 1}", "curveTo": "{0, 1}", "curveMode": 1},
            {"point": "{0.5, 0}", "curveFrom": "{0.5, 0}", "curveTo": "{0.5, 0}", "curveMode": 1},
            {"point": "{1, 1}", "curveFrom": "{1, 1}", "curveTo": "{1, 1}", "curveMode": 1}
        ]}),
        solid(rgb(0.0, 0.6, 0.0)),
    );
    let text = json!({"_class": "text", "do_objectID": "T", "frame": frame(55.0, 50.0, 40.0, 20.0),
        "textBehaviour": 0, "attributedString": {"_class": "attributedString", "string": "Hi",
        "attributes": [{"location": 0, "length": 2, "attributes": {
            "MSAttributedStringFontAttribute": {"attributes": {"name": "Helvetica-Bold", "size": 16}},
            "MSAttributedStringColorAttribute": rgb(0.0, 0.0, 0.0)}}]}});
    let hidden = merge(
        json!({"_class": "rectangle", "frame": frame(0.0, 0.0, 100.0, 100.0), "isVisible": false}),
        solid(rgb(0.0, 0.0, 0.0)),
    );
    let board = json!({"_class": "artboard", "name": "Home", "frame": frame(0.0, 0.0, 100.0, 100.0),
        "hasBackgroundColor": true, "backgroundColor": rgb(1.0, 1.0, 0.0),
        "layers": [rect, oval, tri, text, hidden]});
    let loose = merge(
        json!({"_class": "rectangle", "frame": frame(200.0, 0.0, 20.0, 10.0)}),
        solid(rgb(0.0, 0.0, 0.0)),
    );
    sketch_file(
        json!([board, loose]),
        &[("previews/preview.png", b"PNGDATA")],
    )
}

#[test]
fn draws_artboards_and_loose_layers() {
    let d = sample();
    assert!(probe_sketch_file(&d));
    assert!(!probe_figma_file(&d));
    let doc = open_sketch(&d, None).unwrap();
    assert_eq!(doc.pages.len(), 2);
    assert_eq!(doc.pages[0].name, "Page 1 / Home");
    assert_eq!(doc.pages[0].size, (100.0, 100.0));
    assert_eq!(doc.pages[1].size, (20.0, 10.0));
    assert_eq!(doc.facts.app_version.as_deref(), Some("70.3"));
    assert_eq!(doc.facts.fonts, vec!["Helvetica-Bold".to_string()]);
    assert_eq!(
        doc.facts.describe(0),
        "Sketch document (Sketch 70.3), artboard \"Home\""
    );
    assert!(doc.facts.describe(1).contains("outside artboards"));
    let b = render(&doc.pages[0]);
    assert_eq!(px(&b, 100, 2, 2), [255, 255, 0, 255], "background");
    assert_eq!(px(&b, 100, 30, 25), [255, 0, 0, 255], "rectangle");
    assert_eq!(px(&b, 100, 75, 25), [0, 0, 255, 255], "oval");
    assert_eq!(px(&b, 100, 61, 12), [255, 255, 0, 255], "oval corner empty");
    assert_eq!(px(&b, 100, 30, 85)[1], 153, "triangle");
    assert_eq!(
        px(&b, 100, 12, 55),
        [255, 255, 0, 255],
        "outside the triangle"
    );
    assert!(doc.pages[0].ops.iter().any(|o| matches!(o, Op::Text(_))));
    let b = render(&doc.pages[1]);
    assert_eq!(px(&b, 20, 5, 5), [0, 0, 0, 255]);
    assert_eq!(sketch_preview(&d).unwrap().get(&d), b"PNGDATA");
    let (ov, cur) = open_sketch_overview(&d, None).unwrap();
    assert_eq!((ov.len(), cur), (1, 0));
    assert_eq!(ov[0].size, (220.0, 100.0));
}

#[test]
fn groups_symbols_and_masks() {
    let master = json!({"_class": "symbolMaster", "name": "Btn", "symbolID": "S1",
    "frame": frame(500.0, 0.0, 10.0, 10.0), "layers": [
        merge(json!({"_class": "rectangle", "frame": frame(0.0, 0.0, 10.0, 10.0)}),
            solid(rgb(0.0, 0.0, 1.0))),
        // A symbol using itself is drawn once.
        {"_class": "symbolInstance", "symbolID": "S1", "frame": frame(0.0, 0.0, 5.0, 5.0)}
    ]});
    let inst = json!({"_class": "symbolInstance", "symbolID": "S1",
        "frame": frame(0.0, 0.0, 40.0, 40.0)});
    let mask = merge(
        json!({"_class": "oval", "frame": frame(0.0, 0.0, 40.0, 40.0), "hasClippingMask": true}),
        solid(rgb(0.0, 1.0, 0.0)),
    );
    let masked = merge(
        json!({"_class": "rectangle", "frame": frame(0.0, 0.0, 40.0, 40.0)}),
        solid(rgb(1.0, 0.0, 0.0)),
    );
    let group = json!({"_class": "group", "frame": frame(50.0, 0.0, 40.0, 40.0),
        "isFlippedHorizontal": true, "layers": [mask, masked]});
    let half = merge(
        json!({"_class": "rectangle", "frame": frame(0.0, 50.0, 40.0, 40.0),
            "style": {"contextSettings": {"opacity": 0.5},
            "fills": [{"isEnabled": true, "fillType": 0, "color": rgb(0.0, 0.0, 0.0)}]}}),
        json!({}),
    );
    let board = json!({"_class": "artboard", "name": "A", "frame": frame(0.0, 0.0, 100.0, 100.0),
        "layers": [inst, group, half]});
    let d = sketch_file(json!([board, master]), &[]);
    let doc = open_sketch(&d, None).unwrap();
    // The artboard only: the symbol master is not a page of its own.
    assert_eq!((doc.pages.len(), doc.facts.symbol_masters), (1, 1));
    let b = render(&doc.pages[0]);
    assert_eq!(
        px(&b, 100, 20, 20),
        [0, 0, 255, 255],
        "symbol scaled to its instance"
    );
    assert_eq!(px(&b, 100, 70, 20), [255, 0, 0, 255], "inside the mask");
    assert_eq!(px(&b, 100, 51, 1), [255, 255, 255, 255], "outside the mask");
    let g = px(&b, 100, 20, 70);
    assert!((120..136).contains(&g[0]), "half opacity {g:?}");
}

#[test]
fn shared_style_and_text_overrides() {
    let master = json!({"_class": "symbolMaster", "name": "Icon", "symbolID": "S",
    "frame": frame(0.0, 0.0, 10.0, 10.0), "layers": [
        merge(json!({"_class": "rectangle", "do_objectID": "SHAPE",
            "frame": frame(0.0, 0.0, 10.0, 10.0)}), solid(rgb(0.0, 0.0, 0.0))),
        {"_class": "text", "do_objectID": "LABEL", "frame": frame(0.0, 0.0, 10.0, 10.0),
         "attributedString": {"string": "old", "attributes": [{"location": 0, "length": 3,
           "attributes": {"MSAttributedStringFontAttribute":
             {"attributes": {"name": "Helvetica", "size": 8}}}}]}}
    ]});
    let inst = json!({"_class": "symbolInstance", "symbolID": "S",
        "frame": frame(0.0, 0.0, 10.0, 10.0), "overrideValues": [
            {"overrideName": "SHAPE_layerStyle", "value": "WHITE"},
            {"overrideName": "LABEL_stringValue", "value": "new"}]});
    let board = json!({"_class": "artboard", "name": "A", "frame": frame(0.0, 0.0, 10.0, 10.0),
        "layers": [inst]});
    let doc_json = json!({"_class": "document", "pages": [{"_ref": "pages/P1"}],
        "layerStyles": {"objects": [{"do_objectID": "WHITE", "value":
            {"fills": [{"isEnabled": true, "fillType": 0, "color": rgb(0.0, 1.0, 0.0)}]}}]}});
    let page = json!({"_class": "page", "name": "P", "layers": [board, master]}).to_string();
    let d = zip(&[
        ("document.json", doc_json.to_string().as_bytes()),
        ("meta.json", b"{}"),
        ("pages/P1.json", page.as_bytes()),
    ]);
    let doc = open_sketch(&d, None).unwrap();
    let b = render(&doc.pages[0]);
    assert_eq!(
        px(&b, 10, 9, 9),
        [0, 255, 0, 255],
        "shared style swapped in"
    );
    let text = doc.pages[0]
        .ops
        .iter()
        .find_map(|o| match o {
            Op::Text(t) => Some(t.paragraphs[0].runs[0].text.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(text, "new");
}

#[test]
fn borders_inside_and_outside() {
    let mk = |x: f64, pos: u64| {
        json!({"_class": "rectangle", "frame": frame(x, 10.0, 20.0, 20.0), "style": {
            "borders": [{"isEnabled": true, "fillType": 0, "color": rgb(0.0, 0.0, 0.0),
                "position": pos, "thickness": 4}]}})
    };
    let board = json!({"_class": "artboard", "name": "A", "frame": frame(0.0, 0.0, 100.0, 40.0),
        "layers": [mk(10.0, 1), mk(50.0, 2)]});
    let d = sketch_file(json!([board]), &[]);
    let doc = open_sketch(&d, None).unwrap();
    let b = render(&doc.pages[0]);
    assert_eq!(px(&b, 100, 12, 20)[0], 0, "inside border inside");
    assert_eq!(px(&b, 100, 8, 20)[0], 255, "inside border not outside");
    assert_eq!(px(&b, 100, 48, 20)[0], 0, "outside border outside");
    assert_eq!(px(&b, 100, 52, 20)[0], 255, "outside border not inside");
}

#[test]
fn images_through_the_decoder() {
    let bitmap = json!({"_class": "bitmap", "frame": frame(0.0, 0.0, 10.0, 10.0),
        "image": {"_class": "MSJSONFileReference", "_ref": "images/a.png"}});
    let missing = json!({"_class": "bitmap", "frame": frame(0.0, 0.0, 10.0, 10.0),
        "image": {"_ref": "images/none.png"}});
    let board = json!({"_class": "artboard", "name": "A", "frame": frame(0.0, 0.0, 10.0, 10.0),
        "layers": [bitmap, missing]});
    let d = sketch_file(json!([board]), &[("images/a.png", b"IMG")]);
    let dec: specular_drawpage::ImageDecoder =
        Arc::new(|b: &[u8]| (b == b"IMG").then(|| (1, 1, vec![0, 128, 0, 255])));
    let doc = open_sketch(&d, Some(dec)).unwrap();
    assert_eq!(doc.facts.images_missing, 1);
    let b = render(&doc.pages[0]);
    assert_eq!(px(&b, 10, 5, 5), [0, 128, 0, 255]);
}

#[test]
fn figma_thumbnails_and_refusals() {
    let meta = br#"{"client_meta":{"background_color":{"r":1,"g":0,"b":0,"a":1},
        "thumbnail_size":{"width":400,"height":300}},"file_name":"Board","exported_at":"2024-01-01"}"#;
    let f = zip(&[
        ("canvas.fig", b"fig-kiwi\x10\0\0\0"),
        ("thumbnail.png", b"THUMB"),
        ("meta.json", meta),
    ]);
    assert!(probe_figma_file(&f));
    assert!(!probe_sketch_file(&f));
    assert_eq!(figma_thumbnail(&f).unwrap().get(&f), b"THUMB");
    let facts = figma_facts(&f);
    assert_eq!(facts.file_name.as_deref(), Some("Board"));
    assert_eq!(facts.background, Some([255, 0, 0, 255]));
    assert_eq!(facts.thumbnail_size, Some((400, 300)));
    let bare = b"fig-kiwi\x10\0\0\0rest".to_vec();
    assert!(probe_figma(&bare) && probe_figma_file(&bare));
    let e = figma_thumbnail(&bare).unwrap_err();
    assert!(e.to_string().contains("undocumented"), "{e}");
    let no_thumb = zip(&[("canvas.fig", b"fig-kiwi"), ("meta.json", b"{}")]);
    assert!(matches!(
        figma_thumbnail(&no_thumb),
        Err(Error::Unsupported(_))
    ));
    // Old Sketch: a SQLite database.
    let old = b"SQLite format 3\0........".to_vec();
    let e = open_sketch(&old, None).unwrap_err();
    assert!(e.to_string().contains("before version 43"), "{e}");
    assert!(matches!(sketch_preview(&old), Err(Error::Unsupported(_))));
    assert_eq!(open_sketch(b"nothing", None).unwrap_err(), Error::NotSketch);
}

#[test]
fn hostile_documents_terminate() {
    // Deep nesting: groups inside groups past the depth limit.
    let mut l = merge(
        json!({"_class": "rectangle", "frame": frame(0.0, 0.0, 1.0, 1.0)}),
        solid(rgb(0.0, 0.0, 0.0)),
    );
    for _ in 0..100 {
        l = json!({"_class": "group", "frame": frame(0.0, 0.0, 1.0, 1.0), "layers": [l]});
    }
    // Mutually recursive symbols.
    let a = json!({"_class": "symbolMaster", "symbolID": "A", "frame": frame(0.0, 0.0, 10.0, 10.0),
        "layers": [{"_class": "symbolInstance", "symbolID": "B", "frame": frame(0.0, 0.0, 10.0, 10.0)}]});
    let b = json!({"_class": "symbolMaster", "symbolID": "B", "frame": frame(0.0, 0.0, 10.0, 10.0),
        "layers": [{"_class": "symbolInstance", "symbolID": "A", "frame": frame(0.0, 0.0, 10.0, 10.0)}]});
    let bad = json!({"_class": "shapePath", "frame": {"x": "NaN"}, "points": [{"point": "{x}"}]});
    let board = json!({"_class": "artboard", "name": "A", "frame": frame(0.0, 0.0, 10.0, 10.0),
        "layers": [l, bad.clone(), {"_class": "symbolInstance", "symbolID": "A", "frame": frame(0.0, 0.0, 10.0, 10.0)}]});
    let d = sketch_file(json!([board, a.clone(), b.clone()]), &[]);
    // JSON nested past serde_json's limit is refused as damaged.
    assert!(matches!(open_sketch(&d, None), Err(Error::Malformed(_))));
    let board = json!({"_class": "artboard", "name": "A", "frame": frame(0.0, 0.0, 10.0, 10.0),
        "layers": [bad, {"_class": "symbolInstance", "symbolID": "A", "frame": frame(0.0, 0.0, 10.0, 10.0)}]});
    let d = sketch_file(json!([board, a.clone(), b.clone()]), &[]);
    let doc = open_sketch(&d, None).unwrap();
    assert_eq!((doc.pages.len(), doc.facts.symbol_masters), (1, 2));
    // A pure symbol library: its masters are the pages.
    let d = sketch_file(json!([a, b]), &[]);
    let doc = open_sketch(&d, None).unwrap();
    assert_eq!((doc.pages.len(), doc.facts.symbol_masters), (2, 2));
    assert_eq!(doc.facts.artboards.len(), 2);
    assert_eq!(doc.facts.page_labels.len(), 2);
    // Truncations of a valid file fail or draw, never panic.
    let full = sample();
    for cut in (0..full.len()).step_by(37) {
        let _ = open_sketch(&full[..cut], None);
        let _ = sketch_preview(&full[..cut]);
        let _ = probe_sketch_file(&full[..cut]);
    }
}

#[test]
fn font_names() {
    assert_eq!(
        sketch::family_of_for_tests("Helvetica-BoldOblique"),
        ("Helvetica".to_string(), true, true)
    );
    assert_eq!(
        sketch::family_of_for_tests("OpenSans-Regular"),
        ("Open Sans".to_string(), false, false)
    );
    assert_eq!(
        sketch::family_of_for_tests("TimesNewRomanPSMT"),
        ("Times New Roman".to_string(), false, false)
    );
    assert_eq!(
        sketch::family_of_for_tests("ArialMT"),
        ("Arial".to_string(), false, false)
    );
}

/// Opens every file of the corpus (tools/corpus/design2/sketch and figma, or
/// SPECULAR_SKETCH_CORPUS) without panicking; skipped when absent.
#[test]
fn corpus_opens() {
    let root = std::env::var_os("SPECULAR_SKETCH_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/design2")
        });
    let mut n = 0;
    for sub in ["sketch", "figma"] {
        let Ok(rd) = std::fs::read_dir(root.join(sub)) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let ext = p
                .extension()
                .and_then(|x| x.to_str())
                .map(str::to_ascii_lowercase);
            let Ok(d) = std::fs::read(&p) else { continue };
            match ext.as_deref() {
                Some("sketch") => {
                    let r = open_sketch(&d, None);
                    if let Ok(doc) = &r {
                        assert!(!doc.pages.is_empty());
                        for (i, pg) in doc.pages.iter().enumerate() {
                            assert!(pg.size.0 > 0.0 && pg.size.1 > 0.0, "{p:?} page {i}");
                        }
                    }
                    let _ = sketch_preview(&d);
                    n += 1;
                }
                Some("fig") => {
                    let _ = figma_thumbnail(&d);
                    let _ = figma_facts(&d);
                    n += 1;
                }
                _ => {}
            }
        }
    }
    eprintln!("corpus files: {n}");
}

/// A 10 000-point path placed 8^4 = 4096 times through nested symbols (41M path elements,
/// about 2 GB): drawing stops at `MAX_PATH_ELEMENTS`.
#[test]
fn instanced_path_elements_are_bounded() {
    let pts: Vec<serde_json::Value> = (0..10_000)
        .map(|i| {
            let a = f64::from(i) * 0.001;
            json!({"_class": "curvePoint", "point": format!("{{{}, {}}}", a.cos(), a.sin())})
        })
        .collect();
    let path = merge(
        json!({"_class": "shapePath", "frame": frame(0.0, 0.0, 10.0, 10.0), "isClosed": true,
            "points": pts}),
        solid(rgb(0.0, 0.0, 1.0)),
    );
    let mut layers = vec![
        json!({"_class": "symbolMaster", "name": "S0", "symbolID": "S0",
        "frame": frame(500.0, 0.0, 10.0, 10.0), "layers": [path]}),
    ];
    for k in 1..=4 {
        let inst = json!({"_class": "symbolInstance", "symbolID": format!("S{}", k - 1),
            "frame": frame(0.0, 0.0, 10.0, 10.0)});
        layers.push(json!({"_class": "symbolMaster", "name": format!("S{k}"),
            "symbolID": format!("S{k}"), "frame": frame(500.0, 0.0, 10.0, 10.0),
            "layers": vec![inst; 8]}));
    }
    layers.push(
        json!({"_class": "artboard", "name": "A", "frame": frame(0.0, 0.0, 100.0, 100.0),
        "layers": [{"_class": "symbolInstance", "symbolID": "S4",
            "frame": frame(0.0, 0.0, 10.0, 10.0)}]}),
    );
    let d = sketch_file(serde_json::Value::Array(layers), &[]);
    let doc = open_sketch(&d, None).unwrap();
    let held: usize = doc.pages[0].ops.iter().map(|o| o.path_elements()).sum();
    assert!(held <= specular_drawpage::MAX_PATH_ELEMENTS, "{held}");
    assert!(held > 1_000_000, "{held}");
}
