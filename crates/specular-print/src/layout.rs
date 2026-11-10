//! Paper and page layout: pure maths, no drawing.
//!
//! Every length is in millimetres, measured from the top-left corner of the sheet. A
//! [`PageSetup`] names the sheet (paper, orientation, margins) and the raster resolution; a
//! [`Kind`] says what goes on it (one image per page, or a contact sheet of a grid of images
//! per page); [`layout`] places the [`Item`]s and returns one [`Page`] per sheet.

/// Millimetres per inch.
pub const MM_PER_IN: f64 = 25.4;
/// PDF points per inch.
pub const PT_PER_IN: f64 = 72.0;

/// Caption text size (points) and the header's.
pub const CAPTION_PT: f64 = 8.0;
pub const HEADER_PT: f64 = 10.0;
/// Line height of captions and the header, as a multiple of the text size.
pub const LINE: f64 = 1.35;
/// Gap between an image and its caption, and between contact sheet cells (mm).
pub const CAPTION_GAP_MM: f64 = 1.2;
pub const CELL_GAP_MM: f64 = 4.0;
/// Gap under the header (mm).
pub const HEADER_GAP_MM: f64 = 3.0;
/// The strip under a page printed to scale: the scale bar and the note (mm).
pub const SCALE_STRIP_MM: f64 = 9.0;
/// Height of the scale bar (mm), its text size (points) and its longest length (mm).
pub const SCALE_BAR_MM: f64 = 2.0;
pub const SCALE_PT: f64 = 7.0;
pub const SCALE_BAR_MAX_MM: f64 = 100.0;

/// Points to millimetres.
pub fn pt_to_mm(pt: f64) -> f64 {
    pt / PT_PER_IN * MM_PER_IN
}

/// Millimetres to PDF points.
pub fn mm_to_pt(mm: f64) -> f64 {
    mm / MM_PER_IN * PT_PER_IN
}

/// A paper size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paper {
    A4,
    Letter,
    A3,
    /// Width and height in mm (portrait: the shorter side first).
    Custom {
        w: f64,
        h: f64,
    },
}

impl Paper {
    /// The sizes the dialog offers, in its order.
    pub const STANDARD: [Paper; 3] = [Paper::A4, Paper::Letter, Paper::A3];

    /// Portrait size in mm.
    pub fn size_mm(self) -> (f64, f64) {
        match self {
            Paper::A4 => (210.0, 297.0),
            Paper::Letter => (215.9, 279.4),
            Paper::A3 => (297.0, 420.0),
            Paper::Custom { w, h } => {
                let (w, h) = (w.clamp(20.0, 2000.0), h.clamp(20.0, 2000.0));
                (w.min(h), w.max(h))
            }
        }
    }

    /// The name of a standard size ("A4"); custom sizes have none.
    pub fn name(self) -> Option<&'static str> {
        match self {
            Paper::A4 => Some("A4"),
            Paper::Letter => Some("Letter"),
            Paper::A3 => Some("A3"),
            Paper::Custom { .. } => None,
        }
    }

    /// The PWG media name the print portal and CUPS understand (`iso_a4_210x297mm`).
    pub fn pwg_name(self) -> String {
        match self {
            Paper::A4 => "iso_a4_210x297mm".into(),
            Paper::Letter => "na_letter_8.5x11in".into(),
            Paper::A3 => "iso_a3_297x420mm".into(),
            Paper::Custom { .. } => {
                let (w, h) = self.size_mm();
                format!("custom_{w:.0}x{h:.0}mm_{w:.0}x{h:.0}mm")
            }
        }
    }

    /// The usual paper where `locale` (a POSIX locale, `en_US.UTF-8`) is: Letter in the
    /// United States, Canada, Mexico and the Philippines, A4 elsewhere.
    pub fn for_locale(locale: &str) -> Paper {
        let region = locale
            .split(['.', '@'])
            .next()
            .and_then(|l| l.split(['_', '-']).nth(1))
            .unwrap_or("");
        match region {
            "US" | "CA" | "MX" | "PH" | "CL" | "CO" | "VE" => Paper::Letter,
            _ => Paper::A4,
        }
    }
}

