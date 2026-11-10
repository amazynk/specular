//! The IPC protocol, versions 1 to 3 (the published spec: `docs/ipc.md`): one JSON object
//! per line (UTF-8, `\n`-terminated, at most [`MAX_LINE`] bytes) in each direction. Every
//! request gets exactly one response, in order, in the request's version. A connection that
//! subscribed (version 3) also gets event lines, which carry `"event"` and no `"id"`.
//!
//! Request: `{"v":2, "id":<optional number or string>, "cmd":"<command>", …arguments}`
//!
//! Response: `{"v":2, "id":<the request's id or null>, "ok":true, "result":{…}}`, or
//! `{"v":2, "id":…, "ok":false, "error":{"code":"<code>", "message":"<English text>"}}`.
//!
//! The request types and their encoding are shared with the client crate
//! (`specular_client::protocol`); parsing, with its limits, is the viewer's and lives here.
//! Version 1 requests are answered as before; a version 2 command sent as version 1 (or a
//! version 3 command as 1 or 2) is a `version` error. Unknown members of a request are ignored (forward compatible); unknown
//! commands are errors.

use std::path::PathBuf;

use serde_json::{Map, Value, json};
pub use specular_client::protocol::request_line;
#[cfg(test)]
pub use specular_client::protocol::request_line_v;
pub use specular_client::protocol::{
    ArraySource, COMMANDS_V1, COMMANDS_V2, COMMANDS_V3, Channel, EVENTS, Flag, LABELS, Lang,
    Levels, MAX_CHUNK, MAX_DIMS, MAX_EVENT_NAMES, MAX_FILES, MAX_POINTS, MAX_SHAPES, MarkSet, Nav,
    Request, RoiFormat, Shape, ShapeKind, VERSION, VERSIONS, ViewSet, Window, Zoom, label_name,
    parse_color, path_value,
};

/// The longest line accepted (requests) or sent (responses), without the newline.
pub const MAX_LINE: usize = specular_platform::ipc::MAX_LINE;
const _: () = assert!(MAX_LINE == specular_client::protocol::MAX_LINE);

/// The longest string argument (a path, a URL, an action id), in bytes.
const MAX_STRING: usize = 16 * 1024;
/// The longest overlay text or layer name, in bytes.
const MAX_TEXT: usize = 1024;
/// The most points one `overlay-add` may carry in all.
pub const MAX_ADD_POINTS: usize = 1_000_000;
/// The largest blob or array, in bytes (the memory budget may allow less).
pub const MAX_BLOB: u64 = 4 << 30;

/// The NumPy types `array-show` accepts (any byte order).
pub const DTYPES: &[&str] = &[
    "b1", "u1", "i1", "u2", "i2", "u4", "i4", "u8", "i8", "f4", "f8",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Parse,
    Version,
    TooLong,
    UnknownCommand,
    BadArguments,
    UnknownAction,
    Disabled,
    NotFound,
    Refused,
    Mode,
    Busy,
    Timeout,
    NoMemory,
    Internal,
    UnknownEvent,
    UnknownStyle,
    UnknownRecipe,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Parse => "parse",
            Code::Version => "version",
            Code::TooLong => "too-long",
            Code::UnknownCommand => "unknown-command",
            Code::BadArguments => "bad-arguments",
            Code::UnknownAction => "unknown-action",
            Code::Disabled => "disabled",
            Code::NotFound => "not-found",
            Code::Refused => "refused",
            Code::Mode => "mode",
            Code::Busy => "busy",
            Code::Timeout => "timeout",
            Code::NoMemory => "no-memory",
            Code::Internal => "internal",
            Code::UnknownEvent => "unknown-event",
            Code::UnknownStyle => "unknown-style",
            Code::UnknownRecipe => "unknown-recipe",
        }
    }
}

/// An error response.
#[derive(Clone, Debug, PartialEq)]
pub struct Fail {
    pub code: Code,
    pub message: String,
}

impl Fail {
    pub fn new(code: Code, message: impl Into<String>) -> Fail {
        Fail {
            code,
            message: message.into(),
        }
    }
}

