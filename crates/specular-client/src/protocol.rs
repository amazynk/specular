//! The requests of the IPC protocol (`docs/ipc.md`) and their wire encoding.
//!
//! One JSON object per line in each direction (UTF-8, `\n`-terminated, at most [`MAX_LINE`]
//! bytes). Request: `{"v":2, "id":…, "cmd":"…", …arguments}`. Response:
//! `{"v":2, "id":…, "ok":true, "result":{…}}` or `{"v":2, "id":…, "ok":false,
//! "error":{"code":"…", "message":"…"}}`. A version 1 request gets a version 1 response.
//!
//! Version 3 adds events: after `subscribe`, the viewer also sends event lines, which carry
//! `"event"` and no `"id"` (`{"v":3, "event":"marked", "seq":1043, "files":[…], …}`).

use std::path::PathBuf;

use serde_json::{Map, Value, json};

/// The newest protocol version.
pub const VERSION: u64 = 3;
/// Every version a viewer answers.
pub const VERSIONS: &[u64] = &[1, 2, 3];
/// The longest line, either way, without the newline.
pub const MAX_LINE: usize = 1 << 20;
/// The most files one `open` may name.
pub const MAX_FILES: usize = 10_000;
/// The most raw bytes one `blob-write` or `blob-read` carries (base64 makes it a third more).
pub const MAX_CHUNK: usize = 512 * 1024;
/// The most dimensions of an array.
pub const MAX_DIMS: usize = 8;
/// The most shapes one `overlay-add` may carry.
pub const MAX_SHAPES: usize = 10_000;
/// The most points one shape may have.
pub const MAX_POINTS: usize = 100_000;

/// The commands of version 1.
pub const COMMANDS_V1: &[&str] = &[
    "hello", "open", "open-url", "navigate", "state", "marked", "action", "activate", "quit",
];

/// The commands version 2 adds.
pub const COMMANDS_V2: &[&str] = &[
    "view-get",
    "view-set",
    "metadata",
    "blob-new",
    "blob-write",
    "blob-read",
    "blob-free",
    "array-show",
    "overlay-add",
    "overlay-clear",
    "overlay-list",
    "rois",
    "screenshot",
    "view-command",
];

/// The commands version 3 adds.
pub const COMMANDS_V3: &[&str] = &[
    "subscribe",
    "unsubscribe",
    "select",
    "mark",
    "apply-style",
    "export",
];

/// Test commands, answered by debug builds of the viewer only (release builds answer
/// `unknown-command`): `idle` waits until the window has settled, `ui-state` tells what is
/// open (the real-use runs, `tools/realuse/`).
pub const COMMANDS_TEST: &[&str] = &["idle", "ui-state"];

/// The events a client may subscribe to (`docs/ipc.md` §9).
pub const EVENTS: &[&str] = &[
    "opened",
    "marked",
    "keywords",
    "exported",
    "imported",
    "captured",
    "develop-saved",
];

/// Events every subscriber gets unasked: `ping` (every minute) and `lost` (events dropped
/// because the client read too slowly).
pub const CONTROL_EVENTS: &[&str] = &["ping", "lost"];

/// The most event names one `subscribe` may give.
pub const MAX_EVENT_NAMES: usize = 64;

/// The file next to a socket that holds the key a client sends in `hello`, where the viewer
/// asks for one (Windows, whose sockets have no peer check).
pub fn key_path(socket: &std::path::Path) -> PathBuf {
    let mut s = socket.as_os_str().to_owned();
    s.push(".key");
    PathBuf::from(s)
}

/// The pick flag `mark` sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Pick,
    Reject,
    /// Neither.
    None,
}

impl Flag {
    pub fn as_str(self) -> &'static str {
        match self {
            Flag::Pick => "pick",
            Flag::Reject => "reject",
            Flag::None => "none",
        }
    }

    pub fn from_key(s: &str) -> Option<Flag> {
        Some(match s {
            "pick" => Flag::Pick,
            "reject" => Flag::Reject,
            "none" => Flag::None,
            _ => return None,
        })
    }
}

/// The colour labels as `mark` and the events name them (`none` clears).
pub const LABELS: &[&str] = &["red", "yellow", "green", "blue"];

/// The name of label `l` (1 to 4), `None` for no label.
pub fn label_name(l: u8) -> Option<&'static str> {
    LABELS.get(usize::from(l).checked_sub(1)?).copied()
}

