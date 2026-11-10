use super::*;
use crate::fixture::{Page, build, encrypted, fill};

fn never() -> bool {
    false
}

fn two_pages() -> Vec<u8> {
    let red = fill(200.0, 100.0, [1.0, 0.0, 0.0]);
    // Page 2: white page with a blue square in the lower-left quarter (PDF y points up).
    let blue = "0 0 1 rg 0 0 50 75 re f";
    build(
        &[
            Page {
                width: 200.0,
                height: 100.0,
                content: &red,
            },
            Page {
                width: 100.0,
                height: 150.0,
                content: blue,
            },
        ],
        "",
        Some("/Title (Test \\(doc\\)) /Author <FEFF00C40062> /Keywords (a, b; c) /Producer (gen)"),
        "",
        &[],
    )
}

fn open(data: Vec<u8>) -> Result<Document, OpenError> {
    Document::open(Arc::new(data))
}

fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = (y * w + x) as usize * 4;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

fn full(page: usize, w: u32, h: u32, scale: f64) -> RenderRequest {
    RenderRequest {
        page,
        scale_x: scale,
        scale_y: scale,
        x: 0,
        y: 0,
        width: w,
        height: h,
    }
}

#[test]
fn pages_and_sizes() {
    let d = open(two_pages()).unwrap();
    assert_eq!(d.page_count(), 2);
    assert_eq!(d.page_size(0), Some((200.0, 100.0)));
    assert_eq!(d.page_size(1), Some((100.0, 150.0)));
    assert_eq!(d.page_size(2), None);
    assert_eq!(d.version(), "1.7");
}

#[test]
fn renders_pages() {
    let d = open(two_pages()).unwrap();
    let a = d.render(full(0, 400, 200, 2.0), &never).unwrap();
    assert_eq!(a.len(), 400 * 200 * 4);
    for (x, y) in [(0, 0), (399, 199), (200, 100)] {
        assert_eq!(px(&a, 400, x, y), [255, 0, 0, 255]);
    }
    let b = d.render(full(1, 100, 150, 1.0), &never).unwrap();
    // Blue in the lower-left 50×75 (raster rows 75..150), white elsewhere.
    assert_eq!(px(&b, 100, 10, 140), [0, 0, 255, 255]);
    assert_eq!(px(&b, 100, 10, 10), [255, 255, 255, 255]);
    assert_eq!(px(&b, 100, 90, 140), [255, 255, 255, 255]);
    assert!(b.chunks_exact(4).all(|p| p[3] == 255));
}

#[test]
fn region_matches_full_render() {
    let d = open(two_pages()).unwrap();
    let scale = 3.0;
    let (w, h) = (300, 450);
    let whole = d.render(full(1, w, h, scale), &never).unwrap();
    let (rx, ry, rw, rh) = (120, 200, 64, 48);
    let part = d
        .render(
            RenderRequest {
                page: 1,
                scale_x: scale,
                scale_y: scale,
                x: rx,
                y: ry,
                width: rw,
                height: rh,
            },
            &never,
        )
        .unwrap();
    for y in 0..rh {
        for x in 0..rw {
            assert_eq!(
                px(&part, rw, x, y),
                px(&whole, w, rx + x, ry + y),
                "{x},{y}"
            );
        }
    }
}

#[test]
fn info_and_xmp() {
    let d = open(two_pages()).unwrap();
    let info = d.info();
    assert_eq!(info.title.as_deref(), Some("Test (doc)"));
    assert_eq!(info.author.as_deref(), Some("Äb"));
    assert_eq!(info.producer.as_deref(), Some("gen"));
    let xmp = String::from_utf8(d.xmp().unwrap()).unwrap();
    assert!(xmp.contains("<rdf:li xml:lang=\"x-default\">Test (doc)</rdf:li>"));
    assert!(xmp.contains("<dc:creator><rdf:Seq><rdf:li>Äb</rdf:li></rdf:Seq></dc:creator>"));
    assert!(xmp.contains("<rdf:li>a</rdf:li><rdf:li>b</rdf:li><rdf:li>c</rdf:li>"));
}

