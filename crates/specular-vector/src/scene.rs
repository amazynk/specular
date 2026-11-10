//! The vector scene model: what a 2D drawing format (DXF today; DWG, HPGL, CGM later) is
//! parsed into, independent of how it is drawn.
//!
//! Geometry is in drawing units (model space) or paper units (layouts), y up, as `kurbo`
//! paths. Block references are already expanded: an [`Item`] carries its effective layer
//! (the reference's for entities on layer `0`) and the layer of the outermost reference, so
//! a layer switched off hides what was drawn on it, and a reference on a hidden layer
//! hides its whole block. Colours and lineweights are resolved (BYLAYER, BYBLOCK); only
//! ACI 7 stays symbolic ([`Color::Foreground`]) so the background can be swapped. Text
//! stays text ([`TextBlock`]) until it is laid out with fonts ([`crate::prep`]).

use kurbo::{Affine, BezPath, Point, Rect, Vec2};

/// One drawing.
#[derive(Clone, Debug, Default)]
pub struct VectorScene {
    /// File format, e.g. "DXF".
    pub format: String,
    /// The format's version string as stored (DXF `$ACADVER`, e.g. "AC1032").
    pub version: String,
    /// The release it corresponds to, e.g. "AutoCAD 2018", when known.
    pub release: Option<String>,
    /// Drawing units of model space.
    pub units: Units,
    pub layers: Vec<Layer>,
    pub linetypes: Vec<Linetype>,
    /// Text styles; [`Span::font`] indexes this.
    pub fonts: Vec<FontSpec>,
    /// Model space first, then the layouts in tab order.
    pub sheets: Vec<Sheet>,
    /// Entities by type as stored in the file (model and paper space, block definitions
    /// counted once), sorted by name.
    pub counts: Vec<(String, u64)>,
    /// Problems that did not stop the parse (skipped entities, limits hit), deduplicated.
    pub warnings: Vec<String>,
    /// The file asks for lineweights to be shown (`$LWDISPLAY`).
    pub lineweight_display: bool,
    /// Point display: `$PDMODE` and `$PDSIZE`.
    pub point_mode: i32,
    pub point_size: f64,
    /// Code page of the text, when the file named one (`$DWGCODEPAGE`).
    pub codepage: Option<String>,
    /// Radius of point dots in CSS pixels; 0 for the default (1 px). Map data uses larger
    /// dots than CAD point entities.
    pub dot_radius: f32,
    /// What a plot file (HP-GL) says about the plot; `None` for drawings.
    pub plot: Option<PlotInfo>,
    /// Cells placed by reference (IC layouts); sheets draw them through [`Sheet::refs`].
    pub layout: Option<Box<Layout>>,
    /// Each run of consecutive items on one layer draws as a group at this opacity (by layer
    /// index, 1.0 past the end), so overlapping shapes of a layer do not darken each other
    /// and [`Color::Clear`] erases only what its group drew. Empty: no groups.
    pub layer_opacity: Vec<f32>,
    /// Facts of a circuit board or schematic (Gerber, Excellon, KiCad), for the info panel.
    pub pcb: Option<Box<PcbInfo>>,
    /// Facts of survey data (LandXML), for the info panel.
    pub survey: Option<Box<SurveyInfo>>,
}

/// What survey data (LandXML) says about itself, for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurveyInfo {
    /// The coordinate system as named in the file ("NAD83 / Ohio North, EPSG:3734").
    pub crs: Option<String>,
    /// The file's linear unit as written ("meter", "USSurveyFoot").
    pub linear_unit: String,
    pub application: Option<String>,
    pub project: Option<String>,
    pub surfaces: Vec<SurfaceInfo>,
    pub alignments: Vec<AlignmentInfo>,
}

/// One TIN surface: counts, areas (plan and sloped), the volume above its lowest point,
/// its elevations and the contour interval drawn; in the file's units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceInfo {
    pub name: String,
    pub points: u64,
    pub triangles: u64,
    pub area_2d: f64,
    pub area_3d: f64,
    pub volume: f64,
    pub z_min: f64,
    pub z_max: f64,
    pub contour_interval: f64,
}

/// One alignment: its computed length and stations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AlignmentInfo {
    pub name: String,
    pub length: f64,
    pub sta_start: f64,
    pub sta_end: f64,
    pub profiles: u32,
}