/// What `mark` changes; `None` leaves that part as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkSet {
    pub flag: Option<Flag>,
    /// 0 to 5.
    pub stars: Option<u8>,
    /// 0 clears; 1 to 4 are [`LABELS`].
    pub label: Option<u8>,
}

impl MarkSet {
    pub fn is_empty(&self) -> bool {
        *self == MarkSet::default()
    }
}

/// Where `open` puts the files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Current,
    New,
}

/// A `navigate` target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Next,
    Prev,
    First,
    Last,
    /// 0-based position in the listing.
    Index(usize),
    /// Steps forward (positive) or back.
    By(i64),
}

/// A zoom for `view-set`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    Fit,
    /// Percent of the image's pixels (100 = one image pixel per screen pixel).
    Percent(f64),
}

/// Window/level for `view-set`, in the image's own units (after any rescale, e.g. HU).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Levels {
    /// Back to the automatic range.
    Auto,
    Set {
        center: f64,
        width: f64,
    },
}

/// The channel shown, for `view-set`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// All channels (the composite).
    All,
    /// One channel, 0-based.
    Index(u32),
}

/// What `view-set` changes; `None` leaves that part as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewSet {
    pub zoom: Option<Zoom>,
    /// The image point at the centre of the view, in image pixels.
    pub center: Option<[f64; 2]>,
    pub levels: Option<Levels>,
    /// A colour map id (`hello` lists them under `capabilities.colormaps`).
    pub colormap: Option<String>,
    pub channel: Option<Channel>,
}

impl ViewSet {
    pub fn is_empty(&self) -> bool {
        *self == ViewSet::default()
    }
}

/// Where the pixels of `array-show` come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArraySource {
    /// A blob this connection filled (`blob-new`, `blob-write`).
    Blob(u64),
    /// A file the client wrote into the shared folder `hello` names (a bare file name).
    File(String),
}

/// The kind of an overlay shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// One point: a small cross.
    Point,
    /// Two points.
    Line,
    /// Two or more points, open.
    Polyline,
    /// Three or more points, closed.
    Polygon,
    /// Two opposite corners.
    Rect,
    /// One point: the text's top-left corner.
    Text,
}

impl ShapeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ShapeKind::Point => "point",
            ShapeKind::Line => "line",
            ShapeKind::Polyline => "polyline",
            ShapeKind::Polygon => "polygon",
            ShapeKind::Rect => "rect",
            ShapeKind::Text => "text",
        }
    }

    pub fn from_key(s: &str) -> Option<ShapeKind> {
        Some(match s {
            "point" => ShapeKind::Point,
            "line" => ShapeKind::Line,
            "polyline" => ShapeKind::Polyline,
            "polygon" => ShapeKind::Polygon,
            "rect" => ShapeKind::Rect,
            "text" => ShapeKind::Text,
            _ => return None,
        })
    }

    /// The fewest and most points a shape of this kind has.
    pub fn points(self) -> (usize, usize) {
        match self {
            ShapeKind::Point | ShapeKind::Text => (1, 1),
            ShapeKind::Line | ShapeKind::Rect => (2, 2),
            ShapeKind::Polyline => (2, MAX_POINTS),
            ShapeKind::Polygon => (3, MAX_POINTS),
        }
    }
}

/// One overlay shape, in image pixel coordinates (x right, y down, 0,0 the top-left corner of
/// the top-left pixel).
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    pub points: Vec<[f64; 2]>,
    /// The text of a `text` shape, else a label shown next to the shape (may be empty).
    pub text: String,
    /// Straight (not premultiplied) sRGB and alpha.
    pub color: [u8; 4],
    /// Line width in screen pixels.
    pub width: f32,
}

impl Shape {
    pub fn new(kind: ShapeKind, points: Vec<[f64; 2]>) -> Shape {
        Shape {
            kind,
            points,
            text: String::new(),
            color: [255, 255, 0, 255],
            width: 2.0,
        }
    }