#[test]
fn catalog_xmp_is_preferred() {
    let packet = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"></x:xmpmeta>";
    let content = fill(10.0, 10.0, [0.0, 0.0, 0.0]);
    let pdf = build(
        &[Page {
            width: 10.0,
            height: 10.0,
            content: &content,
        }],
        "/Metadata 5 0 R",
        None,
        "",
        &[&format!(
            "<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n{packet}\nendstream",
            packet.len()
        )],
    );
    let d = open(pdf).unwrap();
    assert_eq!(d.xmp().unwrap(), packet.as_bytes());
}

#[test]
fn text_strings() {
    assert_eq!(decode_text_string(b"plain"), Some("plain".into()));
    assert_eq!(decode_text_string(b"\x80 \x92 \xA0"), Some("• ™ €".into()));
    assert_eq!(
        decode_text_string(b"\xEF\xBB\xBFgr\xC3\xBC\xC3\x9F"),
        Some("grüß".into())
    );
    assert_eq!(
        decode_text_string(&[0xFE, 0xFF, 0xD8, 0x3D, 0xDE, 0x00]),
        Some("😀".into())
    );
    assert_eq!(decode_text_string(b"  \x01 "), None);
    assert_eq!(decode_text_string(b"a\nb"), Some("a b".into()));
}

#[test]
fn encrypted_is_protected() {
    assert_eq!(open(encrypted()).unwrap_err(), OpenError::Protected);
    assert_eq!(
        OpenError::Protected.to_string(),
        "password-protected PDF: not shown"
    );
}

#[test]
fn malformed_never_panics() {
    let good = two_pages();
    assert!(open(Vec::new()).is_err());
    assert!(open(b"%PDF-1.7\n".to_vec()).is_err());
    assert!(open(b"not a pdf at all".to_vec()).is_err());
    // Every truncation, and bytes flipped through the file: errors or renders, never panics.
    for cut in (0..good.len()).step_by(7) {
        if let Ok(d) = open(good[..cut].to_vec()) {
            for p in 0..d.page_count() {
                let _ = d.render(full(p, 20, 20, 0.1), &never);
            }
        }
    }
    let mut state = 0x2545_f491_u32;
    for _ in 0..200 {
        let mut bad = good.clone();
        for _ in 0..4 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let i = state as usize % bad.len();
            bad[i] = (state >> 24) as u8;
        }
        if let Ok(d) = open(bad) {
            for p in 0..d.page_count() {
                let _ = d.render(full(p, 20, 20, 0.1), &never);
                let _ = d.info();
                let _ = d.xmp();
            }
        }
    }
}

/// 100 000 nested arrays and dictionaries in the catalog, the info dictionary and a page's
/// content stream: opening (on the caller's small stack), the facts and the render (on its
/// own thread) stop at the parser's nesting cap instead of overflowing.
#[test]
fn deep_nesting_is_bounded() {
    let deep_array = "[".repeat(100_000) + &"]".repeat(100_000);
    let deep_dict = "<< /A ".repeat(100_000) + "1" + &" >>".repeat(100_000);
    let content = format!("{deep_array} pop 0 0 1 rg 0 0 10 10 re f");
    let page = [Page {
        width: 100.0,
        height: 100.0,
        content: &content,
    }];
    let deep_info = build(
        &page,
        "",
        Some(&format!("/Title (t) /Deep {deep_dict}")),
        "",
        &[],
    );
    let deep_catalog = build(
        &page,
        &format!("/Deep {deep_array} /Deeper {deep_dict}"),
        None,
        "",
        &[],
    );
    specular_thread::on_stack(512 * 1024, || {
        let d = open(deep_info).unwrap();
        assert_eq!(d.page_count(), 1);
        let _ = d.info();
        let _ = d.xmp();
        let _ = d.render(full(0, 20, 20, 0.2), &never);
        // A catalog nested past the cap is unreadable: an error, not a crash.
        if let Ok(d) = open(deep_catalog) {
            let _ = d.render(full(0, 20, 20, 0.2), &never);
        }
    });
}

#[test]
fn render_bounds() {
    let d = open(two_pages()).unwrap();
    let mut r = full(0, 0, 10, 1.0);
    assert!(matches!(
        d.render(r, &never),
        Err(RenderError::TooLarge(..))
    ));
    r.width = 70_000;
    assert!(matches!(
        d.render(r, &never),
        Err(RenderError::TooLarge(..))
    ));
    r = full(0, 10, 10, f64::NAN);
    assert!(matches!(
        d.render(r, &never),
        Err(RenderError::TooLarge(..))
    ));
    assert_eq!(
        d.render(full(5, 10, 10, 1.0), &never),
        Err(RenderError::NoSuchPage(5))
    );
}