/// What a board or schematic file says about itself, for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PcbInfo {
    pub kind: PcbKind,
    /// The CAD tool that wrote it (X2 `.GenerationSoftware`, KiCad `generator`).
    pub generator: Option<String>,
    /// Project or board name (X2 `.ProjectId`, the KiCad title block).
    pub project: Option<String>,
    /// Revision and date from the title block or X2 attributes.
    pub revision: Option<String>,
    pub date: Option<String>,
    /// Board outline width × height, millimetres (the outline's bounds, else the copper's).
    pub size_mm: Option<(f64, f64)>,
    /// Board thickness, millimetres, when stated.
    pub thickness_mm: Option<f64>,
    /// Copper layers.
    pub copper_layers: u32,
    /// The layers or files, in stack order.
    pub layers: Vec<PcbLayer>,
    /// Drilled holes (Excellon hits, KiCad pad and via holes) and distinct drill sizes.
    pub holes: u64,
    pub drill_sizes: u32,
    /// Footprints (boards) or symbols (schematics), nets, tracks (or wires), vias.
    pub components: u64,
    pub nets: u64,
    pub tracks: u64,
    pub vias: u64,
    /// Schematic sheets.
    pub sheets: u32,
    /// Apertures defined (Gerber), aperture macros.
    pub apertures: u32,
    pub macros: u32,
}

/// Which kind of electronics file a [`PcbInfo`] describes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PcbKind {
    /// One Gerber file.
    #[default]
    Gerber,
    /// One Excellon drill file.
    Drill,
    /// A board's Gerber and drill files together (a folder or a ZIP).
    GerberSet,
    /// A KiCad board.
    KicadBoard,
    /// A KiCad schematic.
    KicadSchematic,
    /// An ODB++ job (folder or archive).
    Odb,
    /// An IPC-2581 file.
    Ipc2581,
}

/// One layer of a board.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PcbLayer {
    /// The layer's name as shown (KiCad's "F.Cu", or the file name of a Gerber).
    pub name: String,
    /// What it is, in English ("Top copper", "Bottom solder mask", "Drill").
    pub role: String,
    /// Where the role came from: X2 `.FileFunction`, the file name, or the board.
    pub from_x2: bool,
}

/// A hierarchical layout (GDSII): cells whose geometry is prepared once and drawn
/// instanced wherever a reference places it, so millions of placements are never
/// expanded.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub cells: Vec<Cell>,
    /// Opacity of fills, 1-255, each with an opaque hairline outline; 0 draws fills
    /// opaque without one.
    pub fill_alpha: u8,
    /// Text whose glyphs are smaller than this many pixels is not drawn (0: always).
    pub label_min_px: f32,
    pub info: LayoutInfo,
}

/// Facts of a layout, for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutInfo {
    /// The library's name.
    pub library: String,
    /// Metres per database unit.
    pub db_unit_m: f64,
    /// User units per database unit.
    pub user_unit: f64,
    pub cells: usize,
    /// Cells nothing references, in file order.
    pub top_cells: Vec<String>,
    /// Layer/datatype pairs used.
    pub layers: usize,
    /// Polygons, paths and boxes as stored (each cell counted once).
    pub shapes: u64,
    pub texts: u64,
    /// References (SREF and AREF) as stored, and the cell placements they make (array
    /// elements counted).
    pub references: u64,
    pub placements: u64,
    /// Shapes of the top cells with every reference expanded (saturating).
    pub flat_shapes: u64,
    /// Bounds of the top cells, micrometres.
    pub bbox_um: Option<Rect>,
}

/// A cell: its own items (in cell coordinates) and the cells it places.
#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub name: String,
    pub items: Vec<Item>,
    pub refs: Vec<CellRef>,
}

/// A placement of [`Layout::cells`]`[cell]`: cell coordinates → the parent's.
#[derive(Clone, Debug, PartialEq)]
pub struct CellRef {
    pub cell: u32,
    pub transform: Affine,
    pub array: Option<CellArray>,
}

/// A regular array of placements: element (i, j) is moved by `i·col + j·row` (parent
/// coordinates) from the first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellArray {
    pub cols: u32,
    pub rows: u32,
    pub col: Vec2,
    pub row: Vec2,
}