/// What a handler answers: a `result` object or an error.
pub type Reply = Result<Value, Fail>;

/// A request, the version it came in and the id to answer with.
#[derive(Clone, Debug, PartialEq)]
pub struct Envelope {
    pub v: u64,
    pub id: Value,
    pub req: Request,
}

/// Parses one request line. On error, the version and id (when they could be read) for the
/// response.
pub fn parse_request(line: &[u8]) -> Result<Envelope, (u64, Value, Fail)> {
    if line.len() > MAX_LINE {
        return Err((
            VERSION,
            Value::Null,
            Fail::new(Code::TooLong, "request too long"),
        ));
    }
    let v: Value = serde_json::from_slice(line).map_err(|e| {
        (
            VERSION,
            Value::Null,
            Fail::new(Code::Parse, format!("not JSON: {e}")),
        )
    })?;
    let Value::Object(o) = v else {
        return Err((
            VERSION,
            Value::Null,
            Fail::new(Code::Parse, "a request is a JSON object"),
        ));
    };
    let id = match o.get("id") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::Number(n)) => Value::Number(n.clone()),
        Some(Value::String(s)) if s.len() <= 256 => Value::String(s.clone()),
        Some(_) => {
            return Err((
                VERSION,
                Value::Null,
                Fail::new(Code::BadArguments, "id must be a number or a short string"),
            ));
        }
    };
    let version = match o.get("v") {
        Some(Value::Number(n)) => match n.as_u64().filter(|v| VERSIONS.contains(v)) {
            Some(v) => v,
            None => {
                return Err((
                    VERSION,
                    id,
                    Fail::new(
                        Code::Version,
                        "unsupported protocol version (this viewer speaks 1, 2 and 3)",
                    ),
                ));
            }
        },
        _ => {
            return Err((
                VERSION,
                id,
                Fail::new(Code::BadArguments, "missing \"v\" (1, 2 or 3)"),
            ));
        }
    };
    let fail = |code, msg: &str| Err((version, id.clone(), Fail::new(code, msg)));
    let Some(Value::String(cmd)) = o.get("cmd") else {
        return fail(Code::BadArguments, "missing \"cmd\"");
    };
    if version < 2 && COMMANDS_V2.contains(&cmd.as_str()) {
        return fail(Code::Version, "this command needs protocol version 2");
    }
    if version < 3
        && (COMMANDS_V3.contains(&cmd.as_str())
            || specular_client::protocol::COMMANDS_TEST.contains(&cmd.as_str()))
    {
        return fail(Code::Version, "this command needs protocol version 3");
    }
    let req = command(cmd, &o, version).map_err(|f| (version, id.clone(), f))?;
    Ok(Envelope {
        v: version,
        id,
        req,
    })
}