/// Sheet orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

/// Units the dialog shows lengths in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Mm,
    Cm,
    In,
}

impl Unit {
    /// mm per unit.
    pub fn mm(self) -> f64 {
        match self {
            Unit::Mm => 1.0,
            Unit::Cm => 10.0,
            Unit::In => MM_PER_IN,
        }
    }

    /// The step the dialog's length steppers take (mm): 0.5 cm or 1/4 in.
    pub fn step_mm(self) -> f64 {
        match self {
            Unit::Mm => 1.0,
            Unit::Cm => 5.0,
            Unit::In => MM_PER_IN / 4.0,
        }
    }

    /// `mm` in this unit, rounded to the unit's precision ("2.5", "1.25").
    pub fn format(self, mm: f64) -> String {
        let v = mm / self.mm();
        let s = match self {
            Unit::Mm | Unit::Cm => format!("{v:.1}"),
            Unit::In => format!("{v:.2}"),
        };
        // 2.50 → 2.5, 3.0 → 3.
        let s = if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        };
        if s == "-0" { "0".into() } else { s }
    }

    /// The unit's symbol, the same in every language.
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::In => "in",
        }
    }

    /// The usual unit for a paper: inches for Letter, else centimetres (the dialog's
    /// one-per-page and contact sheet layouts).
    pub fn for_paper(p: Paper) -> Unit {
        if p == Paper::Letter {
            Unit::In
        } else {
            Unit::Cm
        }
    }

    /// `mm` snapped to the unit's stepper grid.
    pub fn snap(self, mm: f64) -> f64 {
        let s = self.step_mm();
        (mm / s).round() * s
    }

    /// The unit lengths are typed in where `locale` (a POSIX locale, `en_US.UTF-8`) is:
    /// inches in the United States, Liberia and Myanmar, millimetres elsewhere.
    pub fn for_locale(locale: &str) -> Unit {
        let region = locale
            .split(['.', '@'])
            .next()
            .and_then(|l| l.split(['_', '-']).nth(1))
            .unwrap_or("");
        match region {
            "US" | "LR" | "MM" => Unit::In,
            _ => Unit::Mm,
        }
    }

    /// The next unit offered (mm, cm, in).
    pub fn next(self, d: i32) -> Unit {
        let all = [Unit::Mm, Unit::Cm, Unit::In];
        let i = all.iter().position(|u| *u == self).unwrap_or(0) as i32;
        all[(i + d).rem_euclid(3) as usize]
    }

    /// A length typed in this unit, in mm: `12.5`, `12,5` (a decimal comma), with an
    /// optional unit of its own (`3 in`, `3"`, `12 mm`, `1.5cm`) that wins over this one.
    /// `None` for anything else, or a negative or absurd length.
    pub fn parse(self, text: &str) -> Option<f64> {
        let t = text.trim().to_ascii_lowercase();
        let (num, unit) = match t.find(|c: char| c.is_ascii_alphabetic() || c == '"') {
            Some(i) => (t[..i].trim(), t[i..].trim()),
            None => (t.as_str(), ""),
        };
        let mm = match unit {
            "" => self.mm(),
            "mm" => 1.0,
            "cm" => 10.0,
            "in" | "\"" | "inch" | "inches" => MM_PER_IN,
            _ => return None,
        };
        // One decimal separator; a comma is one when there is no point.
        let num = if num.contains('.') {
            num.to_owned()
        } else {
            num.replacen(',', ".", 1)
        };
        if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return None;
        }
        let v: f64 = num.parse().ok()?;
        let v = v * mm;
        (v.is_finite() && (0.0..=2000.0).contains(&v)).then_some(v)
    }
}

/// The sheet and its raster resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSetup {
    pub paper: Paper,
    pub orientation: PageOrientation,
    /// Margin on every side (mm).
    pub margin: f64,
    /// Raster resolution of the printed page (dots per inch).
    pub dpi: u32,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            paper: Paper::A4,
            orientation: PageOrientation::Portrait,
            margin: 10.0,
            dpi: 300,
        }
    }
}