impl CellArray {
    /// How many placements it makes.
    pub fn len(&self) -> u64 {
        u64::from(self.cols) * u64::from(self.rows)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Facts of a plot file (HP-GL, HP-GL/2), for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlotInfo {
    /// HP-GL/2 (else the original HP-GL).
    pub hpgl2: bool,
    /// Wrapped in a PCL 5 printer job.
    pub pcl: bool,
    /// Pens that drew something, ascending.
    pub pens: Vec<u32>,
    /// The paper or picture frame the file states (`PS`, PCL frame), mm, width × height.
    pub paper_mm: Option<(f64, f64)>,
    /// The plot's name (`BP`), when it has one.
    pub name: Option<String>,
}

impl VectorScene {
    /// All items of every sheet.
    pub fn item_count(&self) -> usize {
        self.sheets.iter().map(|s| s.items.len()).sum()
    }

    /// Index of the layer named `name` (case-insensitive, as AutoCAD compares names).
    pub fn layer_index(&self, name: &str) -> Option<u32> {
        self.layers
            .iter()
            .position(|l| l.name.eq_ignore_ascii_case(name))
            .map(|i| i as u32)
    }
}

/// Drawing units (DXF `$INSUNITS`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Units {
    /// The `$INSUNITS` code (0 unitless, 1 inches, 4 millimetres, 6 metres, ...).
    pub code: i16,
}

impl Units {
    /// The English name of the unit ("millimetres"), or `None` for unitless.
    pub fn name(self) -> Option<&'static str> {
        Some(match self.code {
            1 => "inches",
            2 => "feet",
            3 => "miles",
            4 => "millimetres",
            5 => "centimetres",
            6 => "metres",
            7 => "kilometres",
            8 => "microinches",
            9 => "mils",
            10 => "yards",
            11 => "ångströms",
            12 => "nanometres",
            13 => "microns",
            14 => "decimetres",
            15 => "decametres",
            16 => "hectometres",
            17 => "gigametres",
            18 => "astronomical units",
            19 => "light years",
            20 => "parsecs",
            21 => "US survey feet",
            _ => return None,
        })
    }

    /// The short symbol ("mm"), or `None` for unitless.
    pub fn symbol(self) -> Option<&'static str> {
        Some(match self.code {
            1 => "in",
            2 => "ft",
            3 => "mi",
            4 => "mm",
            5 => "cm",
            6 => "m",
            7 => "km",
            8 => "µin",
            9 => "mil",
            10 => "yd",
            11 => "Å",
            12 => "nm",
            13 => "µm",
            14 => "dm",
            15 => "dam",
            16 => "hm",
            17 => "Gm",
            18 => "au",
            19 => "ly",
            20 => "pc",
            21 => "ft (US)",
            _ => return None,
        })
    }

    /// Metres in one unit, or `None` for unitless.
    pub fn metres(self) -> Option<f64> {
        Some(match self.code {
            1 => 0.0254,
            2 => 0.3048,
            3 => 1609.344,
            4 => 1e-3,
            5 => 1e-2,
            6 => 1.0,
            7 => 1e3,
            8 => 0.0254e-6,
            9 => 0.0254e-3,
            10 => 0.9144,
            11 => 1e-10,
            12 => 1e-9,
            13 => 1e-6,
            14 => 0.1,
            15 => 10.0,
            16 => 100.0,
            17 => 1e9,
            18 => 149_597_870_700.0,
            19 => 9_460_730_472_580_800.0,
            20 => 30_856_775_814_913_673.0,
            21 => 1200.0 / 3937.0,
            _ => return None,
        })
    }
}

/// A resolved colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Rgb(u8, u8, u8),
    /// ACI 7: white on a dark background, black on a light one.
    Foreground,
    /// The background colour (wipeouts).
    Background,
    /// Erases what the item's layer group drew before it (Gerber clear polarity); the
    /// background colour where layers are not grouped ([`VectorScene::layer_opacity`]).
    Clear,
}

impl Color {
    /// The colour on a background: `dark` true for a dark one.
    pub fn on(self, dark: bool, background: [u8; 3]) -> [u8; 3] {
        match self {
            Color::Rgb(r, g, b) => [r, g, b],
            Color::Foreground if dark => [255, 255, 255],
            Color::Foreground => [0, 0, 0],
            Color::Background | Color::Clear => background,
        }
    }
}