fn command(cmd: &str, o: &Map<String, Value>, version: u64) -> Result<Request, Fail> {
    Ok(match cmd {
        "hello" => Request::Hello,
        "open" => open(o, version)?,
        "open-url" => Request::OpenUrl {
            url: string(o, "url")?,
        },
        "navigate" => Request::Navigate(navigate(o)?),
        "state" => Request::State,
        "marked" => Request::Marked,
        "action" => Request::Action {
            name: string(o, "name")?,
        },
        "activate" => Request::Activate,
        "quit" => Request::Quit,
        "view-get" => Request::ViewGet,
        "view-set" => Request::ViewSet(view_set(o)?),
        "metadata" => Request::Metadata,
        "blob-new" => Request::BlobNew {
            bytes: whole(o, "bytes")?
                .filter(|&b| (1..=MAX_BLOB).contains(&b))
                .ok_or_else(|| bad(format!("\"bytes\" is 1 to {MAX_BLOB}")))?,
        },
        "blob-write" => {
            let data = match o.get("data") {
                Some(Value::String(s)) if s.len() <= MAX_CHUNK.div_ceil(3) * 4 => {
                    specular_client::base64::decode(s)
                        .ok_or_else(|| bad("\"data\" is not base64"))?
                }
                Some(Value::String(_)) => {
                    return Err(bad(format!("\"data\" holds at most {MAX_CHUNK} bytes")));
                }
                _ => return Err(bad("\"data\" must be a base64 string")),
            };
            Request::BlobWrite {
                blob: required(o, "blob")?,
                offset: required(o, "offset")?,
                data,
            }
        }
        "blob-read" => Request::BlobRead {
            blob: required(o, "blob")?,
            offset: required(o, "offset")?,
            length: required(o, "length")?.min(MAX_CHUNK as u64).max(1),
        },
        "blob-free" => Request::BlobFree {
            blob: required(o, "blob")?,
        },
        "array-show" => array_show(o)?,
        "overlay-add" => overlay_add(o)?,
        "overlay-clear" => Request::OverlayClear {
            layer: match o.get("layer") {
                None | Some(Value::Null) => None,
                Some(_) => Some(layer(o)?),
            },
        },
        "overlay-list" => Request::OverlayList,
        "rois" => Request::Rois {
            format: match o.get("format").and_then(Value::as_str) {
                None if o.get("format").is_none_or(Value::is_null) => RoiFormat::GeoJson,
                Some("geojson") => RoiFormat::GeoJson,
                Some("json") => RoiFormat::Json,
                _ => return Err(bad("\"format\" is \"json\" or \"geojson\"")),
            },
        },
        "screenshot" => Request::Screenshot,
        "view-command" => Request::ViewCommand {
            lang: match o.get("lang").and_then(Value::as_str) {
                None if o.get("lang").is_none_or(Value::is_null) => Lang::Shell,
                Some("shell") => Lang::Shell,
                Some("python") => Lang::Python,
                _ => return Err(bad("\"lang\" is \"shell\" or \"python\"")),
            },
        },
        "subscribe" => Request::Subscribe {
            events: event_names(o)?
                .ok_or_else(|| bad("\"events\" must be a list of event names (or [\"*\"])"))?,
        },
        "unsubscribe" => Request::Unsubscribe {
            events: event_names(o)?,
        },
        "select" => Request::Select {
            files: files(o)?,
            add: match o.get("add") {
                None | Some(Value::Null) => false,
                Some(Value::Bool(b)) => *b,
                Some(_) => return Err(bad("\"add\" is true or false")),
            },
        },
        "mark" => mark(o)?,
        "apply-style" => Request::ApplyStyle {
            files: files(o)?,
            style: string(o, "style")?,
        },
        "export" => Request::Export {
            files: files(o)?,
            recipe: string(o, "recipe")?,
        },
        // Test commands: debug builds only (`tools/realuse/`).
        #[cfg(debug_assertions)]
        "idle" => Request::Idle {
            quiet_ms: whole(o, "quiet_ms")?.unwrap_or(1000).min(60_000),
            timeout_ms: whole(o, "timeout_ms")?
                .unwrap_or(30_000)
                .min(crate::remote::server::IDLE_MAX_MS),
        },
        #[cfg(debug_assertions)]
        "ui-state" => Request::UiState,
        _ => return Err(Fail::new(Code::UnknownCommand, "unknown command")),
    })
}

/// What a `hello` line may carry besides the command: the viewer's key (where it asks for
/// one) and the hook the client runs under (a short name of printable characters).
pub fn hello_extras(line: &[u8]) -> (Option<String>, Option<String>) {
    let Ok(Value::Object(o)) = serde_json::from_slice::<Value>(line) else {
        return (None, None);
    };
    let text = |k: &str| {
        o.get(k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
            .map(str::to_owned)
    };
    (text("key"), text("hook"))
}

/// `events`: names from [`EVENTS`] or `*`; `None` when absent.
fn event_names(o: &Map<String, Value>) -> Result<Option<Vec<String>>, Fail> {
    let list = match o.get("events") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Array(l)) => l,
        Some(_) => return Err(bad("\"events\" must be a list of event names")),
    };
    if list.is_empty() || list.len() > MAX_EVENT_NAMES {
        return Err(bad(format!(
            "\"events\" holds 1 to {MAX_EVENT_NAMES} names"
        )));
    }
    let mut out = Vec::with_capacity(list.len());
    for e in list {
        let Some(name) = e.as_str() else {
            return Err(bad("\"events\" must be a list of strings"));
        };
        if name != "*" && !EVENTS.contains(&name) {
            let shown: String = name.chars().take(64).collect();
            return Err(Fail::new(
                Code::UnknownEvent,
                format!("no event {shown:?} (events: {})", EVENTS.join(", ")),
            ));
        }
        if !out.iter().any(|x| x == name) {
            out.push(name.to_owned());
        }
    }
    Ok(Some(out))
}

