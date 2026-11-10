//! A board's fabrication files as one scene: a layer per file in stack order, and the top and
//! bottom of the board in board colours. One Gerber or drill file opens alone as a scene of
//! one layer.

mod outline;
mod role;

pub use role::{Plating, Role, Side, looks_like_board, role_by_name, role_of, role_of_x2};

use specular_vector::kurbo::{Affine, BezPath, Rect, Shape as _};
use specular_vector::{
    Color, FontSpec, Item, Layer, PcbInfo, PcbKind, PcbLayer, Shape, Sheet, SheetKind, Units,
    VectorScene,
};

use crate::excellon::{self, Drill};
use crate::gerber::{self, Gerber, Object, ObjectKind, Segment};
use crate::{Error, Limits, geom};

/// Objects merged into one item at most (consecutive, same polarity).
const MERGE: usize = 64;
/// Holes merged into one item at most.
const MERGE_HOLES: usize = 256;
/// Holes of tools without a size are drawn this wide (mm).
const UNSIZED: f64 = 0.1;
/// Opacity of each layer's group: the layer view shows the layers under each other.
const LAYER_OPACITY: f32 = 0.8;

/// Board colours (sRGB).
const SUBSTRATE: [u8; 3] = [150, 140, 85];
/// A panel's boards' edges.
const BOARD_EDGE: [u8; 3] = [70, 64, 30];
const COPPER: [u8; 3] = [212, 170, 80];
const MASK: [u8; 3] = [22, 98, 52];
const MASK_ON_COPPER: [u8; 3] = [64, 140, 66];
const SILK: [u8; 3] = [244, 244, 238];
/// Component outlines and reference designators.
const COMPONENTS: [u8; 3] = [90, 170, 230];

/// One parsed file of a set.
enum Parsed {
    Gerber(Box<Gerber>),
    Drill(Box<Drill>),
}

struct File {
    name: String,
    role: Role,
    from_x2: bool,
    parsed: Parsed,
}

/// Builds items from outlines.
struct Out<'a> {
    items: &'a mut Vec<Item>,
    layer: u32,
    transform: Affine,
}

impl Out<'_> {
    fn fill(&mut self, path: BezPath, color: Color, even_odd: bool) {
        if path.elements().is_empty() {
            return;
        }
        let path = if self.transform == Affine::IDENTITY {
            path
        } else {
            geom::transformed(&path, self.transform)
        };
        self.items.push(Item {
            layer: self.layer,
            outer: self.layer,
            color,
            shape: Shape::Fill { path, even_odd },
        });
    }

    fn hairline(&mut self, path: BezPath, color: Color) {
        self.items.push(Item {
            layer: self.layer,
            outer: self.layer,
            color,
            shape: Shape::Stroke {
                path: self.transform * path,
                weight: 0,
                dash: None,
            },
        });
    }

    /// A Gerber's objects in order, consecutive ones of one polarity merged; dark in
    /// `dark`, clear in `clear`.
    fn objects(&mut self, objs: &[Object], dark: Color, clear: Color) {
        let mut acc = BezPath::new();
        let mut acc_dark = true;
        let mut n = 0;
        for o in objs {
            let color = if o.dark { dark } else { clear };
            if o.kind == ObjectKind::Hairline {
                if n > 0 {
                    self.fill(
                        std::mem::take(&mut acc),
                        if acc_dark { dark } else { clear },
                        false,
                    );
                    n = 0;
                }
                self.hairline(o.path.clone(), color);
                continue;
            }
            if n > 0 && (o.dark != acc_dark || n >= MERGE || o.holes) {
                self.fill(
                    std::mem::take(&mut acc),
                    if acc_dark { dark } else { clear },
                    false,
                );
                n = 0;
            }
            acc_dark = o.dark;
            for el in o.path.elements() {
                acc.push(*el);
            }
            n += 1;
            if o.holes {
                self.fill(std::mem::take(&mut acc), color, false);
                n = 0;
            }
        }
        if n > 0 {
            self.fill(acc, if acc_dark { dark } else { clear }, false);
        }
    }

    /// A drill file's holes and slots.
    fn drill(&mut self, d: &Drill, color: Color) {
        let r = |t: usize| d.tools.get(t).map_or(0.0, |t| t.diameter / 2.0);
        let mut acc = BezPath::new();
        let mut n = 0;
        for h in &d.holes {
            // A tool of no stated size: a small mark.
            let rr = match r(h.tool) {
                rr if rr > 0.0 => rr,
                _ => UNSIZED / 2.0,
            };
            for el in geom::circle(h.at, rr).elements() {
                acc.push(*el);
            }
            n += 1;
            if n >= MERGE_HOLES {
                self.fill(std::mem::take(&mut acc), color, false);
                n = 0;
            }
        }
        if n > 0 {
            self.fill(acc, color, false);
        }
        let mut acc = BezPath::new();
        for s in &d.slots {
            let rr = r(s.tool);
            if rr <= 0.0 {
                let mut c = BezPath::new();
                c.move_to(s.segment.start());
                c.line_to(s.segment.end());
                self.hairline(c, color);
                continue;
            }
            let p = match s.segment {
                Segment::Line(a, b) => geom::capsule(a, b, rr),
                Segment::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                } => geom::arc_band(center, radius, start, sweep, rr),
            };
            for el in p.elements() {
                acc.push(*el);
            }
        }
        self.fill(acc, color, false);
    }
}