/// A lineweight in hundredths of a millimetre; [`LW_DEFAULT`] when the drawing uses the
/// default weight.
pub type Lineweight = i16;

/// The default lineweight (0.25 mm), which DXF writes as -3.
pub const LW_DEFAULT: Lineweight = 25;

/// A name Specular made up because the file has none ("Pen 3", "Page 2"). The reader keeps
/// the English form in `name` (logs, tests, search); what is shown is rendered from this in
/// the reader's language, so format crates need no translations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Generated {
    /// An HP-GL pen ("Pen 3").
    Pen(u32),
    /// A page of a plot file ("Page 2").
    Page(u32),
    /// An unnamed layer, numbered from 1 ("Layer 4").
    Layer(u32),
    /// The single sheet of a map ("Map").
    Map,
    /// A view of a board's Gerber set: its layers stacked ("Layers"), or its top or bottom
    /// in board colours ("Top", "Bottom").
    BoardLayers,
    BoardTop,
    BoardBottom,
    /// A plot's frame ("Frame").
    ChartFrame,
    /// A plot's trace without a name, numbered from 1 ("Trace 2").
    Trace(u32),
    /// The contours of a 2-D spectrum above or below zero ("Positive contours").
    Contours {
        negative: bool,
    },
    /// A page showing survey data as a 3D model ("3D model").
    Model3d,
}