    /// `#rrggbbaa`.
    pub fn color_hex(&self) -> String {
        let [r, g, b, a] = self.color;
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

/// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`.
pub fn parse_color(s: &str) -> Option<[u8; 4]> {
    let h = s.strip_prefix('#')?;
    if !h.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match h.len() {
        3 => {
            let n = |i: usize| u8::from_str_radix(h.get(i..=i)?, 16).ok().map(|v| v * 17);
            Some([n(0)?, n(1)?, n(2)?, 255])
        }
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

/// The form `rois` answers in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoiFormat {
    Json,
    GeoJson,
}

/// The language of `view-command`'s snippet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Shell,
    Python,
}

/// A request.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    // Version 1.
    Hello,
    Open {
        files: Vec<PathBuf>,
        window: Window,
        /// Version 2: the view to show the first file with, once it is shown.
        view: Option<ViewSet>,
        /// The files (2–4) compared in this view (`side`, `split`, `overlay`, `diff`)
        /// instead of opened; servers before it open them.
        compare: Option<String>,
    },
    OpenUrl {
        url: String,
    },
    Navigate(Nav),
    State,
    Marked,
    Action {
        name: String,
    },
    Activate,
    Quit,
    // Version 2.
    ViewGet,
    ViewSet(ViewSet),
    Metadata,
    BlobNew {
        bytes: u64,
    },
    BlobWrite {
        blob: u64,
        offset: u64,
        data: Vec<u8>,
    },
    BlobRead {
        blob: u64,
        offset: u64,
        length: u64,
    },
    BlobFree {
        blob: u64,
    },
    ArrayShow {
        source: ArraySource,
        /// Outermost first, as NumPy's `shape`.
        shape: Vec<u64>,
        /// A NumPy type string: `|u1`, `<u2`, `<f4`, `>i4`, `|b1`…
        dtype: String,
        /// Byte steps per dimension, as NumPy's `strides`; `None` is C order.
        strides: Option<Vec<i64>>,
        /// Shown as the file name (letters, digits, `-`, `_`, `.`, space).
        name: Option<String>,
    },
    OverlayAdd {
        layer: String,
        shapes: Vec<Shape>,
    },
    OverlayClear {
        /// `None`: every layer.
        layer: Option<String>,
    },
    OverlayList,
    Rois {
        format: RoiFormat,
    },
    Screenshot,
    ViewCommand {
        lang: Lang,
    },
    // Version 3.
    /// Events to receive on this connection (`*`: all).
    Subscribe {
        events: Vec<String>,
    },
    /// Events to stop receiving; `None`: all of them.
    Unsubscribe {
        events: Option<Vec<String>>,
    },
    /// The gallery's selection: these files, or with `add` these as well.
    Select {
        files: Vec<PathBuf>,
        add: bool,
    },
    Mark {
        files: Vec<PathBuf>,
        mark: MarkSet,
    },
    /// A Develop style by name, applied in the background.
    ApplyStyle {
        files: Vec<PathBuf>,
        style: String,
    },
    /// An export recipe by name, run in the background; `exported` says when it is done.
    Export {
        files: Vec<PathBuf>,
        recipe: String,
    },
    // Test commands ([`COMMANDS_TEST`]).
    /// Answered once nothing has drawn for `quiet_ms`, no timer is set, no tiles or decodes
    /// are on their way and the current image is sharp; or after `timeout_ms` with
    /// `"settled": false`.
    Idle {
        quiet_ms: u64,
        timeout_ms: u64,
    },
    /// What is open: the mode, dialogs, focus, background jobs, the Library's counts.
    UiState,
}

impl Request {
    /// The `cmd` member.
    pub fn cmd(&self) -> &'static str {
        match self {
            Request::Hello => "hello",
            Request::Open { .. } => "open",
            Request::OpenUrl { .. } => "open-url",
            Request::Navigate(_) => "navigate",
            Request::State => "state",
            Request::Marked => "marked",
            Request::Action { .. } => "action",
            Request::Activate => "activate",
            Request::Quit => "quit",
            Request::ViewGet => "view-get",
            Request::ViewSet(_) => "view-set",
            Request::Metadata => "metadata",
            Request::BlobNew { .. } => "blob-new",
            Request::BlobWrite { .. } => "blob-write",
            Request::BlobRead { .. } => "blob-read",
            Request::BlobFree { .. } => "blob-free",
            Request::ArrayShow { .. } => "array-show",
            Request::OverlayAdd { .. } => "overlay-add",
            Request::OverlayClear { .. } => "overlay-clear",
            Request::OverlayList => "overlay-list",
            Request::Rois { .. } => "rois",
            Request::Screenshot => "screenshot",
            Request::ViewCommand { .. } => "view-command",
            Request::Subscribe { .. } => "subscribe",
            Request::Unsubscribe { .. } => "unsubscribe",
            Request::Select { .. } => "select",
            Request::Mark { .. } => "mark",
            Request::ApplyStyle { .. } => "apply-style",
            Request::Export { .. } => "export",
            Request::Idle { .. } => "idle",
            Request::UiState => "ui-state",
        }
    }

