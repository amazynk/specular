//! SVG output: one `svg` element sized in px, pictures as embedded PNG (16-bit when the
//! figure is deep) drawn pixelated, every item a `path` with its role and facts as `data-`
//! attributes (text items carry their text as `aria-label`), and the provenance in
//! `metadata` (namespace [`NS`]) and in `desc`.

use std::fmt::Write as _;

use crate::{El, Figure, Item, base64, xml_escape};

/// The namespace of the provenance elements.
pub const NS: &str = "urn:specular:figure:1";

/// Path data, coordinates to 1/1000 px.
pub fn path_data(p: &[El]) -> String {
    let mut d = String::new();
    let n = |v: f32| {
        let s = format!("{:.3}", v);
        let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
        if s == "-0" { "0".to_owned() } else { s }
    };
    for e in p {
        let _ = match *e {
            El::Move(a) => write!(d, "M{} {}", n(a[0]), n(a[1])),
            El::Line(a) => write!(d, "L{} {}", n(a[0]), n(a[1])),
            El::Quad(c, a) => write!(d, "Q{} {} {} {}", n(c[0]), n(c[1]), n(a[0]), n(a[1])),
            El::Cubic(c1, c2, a) => write!(
                d,
                "C{} {} {} {} {} {}",
                n(c1[0]),
                n(c1[1]),
                n(c2[0]),
                n(c2[1]),
                n(a[0]),
                n(a[1])
            ),
            El::Close => write!(d, "Z"),
        };
    }
    d
}

fn item(o: &mut String, it: &Item) {
    if it.path.is_empty() {
        return;
    }
    let _ = write!(o, "<path data-role=\"{}\"", it.role.id());
    for (k, v) in &it.data {
        let _ = write!(o, " data-{k}=\"{}\"", xml_escape(v));
    }
    if let Some(t) = &it.text {
        let _ = write!(o, " role=\"img\" aria-label=\"{}\"", xml_escape(t));
    }
    match it.fill {
        Some(c) => {
            let _ = write!(o, " fill=\"{}\"", c.hex());
            if c.a < 1.0 {
                let _ = write!(o, " fill-opacity=\"{:.3}\"", c.a);
            }
        }
        None => o.push_str(" fill=\"none\""),
    }
    if let Some(s) = it.stroke {
        let _ = write!(
            o,
            " stroke=\"{}\" stroke-width=\"{:.3}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"",
            s.colour.hex(),
            s.width
        );
        if s.colour.a < 1.0 {
            let _ = write!(o, " stroke-opacity=\"{:.3}\"", s.colour.a);
        }
    }
    let _ = writeln!(o, " d=\"{}\"/>", path_data(&it.path));
}

/// The figure as an SVG document.
pub fn write(f: &Figure) -> Result<String, String> {
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        o,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
         xmlns:specular=\"{NS}\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">",
        w = f.width,
        h = f.height
    );
    let _ = writeln!(o, "<title>{}</title>", xml_escape(&f.title));
    if !f.meta.is_empty() {
        let lines: Vec<String> = f.meta.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        let _ = writeln!(o, "<desc>{}</desc>", xml_escape(&lines.join("\n")));
        o.push_str("<metadata>\n<specular:provenance>\n");
        for (k, v) in &f.meta {
            let _ = writeln!(
                o,
                "<specular:field key=\"{}\">{}</specular:field>",
                xml_escape(k),
                xml_escape(v)
            );
        }
        o.push_str("</specular:provenance>\n</metadata>\n");
    }
    let _ = writeln!(
        o,
        "<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>",
        f.width,
        f.height,
        f.background.hex()
    );
    for p in &f.pictures {
        let png = crate::png(&p.px, f.deep, &[])?;
        let _ = writeln!(
            o,
            "<image data-name=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
             preserveAspectRatio=\"none\"{} href=\"data:image/png;base64,{}\"/>",
            xml_escape(&p.name),
            p.x,
            p.y,
            p.w,
            p.h,
            if p.smooth {
                ""
            } else {
                " image-rendering=\"pixelated\" style=\"image-rendering:pixelated\""
            },
            base64(&png)
        );
    }
    for it in &f.items {
        item(&mut o, it);
    }
    o.push_str("</svg>\n");
    Ok(o)
}