fn rgb(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

fn layer(name: String, color: [u8; 3]) -> Layer {
    Layer {
        name,
        generated: None,
        color: rgb(color),
        linetype: None,
        lineweight: 0,
        on: true,
        frozen: false,
        plot: true,
        items: 0,
    }
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, b) => a.or(b),
    }
}

fn items_bounds(items: &[Item]) -> Option<Rect> {
    let mut r = None;
    for it in items {
        let b = match &it.shape {
            Shape::Fill { path, .. } | Shape::Stroke { path, .. } => path.bounding_box(),
            _ => continue,
        };
        if b.x0.is_finite() && b.y1.is_finite() {
            r = union(r, Some(b));
        }
    }
    r
}

fn sheet(name: &str, items: Vec<Item>) -> Sheet {
    let mut s = Sheet::new(name, SheetKind::Model);
    s.extents = items_bounds(&items);
    s.items = items;
    s
}

fn count_items(scene: &mut VectorScene) {
    for l in &mut scene.layers {
        l.items = 0;
    }
    for s in &scene.sheets {
        for it in &s.items {
            if let Some(l) = scene.layers.get_mut(it.layer as usize) {
                l.items += 1;
            }
        }
    }
}

fn counts_of(files: &[File]) -> Vec<(String, u64)> {
    let mut c: std::collections::BTreeMap<&'static str, u64> = Default::default();
    for f in files {
        match &f.parsed {
            Parsed::Gerber(g) => {
                for o in &g.objects {
                    let k = match o.kind {
                        ObjectKind::Flash => "Flash",
                        ObjectKind::Draw | ObjectKind::Hairline => "Draw",
                        ObjectKind::Region => "Region",
                    };
                    *c.entry(k).or_default() += 1;
                }
            }
            Parsed::Drill(d) => {
                *c.entry("Hole").or_default() += d.holes.len() as u64;
                *c.entry("Slot").or_default() += d.slots.len() as u64;
            }
        }
    }
    c.into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(k, n)| (k.to_string(), n))
        .collect()
}

fn scene_base(format: &str, version: String) -> VectorScene {
    VectorScene {
        format: format.into(),
        version,
        units: Units { code: 4 },
        fonts: vec![FontSpec::default()],
        ..VectorScene::default()
    }
}

/// Size of a rectangle, mm.
fn size(r: Option<Rect>) -> Option<(f64, f64)> {
    r.map(|r| (r.width(), r.height()))
}

