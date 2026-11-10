//! Tiny hand-built PDFs for tests (here and in `specular-decode`): uncompressed, with a correct
//! cross-reference table.

/// One page: its media box size in points and its content stream.
pub struct Page<'a> {
    pub width: f32,
    pub height: f32,
    pub content: &'a str,
}

/// Content stream filling the whole `w × h` page with an RGB colour.
pub fn fill(w: f32, h: f32, rgb: [f32; 3]) -> String {
    format!("{} {} {} rg 0 0 {w} {h} re f", rgb[0], rgb[1], rgb[2])
}

/// Builds a PDF with `pages`, an optional `/Info` dictionary body (e.g. `/Title (x)`) and an
/// optional extra trailer entry (e.g. an `/Encrypt` reference and `/ID`). `extra_objects` are
/// appended as numbered objects after the pages and may be referenced from `trailer_extra` or
/// `catalog_extra` (extra catalog entries).
pub fn build(
    pages: &[Page<'_>],
    catalog_extra: &str,
    info: Option<&str>,
    trailer_extra: &str,
    extra_objects: &[&str],
) -> Vec<u8> {
    // Objects: 1 catalog, 2 page tree, then (page, content) pairs, then info, then extras.
    let n = pages.len();
    let mut objects: Vec<Vec<u8>> = Vec::new();
    objects.push(format!("<< /Type /Catalog /Pages 2 0 R {catalog_extra} >>").into_bytes());
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    objects.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).into_bytes());
    for (i, p) in pages.iter().enumerate() {
        let content_id = 4 + 2 * i;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents {content_id} 0 R \
                 /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 \
                 /BaseFont /Helvetica >> /F2 << /Type /Font /Subtype /Type1 \
                 /BaseFont /Times-Roman >> >> >> >>",
                p.width, p.height
            )
            .into_bytes(),
        );
        let mut s = format!("<< /Length {} >>\nstream\n", p.content.len()).into_bytes();
        s.extend_from_slice(p.content.as_bytes());
        s.extend_from_slice(b"\nendstream");
        objects.push(s);
    }
    let info_id = info.map(|body| {
        objects.push(format!("<< {body} >>").into_bytes());
        objects.len()
    });
    for o in extra_objects {
        objects.push(o.as_bytes().to_vec());
    }

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
    let info_entry = info_id
        .map(|id| format!(" /Info {id} 0 R"))
        .unwrap_or_default();
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R{info_entry} {trailer_extra} >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// A PDF of the numbered objects `objects` (object 1 first, the catalog) with a
/// cross-reference table.
pub fn raw(objects: &[String]) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
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
    out
}

/// A stream object with `body`.
pub fn stream(dict: &str, body: &str) -> String {
    format!(
        "<< {dict} /Length {} >>\nstream\n{body}\nendstream",
        body.len()
    )
}

/// Metres per point of a sheet drawn at 1:50.
pub const M_PER_PT_1_50: f64 = 0.0254 / 72.0 * 50.0;

/// An 800 × 600 pt drawing sheet with three viewports (`/VP`):
///
/// 1. `[50 50 450 550]` at 1:50 in metres (`/R (1:50)`; X, D and A arrays), with a line from
///    (100, 100) to (383.4646, 100): 100 mm on the paper, 5 m drawn;
/// 2. `[500 300 750 550]` (written reversed) at `1 in = 4 ft` in feet and inches (eighths),
///    with a line from (520, 320) to (610, 320): 1.25 in on the paper, 5 ft drawn;
/// 3. a GEO viewport (not a drawing scale).
pub fn drawing_scales() -> Vec<u8> {
    let c = M_PER_PT_1_50;
    let content = "1 w 0 0 0 RG 100 100 m 383.4646 100 l S 520 320 m 610 320 l S \
                   50 50 400 500 re S 500 300 250 250 re S";
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 800 600] /Contents 4 0 R \
         /VP [5 0 R 6 0 R 7 0 R] >>"
            .to_string(),
        stream("", content),
        format!(
            "<< /Type /Viewport /BBox [50 50 450 550] /Name (Plan) /Measure << /Type /Measure \
             /Subtype /RL /R (1:50) /X [<< /Type /NumberFormat /U (m) /C {c} /D 100 >>] \
             /D [<< /Type /NumberFormat /U (m) /C {c} /D 100 >>] \
             /A [<< /Type /NumberFormat /U (sq m) /C {} /D 100 >>] >> >>",
            c * c
        ),
        format!(
            "<< /Type /Viewport /BBox [750 550 500 300] /Name (Detail) /Measure << \
             /Type /Measure /R (1 in = 4 ft) /X [<< /U (ft) /C {} >> \
             << /U (in) /C 12 /F /F /D 8 >>] >> >>",
            4.0 / 72.0
        ),
        "<< /Type /Viewport /BBox [0 0 40 40] /Measure << /Type /Measure /Subtype /GEO \
         /GPTS [0 0 0 1 1 1 1 0] /LPTS [0 0 0 1 1 1 1 0] >> >>"
            .to_string(),
    ];
    raw(&objects)
}

/// A one-page PDF encrypted with the standard handler (RC4, revision 3) whose user password is
/// not empty, so it cannot be opened without one.
pub fn encrypted() -> Vec<u8> {
    let page = Page {
        width: 100.0,
        height: 100.0,
        content: "0 0 1 rg 0 0 100 100 re f",
    };
    // Object 5 (after catalog, tree, page, content) is the encryption dictionary. /O and /U are
    // arbitrary: no empty password matches them.
    let hex32 = "3a1f5c7e9b2d4f6081a3c5e7092b4d6f8091a2b3c4d5e6f708192a3b4c5d6e7f";
    let encrypt =
        format!("<< /Filter /Standard /V 2 /R 3 /Length 128 /P -3904 /O <{hex32}> /U <{hex32}> >>");
    build(
        &[page],
        "",
        None,
        "/Encrypt 5 0 R /ID [<00112233445566778899aabbccddeeff> <00112233445566778899aabbccddeeff>]",
        &[&encrypt],
    )
}
