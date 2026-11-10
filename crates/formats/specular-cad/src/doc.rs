//! The file's structure: sections, header variables, table entries, block definitions,
//! entity records (with their VERTEX / ATTRIB followers) and objects.

use kurbo::{Point, Rect};

use crate::geom::V3;
use crate::tokens::Group;

/// One record: the groups after its `0 TYPE` group, and the records that belong to it
/// (a POLYLINE's vertices, an INSERT's attributes).
pub(crate) struct Ent<'a> {
    pub ty: &'a [u8],
    /// The groups before extended data (1000+) and embedded objects (101).
    pub g: &'a [Group<'a>],
    /// All groups, extended data included.
    pub all: &'a [Group<'a>],
    pub subs: Vec<Ent<'a>>,
}

impl<'a> Ent<'a> {
    pub fn is(&self, ty: &str) -> bool {
        self.ty.eq_ignore_ascii_case(ty.as_bytes())
    }

    pub fn get(&self, code: i32) -> Option<&'a Group<'a>> {
        self.g.iter().find(|g| g.code == code)
    }

    pub fn has(&self, code: i32) -> bool {
        self.get(code).is_some()
    }

    pub fn f(&self, code: i32, def: f64) -> f64 {
        self.get(code).map_or(def, |g| g.f64())
    }

    pub fn i(&self, code: i32, def: i64) -> i64 {
        self.get(code).map_or(def, |g| g.i64())
    }

    pub fn s(&self, code: i32) -> &'a [u8] {
        self.get(code).map_or(b"", |g| g.tbytes())
    }

    /// The point of code `x` (y is `x + 10`).
    pub fn p2(&self, x: i32) -> Point {
        Point::new(self.f(x, 0.0), self.f(x + 10, 0.0))
    }

    pub fn p3(&self, x: i32) -> V3 {
        V3::new(self.f(x, 0.0), self.f(x + 10, 0.0), self.f(x + 20, 0.0))
    }

    pub fn extrusion(&self) -> V3 {
        V3::new(self.f(210, 0.0), self.f(220, 0.0), self.f(230, 1.0))
    }

    /// The handle (group 5).
    pub fn handle(&self) -> u64 {
        self.get(5).map_or(0, |g| g.handle())
    }
}

/// Splits a section into `0` records.
pub(crate) fn records<'a>(gs: &'a [Group<'a>]) -> Vec<Ent<'a>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < gs.len() {
        if gs[i].code != 0 {
            i += 1;
            continue;
        }
        let ty = gs[i].tbytes();
        let start = i + 1;
        let mut j = start;
        while j < gs.len() && gs[j].code != 0 {
            j += 1;
        }
        let all = &gs[start..j];
        let end = all
            .iter()
            .position(|g| g.code == 101 || g.code >= 1000)
            .unwrap_or(all.len());
        out.push(Ent {
            ty,
            g: &all[..end],
            all,
            subs: Vec::new(),
        });
        i = j;
    }
    out
}

/// Attaches VERTEX records to their POLYLINE and ATTRIB records to their INSERT, dropping
/// the closing SEQEND.
pub(crate) fn attach<'a>(recs: Vec<Ent<'a>>) -> Vec<Ent<'a>> {
    let mut out: Vec<Ent<'a>> = Vec::with_capacity(recs.len());
    let mut open: Option<&'static str> = None;
    for r in recs {
        if let Some(kind) = open {
            if r.is(kind) {
                if let Some(parent) = out.last_mut() {
                    parent.subs.push(r);
                }
                continue;
            }
            open = None;
            if r.is("SEQEND") {
                continue;
            }
        }
        if r.is("POLYLINE") {
            open = Some("VERTEX");
        } else if r.is("INSERT") {
            open = Some("ATTRIB");
        }
        out.push(r);
    }
    out
}