    /// The first protocol version with this command.
    pub fn since(&self) -> u64 {
        since(self.cmd())
    }
}

/// The first protocol version with the command `cmd` (unknown ones: the newest).
pub fn since(cmd: &str) -> u64 {
    if COMMANDS_V1.contains(&cmd) {
        1
    } else if COMMANDS_V2.contains(&cmd) {
        2
    } else {
        VERSION
    }
}

/// A path as a protocol string (non-UTF-8 bytes replaced: such names cannot be named over
/// JSON exactly).
pub fn path_value(p: &std::path::Path) -> Value {
    Value::String(p.to_string_lossy().into_owned())
}

/// The arguments of a shape, as sent.
pub fn shape_value(s: &Shape) -> Value {
    let pts: Vec<Value> = s.points.iter().map(|p| json!([p[0], p[1]])).collect();
    let mut o = Map::new();
    o.insert("kind".into(), json!(s.kind.as_str()));
    o.insert("points".into(), Value::Array(pts));
    if !s.text.is_empty() {
        o.insert("text".into(), json!(s.text));
    }
    o.insert("color".into(), json!(s.color_hex()));
    o.insert("width".into(), json!(s.width));
    Value::Object(o)
}

/// The members of a `view-set` (also `open`'s `view`).
pub fn view_value(s: &ViewSet) -> Map<String, Value> {
    let mut o = Map::new();
    match s.zoom {
        Some(Zoom::Fit) => {
            o.insert("zoom".into(), json!("fit"));
        }
        Some(Zoom::Percent(p)) => {
            o.insert("zoom".into(), json!(p));
        }
        None => {}
    }
    if let Some(c) = s.center {
        o.insert("center".into(), json!([c[0], c[1]]));
    }
    match s.levels {
        Some(Levels::Auto) => {
            o.insert("window".into(), json!("auto"));
        }
        Some(Levels::Set { center, width }) => {
            o.insert("window".into(), json!({"center": center, "width": width}));
        }
        None => {}
    }
    if let Some(m) = &s.colormap {
        o.insert("colormap".into(), json!(m));
    }
    match s.channel {
        Some(Channel::All) => {
            o.insert("channel".into(), json!("all"));
        }
        Some(Channel::Index(i)) => {
            o.insert("channel".into(), json!(i));
        }
        None => {}
    }
    o
}

