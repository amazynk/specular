//! What the export recipes sheet shows, drawn from a [`View`] alone (so screenshots need no
//! app): the recipe list, the recipe editor, a confirmation, the run and its end report.
//! Drawn like Convert's sheet, whose pieces it shares.

use std::sync::Arc;

use specular_gfx::{Align, DrawList, Icon, Rect, Text, TextBlock, draw_icon};
use specular_i18n::t;
use specular_theme::metrics::RADIUS_SM;
use specular_theme::{Color, Palette};
use specular_widgets::accesskit;

use super::super::convert::{
    Mirror, Styles, VALUE_PAD, button, draw_field, ellipsize, sheet_rect, stepper_arrow,
    stepper_width,
};

pub(super) const A11Y_BASE: u64 = 0x7600_0000_0000_0000;

/// One stepper row of the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum Row {
    Images,
    Format,
    Quality,
    Lossless,
    Bits,
    Space,
    Profile,
    Folder,
    Collision,
    After,
    Resize,
    Edge,
    Width,
    Height,
    Megapixels,
    Percent,
    Enlarge,
    Sharpen,
    Amount,
    Metadata,
    Location,
    People,
    Mark,
    Position,
    Opacity,
    Scale,
}

/// A text field of the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum Field {
    Name,
    Pattern,
    Text,
    MarkText,
    Subfolder,
}