/// `0 SECTION / 2 NAME ... 0 ENDSEC` spans (a missing ENDSEC ends at the next SECTION or
/// the end of the file).
pub(crate) fn sections<'a>(gs: &'a [Group<'a>]) -> Vec<(&'a [u8], &'a [Group<'a>])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < gs.len() {
        if gs[i].code == 0 && gs[i].is("SECTION") {
            let name = gs
                .get(i + 1)
                .filter(|g| g.code == 2)
                .map_or(&b""[..], |g| g.tbytes());
            let start = (i + 2).min(gs.len());
            let mut j = start;
            while j < gs.len()
                && !(gs[j].code == 0
                    && (gs[j].is("ENDSEC") || gs[j].is("SECTION") || gs[j].is("EOF")))
            {
                j += 1;
            }
            out.push((name, &gs[start..j]));
            i = if j < gs.len() && gs[j].is("ENDSEC") {
                j + 1
            } else {
                j
            };
        } else {
            i += 1;
        }
    }
    out
}

/// Header variables.
pub(crate) struct Header {
    pub version: String,
    pub codepage: Option<String>,
    pub insunits: i16,
    pub ext: Option<Rect>,
    pub pext: Option<Rect>,
    pub plim: Option<Rect>,
    pub ltscale: f64,
    pub lwdisplay: bool,
    pub pdmode: i32,
    pub pdsize: f64,
    pub dimasz: f64,
    pub dimscale: f64,
    /// `$MEASUREMENT`: 0 imperial, 1 metric (`None` when not stated).
    pub measurement: Option<i16>,
    /// `$LUNITS` and `$LUPREC`.
    pub lunits: i16,
    pub luprec: i16,
}

impl Default for Header {
    fn default() -> Header {
        Header {
            version: String::new(),
            codepage: None,
            insunits: 0,
            ext: None,
            pext: None,
            plim: None,
            ltscale: 1.0,
            lwdisplay: false,
            pdmode: 0,
            pdsize: 0.0,
            dimasz: 2.5,
            dimscale: 1.0,
            measurement: None,
            lunits: 2,
            luprec: 4,
        }
    }
}

/// A rectangle from two corners, when both are finite, ordered, and not the "empty"
/// ±1e20 AutoCAD writes.
pub(crate) fn valid_rect(a: Point, b: Point) -> Option<Rect> {
    let ok = |v: f64| v.is_finite() && v.abs() < 1e19;
    (ok(a.x) && ok(a.y) && ok(b.x) && ok(b.y) && a.x <= b.x && a.y <= b.y)
        .then(|| Rect::new(a.x, a.y, b.x, b.y))
}