/// One Gerber or drill file alone.
pub fn single_scene(name: &str, bytes: &[u8], limits: &Limits) -> Result<VectorScene, Error> {
    let head = &bytes[..bytes.len().min(16 * 1024)];
    let drill =
        excellon::is_excellon(head) || (!gerber::is_gerber(head) && role_by_name(name).is_drill());
    let file = if drill {
        let d = excellon::parse(bytes, limits)?;
        let role = role_of(name, d.file_function().as_deref());
        File {
            name: name.to_string(),
            from_x2: d.file_function().is_some(),
            role: if role.is_drill() {
                role
            } else {
                Role::Drill(Plating::Mixed)
            },
            parsed: Parsed::Drill(Box::new(d)),
        }
    } else {
        let g = gerber::parse(bytes, limits)?;
        let ff = g.file_function();
        File {
            name: name.to_string(),
            from_x2: ff.as_deref().and_then(role_of_x2).is_some(),
            role: role_of(name, ff.as_deref()),
            parsed: Parsed::Gerber(Box::new(g)),
        }
    };
    let mut items = Vec::new();
    let colour = file.role.colour();
    let mut out = Out {
        items: &mut items,
        layer: 0,
        transform: Affine::IDENTITY,
    };
    let mut info = PcbInfo {
        layers: vec![PcbLayer {
            name: file.name.clone(),
            role: file.role.english(),
            from_x2: file.from_x2,
        }],
        ..PcbInfo::default()
    };
    let (mut scene, warnings) = match &file.parsed {
        Parsed::Gerber(g) => {
            out.objects(&g.objects, rgb(colour), Color::Clear);
            info.kind = PcbKind::Gerber;
            info.generator = g.generator();
            info.project = g.project();
            info.revision = g.revision();
            info.date = g.date();
            info.size_mm = size(g.bounds);
            info.apertures = g.apertures;
            info.macros = g.macros;
            info.copper_layers = u32::from(file.role.is_copper());
            (scene_base("Gerber", g.version()), g.warnings.clone())
        }
        Parsed::Drill(d) => {
            out.drill(d, rgb(colour));
            info.kind = PcbKind::Drill;
            info.generator = d.generator.clone();
            info.size_mm = size(d.bounds);
            info.holes = (d.holes.len() + d.slots.len()) as u64;
            info.drill_sizes = d.drill_sizes();
            (scene_base("Excellon", String::new()), d.warnings.clone())
        }
    };
    scene.layers = vec![layer(
        format!("{} ({})", file.role.english(), file.name),
        colour,
    )];
    scene.layer_opacity = vec![1.0];
    scene.counts = counts_of(std::slice::from_ref(&file));
    scene.warnings = warnings;
    scene.sheets = vec![sheet(&file.name, items)];
    scene.pcb = Some(Box::new(info));
    count_items(&mut scene);
    Ok(scene)
}