/// What a click, a key or a screen reader hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Hit {
    Dec(Row),
    Inc(Row),
    Row(Row),
    /// A recipe of the list (focus), and its check box.
    Recipe(usize),
    Check(usize),
    New,
    Edit,
    Duplicate,
    Delete,
    Field(Field),
    ChooseFolder,
    ChooseImage,
    Cancel,
    Start,
    Save,
    /// Confirmations: the action, and keeping things as they are.
    Confirm,
    Keep,
    Stop,
    Hide,
    OpenFolder,
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct RowView {
    pub row: Row,
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct RecipeView {
    pub name: String,
    pub summary: String,
    pub checked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct FieldView {
    pub field: Field,
    pub label: String,
    pub text: String,
    pub caret: usize,
    pub selection: (usize, usize),
    pub preedit: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum PhaseView {
    List {
        images: RowView,
        recipes: Vec<RecipeView>,
        /// The recipe Edit, Duplicate and Delete act on.
        current: usize,
        /// What stops the run, when something does (no recipe checked, no images).
        blocked: Option<String>,
    },
    Edit {
        title: String,
        columns: [Vec<RowView>; 3],
        fields: Vec<FieldView>,
        /// The first image's new name, or what is wrong with the pattern.
        preview: Result<String, String>,
        choose_folder: bool,
        choose_image: bool,
    },
    /// A question before something is removed or replaced.
    Confirm {
        title: String,
        lines: Vec<String>,
        confirm: String,
        keep: String,
    },
    /// Folders being asked for, names being settled.
    Waiting { status: String },
    Running {
        status: String,
        progress: f32,
        stopping: bool,
    },
    Done {
        summary: String,
        lines: Vec<String>,
        can_open: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct View {
    pub phase: PhaseView,
    /// Index into [`View::focusables`].
    pub focus: Option<usize>,
}

impl View {
    /// The keyboard focus order.
    pub fn focusables(&self) -> Vec<Hit> {
        match &self.phase {
            PhaseView::List { recipes, .. } => [Hit::Row(Row::Images)]
                .into_iter()
                .chain((0..recipes.len()).map(Hit::Recipe))
                .chain([
                    Hit::New,
                    Hit::Edit,
                    Hit::Duplicate,
                    Hit::Delete,
                    Hit::Cancel,
                    Hit::Start,
                ])
                .collect(),
            PhaseView::Edit {
                columns,
                fields,
                choose_folder,
                choose_image,
                ..
            } => {
                let mut v: Vec<Hit> = fields
                    .iter()
                    .filter(|f| f.field == Field::Name)
                    .map(|f| Hit::Field(f.field))
                    .collect();
                v.extend(columns.iter().flatten().map(|r| Hit::Row(r.row)));
                v.extend(
                    fields
                        .iter()
                        .filter(|f| f.field != Field::Name)
                        .map(|f| Hit::Field(f.field)),
                );
                if *choose_folder {
                    v.push(Hit::ChooseFolder);
                }
                if *choose_image {
                    v.push(Hit::ChooseImage);
                }
                v.extend([Hit::Cancel, Hit::Save]);
                v
            }
            PhaseView::Confirm { .. } => vec![Hit::Confirm, Hit::Keep],
            PhaseView::Waiting { .. } => vec![Hit::Cancel],
            PhaseView::Running { .. } => vec![Hit::Stop, Hit::Hide],
            PhaseView::Done { can_open, .. } => {
                let mut v = Vec::new();
                if *can_open {
                    v.push(Hit::OpenFolder);
                }
                v.push(Hit::Close);
                v
            }
        }
    }

    pub fn focused(&self) -> Option<Hit> {
        self.focus.and_then(|i| self.focusables().get(i).copied())
    }

    pub fn rows(&self) -> Vec<&RowView> {
        match &self.phase {
            PhaseView::List { images, .. } => vec![images],
            PhaseView::Edit { columns, .. } => columns.iter().flatten().collect(),
            _ => Vec::new(),
        }
    }

    pub fn can_start(&self) -> bool {
        matches!(&self.phase, PhaseView::List { blocked: None, .. })
    }

    pub fn can_save(&self) -> bool {
        matches!(&self.phase, PhaseView::Edit { preview: Ok(_), .. })
    }

    /// The default button (Enter).
    pub fn default_hit(&self) -> Hit {
        match &self.phase {
            PhaseView::List { .. } => Hit::Start,
            PhaseView::Edit { .. } => Hit::Save,
            PhaseView::Confirm { .. } => Hit::Keep,
            PhaseView::Waiting { .. } => Hit::Cancel,
            PhaseView::Running { .. } => Hit::Hide,
            PhaseView::Done { .. } => Hit::Close,
        }
    }
}

/// A drawn text field: which, where, and its text with the x it is drawn at.
pub(in crate::app) type DrawnField = (Field, Rect, Option<(Arc<TextBlock>, f32)>);

/// Where things landed.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct Drawn {
    pub sheet: Rect,
    pub hits: Vec<(Rect, Hit)>,
    /// Text fields: their rect, and their text with the x it is drawn at.
    pub fields: Vec<DrawnField>,
    /// Where the input method's window opens.
    pub ime: Option<Rect>,
    /// The texts (summaries, status, report lines), for screen readers.
    pub texts: Vec<(Rect, String)>,
}

pub(super) fn title_text() -> String {
    t!("export-recipes-title")
}

pub(super) fn hit_label(h: Hit) -> String {
    match h {
        Hit::New => t!("export-recipes-new"),
        Hit::Edit => t!("export-recipes-edit"),
        Hit::Duplicate => t!("export-recipes-duplicate"),
        Hit::Delete => t!("export-recipes-delete"),
        Hit::ChooseFolder => t!("convert-choose-folder"),
        Hit::ChooseImage => t!("export-recipes-choose-image"),
        Hit::Cancel => t!("app-cancel"),
        Hit::Start => t!("export-recipes-start"),
        Hit::Save => t!("export-recipes-save"),
        Hit::Stop => t!("convert-stop"),
        Hit::Hide => t!("convert-hide"),
        Hit::OpenFolder => t!("export-recipes-open-folder"),
        Hit::Close => t!("convert-close"),
        _ => String::new(),
    }
}

/// A label and `‹ value ›` at the row's end, the stepper `sw` wide.
#[allow(clippy::too_many_arguments)]
fn draw_row(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    st: &Styles,
    mi: Mirror,
    rv: &RowView,
    r: Rect,
    sw: f32,
    focused: bool,
    drawn: &mut Drawn,
) {
    let row_h = r.h;
    if focused {
        dl.rect_border(
            mi.r(r.inset(-2.0)),
            RADIUS_SM,
            Color::TRANSPARENT,
            pal.focus_ring,
            2.0,
        );
    }
    let sx = r.right() - sw;
    let lab = text.layout(&rv.label, st.label, None, Align::Start);
    let lr = Rect::new(r.x + 4.0, r.y, (sx - 8.0 - r.x - 4.0).max(0.0), row_h);
    dl.push_clip(mi.r(lr));
    text.draw(
        dl,
        &lab,
        mi.x(lr.x, lab.width),
        r.y + (row_h - lab.height) * 0.5,
        pal.text_muted,
    );
    dl.pop_clip();
    let arrow = stepper_arrow(row_h);
    let dec = Rect::new(sx, r.y + (row_h - arrow) * 0.5, arrow, arrow);
    let inc = Rect::new(r.right() - arrow, dec.y, arrow, arrow);
    for (b, icon, hit) in [
        (dec, Icon::ChevronLeft, Hit::Dec(rv.row)),
        (inc, Icon::ChevronRight, Hit::Inc(rv.row)),
    ] {
        let b = mi.r(b);
        dl.rect(b, RADIUS_SM, pal.surface_raised);
        draw_icon(dl, icon, b.inset(arrow * 0.18), pal.text, 1.6, mi.rtl);
        drawn.hits.push((b, hit));
    }
    let inner = sw - 2.0 * arrow;
    let value = ellipsize(text, &rv.value, st.value, inner - 2.0 * VALUE_PAD);
    let val = text.layout(&value, st.value, None, Align::Start);
    let mid = sx + arrow + (inner - val.width.min(inner)) * 0.5;
    dl.push_clip(mi.r(Rect::new(sx + arrow, r.y, inner, row_h)));
    text.draw(
        dl,
        &val,
        mi.x(mid, val.width),
        r.y + (row_h - val.height) * 0.5,
        pal.text,
    );
    dl.pop_clip();
    drawn.hits.push((mi.r(r), Hit::Row(rv.row)));
}

/// A row of buttons ending at `end_x` (the first at the end): `(hit, label, enabled,
/// default)`.
#[allow(clippy::too_many_arguments)]
fn buttons(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    st: &Styles,
    mi: Mirror,
    list: &[(Hit, String, bool, bool)],
    end_x: f32,
    y: f32,
    bh: f32,
    focus: Option<Hit>,
    drawn: &mut Drawn,
) {
    let mut x = end_x;
    for (hit, label, enabled, default) in list {
        let b = text.layout(label, st.button, None, Align::Start);
        let bw = (b.width + 28.0).max(84.0);
        x -= bw;
        let r = Rect::new(x, y, bw, bh);
        x -= 8.0;
        button(
            dl,
            text,
            pal,
            mi,
            &b,
            r,
            *default,
            *enabled,
            focus == Some(*hit),
        );
        drawn.hits.push((mi.r(r), *hit));
    }
}

/// Buttons from the start edge at `x`: `(hit, label, enabled)`.
#[allow(clippy::too_many_arguments)]
fn buttons_from(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    st: &Styles,
    mi: Mirror,
    list: &[(Hit, String, bool)],
    mut x: f32,
    y: f32,
    bh: f32,
    focus: Option<Hit>,
    drawn: &mut Drawn,
) {
    for (hit, label, enabled) in list {
        let b = text.layout(label, st.button, None, Align::Start);
        let r = Rect::new(x, y, (b.width + 24.0).max(72.0), bh);
        x = r.right() + 8.0;
        button(
            dl,
            text,
            pal,
            mi,
            &b,
            r,
            false,
            *enabled,
            focus == Some(*hit),
        );
        drawn.hits.push((mi.r(r), *hit));
    }
}

/// Lays out `lines` and draws them from `y`, clipped above `bottom`; returns the y after.
#[allow(clippy::too_many_arguments)]
fn draw_lines(
    dl: &mut DrawList,
    text: &mut Text,
    mi: Mirror,
    lines: &[String],
    style: specular_gfx::TextStyle,
    x: f32,
    mut y: f32,
    w: f32,
    bottom: f32,
    ink: Color,
    drawn: &mut Drawn,
) -> f32 {
    dl.push_clip(mi.r(Rect::new(x - 2.0, y, w + 4.0, (bottom - y).max(0.0))));
    for l in lines {
        let b = text.layout(l, style, Some(w), Align::Start);
        text.draw(dl, &b, mi.x(x, b.width), y, ink);
        drawn
            .texts
            .push((mi.r(Rect::new(x, y, b.width, b.height)), l.clone()));
        y += b.height + 4.0;
    }
    dl.pop_clip();
    y
}

/// Draws the sheet over a scrim in a window of `size` logical px.
pub(in crate::app) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &View,
    size: (f32, f32),
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ww, wh) = size;
    dl.rect(Rect::new(0.0, 0.0, ww, wh), 0.0, pal.scrim);
    let mi = Mirror { ww, rtl };
    let st = Styles::new(ts);
    let u = ts.max(1.0);
    let pad = 20.0;
    let row_h = (32.0 * u).round();
    let bh = (32.0 * u).round();
    let focus = v.focused();
    let mut drawn = Drawn::default();
    let title_of = |s: &str, text: &mut Text| text.layout(s, st.title, None, Align::Start);

    match &v.phase {
        PhaseView::List {
            images,
            recipes,
            current,
            blocked,
        } => {
            let title = title_of(&title_text(), text);
            let line_h = (46.0 * u).round();
            let w = (ww - 32.0).clamp(240.0, 640.0);
            let list_rows = recipes.len().clamp(3, 7) as f32;
            let h = (2.0 * pad
                + title.height
                + 12.0
                + row_h
                + 12.0
                + list_rows * line_h
                + 12.0
                + bh
                + 22.0 * u
                + 12.0
                + bh)
                .min(wh - 32.0)
                .max(200.0);
            let sheet = sheet_rect(dl, pal, ww, wh, w, h);
            drawn.sheet = sheet;
            text.draw(
                dl,
                &title,
                mi.x(sheet.x + pad, title.width),
                sheet.y + pad,
                pal.text,
            );
            let x = sheet.x + pad;
            let cw = w - 2.0 * pad;
            let mut y = sheet.y + pad + title.height + 12.0;
            // Which images.
            let lw = text.measure(&images.label, st.label);
            let vw = text.measure(&images.value, st.value);
            let sw = stepper_width(cw, stepper_arrow(row_h), lw, vw);
            draw_row(
                dl,
                text,
                pal,
                &st,
                mi,
                images,
                Rect::new(x, y, cw, row_h),
                sw,
                focus == Some(Hit::Row(Row::Images)),
                &mut drawn,
            );
            y += row_h + 12.0;
            // The recipes, each with its check box.
            let list = Rect::new(x, y, cw, list_rows * line_h);
            dl.rect_border(
                mi.r(list),
                RADIUS_SM,
                pal.surface_raised,
                pal.separator,
                1.0,
            );
            dl.push_clip(mi.r(list));
            for (i, r) in recipes.iter().enumerate() {
                let rr = Rect::new(x, y + i as f32 * line_h, cw, line_h);
                if rr.y >= list.bottom() {
                    break;
                }
                if i == *current {
                    dl.rect(mi.r(rr.inset(2.0)), RADIUS_SM, pal.selection);
                }
                if focus == Some(Hit::Recipe(i)) {
                    dl.rect_border(
                        mi.r(rr.inset(2.0)),
                        RADIUS_SM,
                        Color::TRANSPARENT,
                        pal.focus_ring,
                        2.0,
                    );
                }
                let bs = (18.0 * u).round();
                let cb = Rect::new(rr.x + 12.0, rr.y + (line_h - bs) * 0.5, bs, bs);
                let cbm = mi.r(cb);
                if r.checked {
                    dl.rect(cbm, 4.0, pal.accent_fill);
                    draw_icon(
                        dl,
                        Icon::Check,
                        cbm.inset(bs * 0.15),
                        pal.accent_text,
                        2.0,
                        false,
                    );
                } else {
                    dl.rect_border(cbm, 4.0, pal.surface, pal.text_muted, 1.5);
                }
                let tx = cb.right() + 12.0;
                let tw = (rr.right() - tx - 10.0).max(0.0);
                let name = ellipsize(text, &r.name, st.value, tw);
                let nb = text.layout(&name, st.value, None, Align::Start);
                let sum = ellipsize(text, &r.summary, st.small, tw);
                let sb = text.layout(&sum, st.small, None, Align::Start);
                let ty = rr.y + (line_h - nb.height - sb.height - 2.0) * 0.5;
                text.draw(dl, &nb, mi.x(tx, nb.width), ty, pal.text);
                text.draw(
                    dl,
                    &sb,
                    mi.x(tx, sb.width),
                    ty + nb.height + 2.0,
                    pal.text_muted,
                );
                drawn
                    .texts
                    .push((mi.r(rr), format!("{}. {}", r.name, r.summary)));
                drawn.hits.push((mi.r(cb.inset(-6.0)), Hit::Check(i)));
                drawn.hits.push((mi.r(rr), Hit::Recipe(i)));
            }
            dl.pop_clip();
            y = list.bottom() + 12.0;
            let have = !recipes.is_empty();
            buttons_from(
                dl,
                text,
                pal,
                &st,
                mi,
                &[
                    (Hit::New, hit_label(Hit::New), true),
                    (Hit::Edit, hit_label(Hit::Edit), have),
                    (Hit::Duplicate, hit_label(Hit::Duplicate), have),
                    (Hit::Delete, hit_label(Hit::Delete), have),
                ],
                x,
                y,
                bh - 4.0,
                focus,
                &mut drawn,
            );
            y += bh + 8.0;
            if let Some(b) = blocked {
                let tb = text.layout(b, st.small, Some(cw), Align::Start);
                text.draw(dl, &tb, mi.x(x, tb.width), y, pal.text_muted);
                drawn
                    .texts
                    .push((mi.r(Rect::new(x, y, tb.width, tb.height)), b.clone()));
            }
            buttons(
                dl,
                text,
                pal,
                &st,
                mi,
                &[
                    (Hit::Start, hit_label(Hit::Start), blocked.is_none(), true),
                    (Hit::Cancel, hit_label(Hit::Cancel), true, false),
                ],
                sheet.right() - pad,
                sheet.bottom() - pad - bh,
                bh,
                focus,
                &mut drawn,
            );
        }
        PhaseView::Edit {
            title,
            columns,
            fields,
            preview,
            choose_folder,
            choose_image,
        } => {
            let title = title_of(title, text);
            let field_h = 20.0 * u + row_h;
            let most = columns.iter().map(Vec::len).max().unwrap_or(0).max(7) as f32;
            let w = (ww - 32.0).clamp(300.0, 980.0);
            let h = (2.0 * pad
                + title.height
                + 12.0
                + field_h
                + 12.0
                + most * row_h
                + 12.0
                + field_h
                + 6.0
                + 18.0 * u
                + 10.0
                + field_h
                + 16.0
                + bh)
                .min(wh - 32.0)
                .max(240.0);
            let sheet = sheet_rect(dl, pal, ww, wh, w, h);
            drawn.sheet = sheet;
            text.draw(
                dl,
                &title,
                mi.x(sheet.x + pad, title.width),
                sheet.y + pad,
                pal.text,
            );
            let x = sheet.x + pad;
            let cw = w - 2.0 * pad;
            let mut y = sheet.y + pad + title.height + 12.0;
            let field = |f: Field| fields.iter().find(|x| x.field == f);
            // A labelled text field at `r` (the label above it).
            let put_field =
                |dl: &mut DrawList, text: &mut Text, drawn: &mut Drawn, fv: &FieldView, r: Rect| {
                    let lab = text.layout(&fv.label, st.label, None, Align::Start);
                    text.draw(dl, &lab, mi.x(r.x + 4.0, lab.width), r.y, pal.text_muted);
                    let fr = Rect::new(r.x, r.y + 20.0 * u, r.w, row_h);
                    let focused = focus == Some(Hit::Field(fv.field));
                    let pre = if focused { fv.preedit.as_str() } else { "" };
                    let (block, x0, ime) = draw_field(
                        dl,
                        text,
                        pal,
                        &st,
                        mi,
                        fr,
                        &fv.text,
                        fv.caret,
                        fv.selection,
                        focused,
                        pre,
                    );
                    if focused {
                        drawn.ime = ime.or(Some(mi.r(fr)));
                    }
                    drawn
                        .fields
                        .push((fv.field, mi.r(fr), pre.is_empty().then_some((block, x0))));
                    drawn.hits.push((mi.r(fr), Hit::Field(fv.field)));
                };
            if let Some(f) = field(Field::Name) {
                put_field(dl, text, &mut drawn, f, Rect::new(x, y, cw * 0.5, field_h));
            }
            y += field_h + 12.0;
            let gap = 20.0;
            let col_w = (cw - 2.0 * gap) / 3.0;
            let arrow = stepper_arrow(row_h);
            for (c, rows) in columns.iter().enumerate() {
                let (lw, vw) = rows.iter().fold((0.0f32, 0.0f32), |(l, v), rv| {
                    (
                        l.max(text.measure(&rv.label, st.label)),
                        v.max(text.measure(&rv.value, st.value)),
                    )
                });
                let sw = stepper_width(col_w, arrow, lw, vw);
                let cx = x + c as f32 * (col_w + gap);
                for (i, rv) in rows.iter().enumerate() {
                    let r = Rect::new(cx, y + i as f32 * row_h, col_w, row_h);
                    let f = focus == Some(Hit::Row(rv.row));
                    draw_row(dl, text, pal, &st, mi, rv, r, sw, f, &mut drawn);
                }
            }
            y += most * row_h + 12.0;
            // The name pattern and the custom text, with the preview under them.
            if let Some(f) = field(Field::Pattern) {
                put_field(dl, text, &mut drawn, f, Rect::new(x, y, cw * 0.64, field_h));
            }
            if let Some(f) = field(Field::Text) {
                let fx = x + cw * 0.64 + 12.0;
                put_field(
                    dl,
                    text,
                    &mut drawn,
                    f,
                    Rect::new(fx, y, x + cw - fx, field_h),
                );
            }
            y += field_h + 6.0;
            let (msg, ink) = match preview {
                Ok(s) => (s.as_str(), pal.text_muted),
                Err(e) => (e.as_str(), pal.danger),
            };
            let pb = text.layout(msg, st.small, None, Align::Start);
            let pr = Rect::new(x + 4.0, y, cw - 8.0, pb.height);
            dl.push_clip(mi.r(pr));
            text.draw(dl, &pb, mi.x(pr.x, pb.width.min(pr.w)), pr.y, ink);
            dl.pop_clip();
            drawn.texts.push((mi.r(pr), msg.to_owned()));
            y += 18.0 * u + 10.0;
            // The watermark text, the subfolder's name, and the choosers.
            let mut bx = x;
            for f in [Field::MarkText, Field::Subfolder] {
                if let Some(f) = field(f) {
                    put_field(
                        dl,
                        text,
                        &mut drawn,
                        f,
                        Rect::new(bx, y, cw * 0.32, field_h),
                    );
                    bx += cw * 0.32 + 12.0;
                }
            }
            let mut choosers = Vec::new();
            if *choose_image {
                choosers.push((Hit::ChooseImage, hit_label(Hit::ChooseImage), true));
            }
            if *choose_folder {
                choosers.push((Hit::ChooseFolder, hit_label(Hit::ChooseFolder), true));
            }
            if !choosers.is_empty() {
                buttons_from(
                    dl,
                    text,
                    pal,
                    &st,
                    mi,
                    &choosers,
                    bx,
                    y + 20.0 * u,
                    row_h,
                    focus,
                    &mut drawn,
                );
            }
            buttons(
                dl,
                text,
                pal,
                &st,
                mi,
                &[
                    (Hit::Save, hit_label(Hit::Save), v.can_save(), true),
                    (Hit::Cancel, hit_label(Hit::Cancel), true, false),
                ],
                sheet.right() - pad,
                sheet.bottom() - pad - bh,
                bh,
                focus,
                &mut drawn,
            );
        }
        PhaseView::Confirm {
            title,
            lines,
            confirm,
            keep,
        } => {
            let w = (ww - 32.0).clamp(240.0, 480.0);
            let tw = w - 2.0 * pad;
            let title = text.layout(title, st.title, Some(tw), Align::Start);
            let lines_h: f32 = lines
                .iter()
                .map(|l| text.layout(l, st.label, Some(tw), Align::Start).height + 4.0)
                .sum();
            let h = (2.0 * pad + title.height + 14.0 + lines_h + 16.0 + bh)
                .min(wh - 32.0)
                .max(140.0);
            let sheet = sheet_rect(dl, pal, ww, wh, w, h);
            drawn.sheet = sheet;
            text.draw(
                dl,
                &title,
                mi.x(sheet.x + pad, title.width),
                sheet.y + pad,
                pal.text,
            );
            drawn.texts.push((
                mi.r(Rect::new(
                    sheet.x + pad,
                    sheet.y + pad,
                    title.width,
                    title.height,
                )),
                String::new(),
            ));
            let by = sheet.bottom() - pad - bh;
            draw_lines(
                dl,
                text,
                mi,
                lines,
                st.label,
                sheet.x + pad,
                sheet.y + pad + title.height + 14.0,
                tw,
                by - 8.0,
                pal.text,
                &mut drawn,
            );
            // Keep is the default: removing or replacing needs a deliberate choice.
            buttons(
                dl,
                text,
                pal,
                &st,
                mi,
                &[
                    (Hit::Keep, keep.clone(), true, true),
                    (Hit::Confirm, confirm.clone(), true, false),
                ],
                sheet.right() - pad,
                by,
                bh,
                focus,
                &mut drawn,
            );
        }
        PhaseView::Waiting { status } | PhaseView::Running { status, .. } => {
            let title = title_of(&title_text(), text);
            let w = (ww - 32.0).clamp(240.0, 520.0);
            let sb = text.layout(status, st.label, Some(w - 2.0 * pad), Align::Start);
            let h = (2.0 * pad + title.height + 16.0 + sb.height + 14.0 + 6.0 + 24.0 + bh)
                .min(wh - 32.0)
                .max(120.0);
            let sheet = sheet_rect(dl, pal, ww, wh, w, h);
            drawn.sheet = sheet;
            text.draw(
                dl,
                &title,
                mi.x(sheet.x + pad, title.width),
                sheet.y + pad,
                pal.text,
            );
            let y = sheet.y + pad + title.height + 16.0;
            text.draw(dl, &sb, mi.x(sheet.x + pad, sb.width), y, pal.text);
            drawn.texts.push((
                mi.r(Rect::new(sheet.x + pad, y, sb.width, sb.height)),
                status.clone(),
            ));
            let list: Vec<(Hit, String, bool, bool)> = match &v.phase {
                PhaseView::Running {
                    progress, stopping, ..
                } => {
                    let bar = Rect::new(sheet.x + pad, y + sb.height + 14.0, w - 2.0 * pad, 6.0);
                    dl.rect(bar, 3.0, pal.surface_raised);
                    let done = Rect::new(bar.x, bar.y, bar.w * progress.clamp(0.0, 1.0), bar.h);
                    dl.rect(mi.r(done), 3.0, pal.accent);
                    vec![
                        (Hit::Hide, hit_label(Hit::Hide), true, true),
                        (Hit::Stop, hit_label(Hit::Stop), !stopping, false),
                    ]
                }
                _ => vec![(Hit::Cancel, hit_label(Hit::Cancel), true, true)],
            };
            buttons(
                dl,
                text,
                pal,
                &st,
                mi,
                &list,
                sheet.right() - pad,
                sheet.bottom() - pad - bh,
                bh,
                focus,
                &mut drawn,
            );
        }
        PhaseView::Done {
            summary,
            lines,
            can_open,
        } => {
            let title = title_of(&title_text(), text);
            let w = (ww - 32.0).clamp(240.0, 600.0);
            let tw = w - 2.0 * pad;
            let sb = text.layout(summary, st.value, Some(tw), Align::Start);
            let lines_h: f32 = lines
                .iter()
                .map(|l| text.layout(l, st.label, Some(tw), Align::Start).height + 4.0)
                .sum();
            let h = (2.0 * pad + title.height + 16.0 + sb.height + 10.0 + lines_h + 16.0 + bh)
                .min(wh - 32.0)
                .max(120.0);
            let sheet = sheet_rect(dl, pal, ww, wh, w, h);
            drawn.sheet = sheet;
            text.draw(
                dl,
                &title,
                mi.x(sheet.x + pad, title.width),
                sheet.y + pad,
                pal.text,
            );
            let y = sheet.y + pad + title.height + 16.0;
            text.draw(dl, &sb, mi.x(sheet.x + pad, sb.width), y, pal.text);
            drawn.texts.push((
                mi.r(Rect::new(sheet.x + pad, y, sb.width, sb.height)),
                summary.clone(),
            ));
            let by = sheet.bottom() - pad - bh;
            draw_lines(
                dl,
                text,
                mi,
                lines,
                st.label,
                sheet.x + pad,
                y + sb.height + 10.0,
                tw,
                by - 8.0,
                pal.text_muted,
                &mut drawn,
            );
            let mut list = vec![(Hit::Close, hit_label(Hit::Close), true, true)];
            if *can_open {
                list.push((Hit::OpenFolder, hit_label(Hit::OpenFolder), true, false));
            }
            buttons(
                dl,
                text,
                pal,
                &st,
                mi,
                &list,
                sheet.right() - pad,
                by,
                bh,
                focus,
                &mut drawn,
            );
        }
    }
    drawn
}

/// The sheet's AccessKit nodes: a modal dialog with spin buttons, check boxes for the
/// recipes, text inputs, the texts and the buttons.
pub(in crate::app) fn a11y(tree: &mut accesskit::TreeUpdate, v: &View, d: &Drawn, scale: f64) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut dialog = Node::new(Role::Dialog);
    dialog.set_modal();
    dialog.set_label(match &v.phase {
        PhaseView::Edit { title, .. } | PhaseView::Confirm { title, .. } => title.clone(),
        _ => title_text(),
    });
    dialog.set_bounds(bounds(d.sheet));
    let focus_hit = v.focused();
    let mut focus_id = None;
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for (k, hit) in v.focusables().into_iter().enumerate() {
        let id = NodeId(A11Y_BASE + 1 + k as u64);
        let rect = d.hits.iter().find(|(_, h)| *h == hit).map(|x| x.0);
        let mut n = match hit {
            Hit::Row(row) => {
                let mut n = Node::new(Role::SpinButton);
                if let Some(rv) = v.rows().into_iter().find(|r| r.row == row) {
                    n.set_label(rv.label.clone());
                    n.set_value(rv.value.clone());
                }
                n.add_action(accesskit::Action::Increment);
                n.add_action(accesskit::Action::Decrement);
                n
            }
            Hit::Recipe(i) => {
                let mut n = Node::new(Role::CheckBox);
                if let PhaseView::List {
                    recipes, current, ..
                } = &v.phase
                    && let Some(r) = recipes.get(i)
                {
                    n.set_label(r.name.clone());
                    n.set_description(r.summary.clone());
                    n.set_toggled(if r.checked {
                        accesskit::Toggled::True
                    } else {
                        accesskit::Toggled::False
                    });
                    if i == *current {
                        n.set_selected(true);
                    }
                }
                n.add_action(accesskit::Action::Click);
                n
            }
            Hit::Field(f) => {
                let mut n = Node::new(Role::TextInput);
                if let PhaseView::Edit { fields, .. } = &v.phase
                    && let Some(fv) = fields.iter().find(|x| x.field == f)
                {
                    n.set_label(fv.label.clone());
                    let chars = |b: usize| fv.text[..b.min(fv.text.len())].chars().count();
                    let anchor = if fv.caret == fv.selection.0 {
                        fv.selection.1
                    } else {
                        fv.selection.0
                    };
                    let run_id = NodeId(A11Y_BASE + 0x800 + k as u64);
                    let run = specular_widgets::text_run_sel(
                        &mut n,
                        run_id,
                        &fv.text,
                        Some((chars(anchor), chars(fv.caret))),
                    );
                    n.set_children(vec![run_id]);
                    nodes.push((run_id, run));
                }
                n.add_action(accesskit::Action::SetValue);
                n.add_action(accesskit::Action::SetTextSelection);
                n
            }
            h => {
                let mut n = Node::new(Role::Button);
                let label = match (&v.phase, h) {
                    (PhaseView::Confirm { confirm, .. }, Hit::Confirm) => confirm.clone(),
                    (PhaseView::Confirm { keep, .. }, Hit::Keep) => keep.clone(),
                    _ => hit_label(h),
                };
                n.set_label(label);
                let disabled = match h {
                    Hit::Start => !v.can_start(),
                    Hit::Save => !v.can_save(),
                    Hit::Stop => matches!(v.phase, PhaseView::Running { stopping: true, .. }),
                    Hit::Edit | Hit::Duplicate | Hit::Delete => matches!(
                        &v.phase,
                        PhaseView::List { recipes, .. } if recipes.is_empty()
                    ),
                    _ => false,
                };
                if disabled {
                    n.set_disabled();
                } else {
                    n.add_action(accesskit::Action::Click);
                }
                n
            }
        };
        if let Some(r) = rect {
            n.set_bounds(bounds(r));
        }
        n.add_action(accesskit::Action::Focus);
        if focus_hit == Some(hit) {
            focus_id = Some(id);
        }
        ids.push(id);
        nodes.push((id, n));
    }
    for (k, (r, s)) in d.texts.iter().enumerate() {
        if s.is_empty() || matches!(v.phase, PhaseView::List { .. }) {
            continue;
        }
        let id = NodeId(A11Y_BASE + 0x400 + k as u64);
        let mut n = match &v.phase {
            PhaseView::Running { progress, .. } => {
                let mut n = Node::new(Role::ProgressIndicator);
                n.set_numeric_value(f64::from(*progress) * 100.0);
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(100.0);
                n.set_live(accesskit::Live::Polite);
                n
            }
            PhaseView::Edit { .. } | PhaseView::Waiting { .. } => {
                let mut n = Node::new(Role::Status);
                n.set_live(accesskit::Live::Polite);
                n
            }
            _ => Node::new(Role::Label),
        };
        n.set_label(s.clone());
        n.set_bounds(bounds(*r));
        ids.push(id);
        nodes.push((id, n));
    }
    dialog.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_BASE));
    }
    tree.nodes.push((NodeId(A11Y_BASE), dialog));
    tree.nodes.extend(nodes);
    tree.focus = focus_id.unwrap_or(NodeId(A11Y_BASE));
}

/// The control node `id` stands for.
pub(in crate::app) fn a11y_target(v: &View, id: u64) -> Option<Hit> {
    let k = id.checked_sub(A11Y_BASE + 1)?;
    v.focusables().get(usize::try_from(k).ok()?).copied()
}