impl PageSetup {
    /// The sheet in mm, oriented.
    pub fn size_mm(&self) -> (f64, f64) {
        let (w, h) = self.paper.size_mm();
        match self.orientation {
            PageOrientation::Portrait => (w, h),
            PageOrientation::Landscape => (h, w),
        }
    }

    /// The sheet in PDF points.
    pub fn size_pt(&self) -> (f64, f64) {
        let (w, h) = self.size_mm();
        (mm_to_pt(w), mm_to_pt(h))
    }

    /// The sheet in pixels at `dpi`.
    pub fn size_px_at(&self, dpi: f64) -> (u32, u32) {
        let (w, h) = self.size_mm();
        let px = |mm: f64| ((mm / MM_PER_IN * dpi).round() as u32).max(1);
        (px(w), px(h))
    }

    /// The sheet in pixels at the setup's resolution.
    pub fn size_px(&self) -> (u32, u32) {
        self.size_px_at(f64::from(self.dpi.max(1)))
    }

    /// The margin, kept to at most a third of the shorter side.
    pub fn effective_margin(&self) -> f64 {
        let (w, h) = self.size_mm();
        self.margin.clamp(0.0, w.min(h) / 3.0)
    }

    /// The area inside the margins.
    pub fn printable(&self) -> RectMm {
        let (w, h) = self.size_mm();
        let m = self.effective_margin();
        RectMm::new(m, m, w - 2.0 * m, h - 2.0 * m)
    }
}

/// A rectangle in mm.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RectMm {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl RectMm {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> RectMm {
        RectMm { x, y, w, h }
    }
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    /// The overlap of two rectangles (zero-sized when they do not meet).
    pub fn intersect(&self, o: &RectMm) -> RectMm {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        RectMm::new(x, y, (r - x).max(0.0), (b - y).max(0.0))
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
    /// A `w`×`h` rectangle centred in this one.
    pub fn centred(&self, w: f64, h: f64) -> RectMm {
        RectMm::new(
            self.x + (self.w - w) * 0.5,
            self.y + (self.h - h) * 0.5,
            w,
            h,
        )
    }
    /// Whether `o` lies inside (with a little tolerance for rounding).
    pub fn contains(&self, o: &RectMm) -> bool {
        let e = 1e-6;
        o.x >= self.x - e
            && o.y >= self.y - e
            && o.right() <= self.right() + e
            && o.bottom() <= self.bottom() + e
    }
}

/// How one image fills its page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    /// As large as fits inside the margins, whole.
    Fit,
    /// Covering the area inside the margins, cropped.
    Fill,
    /// A given printed width (mm), the height following the image; centred and cropped to
    /// the margins when larger.
    Size { width: f64 },
    /// To scale: drawings with an [`ItemScale`] at real size over `ratio` (1:`ratio`), or as
    /// drawn when `None`, tiled over several pages when larger than the paper, with a scale
    /// bar and a note. Items without a scale are fitted.
    Scale { ratio: Option<f64> },
}

/// How a drawing's pixels measure, for printing it to scale.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ItemScale {
    /// Millimetres of paper per displayed pixel when printed as drawn.
    pub paper_mm_per_px: f64,
    /// The ratio it is drawn at (real size over paper size; 1 for a sheet without a scale).
    pub ratio: f64,
    /// The real-world unit of the scale bar and its length in millimetres.
    pub unit: String,
    pub unit_mm: f64,
}

/// The scale bar and note of a page printed to scale.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleMark {
    /// The bar, divided into `segments` alternating parts; its length is `length` units.
    pub bar: RectMm,
    pub segments: u32,
    /// The bar's length as text, e.g. `5 m`.
    pub length: String,
    /// The ratio printed, e.g. `1:50`.
    pub ratio: String,
    /// This page's tile (row, column) of `tiles` (rows, columns), 0-based.
    pub tile: (u32, u32),
    pub tiles: (u32, u32),
    /// The note beside the bar (the caller may rewrite it in the reader's language).
    pub note: String,
    pub note_rect: RectMm,
}