/// A board's Gerber and drill files as one scene: sheets "Layers", "Top" and "Bottom".
pub fn board_scene(files: &[(String, Vec<u8>)], limits: &Limits) -> Result<VectorScene, Error> {
    let mut warnings: Vec<String> = Vec::new();
    let mut parsed: Vec<File> = Vec::new();
    let mut drills: Vec<(&String, &Vec<u8>)> = Vec::new();
    for (name, bytes) in files {
        let head = &bytes[..bytes.len().min(16 * 1024)];
        // Job files describe the set; they draw nothing.
        if name.to_ascii_lowercase().ends_with(".gbrjob") {
            continue;
        }
        if excellon::is_excellon(head) {
            drills.push((name, bytes));
            continue;
        }
        if !gerber::is_gerber(head) {
            continue;
        }
        match gerber::parse(bytes, limits) {
            Ok(g) => {
                let ff = g.file_function();
                let from_x2 = ff.as_deref().and_then(role_of_x2).is_some();
                let role = match role_of(name, ff.as_deref()) {
                    // A Gerber named like a drill file is a drill drawing or map.
                    r if r.is_drill() => Role::Mechanical,
                    r => r,
                };
                for w in &g.warnings {
                    crate::warn(&mut warnings, format!("{name}: {w}"));
                }
                parsed.push(File {
                    name: name.clone(),
                    role,
                    from_x2,
                    parsed: Parsed::Gerber(Box::new(g)),
                });
            }
            Err(e) => crate::warn(&mut warnings, format!("{name}: {e}")),
        }
    }
    // Drill files: units and zeros they leave out are guessed against the Gerbers.
    let hint = parsed
        .iter()
        .filter_map(|f| match &f.parsed {
            Parsed::Gerber(g) if f.role.is_copper() || f.role == Role::Outline => g.bounds,
            _ => None,
        })
        .reduce(|a, b| a.union(b));
    // Allegro's NC logs size the tools its tapes select without defining.
    let log = |want: &str, route: bool| {
        files
            .iter()
            .find(|(n, _)| {
                n.rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(n)
                    .eq_ignore_ascii_case(want)
            })
            .map(|(_, b)| excellon::allegro_log_tools(&String::from_utf8_lossy(b), route))
            .unwrap_or_default()
    };
    let (drill_log, route_log) = (log("ncdrill.log", false), log("ncroute.log", true));
    for (name, bytes) in drills {
        match excellon::parse_with_hint(bytes, limits, hint) {
            Ok(mut d) => {
                let route = name.to_ascii_lowercase().ends_with(".rou");
                d.size_undefined_tools(if route { &route_log } else { &drill_log });
                let role = role_of(name, d.file_function().as_deref());
                let role = match (role, d.plated) {
                    (Role::Drill(Plating::Mixed), Some(true)) => Role::Drill(Plating::Plated),
                    (Role::Drill(Plating::Mixed), Some(false)) => Role::Drill(Plating::NonPlated),
                    (r, _) if r.is_drill() => r,
                    _ => Role::Drill(Plating::Mixed),
                };
                for w in &d.warnings {
                    crate::warn(&mut warnings, format!("{name}: {w}"));
                }
                parsed.push(File {
                    name: name.clone(),
                    role,
                    from_x2: d.file_function().is_some(),
                    parsed: Parsed::Drill(Box::new(d)),
                });
            }
            Err(e) => crate::warn(&mut warnings, format!("{name}: {e}")),
        }
    }
    if parsed.is_empty() {
        return Err(Error::Malformed(
            "no Gerber or drill file in the set could be read".into(),
        ));
    }
    let thickness = files
        .iter()
        .filter(|(n, _)| n.to_ascii_lowercase().ends_with(".gbrjob"))
        .find_map(|(_, b)| job_thickness(b));
    Ok(assemble(
        parsed,
        warnings,
        Extra {
            hint,
            thickness,
            ..Extra::default()
        },
    ))
}

/// A component placed on the board: its reference designator at its place, and its body's
/// outline when known (board coordinates, mm).
#[derive(Clone, Debug)]
pub(crate) struct Component {
    pub refdes: String,
    pub at: specular_vector::kurbo::Point,
    /// Rotation, degrees counter-clockwise.
    pub rotation: f64,
    pub side: Side,
    pub outline: Option<BezPath>,
}

/// What a board format adds to its layers.
#[derive(Default)]
pub(crate) struct Extra {
    /// "ODB++", "IPC-2581"; empty for a Gerber set.
    pub format: String,
    pub version: String,
    pub kind: Option<PcbKind>,
    /// The board's shape when the format states it (ODB++ profile, IPC-2581 Profile).
    pub shape: Option<BezPath>,
    /// The outlines of the boards on a panel (ODB++ step-and-repeat), drawn over it.
    pub outlines: Vec<BezPath>,
    /// Bounds to fall back on when nothing gives the board's size.
    pub hint: Option<Rect>,
    pub thickness: Option<f64>,
    pub components: Vec<Component>,
    pub generator: Option<String>,
    pub project: Option<String>,
    pub nets: u64,
    /// Entity counts of the format's own (features by type).
    pub counts: Vec<(String, u64)>,
}

/// One layer of a board from another format: a Gerber-like layer or drill holes.
pub(crate) fn gerber_layer(
    name: String,
    role: Role,
    objects: Vec<Object>,
    bounds: Option<Rect>,
) -> LayerFile {
    LayerFile(File {
        name,
        role,
        from_x2: false,
        parsed: Parsed::Gerber(Box::new(Gerber {
            objects,
            bounds,
            ..Gerber::default()
        })),
    })
}

pub(crate) fn drill_layer(name: String, role: Role, drill: Drill) -> LayerFile {
    LayerFile(File {
        name,
        role,
        from_x2: false,
        parsed: Parsed::Drill(Box::new(drill)),
    })
}

/// Names the views of a board with more than one step after `step`.
pub(crate) fn name_step(scene: &mut VectorScene, step: &str) {
    for s in &mut scene.sheets {
        s.group = Some(step.to_string());
    }
}