impl Generated {
    /// The English name, as the readers store it in `name`.
    pub fn english(self) -> String {
        match self {
            Generated::Pen(n) => format!("Pen {n}"),
            Generated::Page(n) => format!("Page {n}"),
            Generated::Layer(n) => format!("Layer {n}"),
            Generated::Map => "Map".to_string(),
            Generated::BoardLayers => "Layers".to_string(),
            Generated::BoardTop => "Top".to_string(),
            Generated::BoardBottom => "Bottom".to_string(),
            Generated::ChartFrame => "Frame".to_string(),
            Generated::Trace(n) => format!("Trace {n}"),
            Generated::Contours { negative: false } => "Positive contours".to_string(),
            Generated::Contours { negative: true } => "Negative contours".to_string(),
            Generated::Model3d => "3D model".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// The name in the file, or the English form of `generated`.
    pub name: String,
    /// Set when Specular invented the name; shown translated.
    pub generated: Option<Generated>,
    pub color: Color,
    /// Index into [`VectorScene::linetypes`]; `None` is continuous.
    pub linetype: Option<u32>,
    pub lineweight: Lineweight,
    /// Off in the file (negative colour number).
    pub on: bool,
    /// Frozen in the file (flag 1).
    pub frozen: bool,
    /// Plottable.
    pub plot: bool,
    /// How many items draw on it, in every sheet.
    pub items: u64,
}

impl Layer {
    /// Shown when the file was saved: on and not frozen.
    pub fn visible(&self) -> bool {
        self.on && !self.frozen
    }
}

/// A simple linetype: dash lengths in drawing units (positive a dash, negative a gap, zero
/// a dot). Shapes and text in complex linetypes are not drawn (their gaps are).
#[derive(Clone, Debug, PartialEq)]
pub struct Linetype {
    pub name: String,
    pub description: String,
    pub pattern: Vec<f64>,
}

impl Linetype {
    /// Length of one repetition.
    pub fn length(&self) -> f64 {
        self.pattern.iter().map(|d| d.abs()).sum()
    }

    /// Whether it draws as a plain line (no pattern, or no gaps).
    pub fn continuous(&self) -> bool {
        self.pattern.iter().all(|&d| d > 0.0) || self.length() <= 0.0
    }
}

/// A text style: which font its text needs.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FontSpec {
    /// The style's name ("Standard").
    pub style: String,
    /// The font file the style names ("arial.ttf", "romans.shx"); may be empty.
    pub file: String,
    /// The TrueType family name, when the file stores it (extended data).
    pub family: Option<String>,
    pub bold: bool,
    pub italic: bool,
    /// Width factor and oblique angle (degrees) of the style.
    pub width_factor: f64,
    pub oblique: f64,
    /// The Big Font file the style names for double-byte text ("bigfont.shx"); may be empty.
    pub bigfont: String,
    /// The SHX fonts found for this style ([`crate::fonts::attach_shx`]); `None` before
    /// they were looked for.
    pub shx: Option<ShxRef>,
}

impl FontSpec {
    /// An SHX (compiled shape) font.
    pub fn is_shx(&self) -> bool {
        let f = self.file.to_ascii_lowercase();
        f.ends_with(".shx") || (!f.is_empty() && !f.contains('.') && self.family.is_none())
    }
}

/// What the search for a style's SHX fonts found.
#[derive(Clone, Debug)]
pub struct ShxRef {
    /// The face that draws the style's text with its shapes; `None` when the main font was
    /// not found (the text is substituted).
    pub face: Option<std::sync::Arc<crate::fonts::Face>>,
    /// Where the main font and the Big Font were found.
    pub main: Option<std::path::PathBuf>,
    pub big: Option<std::path::PathBuf>,
    /// The SHX file names that were not found (or not readable).
    pub missing: Vec<String>,
}

impl PartialEq for ShxRef {
    fn eq(&self, o: &ShxRef) -> bool {
        self.main == o.main && self.big == o.big && self.missing == o.missing
    }
}

/// Model space or a layout.
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum SheetKind {
    Model,
    Layout,
}

/// Model space or one paper-space layout.
#[derive(Clone, Debug)]
pub struct Sheet {
    /// The name in the file, or the English form of `generated`.
    pub name: String,
    /// Set when Specular invented the name; shown translated.
    pub generated: Option<Generated>,
    pub kind: SheetKind,
    /// In draw order.
    pub items: Vec<Item>,
    /// Model-space views shown through the layout (none for model space).
    pub viewports: Vec<Viewport>,
    /// The sheet of paper, in paper units (layouts with a page setup).
    pub paper: Option<Rect>,
    /// Bounds of what is drawn, computed from the geometry (text by its insertion box).
    pub extents: Option<Rect>,
    /// The extents the file states (`$EXTMIN`/`$EXTMAX`, `$PEXTMIN`/`$PEXTMAX`), when valid.
    pub file_extents: Option<Rect>,
    /// Layout cells drawn after the items (sheet coordinates).
    pub refs: Vec<CellRef>,
    /// The sheet is a plot (a spectrum): its axes map sheet coordinates to values.
    pub chart: Option<Box<Chart>>,
    /// This sheet's own group opacities, over [`VectorScene::layer_opacity`] (a board's
    /// top view opaque where its layer view is translucent).
    pub layer_opacity: Option<Vec<f32>>,
    /// How this sheet's coordinates measure (CAD drawings).
    pub units: Option<SheetUnits>,
    /// The part of the file the sheet shows, when a file has several of the same views (an
    /// ODB++ job's step: "pcb", "panel").
    pub group: Option<String>,
}

/// How a CAD sheet's coordinates measure: its unit, the drawing's model unit (what its
/// viewports show) and how the drawing writes lengths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetUnits {
    /// The unit of this sheet's coordinates, as an `$INSUNITS` code (0: unknown): model
    /// space's unit, or a layout's paper unit (millimetres or inches).
    pub code: i16,
    /// Model space's unit code (`$INSUNITS`, else from `$MEASUREMENT`; 0: unknown).
    pub model: i16,
    /// `$LUNITS` (1 scientific, 2 decimal, 3 engineering, 4 architectural, 5 fractional) and
    /// `$LUPREC`.
    pub lunits: i16,
    pub luprec: i16,
    /// `$DIMSCALE`: the scale model space's dimensions were drawn for (1 when unset).
    pub dimscale: f64,
    /// A layout with its own plot scale (DXF plot settings 142/143, or the standard scale
    /// 147): millimetres of paper per sheet unit. `None`: the sheet is drawn in paper
    /// units (1:1).
    pub paper_mm: Option<f64>,
}

impl Default for SheetUnits {
    fn default() -> SheetUnits {
        SheetUnits {
            code: 0,
            model: 0,
            lunits: 2,
            luprec: 4,
            dimscale: 1.0,
            paper_mm: None,
        }
    }
}