/// `mark`'s arguments.
fn mark(o: &Map<String, Value>) -> Result<Request, Fail> {
    let files = files(o)?;
    let mut m = MarkSet::default();
    match o.get("flag") {
        None => {}
        Some(Value::Null) => m.flag = Some(Flag::None),
        Some(Value::String(s)) => {
            m.flag =
                Some(Flag::from_key(s).ok_or_else(|| bad("\"flag\" is pick, reject or none"))?);
        }
        Some(_) => return Err(bad("\"flag\" is pick, reject or none")),
    }
    match o.get("stars") {
        None => {}
        Some(Value::Null) => m.stars = Some(0),
        Some(v) => {
            m.stars = Some(
                v.as_u64()
                    .filter(|s| *s <= 5)
                    .ok_or_else(|| bad("\"stars\" is 0 to 5"))? as u8,
            );
        }
    }
    match o.get("label") {
        None => {}
        Some(Value::Null) => m.label = Some(0),
        Some(Value::String(s)) if s == "none" => m.label = Some(0),
        Some(Value::String(s)) => {
            let i = LABELS
                .iter()
                .position(|l| l.eq_ignore_ascii_case(s))
                .ok_or_else(|| bad("\"label\" is red, yellow, green, blue or none"))?;
            m.label = Some(i as u8 + 1);
        }
        Some(_) => return Err(bad("\"label\" is red, yellow, green, blue or none")),
    }
    if m.is_empty() {
        return Err(bad("give at least one of flag, stars, label"));
    }
    Ok(Request::Mark { files, mark: m })
}

fn bad(msg: impl Into<String>) -> Fail {
    Fail::new(Code::BadArguments, msg)
}

fn string(o: &Map<String, Value>, key: &str) -> Result<String, Fail> {
    match o.get(key) {
        Some(Value::String(s)) if s.is_empty() => Err(bad(format!("\"{key}\" is empty"))),
        Some(Value::String(s)) if s.len() > MAX_STRING => Err(bad(format!("\"{key}\" too long"))),
        Some(Value::String(s)) if s.contains('\0') => Err(bad(format!("\"{key}\" contains NUL"))),
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(bad(format!("\"{key}\" must be a string"))),
    }
}

/// A whole number from 0, `None` when absent.
fn whole(o: &Map<String, Value>, key: &str) -> Result<Option<u64>, Fail> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| bad(format!("\"{key}\" is a whole number from 0"))),
    }
}

fn required(o: &Map<String, Value>, key: &str) -> Result<u64, Fail> {
    whole(o, key)?.ok_or_else(|| bad(format!("missing \"{key}\"")))
}

fn finite(v: &Value) -> Option<f64> {
    v.as_f64().filter(|f| f.is_finite())
}

/// A point: `[x, y]`, finite and within ±1e9 pixels.
fn point(v: &Value) -> Option<[f64; 2]> {
    let [x, y] = v.as_array()?.as_slice() else {
        return None;
    };
    let p = [finite(x)?, finite(y)?];
    p.iter().all(|c| c.abs() <= 1e9).then_some(p)
}

/// The views `open`'s `compare` names.
pub const COMPARE_MODES: [&str; 4] = ["side", "split", "overlay", "diff"];

