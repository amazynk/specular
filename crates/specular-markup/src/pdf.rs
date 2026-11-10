//! A marked-up PDF: the original file with each page's markup added as PDF annotations
//! (Polygon with a cloudy border, FreeText, Line with an arrow, Stamp, Line and Polygon
//! dimensions, and a Text note for a photo pin), each with its appearance, so every reader
//! draws the markup the way Specular does. The page's own content is not touched: it keeps its
//! vectors, its size and its scale. Annotations are flagged to print.
//!
//! Text is drawn in Helvetica (WinAnsi). A line with characters WinAnsi has no code for
//! (Hebrew, Arabic, CJK…) is drawn as outlines set by the caller's text system (an
//! [`OutlineText`]: the viewer's, with its shaping, bidi and fallback fonts, as figures are
//! exported), so it needs no embedded font; without one such characters print as `?`.
//! `/Contents` keeps the full text either way.

use std::fmt::Write as _;
use std::sync::Arc;

use specular_pdf::annotate::{Annot, AnnotateError, add_annotations, num};

use crate::{
    Affine, Doc, Markup, P, STAMP_PAD, STAMP_SUB, Shape, apply, arrow_head, cloud, helvetica,
    invert, label_anchor, pdf_date, pin_marker, stamp_extent_with,
};

/// One step of a text outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathEl {
    Move(P),
    Line(P),
    Quad(P, P),
    Cubic(P, P, P),
    Close,
}

/// A line of text set as outlines: origin at the left end of its baseline, y down, in the
/// units of the size it was set at; and its advance width.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outlined {
    pub path: Vec<PathEl>,
    pub width: f64,
}

/// Sets a line of text (text, size, bold) as outlines; `None` when it cannot.
pub type OutlineText<'a> = dyn Fn(&str, f64, bool) -> Option<Outlined> + 'a;

/// True when Helvetica (WinAnsi) cannot write `s`.
pub fn needs_outlines(s: &str) -> bool {
    s.chars().any(|c| helvetica::win_ansi(c).is_none())
}

/// `pdf` with `doc`'s markup added; `units` is the page's points per sheet unit (1 for a PDF's
/// own sheet, see [`image_sheet`]), `stamp_words` names a stamp's words, `outlines` sets text
/// Helvetica cannot write.
pub fn export(
    pdf: &[u8],
    doc: &Doc,
    units: f64,
    stamp_words: &dyn Fn(&Markup) -> String,
    outlines: Option<&OutlineText<'_>>,
) -> Result<Vec<u8>, AnnotateError> {
    let parsed =
        specular_pdf::Document::open(Arc::new(pdf.to_vec())).map_err(|_| AnnotateError::Damaged)?;
    let mut out = pdf.to_vec();
    for (page, items) in doc.pages() {
        let index = page as usize;
        if index >= parsed.page_count() {
            return Err(AnnotateError::NoPage(index));
        }
        let to_user = parsed
            .user_to_page(index)
            .and_then(|m| invert(&m))
            .map(|m| {
                [
                    m[0] * units,
                    m[1] * units,
                    m[2] * units,
                    m[3] * units,
                    m[4],
                    m[5],
                ]
            })
            .ok_or(AnnotateError::Damaged)?;
        let annots: Vec<Annot> = items
            .iter()
            .map(|m| annotation(m, &to_user, &stamp_words(m), outlines))
            .collect();
        out = add_annotations(&out, index, &annots)?;
    }
    Ok(out)
}

/// Largest page side, in points (Acrobat's limit, 200 inches).
const MAX_PAGE: f64 = 14_400.0;

/// A one-page PDF of an image (`rgb`, `w × h`, rows top down), a point per pixel or smaller
/// to keep the page under 200 inches, for marking up and exporting a sheet that is not a PDF;
/// and its points per pixel.
pub fn image_sheet(rgb: &[u8], w: u32, h: u32) -> (Vec<u8>, f64) {
    let k = (MAX_PAGE / f64::from(w.max(h).max(1))).min(1.0);
    let (pw, ph) = (f64::from(w) * k, f64::from(h) * k);
    let data = miniz_oxide::deflate::compress_to_vec_zlib(rgb, 3);
    let content = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q", num(pw), num(ph));
    let mut image = format!(
        "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB \
         /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
        data.len()
    )
    .into_bytes();
    image.extend_from_slice(&data);
    image.extend_from_slice(b"\nendstream");
    let objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>",
            num(pw),
            num(ph)
        )
        .into_bytes(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).into_bytes(),
        image,
    ];
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    (out, k)
}