#[test]
fn cancelled_render_returns() {
    let d = open(two_pages()).unwrap();
    let r = d.render(full(0, 2000, 1000, 10.0), &|| true);
    // Either it finished inside the first poll or it was cancelled; never hangs.
    assert!(matches!(r, Ok(_) | Err(RenderError::Cancelled)));
}

/// Hostile structure: recursion and nesting driven by the file must neither overflow a stack
/// nor hang.
mod hostile {
    use super::*;
    use crate::fixture::{raw, stream};

    /// Catalog, page tree, a page with `resources` and `content`, then `extra` as objects 5…
    fn page(resources: &str, content: &str, extra: &[String]) -> Vec<u8> {
        let mut o = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 50 50] /Contents 4 0 R \
                 /Resources << {resources} >> >>"
            ),
            stream("", content),
        ];
        o.extend_from_slice(extra);
        raw(&o)
    }

    fn try_render(data: Vec<u8>) -> Option<Result<Vec<u8>, RenderError>> {
        let d = open(data).ok()?;
        let r = d.render(full(0, 20, 20, 0.4), &never);
        let _ = d.info();
        let _ = d.xmp();
        Some(r)
    }

    #[test]
    fn self_referencing_stitching_function() {
        // hayro does not resolve a reference back into the chain being resolved, so cycles
        // render (the shading is dropped); a long acyclic chain is refused.
        let stitch = |next: usize| {
            format!(
                "<< /FunctionType 3 /Domain [0 1] /Functions [{next} 0 R] /Bounds [] \
                 /Encode [0 1] >>"
            )
        };
        let shading =
            "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 50 0] /Function 6 0 R >>";
        let r = try_render(page(
            "/Shading << /Sh1 5 0 R >>",
            "/Sh1 sh",
            &[shading.into(), stitch(7), stitch(6)],
        ));
        assert!(matches!(r, Some(Ok(_))));
        let n = 100_000;
        let mut extra = vec![shading.to_string()];
        extra.extend((7..n + 7).map(stitch));
        extra.push("<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 1 1] /N 1 >>".to_string());
        let r = try_render(page("/Shading << /Sh1 5 0 R >>", "/Sh1 sh", &extra));
        assert_eq!(r, Some(Err(RenderError::Failed)));
    }

    #[test]
    fn self_referencing_form() {
        try_render(page(
            "/XObject << /X1 5 0 R >>",
            "/X1 Do",
            &[stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 50 50] \
                 /Resources << /XObject << /X1 5 0 R >> >>",
                "/X1 Do 0 0 1 rg 0 0 5 5 re f",
            )],
        ));
    }

    /// Forms each drawing the next ten times, `levels` deep (objects 5…), the last a square.
    fn form_dag(levels: usize) -> Vec<u8> {
        let mut forms: Vec<String> = (0..levels)
            .map(|i| {
                stream(
                    &format!(
                        "/Type /XObject /Subtype /Form /BBox [0 0 50 50] \
                         /Resources << /XObject << /X {} 0 R >> >>",
                        i + 6
                    ),
                    &"/X Do ".repeat(10),
                )
            })
            .collect();
        forms.push(stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 50 50]",
            "0 0 1 rg 0 0 5 5 re f",
        ));
        page("/XObject << /X 5 0 R >>", "/X Do", &forms)
    }

    /// 10²⁴ draws through 24 levels of forms: refused at once from the page's work count,
    /// instead of a render thread abandoned at the timeout and running on. Shallow sharing
    /// (10⁴ draws) still renders.
    #[test]
    fn exponential_form_dag() {
        let t = std::time::Instant::now();
        let r = try_render(form_dag(24));
        assert_eq!(r, Some(Err(RenderError::TooComplex)));
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
        assert!(matches!(try_render(form_dag(4)), Some(Ok(_))));
    }

    /// Refused from the work count alone, without running the page (no timing involved).
    fn refused(data: Vec<u8>) {
        let r = try_render(data).map(|r| r.map(|px| px.len()));
        assert_eq!(r, Some(Err(RenderError::TooComplex)));
    }

    /// Tiling patterns whose cell fills ten times with the next pattern, `levels` deep.
    fn pattern_dag(levels: usize) -> Vec<u8> {
        let cell = |res: String, content: &str| {
            stream(
                &format!(
                    "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
                     /BBox [0 0 10 10] /XStep 10 /YStep 10 {res}"
                ),
                content,
            )
        };
        let mut o: Vec<String> = (0..levels)
            .map(|i| {
                cell(
                    format!("/Resources << /Pattern << /P {} 0 R >> >>", i + 6),
                    &format!("/Pattern cs /P scn {}", "0 0 5 5 re f ".repeat(10)),
                )
            })
            .collect();
        o.push(cell(String::new(), "0 0 1 rg 0 0 5 5 re f"));
        page(
            "/Pattern << /P 5 0 R >>",
            "/Pattern cs /P scn 0 0 50 50 re f",
            &o,
        )
    }

    #[test]
    fn exponential_tiling_patterns() {
        refused(pattern_dag(24));
        assert!(matches!(try_render(pattern_dag(2)), Some(Ok(_))));
    }

    /// Type 3 fonts whose glyph `a` shows ten `a`s of the next font, `levels` deep.
    fn glyph_dag(levels: usize) -> Vec<u8> {
        let mut o = Vec::new();
        for i in 0..=levels {
            let (font, proc_) = (5 + 2 * i, 6 + 2 * i);
            let res = if i < levels {
                format!("/Resources << /Font << /F {} 0 R >> >>", font + 2)
            } else {
                String::new()
            };
            o.push(format!(
                "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 10 10] \
                 /FontMatrix [0.1 0 0 0.1 0 0] /CharProcs << /a {proc_} 0 R >> \
                 /Encoding << /Differences [97 /a] >> /FirstChar 97 /LastChar 97 \
                 /Widths [10] {res} >>"
            ));
            o.push(stream(
                "",
                if i < levels {
                    "10 0 d0 BT /F 1 Tf (aaaaaaaaaa) Tj ET"
                } else {
                    "10 0 d0 0 0 5 5 re f"
                },
            ));
        }
        page("/Font << /F 5 0 R >>", "BT /F 12 Tf 0 0 Td (a) Tj ET", &o)
    }

    #[test]
    fn exponential_type3_glyphs() {
        refused(glyph_dag(24));
        assert!(matches!(try_render(glyph_dag(2)), Some(Ok(_))));
    }

    /// Soft masks whose group sets the next mask ten times, each at a new position (hayro
    /// draws a mask once per state and transform), `levels` deep.
    fn mask_dag(levels: usize) -> Vec<u8> {
        let mut o = Vec::new();
        for i in 0..=levels {
            let form = 6 + 2 * i;
            o.push(format!(
                "<< /Type /ExtGState /SMask << /S /Luminosity /G {form} 0 R >> >>"
            ));
            let group = "/Type /XObject /Subtype /Form /BBox [0 0 50 50] \
                         /Group << /S /Transparency /CS /DeviceGray >>";
            o.push(if i < levels {
                stream(
                    &format!(
                        "{group} /Resources << /ExtGState << /G {} 0 R >> >>",
                        form + 1
                    ),
                    &"q 1 0 0 1 1 0 cm /G gs 1 g 0 0 5 5 re f Q ".repeat(10),
                )
            } else {
                stream(group, "1 g 0 0 5 5 re f")
            });
        }
        page("/ExtGState << /G 5 0 R >>", "/G gs 0 0 50 50 re f", &o)
    }

    #[test]
    fn exponential_soft_masks() {
        refused(mask_dag(24));
        assert!(matches!(try_render(mask_dag(2)), Some(Ok(_))));
    }

    /// A page drawing nothing itself, with `n` annotations whose appearance draws a form ten
    /// times, `levels` deep.
    fn annotated(n: usize, levels: usize) -> Vec<u8> {
        let annots = n + 5;
        let mut o = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 50 50] /Contents 4 0 R \
                 /Annots [{}] >>",
                (5..annots)
                    .map(|i| format!("{i} 0 R"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            stream("", ""),
        ];
        o.extend((0..n).map(|_| {
            format!(
                "<< /Type /Annot /Subtype /Square /Rect [0 0 50 50] /AP << /N {annots} 0 R >> >>"
            )
        }));
        for i in 0..levels {
            o.push(stream(
                &format!(
                    "/Type /XObject /Subtype /Form /BBox [0 0 50 50] \
                     /Resources << /XObject << /X {} 0 R >> >>",
                    annots + i + 1
                ),
                &"/X Do ".repeat(10),
            ));
        }
        o.push(stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 50 50]",
            "0 0 1 rg 0 0 5 5 re f",
        ));
        raw(&o)
    }

    #[test]
    fn exponential_annotation_appearances() {
        refused(annotated(1, 24));
        // Each appearance is cheap; together they are not.
        refused(annotated(3000, 4));
        assert!(matches!(try_render(annotated(2, 2)), Some(Ok(_))));
    }

    /// Content that runs itself through its caller's resources (hayro-syntax cuts a cycle of
    /// references, not one through inherited resources): hayro follows a form 50 deep (2⁵⁰
    /// draws when it draws itself twice) and a Type 3 glyph without a limit.
    #[test]
    fn content_running_itself() {
        refused(page(
            "/XObject << /X1 5 0 R >>",
            "/X1 Do",
            &[stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 50 50]",
                "/X1 Do /X1 Do",
            )],
        ));
        refused(page(
            "/Font << /F 5 0 R >>",
            "BT /F 12 Tf (a) Tj ET",
            &[
                "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 10 10] \
                 /FontMatrix [0.1 0 0 0.1 0 0] /CharProcs << /a 6 0 R >> \
                 /Encoding << /Differences [97 /a] >> /FirstChar 97 /LastChar 97 \
                 /Widths [10] >>"
                    .to_string(),
                stream("", "10 0 d0 BT /F 1 Tf (a) Tj ET"),
            ],
        ));
        // A reference back into its own chain is not followed: still renders.
        assert!(matches!(
            try_render(page(
                "/XObject << /X1 5 0 R >>",
                "/X1 Do",
                &[stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 50 50] \
                     /Resources << /XObject << /X1 5 0 R >> >>",
                    "/X1 Do /X1 Do",
                )],
            )),
            Some(Ok(_))
        ));
    }

    #[test]
    fn deep_arrays_in_content() {
        let n = 200_000;
        let content = format!("{}{} TJ", "[".repeat(n), "]".repeat(n));
        try_render(page("", &content, &[]));
    }

    #[test]
    fn deep_dicts_in_objects() {
        let n = 200_000;
        let deep = format!("{}{}", "<< /A ".repeat(n), ">> ".repeat(n));
        try_render(page("/ExtGState << /G1 5 0 R >>", "/G1 gs", &[deep]));
        let deep = format!("{}{}", "[".repeat(n), "]".repeat(n));
        try_render(page("/ColorSpace << /C1 5 0 R >>", "/C1 cs", &[deep]));
    }

    #[test]
    fn deep_postscript_function() {
        let n = 200_000;
        let body = format!("{{{}{}}}", "{ ".repeat(n), "} if ".repeat(n));
        let r = try_render(page(
            "/Shading << /Sh1 5 0 R >>",
            "/Sh1 sh",
            &[
                "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 50 0] /Function 6 0 R >>"
                    .into(),
                stream("/FunctionType 4 /Domain [0 1] /Range [0 1]", &body),
            ],
        ));
        assert_eq!(r, Some(Err(RenderError::Failed)));
    }

    #[test]
    fn shallow_functions_still_render() {
        let r = try_render(page(
            "/Shading << /Sh1 5 0 R >>",
            "/Sh1 sh",
            &[
                "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 50 0] /Function 6 0 R >>"
                    .into(),
                "<< /FunctionType 3 /Domain [0 1] /Functions [7 0 R 7 0 R] /Bounds [0.5] \
                 /Encode [0 1 0 1] >>"
                    .into(),
                stream(
                    "/FunctionType 4 /Domain [0 1] /Range [0 1]",
                    "{ dup 0.5 gt { pop 1 } { pop 0 } ifelse }",
                ),
            ],
        ));
        let px = r.unwrap().unwrap();
        // Black on the left half, white on the right.
        assert_eq!(&px[..3], &[0, 0, 0]);
        assert_eq!(&px[px.len() - 4..px.len() - 1], &[255, 255, 255]);
    }

    #[test]
    fn reference_cycles() {
        // Objects that are references to each other, and a page tree that contains itself.
        try_render(page(
            "/ExtGState 5 0 R",
            "/G1 gs",
            &["6 0 R".into(), "5 0 R".into()],
        ));
        let o = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R 2 0 R] /Count 2 /Parent 2 0 R >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>".to_string(),
            stream("", "0 0 1 rg 0 0 5 5 re f"),
        ];
        try_render(raw(&o));
    }

    #[test]
    fn self_referencing_pattern_and_soft_mask() {
        try_render(page(
            "/Pattern << /P1 5 0 R >>",
            "/Pattern cs /P1 scn 0 0 50 50 re f",
            &[stream(
                "/PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] /XStep 10 \
                 /YStep 10 /Resources << /Pattern << /P1 5 0 R >> >>",
                "/Pattern cs /P1 scn 0 0 10 10 re f",
            )],
        ));
        try_render(page(
            "/ExtGState << /G1 5 0 R >>",
            "/G1 gs 0 0 50 50 re f",
            &[
                "<< /SMask << /S /Luminosity /G 6 0 R >> >>".into(),
                stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 50 50] \
                     /Group << /S /Transparency >> /Resources << /ExtGState << /G1 5 0 R >> >>",
                    "/G1 gs 0 0 50 50 re f",
                ),
            ],
        ));
    }
}