fn open(o: &Map<String, Value>, version: u64) -> Result<Request, Fail> {
    let window = match o.get("window") {
        None | Some(Value::Null) => Window::Current,
        Some(Value::String(s)) if s == "current" => Window::Current,
        Some(Value::String(s)) if s == "new" => Window::New,
        _ => return Err(bad("\"window\" is \"current\" or \"new\"")),
    };
    let files = files(o)?;
    // Version 1 ignores unknown members, `view` among them.
    let view = match o.get("view") {
        _ if version < 2 => None,
        None | Some(Value::Null) => None,
        Some(Value::Object(v)) => Some(view_set(v)?),
        Some(_) => return Err(bad("\"view\" is an object like view-set's arguments")),
    };
    // `compare`: the files are compared (2–4 of them) in this view.
    let compare = match o.get("compare") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if COMPARE_MODES.contains(&s.as_str()) => {
            if !(2..=4).contains(&files.len()) {
                return Err(bad("\"compare\" takes 2 to 4 files"));
            }
            Some(s.clone())
        }
        Some(_) => {
            return Err(bad(
                "\"compare\" is \"side\", \"split\", \"overlay\" or \"diff\"",
            ));
        }
    };
    Ok(Request::Open {
        files,
        window,
        view,
        compare,
    })
}

/// `files`: 1 to [`MAX_FILES`] absolute paths (or `file://` URLs).
fn files(o: &Map<String, Value>) -> Result<Vec<PathBuf>, Fail> {
    let Some(Value::Array(list)) = o.get("files") else {
        return Err(bad("\"files\" must be an array of paths"));
    };
    if list.is_empty() || list.len() > MAX_FILES {
        return Err(bad(format!("\"files\" holds 1 to {MAX_FILES} paths")));
    }
    let mut files = Vec::with_capacity(list.len());
    for f in list {
        let Value::String(s) = f else {
            return Err(bad("\"files\" must be an array of strings"));
        };
        if s.is_empty() || s.len() > MAX_STRING || s.contains('\0') {
            return Err(bad("an empty, overlong or NUL-containing path"));
        }
        let p = if s.len() >= 7 && s[..7].eq_ignore_ascii_case("file://") {
            specular_platform::launch::path_from_uri(&format!("file://{}", &s[7..]))
                .ok_or_else(|| bad("a file:// URL that is not a local path"))?
        } else {
            PathBuf::from(s)
        };
        if !p.is_absolute() {
            return Err(bad("paths must be absolute"));
        }
        files.push(p);
    }
    Ok(files)
}

fn navigate(o: &Map<String, Value>) -> Result<Nav, Fail> {
    let given = ["to", "index", "by"]
        .iter()
        .filter(|k| o.get(**k).is_some_and(|v| !v.is_null()))
        .count();
    if given != 1 {
        return Err(bad("give exactly one of \"to\", \"index\", \"by\""));
    }
    if let Some(Value::String(s)) = o.get("to") {
        return match s.as_str() {
            "next" => Ok(Nav::Next),
            "prev" | "previous" => Ok(Nav::Prev),
            "first" => Ok(Nav::First),
            "last" => Ok(Nav::Last),
            _ => Err(bad("\"to\" is next, prev, first or last")),
        };
    }
    if let Some(v) = o.get("index") {
        return v
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .map(Nav::Index)
            .ok_or_else(|| bad("\"index\" is a whole number from 0"));
    }
    if let Some(v) = o.get("by") {
        return v
            .as_i64()
            .filter(|n| n.unsigned_abs() <= 1 << 31)
            .map(Nav::By)
            .ok_or_else(|| bad("\"by\" is a whole number"));
    }
    Err(bad("\"to\" is next, prev, first or last"))
}