/// A PDF text string: literal when plain ASCII, else UTF-16BE with a byte order mark.
pub fn text_string(s: &str) -> String {
    if s.chars().all(|c| (' '..='~').contains(&c)) {
        let mut o = String::from("(");
        for c in s.chars() {
            if matches!(c, '(' | ')' | '\\') {
                o.push('\\');
            }
            o.push(c);
        }
        o.push(')');
        o
    } else {
        let mut o = String::from("<FEFF");
        for u in s.encode_utf16() {
            let _ = write!(o, "{u:04X}");
        }
        o.push('>');
        o
    }
}

fn rgb(c: [u8; 3]) -> String {
    let [r, g, b] = c.map(|v| num(f64::from(v) / 255.0));
    format!("{r} {g} {b}")
}

/// One object as an annotation, `to_user` taking the page as displayed to user space.
pub fn annotation(
    m: &Markup,
    to_user: &Affine,
    words: &str,
    outlines: Option<&OutlineText<'_>>,
) -> Annot {
    let u = |p: P| apply(to_user, p);
    let scale = (to_user[0] * to_user[3] - to_user[1] * to_user[2])
        .abs()
        .sqrt();
    let b = m.bounds_with(words, &|s, size, bold| {
        Pen(Vec::new(), outlines).width(s, size, bold)
    });
    let corners = [
        u(P::new(b[0], b[1])),
        u(P::new(b[2], b[1])),
        u(P::new(b[0], b[3])),
        u(P::new(b[2], b[3])),
    ];
    let rect = corners.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |a, p| [a[0].min(p.x), a[1].min(p.y), a[2].max(p.x), a[3].max(p.y)],
    );
    let pts = |v: &[P]| {
        v.iter()
            .flat_map(|p| {
                let q = u(*p);
                [num(q.x), num(q.y)]
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut e = format!(
        "/C [{}] /BS << /W {} >> /T {} /M ({}) /CreationDate ({}) /NM {} /F 4 /Contents {}",
        rgb(m.colour),
        num(m.width * scale),
        text_string(&m.author),
        pdf_date(&m.date),
        pdf_date(&m.date),
        text_string(&m.id),
        text_string(match &m.shape {
            Shape::Stamp { .. } => words,
            Shape::Pin { photo, .. } => photo.to_str().unwrap_or(""),
            _ => &m.text,
        })
    );
    let _ = match &m.shape {
        Shape::Cloud(v) => write!(
            e,
            " /Subtype /Polygon /Subj (Cloud) /Vertices [{}] /BE << /S /C /I 1 >>",
            pts(v)
        ),
        Shape::Area(v) => write!(
            e,
            " /Subtype /Polygon /Subj (Area Measurement) /IT /PolygonDimension /Vertices [{}]",
            pts(v)
        ),
        Shape::Arrow { tail, head } => write!(
            e,
            " /Subtype /Line /Subj (Arrow) /L [{}] /LE [/None /OpenArrow]",
            pts(&[*tail, *head])
        ),
        Shape::Length(a, b) => write!(
            e,
            " /Subtype /Line /Subj (Length Measurement) /IT /LineDimension /L [{}] /LE [/Butt /Butt]",
            pts(&[*a, *b])
        ),
        Shape::Text(_) => write!(
            e,
            " /Subtype /FreeText /Subj (Text) /DA (/Helv {} Tf {} rg)",
            num(m.size * scale),
            rgb(m.colour)
        ),
        Shape::Stamp { kind, .. } => {
            write!(
                e,
                " /Subtype /Stamp /Subj (Stamp) /Name /{}",
                kind.pdf_name()
            )
        }
        Shape::Pin { .. } => write!(e, " /Subtype /Text /Subj (Photo) /Name /Comment"),
    };
    Annot {
        entries: e,
        rect,
        appearance: appearance(m, to_user, words, outlines),
    }
}

/// Content-stream builder in the page's displayed coordinates.
pub(crate) struct Pen<'a>(pub(crate) Vec<u8>, pub(crate) Option<&'a OutlineText<'a>>);

impl Pen<'_> {
    pub(crate) fn op(&mut self, s: &str) {
        self.0.extend_from_slice(s.as_bytes());
        self.0.push(b'\n');
    }
    pub(crate) fn poly(&mut self, pts: &[P], close: bool) {
        for (i, p) in pts.iter().enumerate() {
            self.op(&format!(
                "{} {} {}",
                num(p.x),
                num(p.y),
                if i == 0 { "m" } else { "l" }
            ));
        }
        if close {
            self.op("h");
        }
    }
    fn curves(&mut self, segs: &[[P; 4]]) {
        for (i, s) in segs.iter().enumerate() {
            if i == 0 {
                self.op(&format!("{} {} m", num(s[0].x), num(s[0].y)));
            }
            self.op(&format!(
                "{} {} {} {} {} {} c",
                num(s[1].x),
                num(s[1].y),
                num(s[2].x),
                num(s[2].y),
                num(s[3].x),
                num(s[3].y)
            ));
        }
    }
    fn circle(&mut self, c: P, r: f64) {
        let arc = cloud::Arc {
            c,
            r,
            start: 0.0,
            sweep: std::f64::consts::TAU,
        };
        self.curves(&cloud::beziers(&[arc]));
        self.op("h");
    }
    /// The outlines of `s` when Helvetica cannot write it and the caller can set it.
    fn outlined(&self, s: &str, size: f64, bold: bool) -> Option<Outlined> {
        if !needs_outlines(s) {
            return None;
        }
        self.1.and_then(|f| f(s, size, bold))
    }
    /// Width of `s` as it is drawn.
    pub(crate) fn width(&self, s: &str, size: f64, bold: bool) -> f64 {
        match self.outlined(s, size, bold) {
            Some(o) => o.width,
            None => helvetica::width(s, size, bold),
        }
    }
    /// `s` with its baseline's left end at `(x, y)`.
    pub(crate) fn text(&mut self, s: &str, x: f64, y: f64, size: f64, bold: bool) {
        if let Some(o) = self.outlined(s, size, bold) {
            let q = |p: P| format!("{} {}", num(p.x + x), num(p.y + y));
            // Quadratic pieces become cubics: the current point is the last one written.
            let mut cur = P::new(0.0, 0.0);
            let mut start = cur;
            for e in &o.path {
                match *e {
                    PathEl::Move(a) => {
                        self.op(&format!("{} m", q(a)));
                        (cur, start) = (a, a);
                    }
                    PathEl::Line(a) => {
                        self.op(&format!("{} l", q(a)));
                        cur = a;
                    }
                    PathEl::Quad(c, a) => {
                        let c1 = P::new(
                            cur.x + (c.x - cur.x) * 2.0 / 3.0,
                            cur.y + (c.y - cur.y) * 2.0 / 3.0,
                        );
                        let c2 =
                            P::new(a.x + (c.x - a.x) * 2.0 / 3.0, a.y + (c.y - a.y) * 2.0 / 3.0);
                        self.op(&format!("{} {} {} c", q(c1), q(c2), q(a)));
                        cur = a;
                    }
                    PathEl::Cubic(c1, c2, a) => {
                        self.op(&format!("{} {} {} c", q(c1), q(c2), q(a)));
                        cur = a;
                    }
                    PathEl::Close => {
                        self.op("h");
                        cur = start;
                    }
                }
            }
            if !o.path.is_empty() {
                self.op("f");
            }
            return;
        }
        self.op(&format!(
            "BT /{} {} Tf 1 0 0 -1 {} {} Tm",
            if bold { "HeBo" } else { "Helv" },
            num(size),
            num(x),
            num(y)
        ));
        self.0.extend_from_slice(&helvetica::literal(s));
        self.op(" Tj ET");
    }
    /// `s` centred on `x`, its baseline at `y`.
    fn centred(&mut self, s: &str, x: f64, y: f64, size: f64, bold: bool) {
        let w = self.width(s, size, bold);
        self.text(s, x - w / 2.0, y, size, bold);
    }
}

fn appearance(
    m: &Markup,
    to_user: &Affine,
    words: &str,
    outlines: Option<&OutlineText<'_>>,
) -> Vec<u8> {
    let mut p = Pen(Vec::new(), outlines);
    let c = rgb(m.colour);
    let w = m.width.max(0.0);
    let t = to_user;
    p.op(&format!(
        "q {} {} {} {} {} {} cm 1 J 1 j {} w {c} RG {c} rg",
        num(t[0]),
        num(t[1]),
        num(t[2]),
        num(t[3]),
        num(t[4]),
        num(t[5]),
        num(w)
    ));
    let s = m.size;
    match &m.shape {
        Shape::Cloud(v) => {
            let arcs = cloud::arcs(v, cloud::arc_length(m.width, m.size));
            p.curves(&cloud::beziers(&arcs));
            p.op("S");
        }
        Shape::Area(v) => {
            p.poly(v, true);
            p.op("S");
            let a = label_anchor(&m.shape);
            p.centred(&m.text, a.x, a.y + s * 0.35, s, false);
        }
        Shape::Length(a, b) => {
            p.poly(&[*a, *b], false);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = dx.hypot(dy).max(1e-9);
            let (nx, ny) = (-dy / len * s * 0.4, dx / len * s * 0.4);
            for e in [a, b] {
                p.poly(
                    &[P::new(e.x - nx, e.y - ny), P::new(e.x + nx, e.y + ny)],
                    false,
                );
            }
            p.op("S");
            let mid = label_anchor(&m.shape);
            p.centred(&m.text, mid.x, mid.y - s * 0.45, s, false);
        }
        Shape::Arrow { tail, head } => {
            let tri = arrow_head(*tail, *head, m.width, m.size);
            let base = P::new((tri[1].x + tri[2].x) / 2.0, (tri[1].y + tri[2].y) / 2.0);
            p.poly(&[*tail, base], false);
            p.op("S");
            p.poly(&tri, true);
            p.op("f");
            if !m.text.is_empty() {
                let lw = p.width(&m.text, s, false);
                let x = if head.x >= tail.x {
                    tail.x - lw - s * 0.3
                } else {
                    tail.x + s * 0.3
                };
                p.text(&m.text, x, tail.y + s * 0.35, s, false);
            }
        }
        Shape::Text(at) => {
            for (i, line) in m.text.split('\n').enumerate() {
                p.text(line, at.x, at.y + s * (1.2 * i as f64 + 0.95), s, false);
            }
        }
        Shape::Stamp { at, .. } => {
            let line = m.stamp_line();
            let (fw, fh) = stamp_extent_with(words, &line, s, &|t, z, b| p.width(t, z, b));
            let (x0, y0) = (at.x - fw / 2.0, at.y - fh / 2.0);
            p.op(&format!(
                "{} w {} {} {} {} re S",
                num(s * 0.1),
                num(x0),
                num(y0),
                num(fw),
                num(fh)
            ));
            let pad = s * STAMP_PAD;
            p.centred(words, at.x, y0 + pad + s * 0.82, s, true);
            if !line.is_empty() {
                p.centred(
                    &line,
                    at.x,
                    y0 + pad + s + s * STAMP_SUB * 1.1,
                    s * STAMP_SUB,
                    false,
                );
            }
        }
        Shape::Pin { at, .. } => {
            let (c, r) = pin_marker(*at, s);
            p.poly(
                &[
                    P::new(c.x - r * 0.55, c.y + r * 0.8),
                    *at,
                    P::new(c.x + r * 0.55, c.y + r * 0.8),
                ],
                true,
            );
            p.op("f");
            p.circle(c, r);
            p.op("f 1 g");
            p.circle(c, r * 0.4);
            p.op("f");
        }
    }
    p.op("Q");
    p.0
}