/// What a caption says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caption {
    Off,
    Name,
    NameDate,
}

impl Caption {
    /// Lines of text a caption takes.
    pub fn lines(self) -> usize {
        match self {
            Caption::Off => 0,
            Caption::Name => 1,
            Caption::NameDate => 2,
        }
    }
}

/// What the document holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// One image per page.
    Single { fit: Fit, caption: Caption },
    /// `cols`×`rows` images per page, each fitted in its cell, with an optional header
    /// (folder name, page x of y).
    Sheet {
        cols: u32,
        rows: u32,
        caption: Caption,
        header: bool,
    },
}

impl Kind {
    pub fn caption(&self) -> Caption {
        match *self {
            Kind::Single { caption, .. } | Kind::Sheet { caption, .. } => caption,
        }
    }

    /// Images on one page.
    pub fn per_page(&self) -> usize {
        match *self {
            Kind::Single { .. } => 1,
            Kind::Sheet { cols, rows, .. } => (cols.max(1) * rows.max(1)) as usize,
        }
    }
}

/// One image to place.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Item {
    /// Displayed size (orientation applied), pixels.
    pub size: (u32, u32),
    /// The file name, for the caption.
    pub name: String,
    /// The date, already formatted, for the caption.
    pub date: Option<String>,
    /// How it measures, for printing to scale (drawings).
    pub scale: Option<ItemScale>,
}

/// An image placed on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// Index into the items.
    pub item: usize,
    /// Where the image is drawn (may reach past `clip` when cropped).
    pub image: RectMm,
    /// What of it is visible.
    pub clip: RectMm,
    /// Caption lines and the rectangle they are centred in.
    pub caption: Vec<String>,
    pub caption_rect: RectMm,
}

/// The header of a contact sheet page.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// At the start: the folder (or selection) name.
    pub title: String,
    /// At the end: "Page 1 of 3", already localised.
    pub page: String,
    pub rect: RectMm,
}

/// One sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub images: Vec<Placed>,
    pub header: Option<Header>,
    /// The scale bar and note of a page printed to scale.
    pub scale: Option<ScaleMark>,
}

/// A whole document: the setup and its pages.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub setup: PageSetup,
    pub pages: Vec<Page>,
    /// Printed from a right-to-left UI: the header's title at the right and its page
    /// number at the left, the scale note at the right, names read as right-to-left text.
    /// Set by [`Document::right_to_left`].
    pub rtl: bool,
}

impl Document {
    /// The document for a right-to-left UI ([`Document::rtl`]): contact sheets read from
    /// the right (the first image at the top right); single pages are centred and stay as
    /// they are, and pages printed to scale keep the drawing's own left and right.
    pub fn right_to_left(mut self) -> Document {
        if self.rtl {
            return self;
        }
        self.rtl = true;
        let (pw, _) = self.setup.size_mm();
        let flip = |r: &mut RectMm| r.x = pw - r.x - r.w;
        for p in self.pages.iter_mut().filter(|p| p.scale.is_none()) {
            for im in &mut p.images {
                flip(&mut im.image);
                flip(&mut im.clip);
                flip(&mut im.caption_rect);
            }
        }
        self
    }
}

/// Height of `lines` lines of `pt`-point text (mm).
pub fn text_height(lines: usize, pt: f64) -> f64 {
    lines as f64 * pt_to_mm(pt) * LINE
}

/// The caption lines of `item`.
fn caption_lines(item: &Item, caption: Caption) -> Vec<String> {
    match caption {
        Caption::Off => Vec::new(),
        Caption::Name => vec![item.name.clone()],
        Caption::NameDate => {
            let mut v = vec![item.name.clone()];
            v.push(item.date.clone().unwrap_or_default());
            v
        }
    }
}

/// The image's rectangle when fitted (`cover == false`) or filled into `area`.
fn place(area: RectMm, size: (u32, u32), cover: bool) -> RectMm {
    let (iw, ih) = (f64::from(size.0.max(1)), f64::from(size.1.max(1)));
    let s = if cover {
        (area.w / iw).max(area.h / ih)
    } else {
        (area.w / iw).min(area.h / ih)
    };
    area.centred(iw * s, ih * s)
}