/// The request line for `req` with protocol version `v` (a client normally sends
/// [`VERSION`]; version 1 suits the version 1 commands only).
pub fn request_line_v(v: u64, id: u64, req: &Request) -> String {
    let mut o = Map::new();
    o.insert("v".into(), json!(v));
    o.insert("id".into(), json!(id));
    let mut put = |k: &str, val: Value| {
        o.insert(k.into(), val);
    };
    let paths = |files: &[PathBuf]| Value::Array(files.iter().map(|p| path_value(p)).collect());
    match req {
        Request::Hello
        | Request::State
        | Request::Marked
        | Request::Activate
        | Request::Quit
        | Request::ViewGet
        | Request::Metadata
        | Request::OverlayList
        | Request::Screenshot
        | Request::UiState => {}
        Request::Open {
            files,
            window,
            view,
            compare,
        } => {
            let files: Vec<Value> = files.iter().map(|p| path_value(p)).collect();
            put("files", Value::Array(files));
            if *window == Window::New {
                put("window", json!("new"));
            }
            if let Some(s) = view {
                put("view", Value::Object(view_value(s)));
            }
            if let Some(m) = compare {
                put("compare", json!(m));
            }
        }
        Request::OpenUrl { url } => put("url", json!(url)),
        Request::Navigate(n) => match n {
            Nav::Next => put("to", json!("next")),
            Nav::Prev => put("to", json!("prev")),
            Nav::First => put("to", json!("first")),
            Nav::Last => put("to", json!("last")),
            Nav::Index(i) => put("index", json!(i)),
            Nav::By(d) => put("by", json!(d)),
        },
        Request::Action { name } => put("name", json!(name)),
        Request::ViewSet(s) => {
            for (k, v) in view_value(s) {
                put(&k, v);
            }
        }
        Request::BlobNew { bytes } => put("bytes", json!(bytes)),
        Request::BlobWrite { blob, offset, data } => {
            put("blob", json!(blob));
            put("offset", json!(offset));
            put("data", json!(crate::base64::encode(data)));
        }
        Request::BlobRead {
            blob,
            offset,
            length,
        } => {
            put("blob", json!(blob));
            put("offset", json!(offset));
            put("length", json!(length));
        }
        Request::BlobFree { blob } => put("blob", json!(blob)),
        Request::ArrayShow {
            source,
            shape,
            dtype,
            strides,
            name,
        } => {
            match source {
                ArraySource::Blob(b) => put("blob", json!(b)),
                ArraySource::File(f) => put("file", json!(f)),
            }
            put("shape", json!(shape));
            put("dtype", json!(dtype));
            if let Some(s) = strides {
                put("strides", json!(s));
            }
            if let Some(n) = name {
                put("name", json!(n));
            }
        }
        Request::OverlayAdd { layer, shapes } => {
            put("layer", json!(layer));
            put(
                "shapes",
                Value::Array(shapes.iter().map(shape_value).collect()),
            );
        }
        Request::OverlayClear { layer } => {
            if let Some(l) = layer {
                put("layer", json!(l));
            }
        }
        Request::Rois { format } => put(
            "format",
            json!(match format {
                RoiFormat::Json => "json",
                RoiFormat::GeoJson => "geojson",
            }),
        ),
        Request::ViewCommand { lang } => put(
            "lang",
            json!(match lang {
                Lang::Shell => "shell",
                Lang::Python => "python",
            }),
        ),
        Request::Subscribe { events } => put("events", json!(events)),
        Request::Unsubscribe { events } => {
            if let Some(e) = events {
                put("events", json!(e));
            }
        }
        Request::Select { files, add } => {
            put("files", paths(files));
            if *add {
                put("add", json!(true));
            }
        }
        Request::Mark { files, mark } => {
            put("files", paths(files));
            if let Some(f) = mark.flag {
                put("flag", json!(f.as_str()));
            }
            if let Some(s) = mark.stars {
                put("stars", json!(s));
            }
            if let Some(l) = mark.label {
                put("label", json!(label_name(l).unwrap_or("none")));
            }
        }
        Request::ApplyStyle { files, style } => {
            put("files", paths(files));
            put("style", json!(style));
        }
        Request::Export { files, recipe } => {
            put("files", paths(files));
            put("recipe", json!(recipe));
        }
        Request::Idle {
            quiet_ms,
            timeout_ms,
        } => {
            put("quiet_ms", json!(quiet_ms));
            put("timeout_ms", json!(timeout_ms));
        }
    }
    o.insert("cmd".into(), json!(req.cmd()));
    Value::Object(o).to_string()
}

/// The request line for `req` in the newest version.
pub fn request_line(id: u64, req: &Request) -> String {
    request_line_v(VERSION, id, req)
}

/// A `hello` line that also carries the viewer's key (read from [`key_path`]) and, from a
/// program a hook started, the hook's name (`$SPECULAR_HOOK`): what this connection does is
/// then reported as the hook's, which keeps hooks from setting each other off.
pub fn hello_line(v: u64, id: u64, key: Option<&str>, hook: Option<&str>) -> String {
    let mut o = Map::new();
    o.insert("v".into(), json!(v));
    o.insert("id".into(), json!(id));
    o.insert("cmd".into(), json!("hello"));
    if let Some(k) = key {
        o.insert("key".into(), json!(k));
    }
    if let Some(h) = hook {
        o.insert("hook".into(), json!(h));
    }
    Value::Object(o).to_string()
}

/// Whether a line the viewer sent is an event (it has `"event"` and no `"id"`), not a
/// response.
pub fn is_event(v: &Value) -> bool {
    v.get("event").is_some_and(Value::is_string) && v.get("id").is_none()
}