fn view_set(o: &Map<String, Value>) -> Result<ViewSet, Fail> {
    let mut s = ViewSet::default();
    match o.get("zoom") {
        None | Some(Value::Null) => {}
        Some(Value::String(z)) if z == "fit" => s.zoom = Some(Zoom::Fit),
        Some(v) => {
            let p = finite(v)
                .filter(|p| (0.1..=100_000.0).contains(p))
                .ok_or_else(|| bad("\"zoom\" is \"fit\" or a percentage from 0.1"))?;
            s.zoom = Some(Zoom::Percent(p));
        }
    }
    match o.get("center") {
        None | Some(Value::Null) => {}
        Some(v) => s.center = Some(point(v).ok_or_else(|| bad("\"center\" is [x, y]"))?),
    }
    match o.get("window") {
        None | Some(Value::Null) => {}
        Some(Value::String(w)) if w == "auto" => s.levels = Some(Levels::Auto),
        Some(Value::Object(w)) => {
            let c = w.get("center").and_then(finite);
            let wd = w.get("width").and_then(finite).filter(|w| *w > 0.0);
            let (Some(center), Some(width)) = (c, wd) else {
                return Err(bad(
                    "\"window\" is \"auto\" or {\"center\": c, \"width\": w > 0}",
                ));
            };
            s.levels = Some(Levels::Set { center, width });
        }
        Some(_) => {
            return Err(bad(
                "\"window\" is \"auto\" or {\"center\": c, \"width\": w > 0}",
            ));
        }
    }
    match o.get("colormap") {
        None | Some(Value::Null) => {}
        Some(Value::String(m)) if !m.is_empty() && m.len() <= 64 => {
            s.colormap = Some(m.clone());
        }
        Some(_) => return Err(bad("\"colormap\" is a colour map id or \"none\"")),
    }
    match o.get("channel") {
        None | Some(Value::Null) => {}
        Some(Value::String(c)) if c == "all" => s.channel = Some(Channel::All),
        Some(v) => {
            let k = v
                .as_u64()
                .filter(|k| *k < 1024)
                .ok_or_else(|| bad("\"channel\" is \"all\" or a channel number from 0"))?;
            s.channel = Some(Channel::Index(k as u32));
        }
    }
    if s.is_empty() {
        return Err(bad(
            "give at least one of zoom, center, window, colormap, channel",
        ));
    }
    Ok(s)
}

/// Whether `d` is a NumPy type string `array-show` accepts.
pub fn dtype_ok(d: &str) -> bool {
    let body = d.strip_prefix(['<', '>', '|', '=']).unwrap_or(d);
    DTYPES.contains(&body)
}

/// Whether `n` is a safe display name for an array.
pub fn name_ok(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 64
        && !n.starts_with('.')
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
}

fn array_show(o: &Map<String, Value>) -> Result<Request, Fail> {
    let blob = whole(o, "blob")?;
    let file = match o.get("file") {
        None | Some(Value::Null) => None,
        Some(Value::String(f)) if name_ok(f) => Some(f.clone()),
        Some(_) => {
            return Err(bad(
                "\"file\" is a bare file name in the shared folder (letters, digits, - _ . space)",
            ));
        }
    };
    let source = match (blob, file) {
        (Some(b), None) => ArraySource::Blob(b),
        (None, Some(f)) => ArraySource::File(f),
        _ => return Err(bad("give exactly one of \"blob\" and \"file\"")),
    };
    let Some(Value::Array(dims)) = o.get("shape") else {
        return Err(bad("\"shape\" must be an array of sizes"));
    };
    if dims.is_empty() || dims.len() > MAX_DIMS {
        return Err(bad(format!("\"shape\" has 1 to {MAX_DIMS} dimensions")));
    }
    let shape: Vec<u64> = dims
        .iter()
        .map(|d| d.as_u64().filter(|&n| (1..=1 << 31).contains(&n)))
        .collect::<Option<_>>()
        .ok_or_else(|| bad("every size in \"shape\" is a whole number from 1"))?;
    let dtype = match o.get("dtype") {
        Some(Value::String(d)) if dtype_ok(d) => d.clone(),
        _ => {
            return Err(bad(format!(
                "\"dtype\" is a NumPy type string: one of {} with an optional byte order",
                DTYPES.join(", ")
            )));
        }
    };
    let strides = match o.get("strides") {
        None | Some(Value::Null) => None,
        Some(Value::Array(st)) if st.len() == shape.len() => Some(
            st.iter()
                .map(|s| s.as_i64().filter(|s| s.unsigned_abs() <= MAX_BLOB))
                .collect::<Option<Vec<i64>>>()
                .ok_or_else(|| bad("\"strides\" are whole numbers of bytes"))?,
        ),
        Some(_) => return Err(bad("\"strides\" has one entry per dimension")),
    };
    let name = match o.get("name") {
        None | Some(Value::Null) => None,
        Some(Value::String(n)) if name_ok(n) => Some(n.clone()),
        Some(_) => {
            return Err(bad(
                "\"name\" is up to 64 letters, digits, spaces, - _ or . (not first)",
            ));
        }
    };
    Ok(Request::ArrayShow {
        source,
        shape,
        dtype,
        strides,
        name,
    })
}