/// The pages of `item` printed at 1:`ratio` (as drawn when `None`): one, centred, when it
/// fits; else tiles from the top left, row by row.
fn to_scale(
    area: RectMm,
    index: usize,
    item: &Item,
    sc: &ItemScale,
    ratio: Option<f64>,
    cap_h: f64,
    caption: Caption,
) -> Vec<Page> {
    let drawn = if sc.ratio.is_finite() && sc.ratio > 0.0 {
        sc.ratio
    } else {
        1.0
    };
    let n = ratio.filter(|r| r.is_finite() && *r > 0.0).unwrap_or(drawn);
    let k = sc.paper_mm_per_px * drawn / n;
    let (w, h) = (
        f64::from(item.size.0.max(1)) * k,
        f64::from(item.size.1.max(1)) * k,
    );
    let img_area = RectMm::new(
        area.x,
        area.y,
        area.w,
        (area.h - cap_h - SCALE_STRIP_MM).max(1.0),
    );
    // A thousandth of a millimetre over is still one page.
    let tiles_along = |len: f64, room: f64| ((len - 1e-3) / room).ceil().max(1.0) as u32;
    let (cols, rows) = (tiles_along(w, img_area.w), tiles_along(h, img_area.h));
    // The bar: a round length in the drawing's unit, at most a third of the width.
    let unit_mm = if sc.unit_mm > 0.0 { sc.unit_mm } else { 1.0 };
    let max_mm = (area.w / 3.0).min(SCALE_BAR_MAX_MM);
    let length = specular_measure::scale::round_length(max_mm * n / unit_mm);
    let bar_mm = length * unit_mm / n;
    let strip_y = img_area.bottom() + cap_h;
    let bar = RectMm::new(area.x, strip_y + 1.5, bar_mm, SCALE_BAR_MM);
    let ratio_text = specular_measure::scale::ratio_text(n);
    let length_text = format!(
        "{} {}",
        specular_measure::calib::format_sig(length, 3)
            .trim_end_matches('0')
            .trim_end_matches('.'),
        sc.unit
    );
    let mut pages = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let image = if cols == 1 && rows == 1 {
                img_area.centred(w, h)
            } else {
                RectMm::new(
                    img_area.x - f64::from(c) * img_area.w,
                    img_area.y - f64::from(r) * img_area.h,
                    w,
                    h,
                )
            };
            let clip = image.intersect(&img_area);
            let caption_rect = RectMm::new(
                area.x,
                img_area.bottom() + CAPTION_GAP_MM,
                area.w,
                (cap_h - CAPTION_GAP_MM).max(0.0),
            );
            let note_x = bar.right() + 4.0;
            pages.push(Page {
                images: vec![Placed {
                    item: index,
                    image,
                    clip,
                    caption: caption_lines(item, caption),
                    caption_rect,
                }],
                header: None,
                scale: Some(ScaleMark {
                    bar,
                    segments: 4,
                    length: length_text.clone(),
                    ratio: ratio_text.clone(),
                    tile: (r, c),
                    tiles: (rows, cols),
                    note: ratio_text.clone(),
                    note_rect: RectMm::new(
                        note_x,
                        strip_y + 0.5,
                        (area.right() - note_x).max(1.0),
                        SCALE_STRIP_MM - 1.0,
                    ),
                }),
            });
        }
    }
    pages
}