impl SheetUnits {
    /// Metres per unit of an `$INSUNITS` code, for codes with a fixed length.
    pub fn metres(code: i16) -> Option<f64> {
        Some(match code {
            1 => 0.0254,
            2 => 0.3048,
            3 => 1609.344,
            4 => 1e-3,
            5 => 1e-2,
            6 => 1.0,
            7 => 1e3,
            8 => 0.0254e-6,
            9 => 0.0254e-3,
            10 => 0.9144,
            11 => 1e-10,
            12 => 1e-9,
            13 => 1e-6,
            14 => 0.1,
            15 => 10.0,
            16 => 100.0,
            17 => 1e9,
            21 => 1200.0 / 3937.0,
            _ => return None,
        })
    }
}

/// A plot drawn as a sheet: the traces are items in sheet coordinates, and the viewer draws
/// the axes, ticks and marks over them at the exact zoom.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chart {
    /// The plot area, sheet coordinates (y up).
    pub frame: Rect,
    pub x: ChartAxis,
    pub y: ChartAxis,
    /// Peak markers and other labelled points, in values.
    pub marks: Vec<ChartMark>,
    /// The legend: the layers of the traces (empty for a single trace).
    pub legend: Vec<u32>,
}

/// One axis of a [`Chart`]: the values at the frame's two ends.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChartAxis {
    /// What it shows, in English ("Wavenumber"), and its unit as written ("cm⁻¹"; may be
    /// empty).
    pub title: String,
    pub unit: String,
    /// The value at the frame's start (left or bottom) and at its end.
    pub start: f64,
    pub end: f64,
}

impl ChartAxis {
    /// The fraction 0–1 along the frame of value `v`.
    pub fn fraction(&self, v: f64) -> f64 {
        let d = self.end - self.start;
        if d == 0.0 { 0.5 } else { (v - self.start) / d }
    }

    /// The value at fraction `f` along the frame.
    pub fn value(&self, f: f64) -> f64 {
        self.start + (self.end - self.start) * f
    }
}

impl Chart {
    /// Sheet point of the value pair `(x, y)`.
    pub fn to_sheet(&self, x: f64, y: f64) -> Point {
        let f = self.frame;
        Point::new(
            f.x0 + self.x.fraction(x) * f.width(),
            f.y0 + self.y.fraction(y) * f.height(),
        )
    }

    /// Values at sheet point `p`.
    pub fn to_values(&self, p: Point) -> (f64, f64) {
        let f = self.frame;
        (
            self.x.value((p.x - f.x0) / f.width().max(1e-300)),
            self.y.value((p.y - f.y0) / f.height().max(1e-300)),
        )
    }
}

/// A labelled point of a [`Chart`] (a peak).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChartMark {
    pub x: f64,
    pub y: f64,
    /// Shown beside the marker; empty for none.
    pub label: String,
}

impl Sheet {
    pub fn new(name: impl Into<String>, kind: SheetKind) -> Sheet {
        Sheet {
            name: name.into(),
            generated: None,
            kind,
            items: Vec::new(),
            viewports: Vec::new(),
            paper: None,
            extents: None,
            file_extents: None,
            refs: Vec::new(),
            chart: None,
            layer_opacity: None,
            units: None,
            group: None,
        }
    }

    /// A sheet whose name Specular made up (its English form becomes `name`).
    pub fn generated(g: Generated, kind: SheetKind) -> Sheet {
        Sheet {
            generated: Some(g),
            ..Sheet::new(g.english(), kind)
        }
    }

    /// What fitting the sheet shows: the paper of a layout, else the computed extents,
    /// else the file's.
    pub fn fit_rect(&self) -> Option<Rect> {
        let e = self.extents.or(self.file_extents);
        match (self.kind, self.paper, e) {
            (SheetKind::Layout, Some(p), Some(e)) => Some(p.union(e)),
            (SheetKind::Layout, Some(p), None) => Some(p),
            _ => e,
        }
    }
}

/// A paper-space viewport onto model space.
#[derive(Clone, Debug, PartialEq)]
pub struct Viewport {
    /// Its rectangle on the paper.
    pub paper: Rect,
    /// Maps model space into paper space (scale, twist, view centre), already including
    /// the view direction's plan projection.
    pub transform: Affine,
    /// Layers frozen in this viewport only.
    pub frozen_layers: Vec<u32>,
    /// A non-rectangular clip boundary on the paper, when set.
    pub clip: Option<BezPath>,
}