/// Adds another step's views (a scene of [`board_of`]) after the views of `scene`: its
/// layers are matched to `scene`'s by name, those it lacks added.
pub(crate) fn add_step(scene: &mut VectorScene, other: VectorScene, step: &str) {
    let map: Vec<u32> = other
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if let Some(k) = scene.layers.iter().position(|m| m.name == l.name) {
                return k as u32;
            }
            scene.layers.push(l.clone());
            let o = other.layer_opacity.get(i).copied().unwrap_or(LAYER_OPACITY);
            scene.layer_opacity.push(o);
            (scene.layers.len() - 1) as u32
        })
        .collect();
    let at = |l: u32| map.get(l as usize).copied().unwrap_or(l);
    for mut s in other.sheets {
        for it in &mut s.items {
            it.layer = at(it.layer);
            it.outer = at(it.outer);
        }
        s.group = Some(step.to_string());
        scene.sheets.push(s);
    }
    count_items(scene);
}

/// A layer made by [`gerber_layer`] or [`drill_layer`].
pub(crate) struct LayerFile(File);

/// A board of another format (ODB++, IPC-2581) through the same views as a Gerber set.
pub(crate) fn board_of(layers: Vec<LayerFile>, warnings: Vec<String>, extra: Extra) -> VectorScene {
    assemble(layers.into_iter().map(|l| l.0).collect(), warnings, extra)
}