/// Places `items` on pages. `title` heads contact sheets; `page_label(n, of)` (1-based)
/// names their pages.
pub fn layout(
    setup: PageSetup,
    kind: Kind,
    items: &[Item],
    title: &str,
    page_label: &dyn Fn(usize, usize) -> String,
) -> Document {
    let area = setup.printable();
    let caption = kind.caption();
    let cap_h = if caption == Caption::Off {
        0.0
    } else {
        text_height(caption.lines(), CAPTION_PT) + CAPTION_GAP_MM
    };
    let mut pages = Vec::new();
    match kind {
        Kind::Single { fit, .. } => {
            for (i, item) in items.iter().enumerate() {
                if let (Fit::Scale { ratio }, Some(sc)) = (fit, item.scale.as_ref()) {
                    pages.extend(to_scale(area, i, item, sc, ratio, cap_h, caption));
                    continue;
                }
                let img_area = RectMm::new(area.x, area.y, area.w, (area.h - cap_h).max(1.0));
                let image = match fit {
                    Fit::Fit | Fit::Scale { .. } => place(img_area, item.size, false),
                    Fit::Fill => place(img_area, item.size, true),
                    Fit::Size { width } => {
                        let w = width.max(1.0);
                        let h = w * f64::from(item.size.1.max(1)) / f64::from(item.size.0.max(1));
                        img_area.centred(w, h)
                    }
                };
                let clip = image.intersect(&img_area);
                let caption_rect = RectMm::new(
                    area.x,
                    clip.bottom() + CAPTION_GAP_MM,
                    area.w,
                    (cap_h - CAPTION_GAP_MM).max(0.0),
                );
                pages.push(Page {
                    images: vec![Placed {
                        item: i,
                        image,
                        clip,
                        caption: caption_lines(item, caption),
                        caption_rect,
                    }],
                    header: None,
                    scale: None,
                });
            }
        }
        Kind::Sheet {
            cols, rows, header, ..
        } => {
            let (cols, rows) = (cols.clamp(1, 20), rows.clamp(1, 20));
            let per = (cols * rows) as usize;
            let n_pages = items.len().div_ceil(per).max(1);
            let head_h = if header {
                text_height(1, HEADER_PT)
            } else {
                0.0
            };
            let grid = if header {
                RectMm::new(
                    area.x,
                    area.y + head_h + HEADER_GAP_MM,
                    area.w,
                    (area.h - head_h - HEADER_GAP_MM).max(1.0),
                )
            } else {
                area
            };
            let gap = CELL_GAP_MM.min(grid.w / f64::from(cols) * 0.2);
            let cw = ((grid.w - gap * f64::from(cols - 1)) / f64::from(cols)).max(0.5);
            let ch = ((grid.h - gap * f64::from(rows - 1)) / f64::from(rows)).max(0.5);
            for p in 0..n_pages {
                let mut images = Vec::new();
                for k in 0..per {
                    let i = p * per + k;
                    let Some(item) = items.get(i) else { break };
                    let (c, r) = ((k as u32) % cols, (k as u32) / cols);
                    let cell = RectMm::new(
                        grid.x + f64::from(c) * (cw + gap),
                        grid.y + f64::from(r) * (ch + gap),
                        cw,
                        ch,
                    );
                    let img_area = RectMm::new(cell.x, cell.y, cell.w, (cell.h - cap_h).max(0.5));
                    let image = place(img_area, item.size, false);
                    // Captions share a line across the row, under the cells' image areas.
                    let caption_rect = RectMm::new(
                        cell.x,
                        img_area.bottom() + CAPTION_GAP_MM,
                        cell.w,
                        (cap_h - CAPTION_GAP_MM).max(0.0),
                    );
                    images.push(Placed {
                        item: i,
                        image,
                        clip: image,
                        caption: caption_lines(item, caption),
                        caption_rect,
                    });
                }
                pages.push(Page {
                    images,
                    scale: None,
                    header: header.then(|| Header {
                        title: title.to_owned(),
                        page: page_label(p + 1, n_pages),
                        rect: RectMm::new(area.x, area.y, area.w, head_h),
                    }),
                });
            }
        }
    }
    Document {
        setup,
        pages,
        rtl: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(w: u32, h: u32) -> Item {
        Item {
            size: (w, h),
            name: format!("{w}x{h}.jpg"),
            date: Some("2026-10-10".into()),
            scale: None,
        }
    }

    fn label(n: usize, of: usize) -> String {
        format!("{n}/{of}")
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn paper_sizes_and_orientation() {
        let mut s = PageSetup::default();
        assert_eq!(s.size_mm(), (210.0, 297.0));
        assert_eq!(s.size_px(), (2480, 3508));
        s.orientation = PageOrientation::Landscape;
        assert_eq!(s.size_mm(), (297.0, 210.0));
        s.paper = Paper::A3;
        assert_eq!(s.size_px(), (4961, 3508));
        s.paper = Paper::Letter;
        s.orientation = PageOrientation::Portrait;
        let (w, h) = s.size_pt();
        assert!((w - 612.0).abs() < 0.01 && (h - 792.0).abs() < 0.01);
        // Custom sizes are portrait whichever way round they are given, within limits.
        assert_eq!(
            Paper::Custom { w: 150.0, h: 100.0 }.size_mm(),
            (100.0, 150.0)
        );
        assert_eq!(
            Paper::Custom { w: 1.0, h: 9000.0 }.size_mm(),
            (20.0, 2000.0)
        );
        assert_eq!(Paper::A4.pwg_name(), "iso_a4_210x297mm");
    }

    #[test]
    fn locale_picks_the_paper() {
        assert_eq!(Paper::for_locale("en_US.UTF-8"), Paper::Letter);
        assert_eq!(Paper::for_locale("fr_CA"), Paper::Letter);
        assert_eq!(Paper::for_locale("de_DE.UTF-8@euro"), Paper::A4);
        assert_eq!(Paper::for_locale("en-GB"), Paper::A4);
        assert_eq!(Paper::for_locale("C"), Paper::A4);
    }

    #[test]
    fn units_format_and_snap() {
        assert_eq!(Unit::Cm.format(25.0), "2.5");
        assert_eq!(Unit::Cm.format(100.0), "10");
        assert_eq!(Unit::In.format(25.4 * 1.25), "1.25");
        assert_eq!(Unit::In.format(25.4 * 8.5), "8.5");
        assert!(close(Unit::Cm.snap(12.4), 10.0));
        assert!(close(Unit::In.snap(25.4 * 1.3), 25.4 * 1.25));
        assert_eq!(Unit::for_paper(Paper::Letter), Unit::In);
    }

    #[test]
    fn margins_stay_sane() {
        let s = PageSetup {
            margin: 500.0,
            ..PageSetup::default()
        };
        assert!(close(s.effective_margin(), 70.0));
        let p = s.printable();
        assert!(close(p.w, 70.0) && close(p.h, 157.0));
    }

    #[test]
    fn single_fit_is_centred_and_inside() {
        let setup = PageSetup::default();
        let kind = Kind::Single {
            fit: Fit::Fit,
            caption: Caption::Off,
        };
        let doc = layout(setup, kind, &[item(6000, 4000)], "", &label);
        assert_eq!(doc.pages.len(), 1);
        let p = &doc.pages[0].images[0];
        let area = setup.printable();
        assert!(area.contains(&p.image));
        // A landscape image on a portrait page: full width, centred vertically.
        assert!(close(p.image.w, 190.0));
        assert!(close(p.image.h, 190.0 / 1.5));
        assert!(close(p.image.y + p.image.h * 0.5, 297.0 * 0.5));
        assert!(p.image.contains(&p.clip) && p.clip.contains(&p.image));
        assert!(doc.pages[0].header.is_none());
    }

    #[test]
    fn single_fill_covers_and_crops() {
        let setup = PageSetup::default();
        let kind = Kind::Single {
            fit: Fit::Fill,
            caption: Caption::Off,
        };
        let doc = layout(setup, kind, &[item(6000, 4000)], "", &label);
        let p = &doc.pages[0].images[0];
        let area = setup.printable();
        assert!(close(p.image.h, area.h));
        assert!(p.image.w > area.w);
        assert_eq!(p.clip, area);
    }

    #[test]
    fn single_actual_size_keeps_the_width() {
        let setup = PageSetup::default();
        let kind = Kind::Single {
            fit: Fit::Size { width: 100.0 },
            caption: Caption::Name,
        };
        let doc = layout(
            setup,
            kind,
            &[item(4000, 3000), item(3000, 4000)],
            "",
            &label,
        );
        assert_eq!(doc.pages.len(), 2);
        let p = &doc.pages[0].images[0];
        assert!(close(p.image.w, 100.0) && close(p.image.h, 75.0));
        assert_eq!(p.clip, p.image);
        assert_eq!(p.caption, vec!["4000x3000.jpg".to_owned()]);
        assert!(p.caption_rect.y >= p.clip.bottom());
        // Too large for the page: cropped to the area above the caption.
        let kind = Kind::Single {
            fit: Fit::Size { width: 400.0 },
            caption: Caption::Off,
        };
        let doc = layout(setup, kind, &[item(1000, 1000)], "", &label);
        let p = &doc.pages[0].images[0];
        assert!(close(p.image.w, 400.0));
        assert_eq!(p.clip, setup.printable());
    }

    #[test]
    fn captions_take_room_from_the_image() {
        let setup = PageSetup::default();
        let off = layout(
            setup,
            Kind::Single {
                fit: Fit::Fit,
                caption: Caption::Off,
            },
            &[item(1000, 2000)],
            "",
            &label,
        );
        let two = layout(
            setup,
            Kind::Single {
                fit: Fit::Fit,
                caption: Caption::NameDate,
            },
            &[item(1000, 2000)],
            "",
            &label,
        );
        let (a, b) = (&off.pages[0].images[0], &two.pages[0].images[0]);
        assert!(b.image.h < a.image.h);
        assert_eq!(b.caption.len(), 2);
        assert!(b.caption_rect.bottom() <= setup.printable().bottom() + 1e-6);
    }

    #[test]
    fn contact_sheet_grid_pages_and_header() {
        let setup = PageSetup::default();
        let kind = Kind::Sheet {
            cols: 4,
            rows: 6,
            caption: Caption::Name,
            header: true,
        };
        let items: Vec<Item> = (0..50)
            .map(|i| {
                if i % 2 == 0 {
                    item(600, 400)
                } else {
                    item(400, 600)
                }
            })
            .collect();
        let doc = layout(setup, kind, &items, "Holiday", &label);
        assert_eq!(doc.pages.len(), 3);
        assert_eq!(doc.pages[0].images.len(), 24);
        assert_eq!(doc.pages[2].images.len(), 2);
        assert_eq!(doc.pages[2].images[1].item, 49);
        let h = doc.pages[1].header.as_ref().unwrap();
        assert_eq!(h.title, "Holiday");
        assert_eq!(h.page, "2/3");
        let area = setup.printable();
        for page in &doc.pages {
            for (k, p) in page.images.iter().enumerate() {
                assert!(area.contains(&p.image), "{p:?}");
                assert!(area.contains(&p.caption_rect), "{p:?}");
                assert!(p.image.y >= h.rect.bottom());
                // Neighbours in a row do not overlap.
                if let Some(q) = page.images.get(k + 1)
                    && (k + 1) % 4 != 0
                {
                    assert!(p.image.right() <= q.image.x);
                }
            }
        }
        // Same cell size everywhere: landscape images share a width.
        let a = &doc.pages[0].images[0];
        let c = &doc.pages[0].images[2];
        assert!(close(a.image.w, c.image.w));
        assert!(close(a.image.y, c.image.y));
    }

    #[test]
    fn empty_sheet_still_has_a_page() {
        let doc = layout(
            PageSetup::default(),
            Kind::Sheet {
                cols: 3,
                rows: 3,
                caption: Caption::Off,
                header: false,
            },
            &[],
            "",
            &label,
        );
        assert_eq!(doc.pages.len(), 1);
        assert!(doc.pages[0].images.is_empty() && doc.pages[0].header.is_none());
    }

    #[test]
    fn single_without_items_has_no_pages() {
        let doc = layout(
            PageSetup::default(),
            Kind::Single {
                fit: Fit::Fit,
                caption: Caption::Off,
            },
            &[],
            "",
            &label,
        );
        assert!(doc.pages.is_empty());
    }
}