/// One drawn thing.
#[derive(Clone, Debug)]
pub struct Item {
    /// The layer it is drawn on (for entities on layer `0` inside a block: the reference's).
    pub layer: u32,
    /// The layer of the outermost block reference, or `layer` for an entity outside blocks.
    pub outer: u32,
    pub color: Color,
    pub shape: Shape,
}

/// How a path is drawn.
#[derive(Clone, Debug)]
pub enum Shape {
    /// A line along the path.
    Stroke {
        path: BezPath,
        weight: Lineweight,
        /// A linetype pattern, `None` for continuous.
        dash: Option<Dash>,
    },
    /// The inside of a closed path.
    Fill { path: BezPath, even_odd: bool },
    /// Text, laid out with fonts when drawn.
    Text(Box<TextBlock>),
    /// A point entity, drawn as a dot of fixed screen size.
    Point(Point),
}

/// A linetype pattern on a stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dash {
    /// Index into [`VectorScene::linetypes`].
    pub linetype: u32,
    /// Pattern scale: `$LTSCALE` × the entity's own (and `$CELTSCALE`).
    pub scale: f64,
}

/// Horizontal alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
    /// Stretched between two points (TEXT "aligned" and "fit").
    Fit,
}

/// Vertical alignment: where the insertion point sits on the text block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VAlign {
    /// On the first line's baseline (TEXT).
    #[default]
    Baseline,
    /// Below the descenders (TEXT bottom).
    Bottom,
    Middle,
    Top,
}

/// A block of text: one line (TEXT, ATTRIB) or paragraphs that may wrap (MTEXT).
#[derive(Clone, Debug, PartialEq)]
pub struct TextBlock {
    /// Text space → drawing: origin at the insertion point, x along the baseline, y up,
    /// one unit a drawing unit. Rotation, mirroring and the extrusion's plan projection are
    /// in it; the width factor and oblique angle are per span.
    pub transform: Affine,
    pub halign: HAlign,
    pub valign: VAlign,
    /// Wrap width in drawing units (MTEXT reference rectangle), 0 for none.
    pub wrap: f64,
    /// For [`HAlign::Fit`]: the length to fill along the baseline, and whether the height
    /// scales with it ("aligned") or only the width ("fit").
    pub fit: Option<(f64, bool)>,
    /// Line spacing as a factor of the default (1.0 = 5/3 of the text height).
    pub line_spacing: f64,
    pub paragraphs: Vec<Paragraph>,
}

impl TextBlock {
    /// The plain text, paragraphs joined with newlines.
    pub fn plain(&self) -> String {
        let mut s = String::new();
        for (i, p) in self.paragraphs.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            for sp in &p.spans {
                s.push_str(&sp.text);
            }
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Paragraph {
    pub spans: Vec<Span>,
    /// Paragraph alignment (MTEXT `\pq`); the block's otherwise.
    pub align: Option<HAlign>,
}

/// A run of text in one style.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    /// Index into [`VectorScene::fonts`].
    pub font: u32,
    /// Cap height in drawing units (the DXF text height).
    pub height: f64,
    pub width_factor: f64,
    /// Oblique angle in degrees.
    pub oblique: f64,
    /// A colour of its own (MTEXT `\C`, `\c`), else the item's.
    pub color: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub overline: bool,
    pub strike: bool,
    /// A font family named inline (MTEXT `\f`), overriding the style's font.
    pub family: Option<String>,
}

impl Span {
    pub fn new(text: impl Into<String>, font: u32, height: f64) -> Span {
        Span {
            text: text.into(),
            font,
            height,
            width_factor: 1.0,
            oblique: 0.0,
            color: None,
            bold: false,
            italic: false,
            underline: false,
            overline: false,
            strike: false,
            family: None,
        }
    }
}

/// Bounds of a set of points, ignoring non-finite ones.
pub fn bounds_of(points: impl IntoIterator<Item = Point>) -> Option<Rect> {
    let mut r: Option<Rect> = None;
    for p in points {
        if !(p.x.is_finite() && p.y.is_finite()) {
            continue;
        }
        r = Some(match r {
            None => Rect::from_points(p, p),
            Some(r) => r.union_pt(p),
        });
    }
    r
}