pub(crate) fn header(gs: &[Group<'_>]) -> Header {
    let mut h = Header::default();
    let mut i = 0;
    let mut ext = (None, None);
    let mut pext = (None, None);
    let mut plim = (None, None);
    let point = |vals: &[Group<'_>]| -> Point {
        let x = vals.iter().find(|g| g.code == 10).map_or(0.0, |g| g.f64());
        let y = vals.iter().find(|g| g.code == 20).map_or(0.0, |g| g.f64());
        Point::new(x, y)
    };
    while i < gs.len() {
        if gs[i].code != 9 {
            i += 1;
            continue;
        }
        let name = gs[i].tbytes();
        let mut j = i + 1;
        while j < gs.len() && gs[j].code != 9 && gs[j].code != 0 {
            j += 1;
        }
        let vals = &gs[i + 1..j];
        let first = vals.first();
        let num = || first.map_or(0.0, |g| g.f64());
        let int = || first.map_or(0, |g| g.i64());
        match name {
            b"$ACADVER" => {
                h.version = String::from_utf8_lossy(first.map_or(&b""[..], |g| g.tbytes())).into()
            }
            b"$DWGCODEPAGE" => {
                let s =
                    String::from_utf8_lossy(first.map_or(&b""[..], |g| g.tbytes())).into_owned();
                if !s.is_empty() {
                    h.codepage = Some(s);
                }
            }
            b"$INSUNITS" => h.insunits = int() as i16,
            b"$MEASUREMENT" => h.measurement = Some(int() as i16),
            b"$LUNITS" => h.lunits = int().clamp(1, 5) as i16,
            b"$LUPREC" => h.luprec = int().clamp(0, 8) as i16,
            b"$EXTMIN" => ext.0 = Some(point(vals)),
            b"$EXTMAX" => ext.1 = Some(point(vals)),
            b"$PEXTMIN" => pext.0 = Some(point(vals)),
            b"$PEXTMAX" => pext.1 = Some(point(vals)),
            b"$PLIMMIN" => plim.0 = Some(point(vals)),
            b"$PLIMMAX" => plim.1 = Some(point(vals)),
            b"$LTSCALE" => {
                let v = num();
                if v > 0.0 {
                    h.ltscale = v;
                }
            }
            b"$LWDISPLAY" => h.lwdisplay = int() != 0,
            b"$PDMODE" => h.pdmode = int() as i32,
            b"$PDSIZE" => h.pdsize = num(),
            b"$DIMASZ" => {
                let v = num();
                if v > 0.0 {
                    h.dimasz = v;
                }
            }
            b"$DIMSCALE" => {
                let v = num();
                if v > 0.0 {
                    h.dimscale = v;
                }
            }
            _ => {}
        }
        i = j;
    }
    let pair = |p: (Option<Point>, Option<Point>)| match p {
        (Some(a), Some(b)) => valid_rect(a, b),
        _ => None,
    };
    h.ext = pair(ext);
    h.pext = pair(pext);
    h.plim = pair(plim).filter(|r| r.area() > 0.0);
    h
}

/// Table entries by table name (LAYER, LTYPE, STYLE, BLOCK_RECORD, ...).
pub(crate) fn tables<'a>(gs: &'a [Group<'a>]) -> Vec<(&'a [u8], Vec<Ent<'a>>)> {
    let mut out: Vec<(&'a [u8], Vec<Ent<'a>>)> = Vec::new();
    for r in records(gs) {
        if r.is("TABLE") {
            out.push((r.s(2), Vec::new()));
        } else if r.is("ENDTAB") {
            continue;
        } else if let Some(t) = out.last_mut() {
            t.1.push(r);
        }
    }
    out
}

/// A block definition.
pub(crate) struct BlockDef<'a> {
    pub name: String,
    pub base: V3,
    pub flags: i64,
    pub ents: Vec<Ent<'a>>,
}

pub(crate) fn blocks<'a>(
    gs: &'a [Group<'a>],
    decode: &dyn Fn(&[u8]) -> String,
) -> Vec<BlockDef<'a>> {
    let mut out = Vec::new();
    let mut cur: Option<(BlockDef<'a>, Vec<Ent<'a>>)> = None;
    for r in records(gs) {
        if r.is("BLOCK") {
            if let Some((mut b, e)) = cur.take() {
                b.ents = attach(e);
                out.push(b);
            }
            let name = if r.has(2) { r.s(2) } else { r.s(3) };
            cur = Some((
                BlockDef {
                    name: decode(name),
                    base: r.p3(10),
                    flags: r.i(70, 0),
                    ents: Vec::new(),
                },
                Vec::new(),
            ));
        } else if r.is("ENDBLK") {
            if let Some((mut b, e)) = cur.take() {
                b.ents = attach(e);
                out.push(b);
            }
        } else if let Some((_, e)) = cur.as_mut() {
            e.push(r);
        }
    }
    if let Some((mut b, e)) = cur.take() {
        b.ents = attach(e);
        out.push(b);
    }
    out
}

/// A LAYOUT object.
pub(crate) struct LayoutDef {
    pub name: String,
    pub order: i64,
    pub block_record: u64,
    pub paper: Option<Rect>,
    pub ext: Option<Rect>,
    /// The plot settings' paper unit (group 72: inches when true), when stated.
    pub paper_inches: Option<bool>,
    /// The layout's own plot scale, paper units per drawing unit, when it is not 1:1:
    /// the standard scale (group 147) when the plot settings use one (flag 16 of group
    /// 70), else the custom scale (142 / 143). `None` for 1:1 and "scaled to fit".
    pub plot_scale: Option<f64>,
}