fn assemble(mut parsed: Vec<File>, mut warnings: Vec<String>, extra: Extra) -> VectorScene {
    let hint = extra.hint;
    parsed.sort_by(|a, b| a.role.rank().cmp(&b.role.rank()).then(a.name.cmp(&b.name)));

    let mut scene = scene_base(
        if extra.format.is_empty() {
            "Gerber"
        } else {
            &extra.format
        },
        extra.version.clone(),
    );
    scene.layers = parsed
        .iter()
        .map(|f| {
            layer(
                format!("{} ({})", f.role.english(), f.name),
                f.role.colour(),
            )
        })
        .collect();

    // The board's shape and size.
    let outline = parsed.iter().position(|f| f.role == Role::Outline);
    let shape = match &extra.shape {
        Some(p) => Some(outline::BoardShape {
            path: p.clone(),
            bounds: p.bounding_box(),
        }),
        None => outline.and_then(|i| match &parsed[i].parsed {
            Parsed::Gerber(g) => outline::board_shape(g),
            _ => None,
        }),
    };
    let copper_bounds = parsed
        .iter()
        .filter(|f| f.role.is_copper())
        .filter_map(|f| match &f.parsed {
            Parsed::Gerber(g) => g.bounds,
            _ => None,
        })
        .reduce(|a, b| a.union(b));
    let outline_bounds = outline.and_then(|i| match &parsed[i].parsed {
        Parsed::Gerber(g) => g.bounds,
        _ => None,
    });
    let board_bounds = shape
        .as_ref()
        .map(|s| s.bounds)
        .or(outline_bounds)
        .or(copper_bounds)
        .or(hint);
    if shape.is_none() && outline.is_some() {
        crate::warn(
            &mut warnings,
            "The board outline does not close; the board is drawn as its bounding rectangle.",
        );
    }
    let board_path = match (&shape, board_bounds) {
        (Some(s), _) => Some(s.path.clone()),
        (None, Some(b)) => Some(geom::rect(b.center(), b.width(), b.height())),
        _ => None,
    };
    // The substrate's layer: the outline's, else a layer of its own.
    let board_layer = match outline {
        Some(i) => i as u32,
        None => {
            scene.layers.push(layer("Board".into(), SUBSTRATE));
            (scene.layers.len() - 1) as u32
        }
    };

    // Layers: every file in stack order.
    let mut items = Vec::new();
    for (i, f) in parsed.iter().enumerate() {
        let mut out = Out {
            items: &mut items,
            layer: i as u32,
            transform: Affine::IDENTITY,
        };
        let c = rgb(f.role.colour());
        match &f.parsed {
            Parsed::Gerber(g) => out.objects(&g.objects, c, Color::Clear),
            Parsed::Drill(d) => out.drill(d, c),
        }
    }
    // Components: a layer per side with outlines and reference designators.
    let mut comp_layer = [None, None];
    for (k, side) in [Side::Top, Side::Bottom].into_iter().enumerate() {
        if extra.components.iter().any(|c| c.side == side) {
            let name = if side == Side::Top {
                "Top components"
            } else {
                "Bottom components"
            };
            scene.layers.push(layer(name.into(), COMPONENTS));
            comp_layer[k] = Some((scene.layers.len() - 1) as u32);
        }
    }
    let comp_items = |side: Side, mirror: Affine| -> Vec<Item> {
        let k = usize::from(side == Side::Bottom);
        let Some(l) = comp_layer[k] else {
            return Vec::new();
        };
        components(&extra.components, side, l, mirror)
    };
    items.extend(comp_items(Side::Top, Affine::IDENTITY));
    items.extend(comp_items(Side::Bottom, Affine::IDENTITY));
    let mut sheets = vec![sheet("Layers", items)];
    for side in [Side::Top, Side::Bottom] {
        let mirror = match (side, board_bounds) {
            (Side::Bottom, Some(b)) => {
                Affine::translate((2.0 * b.center().x, 0.0)) * Affine::FLIP_X
            }
            _ => Affine::IDENTITY,
        };
        let mut items = board_view(&parsed, side, board_path.as_ref(), board_layer, mirror);
        let mut out = Out {
            items: &mut items,
            layer: board_layer,
            transform: mirror,
        };
        for p in &extra.outlines {
            out.hairline(p.clone(), rgb(BOARD_EDGE));
        }
        items.extend(comp_items(side, mirror));
        sheets.push(sheet(
            if side == Side::Top { "Top" } else { "Bottom" },
            items,
        ));
    }
    scene.sheets = sheets;
    scene.layer_opacity = vec![LAYER_OPACITY; scene.layers.len()];

    // Facts.
    let mut info = PcbInfo {
        kind: extra.kind.unwrap_or(PcbKind::GerberSet),
        size_mm: size(board_bounds),
        generator: extra.generator.clone(),
        project: extra.project.clone(),
        components: extra.components.len() as u64,
        nets: extra.nets,
        ..PcbInfo::default()
    };
    for f in &parsed {
        info.layers.push(PcbLayer {
            name: f.name.clone(),
            role: f.role.english(),
            from_x2: f.from_x2,
        });
        if f.role.is_copper() {
            info.copper_layers += 1;
        }
        match &f.parsed {
            Parsed::Gerber(g) => {
                info.generator = info.generator.take().or_else(|| g.generator());
                info.project = info.project.take().or_else(|| g.project());
                info.revision = info.revision.take().or_else(|| g.revision());
                info.date = info.date.take().or_else(|| g.date());
                info.apertures += g.apertures;
                info.macros += g.macros;
            }
            Parsed::Drill(d) => {
                info.holes += (d.holes.len() + d.slots.len()) as u64;
                info.generator = info.generator.take().or_else(|| d.generator.clone());
            }
        }
    }
    let mut sizes: Vec<u64> = parsed
        .iter()
        .filter_map(|f| match &f.parsed {
            Parsed::Drill(d) => Some(d),
            _ => None,
        })
        .flat_map(|d| {
            d.holes
                .iter()
                .map(|h| h.tool)
                .chain(d.slots.iter().map(|s| s.tool))
                .filter_map(|t| d.tools.get(t))
                .map(|t| (t.diameter * 1e4).round() as u64)
                .collect::<Vec<_>>()
        })
        .collect();
    sizes.sort_unstable();
    sizes.dedup();
    info.drill_sizes = sizes.len() as u32;
    info.thickness_mm = extra.thickness;
    scene.counts = if extra.counts.is_empty() {
        counts_of(&parsed)
    } else {
        extra.counts.clone()
    };
    if !extra.components.is_empty() {
        scene
            .counts
            .push(("Component".into(), extra.components.len() as u64));
    }
    scene.warnings = warnings;
    scene.pcb = Some(Box::new(info));
    count_items(&mut scene);
    scene
}