mod eps {
    use crate::eps::*;

    #[test]
    fn epsi_one_bit() {
        // 10×2 at 1 bit: row bytes 2. Row 0 all black-ish pattern, row 1 white.
        let ps = b"%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 10 2\n%%EndComments\n\
                   %%BeginPreview: 10 2 1 2\n%FFC0\n%0000\n%%EndPreview\nshowpage\n";
        assert!(looks_like_eps(ps));
        assert_eq!(bounding_box(ps), Some([0.0, 0.0, 10.0, 2.0]));
        let p = epsi_preview(ps).unwrap();
        assert_eq!((p.width, p.height, p.depth), (10, 2, 1));
        assert_eq!(&p.gray[..10], &[0; 10]);
        assert_eq!(&p.gray[10..], &[255; 10]);
    }

    #[test]
    fn epsi_eight_bit_split_lines_and_truncation() {
        let ps =
            b"%!PS-Adobe-3.0 EPSF-3.0\n%%BeginPreview: 3 3 8 3\n%00 80\n%FF\n%40\n%%EndPreview\n";
        let p = epsi_preview(ps).unwrap();
        // 0 is white, FF is black; the third row is missing and stays white.
        assert_eq!(p.gray, vec![255, 127, 0, 191, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn epsi_rejects_bad_headers() {
        assert!(epsi_preview(b"%%BeginPreview: 0 3 8 1\n%00\n").is_none());
        assert!(epsi_preview(b"%%BeginPreview: 3 3 3 1\n%00\n").is_none());
        assert!(epsi_preview(b"%%BeginPreview: 99999 99999 8 1\n%00\n").is_none());
        assert!(epsi_preview(b"%!PS-Adobe-3.0 EPSF-3.0\n").is_none());
    }

    #[test]
    fn dos_header_sections() {
        let mut f = vec![0u8; 30];
        f[..4].copy_from_slice(&DOS_MAGIC);
        let ps = b"%!PS-Adobe-3.0 EPSF-3.0\n";
        let put =
            |f: &mut Vec<u8>, at: usize, v: u32| f[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut f, 4, 30);
        put(&mut f, 8, ps.len() as u32);
        put(&mut f, 20, 30 + ps.len() as u32);
        put(&mut f, 24, 4);
        f.extend_from_slice(ps);
        f.extend_from_slice(b"II*\0");
        let d = dos_header(&f).unwrap();
        assert_eq!(&f[d.postscript.clone()], ps);
        assert_eq!(d.tiff, Some(30 + ps.len()..34 + ps.len()));
        assert_eq!(d.wmf, None);
        assert_eq!(postscript(&f), ps);
        assert!(looks_like_eps(&f));
        // A section running past the end is dropped.
        put(&mut f, 24, 400);
        assert_eq!(dos_header(&f).unwrap().tiff, None);
        assert!(dos_header(&f[..20]).is_none());
    }

    #[test]
    fn plain_postscript_is_not_eps() {
        assert!(!looks_like_eps(b"%!PS-Adobe-3.0\n"));
        assert!(looks_like_postscript(b"%!PS-Adobe-3.0\n"));
    }
}
