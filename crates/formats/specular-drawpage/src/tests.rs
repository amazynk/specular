use super::*;

fn req(w: u32, h: u32, s: f64) -> RenderRequest {
    RenderRequest {
        scale_x: s,
        scale_y: s,
        x: 0,
        y: 0,
        width: w,
        height: h,
    }
}

fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

#[test]
fn fills_strokes_gradients_images() {
    let mut p = Page::new("p", (100.0, 100.0));
    p.ops.push(Op::Fill {
        path: Rect::new(0.0, 0.0, 50.0, 50.0).to_path(0.1),
        even_odd: false,
        paint: Paint::Solid([255, 0, 0, 255]),
    });
    p.ops.push(Op::Fill {
        path: Rect::new(50.0, 0.0, 100.0, 50.0).to_path(0.1),
        even_odd: false,
        paint: Paint::Linear {
            start: Point::new(50.0, 0.0),
            end: Point::new(100.0, 0.0),
            stops: vec![
                Stop {
                    offset: 0.0,
                    color: [0, 0, 0, 255],
                },
                Stop {
                    offset: 1.0,
                    color: [255, 255, 255, 255],
                },
            ],
        },
    });
    let img = Arc::new(Image::from_rgba(2, 1, vec![0, 255, 0, 255, 0, 0, 255, 255]).unwrap());
    p.ops.push(Op::Image {
        image: img,
        transform: Affine::translate((0.0, 50.0)) * Affine::scale_non_uniform(25.0, 50.0),
        opacity: 1.0,
    });
    p.ops.push(Op::PushOpacity(0.5));
    p.ops.push(Op::Fill {
        path: Rect::new(50.0, 50.0, 100.0, 100.0).to_path(0.1),
        even_odd: false,
        paint: Paint::Solid([0, 0, 0, 255]),
    });
    p.ops.push(Op::Pop);
    let b = render(&p, req(100, 100, 1.0), &|| false).unwrap();
    assert_eq!(px(&b, 100, 10, 10), [255, 0, 0, 255]);
    let g = px(&b, 100, 75, 10);
    assert!((100..160).contains(&g[0]), "{g:?}");
    assert_eq!(px(&b, 100, 10, 75)[1], 255);
    assert_eq!(px(&b, 100, 40, 75)[2], 255);
    let half = px(&b, 100, 75, 75);
    assert!((120..136).contains(&half[0]), "{half:?}");
    // Region rendering at scale 2: pixel (100, 100) of the scaled page is page (50, 50).
    let r = RenderRequest {
        scale_x: 2.0,
        scale_y: 2.0,
        x: 90,
        y: 10,
        width: 20,
        height: 20,
    };
    let b = render(&p, r, &|| false).unwrap();
    assert_eq!(px(&b, 20, 2, 2), [255, 0, 0, 255]);
    assert!(px(&b, 20, 18, 2)[0] < 30);
}

#[test]
fn clip_and_bounds() {
    let mut p = Page::new("p", (10.0, 10.0));
    p.ops.push(Op::PushClip {
        path: Rect::new(0.0, 0.0, 5.0, 10.0).to_path(0.1),
        even_odd: false,
    });
    p.ops.push(Op::Fill {
        path: Rect::new(0.0, 0.0, 10.0, 10.0).to_path(0.1),
        even_odd: false,
        paint: Paint::BLACK,
    });
    p.ops.push(Op::Pop);
    // An unbalanced pop is ignored.
    p.ops.push(Op::Pop);
    let b = render(&p, req(10, 10, 1.0), &|| false).unwrap();
    assert_eq!(px(&b, 10, 2, 5), [0, 0, 0, 255]);
    assert_eq!(px(&b, 10, 8, 5), [255, 255, 255, 255]);
    let r = content_bounds(&p.ops).unwrap();
    assert_eq!(r, Rect::new(0.0, 0.0, 10.0, 10.0));
    translate_ops(&mut p.ops, kurbo::Vec2::new(1.0, 2.0));
    assert_eq!(
        content_bounds(&p.ops).unwrap(),
        Rect::new(1.0, 2.0, 11.0, 12.0)
    );
    assert!(matches!(
        render(&p, req(0, 10, 1.0), &|| false),
        Err(RenderError::TooLarge(..))
    ));
    assert_eq!(
        render(&p, req(10, 10, 1.0), &|| true),
        Err(RenderError::Cancelled)
    );
}

fn ink_columns(b: &[u8], w: u32, h: u32) -> (u32, u32) {
    let (mut lo, mut hi) = (u32::MAX, 0);
    for y in 0..h {
        for x in 0..w {
            if px(b, w, x, y)[0] < 128 {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
    }
    (lo, hi)
}

#[test]
fn text_wraps_and_aligns() {
    let mut t = TextBlock::new(Affine::translate((10.0, 10.0)), 80.0, 80.0);
    t.paragraphs.push(Paragraph {
        runs: vec![Run::new("Hello wrapping world of text", "Arial", 12.0)],
        align: HAlign::Centre,
        ..Paragraph::default()
    });
    let (glyphs, _) = text::layout(&t);
    let ys: std::collections::BTreeSet<i64> = glyphs.iter().map(|g| g.y.round() as i64).collect();
    assert!(ys.len() >= 2, "wrapped into lines: {ys:?}");
    for g in &glyphs {
        assert!(g.x >= -0.01 && g.x <= 80.0, "{}", g.x);
    }
    let mut p = Page::new("p", (100.0, 100.0));
    p.ops.push(Op::Text(Box::new(t.clone())));
    let b = render(&p, req(100, 100, 1.0), &|| false).unwrap();
    let (lo, hi) = ink_columns(&b, 100, 100);
    assert!(lo >= 10 && hi <= 90, "{lo}..{hi}");
    // Right-aligned, no wrap, anchored at the bottom.
    let mut t2 = t;
    t2.wrap = false;
    t2.valign = VAlign::Bottom;
    t2.paragraphs[0].align = HAlign::Right;
    t2.paragraphs[0].runs[0].text = "Hi".into();
    let (glyphs, _) = text::layout(&t2);
    assert!(glyphs.iter().all(|g| g.x > 60.0 && g.y > 70.0));
}

#[test]
fn hex_colours() {
    assert_eq!(hex_color("#fff"), Some([255, 255, 255, 255]));
    assert_eq!(hex_color("#102030"), Some([16, 32, 48, 255]));
    assert_eq!(hex_color("#10203080"), Some([16, 32, 48, 128]));
    assert_eq!(hex_color("red"), None);
}