/// Component outlines (hairlines) and reference designators of one side.
fn components(comps: &[Component], side: Side, layer: u32, mirror: Affine) -> Vec<Item> {
    use specular_vector::{HAlign, Paragraph, Span, TextBlock, VAlign};
    let mut items = Vec::new();
    let color = rgb(COMPONENTS);
    for c in comps.iter().filter(|c| c.side == side) {
        if let Some(o) = &c.outline {
            items.push(Item {
                layer,
                outer: layer,
                color,
                shape: Shape::Stroke {
                    path: mirror * o.clone(),
                    weight: 0,
                    dash: None,
                },
            });
        }
        if c.refdes.is_empty() {
            continue;
        }
        // Text height from the body's size: a third of its short side, 0.3–1.5 mm.
        let h = c
            .outline
            .as_ref()
            .map(|o| {
                let b = o.bounding_box();
                (b.width().min(b.height()) / 3.0).clamp(0.3, 1.5)
            })
            .unwrap_or(0.8);
        let at = mirror * c.at;
        // Readable on either side: text is never drawn mirrored or upside down.
        let mut rot = c.rotation.rem_euclid(180.0);
        if rot > 90.0 {
            rot -= 180.0;
        }
        items.push(Item {
            layer,
            outer: layer,
            color,
            shape: Shape::Text(Box::new(TextBlock {
                transform: Affine::translate(at.to_vec2()) * Affine::rotate(rot.to_radians()),
                halign: HAlign::Center,
                valign: VAlign::Middle,
                wrap: 0.0,
                fit: None,
                line_spacing: 1.0,
                paragraphs: vec![Paragraph {
                    spans: vec![Span::new(c.refdes.clone(), 0, h)],
                    align: None,
                }],
            })),
        });
    }
    items
}

/// The board thickness a KiCad/Ucamco job file (`.gbrjob`, JSON) states.
fn job_thickness(b: &[u8]) -> Option<f64> {
    let t = String::from_utf8_lossy(b);
    let i = t.find("\"BoardThickness\"")?;
    let rest = &t[i + 16..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// The board seen from `side` in board colours: substrate, copper, solder mask with its
/// openings, silkscreen, holes.
fn board_view(
    files: &[File],
    side: Side,
    board: Option<&BezPath>,
    board_layer: u32,
    mirror: Affine,
) -> Vec<Item> {
    let mut items = Vec::new();
    let copper = match side {
        Side::Top => Role::TopCopper,
        Side::Bottom => Role::BottomCopper,
    };
    let find = |r: Role| {
        files
            .iter()
            .enumerate()
            .filter(move |(_, f)| f.role == r)
            .filter_map(|(i, f)| match &f.parsed {
                Parsed::Gerber(g) => Some((i as u32, g.as_ref())),
                _ => None,
            })
    };
    if let Some(b) = board {
        let mut out = Out {
            items: &mut items,
            layer: board_layer,
            transform: mirror,
        };
        out.fill(b.clone(), rgb(SUBSTRATE), true);
    }
    for (i, g) in find(copper) {
        let mut out = Out {
            items: &mut items,
            layer: i,
            transform: mirror,
        };
        out.objects(&g.objects, rgb(COPPER), Color::Clear);
    }
    for (i, mask) in find(Role::SolderMask(side)) {
        let mut out = Out {
            items: &mut items,
            layer: i,
            transform: mirror,
        };
        // The mask covers the board; copper under it shows through lighter. The mask
        // file's dark shapes are its openings.
        if let Some(b) = board {
            out.fill(b.clone(), rgb(MASK), true);
        }
        for (_, g) in find(copper) {
            out.objects(&g.objects, rgb(MASK_ON_COPPER), rgb(MASK));
        }
        out.objects(&mask.objects, Color::Clear, rgb(MASK));
    }
    for (i, g) in find(Role::Silkscreen(side)) {
        let mut out = Out {
            items: &mut items,
            layer: i,
            transform: mirror,
        };
        out.objects(&g.objects, rgb(SILK), Color::Clear);
    }
    for (i, f) in files.iter().enumerate() {
        if let Parsed::Drill(d) = &f.parsed {
            let mut out = Out {
                items: &mut items,
                layer: i as u32,
                transform: mirror,
            };
            out.drill(d, Color::Background);
        }
    }
    items
}

#[cfg(test)]
mod tests;