fn layer(o: &Map<String, Value>) -> Result<String, Fail> {
    match o.get("layer") {
        None | Some(Value::Null) => Ok("default".into()),
        Some(Value::String(l)) if !l.is_empty() && l.len() <= 64 && !l.contains('\0') => {
            Ok(l.clone())
        }
        Some(_) => Err(bad("\"layer\" is a name of 1 to 64 bytes")),
    }
}

fn overlay_add(o: &Map<String, Value>) -> Result<Request, Fail> {
    let layer = layer(o)?;
    let Some(Value::Array(list)) = o.get("shapes") else {
        return Err(bad("\"shapes\" must be an array"));
    };
    if list.is_empty() || list.len() > MAX_SHAPES {
        return Err(bad(format!("\"shapes\" holds 1 to {MAX_SHAPES} shapes")));
    }
    let mut total = 0usize;
    let mut shapes = Vec::with_capacity(list.len());
    for (i, v) in list.iter().enumerate() {
        let at = |m: &str| bad(format!("shape {i}: {m}"));
        let Value::Object(s) = v else {
            return Err(at("a shape is an object"));
        };
        let kind = s
            .get("kind")
            .and_then(Value::as_str)
            .and_then(ShapeKind::from_key)
            .ok_or_else(|| at("\"kind\" is point, line, polyline, polygon, rect or text"))?;
        let Some(Value::Array(pts)) = s.get("points") else {
            return Err(at("\"points\" must be an array of [x, y]"));
        };
        let (lo, hi) = kind.points();
        if pts.len() < lo || pts.len() > hi {
            return Err(at(&format!("a {} has {lo} to {hi} points", kind.as_str())));
        }
        total += pts.len();
        if total > MAX_ADD_POINTS {
            return Err(bad(format!("at most {MAX_ADD_POINTS} points per request")));
        }
        let points = pts
            .iter()
            .map(point)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| at("a point is [x, y], finite"))?;
        let text = match s.get("text") {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(t)) if t.len() <= MAX_TEXT && !t.contains('\0') => t.clone(),
            Some(_) => {
                return Err(at(&format!(
                    "\"text\" is a string of at most {MAX_TEXT} bytes"
                )));
            }
        };
        if kind == ShapeKind::Text && text.trim().is_empty() {
            return Err(at("a text shape needs \"text\""));
        }
        let color = match s.get("color") {
            None | Some(Value::Null) => [255, 214, 0, 255],
            Some(Value::String(c)) => {
                parse_color(c).ok_or_else(|| at("\"color\" is #rgb, #rrggbb or #rrggbbaa"))?
            }
            Some(_) => return Err(at("\"color\" is #rgb, #rrggbb or #rrggbbaa")),
        };
        let width = match s.get("width") {
            None | Some(Value::Null) => 2.0,
            Some(w) => finite(w)
                .filter(|w| (0.5..=20.0).contains(w))
                .ok_or_else(|| at("\"width\" is 0.5 to 20"))? as f32,
        };
        shapes.push(Shape {
            kind,
            points,
            text,
            color,
            width,
        });
    }
    Ok(Request::OverlayAdd { layer, shapes })
}

/// The response line (without the newline) for `reply` to the request `id`, in version `v`.
pub fn response(v: u64, id: &Value, reply: &Reply) -> String {
    let mut out = match reply {
        Ok(result) => json!({"v": v, "id": id, "ok": true, "result": result}),
        Err(f) => json!({
            "v": v,
            "id": id,
            "ok": false,
            "error": {"code": f.code.as_str(), "message": f.message},
        }),
    };
    let s = out.to_string();
    if s.len() <= MAX_LINE {
        return s;
    }
    // A result too big to send (a huge marked list): an error the client can act on instead.
    out = json!({
        "v": v,
        "id": id,
        "ok": false,
        "error": {"code": Code::TooLong.as_str(), "message": "response too long"},
    });
    out.to_string()
}

#[cfg(test)]
mod tests;