/// What a client reads from `socket`'s key file, and the hook it runs under, for its first
/// `hello` (`None`, `None`: no `hello` is needed first).
pub fn introduction(socket: &std::path::Path) -> (Option<String>, Option<String>) {
    let key = std::fs::read_to_string(key_path(socket))
        .ok()
        .map(|k| k.trim().to_owned())
        .filter(|k| !k.is_empty() && k.len() <= 256);
    let hook = std::env::var("SPECULAR_HOOK")
        .ok()
        .filter(|h| !h.is_empty() && h.len() <= 256);
    (key, hook)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(parse_color("#f00"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("#00ff0080"), Some([0, 255, 0, 128]));
        assert_eq!(parse_color("#0a0B0c"), Some([10, 11, 12, 255]));
        for bad in ["f00", "#ff", "#gg0000", "#ffé", "#12345"] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
        let mut s = Shape::new(ShapeKind::Point, vec![[1.0, 2.0]]);
        s.color = [1, 2, 3, 4];
        assert_eq!(parse_color(&s.color_hex()), Some([1, 2, 3, 4]));
    }

    #[test]
    fn versions_and_commands() {
        assert_eq!(Request::Hello.since(), 1);
        assert_eq!(Request::Screenshot.since(), 2);
        assert_eq!(Request::Unsubscribe { events: None }.since(), 3);
        assert!(COMMANDS_V2.iter().all(|c| !COMMANDS_V1.contains(c)));
        assert!(
            COMMANDS_V3
                .iter()
                .all(|c| !COMMANDS_V1.contains(c) && !COMMANDS_V2.contains(c) && since(c) == 3)
        );
        let line = request_line_v(2, 7, &Request::ViewGet);
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v, json!({"v": 2, "id": 7, "cmd": "view-get"}));
        let v: Value = serde_json::from_str(&request_line(7, &Request::ViewGet)).unwrap();
        assert_eq!(v["v"], 3);
        let v: Value =
            serde_json::from_str(&request_line_v(1, 1, &Request::Navigate(Nav::By(-2)))).unwrap();
        assert_eq!(v, json!({"v": 1, "id": 1, "cmd": "navigate", "by": -2}));
    }

    #[test]
    fn version_3_lines() {
        let line = |r: &Request| serde_json::from_str::<Value>(&request_line(1, r)).unwrap();
        assert_eq!(
            line(&Request::Subscribe {
                events: vec!["marked".into(), "exported".into()]
            }),
            json!({"v": 3, "id": 1, "cmd": "subscribe", "events": ["marked", "exported"]})
        );
        assert_eq!(
            line(&Request::Unsubscribe { events: None }),
            json!({"v": 3, "id": 1, "cmd": "unsubscribe"})
        );
        let files = vec![PathBuf::from("/p/a.cr3")];
        assert_eq!(
            line(&Request::Mark {
                files: files.clone(),
                mark: MarkSet {
                    flag: Some(Flag::Pick),
                    stars: Some(4),
                    label: Some(0),
                },
            }),
            json!({"v": 3, "id": 1, "cmd": "mark", "files": ["/p/a.cr3"], "flag": "pick",
                   "stars": 4, "label": "none"})
        );
        let l = line(&Request::Mark {
            files: files.clone(),
            mark: MarkSet {
                label: Some(2),
                ..MarkSet::default()
            },
        });
        assert_eq!((l["label"].clone(), l.get("flag")), (json!("yellow"), None));
        assert_eq!(
            line(&Request::Select {
                files: files.clone(),
                add: true
            })["add"],
            true
        );
        assert_eq!(
            line(&Request::Export {
                files: files.clone(),
                recipe: "Web".into()
            })["recipe"],
            "Web"
        );
        assert_eq!(
            line(&Request::ApplyStyle {
                files,
                style: "Mono".into()
            })["style"],
            "Mono"
        );
        let h: Value = serde_json::from_str(&hello_line(3, 0, Some("k"), None)).unwrap();
        assert_eq!(h, json!({"v": 3, "id": 0, "cmd": "hello", "key": "k"}));
        assert!(is_event(&json!({"v": 3, "event": "ping", "seq": 1})));
        assert!(!is_event(&json!({"v": 3, "id": 1, "ok": true})));
        assert_eq!(
            key_path(std::path::Path::new("/r/specular.sock")),
            PathBuf::from("/r/specular.sock.key")
        );
        assert_eq!(Flag::from_key("reject"), Some(Flag::Reject));
        assert_eq!(label_name(4), Some("blue"));
        assert_eq!(label_name(0), None);
    }
}