/// A plot settings' scale (paper units per drawing unit), when stated and not 1:1.
fn plot_scale(
    flags: i64,
    standard_type: i64,
    standard: f64,
    paper: f64,
    drawing: f64,
) -> Option<f64> {
    let k = if flags & 16 != 0 {
        // Standard scale type 0 is "scaled to fit": no fixed scale.
        if standard_type == 0 {
            return None;
        }
        standard
    } else {
        paper / drawing
    };
    (k.is_finite() && k > 0.0 && (k - 1.0).abs() > 1e-9).then_some(k)
}

#[cfg(test)]
#[test]
fn plot_scales() {
    // Custom 1:50, standard 1:100, scaled to fit, and 1:1.
    assert_eq!(plot_scale(0, 0, 0.0, 1.0, 50.0), Some(0.02));
    assert_eq!(plot_scale(16, 3, 0.01, 1.0, 1.0), Some(0.01));
    assert_eq!(plot_scale(16, 0, 1.0, 1.0, 50.0), None);
    assert_eq!(plot_scale(0, 0, 0.0, 1.0, 1.0), None);
    assert_eq!(plot_scale(0, 0, 0.0, 1.0, 0.0), None);
}

pub(crate) fn layouts(gs: &[Group<'_>], decode: &dyn Fn(&[u8]) -> String) -> Vec<LayoutDef> {
    let mut out = Vec::new();
    for r in records(gs) {
        if !r.is("LAYOUT") {
            continue;
        }
        let split =
            r.g.iter()
                .position(|g| g.code == 100 && g.is("AcDbLayout"))
                .unwrap_or(r.g.len());
        let (plot, lay) = r.g.split_at(split);
        fn get<'g>(gs: &[Group<'g>], code: i32) -> Option<Group<'g>> {
            gs.iter().find(|g| g.code == code).copied()
        }
        let f = |gs: &[Group<'_>], code: i32| get(gs, code).map_or(0.0, |g| g.f64());
        let name = get(lay, 1).map_or(String::new(), |g| decode(g.tbytes()));
        let scale = plot_scale(
            get(plot, 70).map_or(0, |g| g.i64()),
            get(plot, 75).map_or(0, |g| g.i64()),
            f(plot, 147),
            get(plot, 142).map_or(1.0, |g| g.f64()),
            get(plot, 143).map_or(1.0, |g| g.f64()),
        );
        let limits = valid_rect(
            Point::new(f(lay, 10), f(lay, 20)),
            Point::new(f(lay, 11), f(lay, 21)),
        )
        .filter(|r| r.area() > 0.0);
        let paper = limits.or_else(|| {
            // Plot settings: paper size and margins in mm (or inches).
            let (mut w, mut h) = (f(plot, 44), f(plot, 45));
            let rot = get(plot, 73).map_or(0, |g| g.i64());
            if rot == 1 || rot == 3 {
                std::mem::swap(&mut w, &mut h);
            }
            let unit = if get(plot, 72).map_or(1, |g| g.i64()) == 0 {
                1.0 / 25.4
            } else {
                1.0
            };
            // At a plot scale the paper covers more (or fewer) drawing units.
            let unit = unit / scale.unwrap_or(1.0);
            let (ml, mb) = (f(plot, 40), f(plot, 41));
            (w > 0.0 && h > 0.0)
                .then(|| Rect::new(-ml * unit, -mb * unit, (w - ml) * unit, (h - mb) * unit))
        });
        let ext = valid_rect(
            Point::new(f(lay, 14), f(lay, 24)),
            Point::new(f(lay, 15), f(lay, 25)),
        );
        out.push(LayoutDef {
            name,
            order: get(lay, 71).map_or(0, |g| g.i64()),
            block_record: get(lay, 330).map_or(0, |g| g.handle()),
            paper,
            ext,
            paper_inches: get(plot, 72).map(|g| g.i64() == 0),
            plot_scale: scale,
        });
    }
    out
}
