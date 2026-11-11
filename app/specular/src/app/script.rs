//! Scripting, the window's side of protocol version 2 (`docs/ipc.md`): the view read and
//! set, the current image's metadata and calibration, overlays sent by a client (drawn over
//! the image, kept per file), the user's measurements as JSON or GeoJSON, screenshots, and
//! the command that reproduces the view ("Copy view as command").

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_inspect::{ChannelView, ColourMap};
use specular_render::ZoomMode;
use specular_theme::Color;
use specular_theme::metrics::FONT_SMALL;

use super::App;
use crate::remote::protocol::{
    Channel, Code, Fail, Lang, Levels, Reply, Request, RoiFormat, Shape, ShapeKind, ViewSet,
    Window, Zoom, path_value,
};

/// Overlay shapes kept in all, across files.
pub(super) const MAX_KEPT: usize = 200_000;
/// Labels drawn at once.
const MAX_LABELS: usize = 500;
/// How long an `open` with a view waits for the image (and its values) before applying
/// what it can.
const VIEW_WAIT: Duration = Duration::from_secs(15);

/// One overlay layer: a client's shapes under a name.
#[derive(Clone, Debug)]
pub(super) struct Layer {
    pub name: String,
    pub shapes: Vec<Shape>,
}

#[derive(Default)]
pub(super) struct Script {
    /// Overlays by file, layers in the order first added.
    overlays: HashMap<PathBuf, Vec<Layer>>,
    kept: usize,
    /// A view an `open` asked for, applied once that file shows.
    pending_view: Option<(PathBuf, ViewSet, Instant)>,
    /// A screenshot waiting for the next frame.
    pub(super) screenshot: Option<crate::remote::server::Call>,
    /// When it was asked for.
    pub(super) screenshot_at: Option<Instant>,
}

/// How long a screenshot waits for a frame. A minimised or hidden window draws none (the
/// compositor sends no frame callbacks), so the script hears why instead of timing out.
pub(super) const SCREENSHOT_WAIT: Duration = Duration::from_secs(3);

/// Why a screenshot got no frame.
pub(super) const NO_FRAME: &str = "no frame was drawn: the window is minimised or hidden";

impl Script {
    /// A screenshot asked for at `at` that has waited too long, at `now`: taken, to fail.
    pub(super) fn screenshot_overdue(
        &mut self,
        now: Instant,
    ) -> Option<crate::remote::server::Call> {
        let at = self.screenshot_at?;
        if now.duration_since(at) < SCREENSHOT_WAIT {
            return None;
        }
        self.screenshot_at = None;
        self.screenshot.take()
    }

    /// When the waiting screenshot is due to fail.
    pub(super) fn screenshot_wake(&self) -> Option<Instant> {
        self.screenshot
            .as_ref()
            .and(self.screenshot_at)
            .map(|t| t + SCREENSHOT_WAIT)
    }

    /// Adds `shapes` to `file`'s layer `name`.
    pub(super) fn add(
        &mut self,
        file: &Path,
        name: &str,
        shapes: Vec<Shape>,
    ) -> Result<usize, Fail> {
        if self.kept + shapes.len() > MAX_KEPT {
            return Err(Fail::new(
                Code::Busy,
                format!("at most {MAX_KEPT} overlay shapes in all: clear some first"),
            ));
        }
        self.kept += shapes.len();
        let layers = self.overlays.entry(file.to_path_buf()).or_default();
        let n = shapes.len();
        match layers.iter_mut().find(|l| l.name == name) {
            Some(l) => l.shapes.extend(shapes),
            None => layers.push(Layer {
                name: name.to_owned(),
                shapes,
            }),
        }
        Ok(n)
    }

    /// Removes `file`'s layer `name`, or all its layers. Returns the shapes removed.
    pub(super) fn clear(&mut self, file: &Path, name: Option<&str>) -> usize {
        let Some(layers) = self.overlays.get_mut(file) else {
            return 0;
        };
        let mut gone = 0;
        layers.retain(|l| {
            let drop = name.is_none_or(|n| n == l.name);
            if drop {
                gone += l.shapes.len();
            }
            !drop
        });
        if layers.is_empty() {
            self.overlays.remove(file);
        }
        self.kept -= gone;
        gone
    }

    pub(super) fn layers(&self, file: &Path) -> &[Layer] {
        self.overlays.get(file).map_or(&[], Vec::as_slice)
    }
}

/// A shape placed on screen (logical px).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Placed {
    pub kind: ShapeKind,
    pub pts: Vec<(f32, f32)>,
    pub text: String,
    pub color: [u8; 4],
    pub width: f32,
}

/// Places `layers` on screen with `map` (stored image px to logical px), culling what lies
/// outside `view` and points closer than a pixel.
pub(super) fn place(
    layers: &[Layer],
    map: impl Fn((f32, f32)) -> (f32, f32),
    view: Rect,
) -> Vec<Placed> {
    let near = view.inset(-16.0);
    let mut out = Vec::new();
    for s in layers.iter().flat_map(|l| &l.shapes) {
        let corners;
        let src: &[[f64; 2]] = if s.kind == ShapeKind::Rect {
            let [a, b] = [s.points[0], s.points[1]];
            corners = [a, [b[0], a[1]], b, [a[0], b[1]]];
            &corners
        } else {
            &s.points
        };
        let mut pts: Vec<(f32, f32)> = Vec::with_capacity(src.len());
        let last = src.len() - 1;
        for (i, p) in src.iter().enumerate() {
            let q = map((p[0] as f32, p[1] as f32));
            match pts.last() {
                Some(&r) if i != last && (q.0 - r.0).abs() < 1.0 && (q.1 - r.1).abs() < 1.0 => {}
                _ => pts.push(q),
            }
        }
        let (x0, y0, x1, y1) = pts.iter().fold(
            (
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ),
            |b, &(x, y)| (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y)),
        );
        if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite())
            || x1 < near.x
            || y1 < near.y
            || x0 > near.right()
            || y0 > near.bottom()
        {
            continue;
        }
        out.push(Placed {
            kind: s.kind,
            pts,
            text: s.text.clone(),
            color: s.color,
            width: s.width,
        });
    }
    out
}

fn ink(c: [u8; 4]) -> Color {
    Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        f32::from(c[3]) / 255.0,
    )
}

/// Draws placed shapes clipped to `clip` (logical px), with a dark halo under each line.
pub(super) fn draw(dl: &mut DrawList, text: &mut Text, shapes: &[Placed], clip: Rect, ts: f32) {
    dl.push_clip(clip);
    for pass in 0..2 {
        for s in shapes {
            let (w, c) = if pass == 0 {
                (
                    s.width + 2.0,
                    Color::rgba(0.0, 0.0, 0.0, 0.55 * f32::from(s.color[3]) / 255.0),
                )
            } else {
                (s.width, ink(s.color))
            };
            match s.kind {
                ShapeKind::Text => {}
                ShapeKind::Point => {
                    let (x, y) = s.pts[0];
                    let r = 4.0 + s.width;
                    dl.line((x - r, y), (x + r, y), w, c);
                    dl.line((x, y - r), (x, y + r), w, c);
                }
                _ => {
                    for seg in s.pts.windows(2) {
                        dl.line(seg[0], seg[1], w, c);
                    }
                    let closed = matches!(s.kind, ShapeKind::Polygon | ShapeKind::Rect);
                    if closed
                        && s.pts.len() > 2
                        && let (Some(&a), Some(&b)) = (s.pts.last(), s.pts.first())
                    {
                        dl.line(a, b, w, c);
                    }
                }
            }
        }
    }
    let st = TextStyle::new(FONT_SMALL * ts);
    for s in shapes
        .iter()
        .filter(|s| !s.text.is_empty())
        .take(MAX_LABELS)
    {
        let b = text.layout(&s.text, st, None, Align::Start);
        let (x, y) = if s.kind == ShapeKind::Text {
            s.pts[0]
        } else {
            // Next to the shape's top-most point.
            let p = s
                .pts
                .iter()
                .copied()
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap_or_default();
            (p.0 + 6.0, p.1 - b.height - 6.0)
        };
        let r = Rect::new(x - 3.0, y - 1.0, b.width + 6.0, b.height + 2.0);
        dl.rect(r, 3.0, Color::rgba(0.0, 0.0, 0.0, 0.7));
        text.draw(dl, &b, x, y, ink([s.color[0], s.color[1], s.color[2], 255]));
    }
    dl.pop_clip();
}

/// The text of "Copy view as command": a POSIX shell line for `specular --remote raw`, or a
/// Python snippet for the `specular` package, that opens `file` with `view`.
pub(super) fn command_text(lang: Lang, file: &Path, view: &ViewSet) -> String {
    let view = (!view.is_empty()).then(|| view.clone());
    match lang {
        Lang::Shell => {
            let req = Request::Open {
                files: vec![file.to_path_buf()],
                window: Window::Current,
                view,
                compare: None,
            };
            let line = specular_client::protocol::request_line(1, &req);
            format!(
                "printf '%s\\n' {} | specular --remote raw\n",
                specular_platform::launch::shell_quote(&line)
            )
        }
        Lang::Python => {
            // JSON strings and objects without true/false/null are Python literals too.
            let path = path_value(file).to_string();
            let mut s = String::from("import specular\n\nviewer = specular.connect()\n");
            match view {
                Some(v) => {
                    let v = Value::Object(specular_client::protocol::view_value(&v));
                    s.push_str(&format!("viewer.open({path}, view={v})\n"));
                }
                None => s.push_str(&format!("viewer.open({path})\n")),
            }
            s
        }
    }
}

/// An image point to the centre of the view: display coordinates for `View::center_on`.
fn display_of(view: &specular_render::View, p: [f64; 2]) -> (f32, f32) {
    let (w, h) = view.stored_size();
    specular_render::view::display_from_stored(view.orientation(), w as f32, h as f32)
        .apply((p[0] as f32, p[1] as f32))
}

/// The stored image point at the centre of the viewport.
fn centre_of(view: &specular_render::View) -> [f64; 2] {
    let vp = view.viewport;
    let p = view
        .stored_from_screen()
        .apply((vp.x + vp.w / 2.0, vp.y + vp.h / 2.0));
    let r = |v: f32| (f64::from(v) * 100.0).round() / 100.0;
    [r(p.0), r(p.1)]
}

impl App {
    /// The current file, when an image is shown (not the gallery).
    fn script_file(&self) -> Option<PathBuf> {
        if self.gallery_active() {
            return None;
        }
        let id = self.current_id()?;
        self.images.get(&id).map(|i| i.path.clone())
    }

    fn script_need_file(&self) -> Result<PathBuf, Fail> {
        self.script_file()
            .ok_or_else(|| Fail::new(Code::NotFound, "no image is shown"))
    }

    /// Requests of version 2 the window answers (`None`: not one of them).
    pub(super) fn script_call(&mut self, req: &Request) -> Option<Reply> {
        Some(match req {
            Request::ViewGet => self.script_need_file().map(|_| self.script_view()),
            Request::ViewSet(s) => self.script_view_set(s),
            Request::Metadata => self.script_metadata(),
            Request::OverlayAdd { layer, shapes } => {
                let file = match self.script_need_file() {
                    Ok(f) => f,
                    Err(e) => return Some(Err(e)),
                };
                let r = self.remote.script.add(&file, layer, shapes.clone());
                self.redraw = true;
                r.map(|n| json!({ "added": n, "layer": layer }))
            }
            Request::OverlayClear { layer } => {
                let file = match self.script_need_file() {
                    Ok(f) => f,
                    Err(e) => return Some(Err(e)),
                };
                let n = self.remote.script.clear(&file, layer.as_deref());
                self.redraw = true;
                Ok(json!({ "removed": n }))
            }
            Request::OverlayList => self.script_need_file().map(|f| {
                let layers: Vec<Value> = self
                    .remote
                    .script
                    .layers(&f)
                    .iter()
                    .map(|l| json!({ "layer": l.name, "shapes": l.shapes.len() }))
                    .collect();
                json!({ "file": path_value(&f), "layers": layers })
            }),
            Request::Rois { format } => self.script_rois(*format),
            Request::ViewCommand { lang } => self
                .script_view_command(*lang)
                .map(|text| json!({ "text": text })),
            _ => return None,
        })
    }

    /// The view as `view-get` reports it.
    pub(super) fn script_view(&self) -> Value {
        let file = self.script_file();
        let (w, h) = self.view.stored_size();
        let zoom = (f64::from(self.view.target_scale()) * 10_000.0).round() / 100.0;
        let window = if let Some(((c, wd), unit, range)) = self.medical_script_window() {
            json!({ "center": c, "width": wd, "unit": unit, "range": [range.0, range.1] })
        } else if let Some((lo, hi)) = self.science_range() {
            let (lo, hi) = (f64::from(lo), f64::from(hi));
            json!({ "center": (lo + hi) / 2.0, "width": hi - lo, "unit": Value::Null })
        } else {
            Value::Null
        };
        let colormap = self
            .science_weather()
            .or_else(|| self.chrome.inspect.false_colour())
            .map(ColourMap::id);
        let (channel, channels) = match self.science_channels() {
            Some((names, shown)) => {
                let on: Vec<usize> = (0..shown.len()).filter(|&k| shown[k]).collect();
                let ch = match on.as_slice() {
                    [k] if names.len() > 1 => json!(k),
                    _ => json!("all"),
                };
                let list: Vec<Value> = names
                    .iter()
                    .zip(&shown)
                    .map(|(n, s)| json!({ "name": n, "shown": s }))
                    .collect();
                (ch, json!(list))
            }
            None => {
                let ch = match self.chrome.inspect.tools.channel {
                    ChannelView::Red => json!(0),
                    ChannelView::Green => json!(1),
                    ChannelView::Blue => json!(2),
                    ChannelView::Alpha => json!(3),
                    _ => json!("all"),
                };
                (ch, Value::Null)
            }
        };
        json!({
            "file": file.as_deref().map(path_value),
            "index": self.current,
            "image": { "width": w, "height": h },
            "orientation": self.view.orientation().0,
            "zoom": zoom,
            "fit": self.view.mode == ZoomMode::Fit,
            "center": centre_of(&self.view),
            "window": window,
            "colormap": colormap,
            "channel": channel,
            "channels": channels,
        })
    }

    /// Applies `s` to the current image; the view after, as `view-get`.
    pub(super) fn script_view_set(&mut self, s: &ViewSet) -> Reply {
        self.script_need_file()?;
        // Checked before anything changes: a bad colour map or channel changes nothing.
        let map = match s.colormap.as_deref() {
            None => None,
            Some("none") => Some(None),
            Some(id) => Some(Some(ColourMap::from_id(id).ok_or_else(|| {
                Fail::new(
                    Code::BadArguments,
                    format!("no colour map {id:?} (hello lists them)"),
                )
            })?)),
        };
        if let Some(ch) = s.channel {
            self.script_channel(ch)?;
        }
        if let Some(m) = map {
            if self.science_range().is_some() {
                self.science_set_weather(m);
            } else {
                self.chrome.inspect.tools.false_colour = m.is_some();
                if let Some(m) = m {
                    self.chrome.inspect.tools.colour_map = m;
                }
            }
        }
        if let Some(l) = s.levels {
            let cw = match l {
                Levels::Auto => None,
                Levels::Set { center, width } => Some((center, width)),
            };
            let done = self.medical_script_set_window(cw)
                || self.science_set_range(
                    cw.map(|(c, w)| ((c - w / 2.0) as f32, (c + w / 2.0) as f32)),
                );
            if !done {
                return Err(Fail::new(
                    Code::Mode,
                    "this image has no window/level: only medical and science images do",
                ));
            }
        }
        match s.zoom {
            Some(Zoom::Fit) => self.view.zoom_fit(),
            Some(Zoom::Percent(p)) => self.view.zoom_to((p / 100.0) as f32, None),
            None => {}
        }
        if let Some(c) = s.center {
            let d = display_of(&self.view, c);
            self.view.center_on(d, false);
        }
        self.view.settle();
        self.after_view_change();
        self.redraw = true;
        Ok(self.script_view())
    }

    fn script_channel(&mut self, ch: Channel) -> Result<(), Fail> {
        let only = match ch {
            Channel::All => None,
            Channel::Index(k) => Some(k as usize),
        };
        if self.science_channels().is_some() {
            return if self.science_show_channel(only) {
                Ok(())
            } else {
                Err(Fail::new(Code::BadArguments, "no such channel"))
            };
        }
        self.chrome.inspect.tools.channel = match only {
            None => ChannelView::Normal,
            Some(0) => ChannelView::Red,
            Some(1) => ChannelView::Green,
            Some(2) => ChannelView::Blue,
            Some(3) => ChannelView::Alpha,
            Some(_) => {
                return Err(Fail::new(
                    Code::BadArguments,
                    "this image has channels 0 to 3 (red, green, blue, alpha)",
                ));
            }
        };
        Ok(())
    }

    fn script_metadata(&mut self) -> Reply {
        let file = self.script_need_file()?;
        let id = self.current_id();
        let img = id.and_then(|id| self.images.get(&id));
        let info = img.and_then(|i| i.info.as_ref());
        let cal = self.measure_file_calibration();
        let calibration = if cal.is_calibrated() {
            let (sx, sy) = cal.axis_spacing();
            json!({
                "pixel_size": [sx, sy],
                "unit": cal.unit.symbol(),
                "source": cal.source.key(),
                "matrix": cal.m,
            })
        } else {
            Value::Null
        };
        let session = img.and_then(|i| i.session.clone());
        let science = session.as_ref().and_then(|s| s.science()).map(|s| {
            let axes: Vec<Value> = s
                .axes
                .iter()
                .map(|a| json!({ "name": a.name, "len": a.len, "unit": a.unit }))
                .collect();
            json!({
                "width": s.width,
                "height": s.height,
                "sample": s.sample,
                "unit": s.unit,
                "axes": axes,
            })
        });
        let value_unit = self.medical_script_window().and_then(|w| w.1).or_else(|| {
            session
                .as_ref()
                .and_then(|s| s.science())
                .and_then(|s| s.unit.clone())
        });
        let fields: Vec<Value> = img
            .and_then(|i| i.metadata.as_ref())
            .map(|m| {
                m.all_fields
                    .iter()
                    .flat_map(|g| {
                        g.fields.iter().map(
                            move |f| json!({ "group": g.title, "tag": f.tag, "value": f.display }),
                        )
                    })
                    .take(20_000)
                    .collect()
            })
            .unwrap_or_default();
        let summary: Vec<Value> = img
            .and_then(|i| i.metadata.as_ref())
            .map(|m| {
                m.summary
                    .iter()
                    .flat_map(|g| {
                        g.items.iter().map(move |it| {
                            json!({ "group": g.key, "key": it.key, "label": it.label, "value": it.value })
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let pending = img.is_some_and(|i| i.metadata.is_none());
        let reply = json!({
            "file": path_value(&file),
            "format": info.map(|i| format!("{:?}", i.format)),
            "width": info.map(|i| i.width),
            "height": info.map(|i| i.height),
            "bit_depth": info.map(|i| i.bit_depth),
            "has_alpha": info.map(|i| i.has_alpha),
            "coding": info.map(|i| i.coding.clone()),
            "page": img.map(|i| i.page),
            "pages": img.map(|i| i.pages),
            "calibration": calibration,
            "value_unit": value_unit,
            "science": science,
            "summary": summary,
            "fields": fields,
            "metadata_pending": pending,
        });
        if pending {
            self.request_metadata();
        }
        Ok(reply)
    }

    fn script_rois(&mut self, format: RoiFormat) -> Reply {
        self.script_need_file()?;
        let (items, prov) = self.measure_script();
        Ok(match format {
            RoiFormat::GeoJson => {
                let text = specular_measure::geojson::write(&items, &prov);
                serde_json::from_str(&text).map_err(|e| Fail::new(Code::Internal, e.to_string()))?
            }
            RoiFormat::Json => {
                let list: Vec<Value> = items
                    .iter()
                    .map(|m| {
                        let pts: Vec<Value> =
                            m.shape.points().iter().map(|p| json!([p.x, p.y])).collect();
                        let q = m.primary();
                        json!({
                            "id": m.id,
                            "kind": m.shape.key(),
                            "points": pts,
                            "label": m.label,
                            "frame": m.frame,
                            "value": q.map(|q| q.value),
                            "unit": q.map(|q| q.unit.symbol()),
                            "calibrated": m.cal.is_calibrated(),
                        })
                    })
                    .collect();
                json!({ "measurements": list })
            }
        })
    }

    /// The current view as a ViewSet (what "Copy view as command" reproduces).
    pub(super) fn script_current_view(&self) -> ViewSet {
        let fit = self.view.mode == ZoomMode::Fit;
        let v = self.script_view();
        let levels = match (&v["window"]["center"], &v["window"]["width"]) {
            (Value::Number(c), Value::Number(w)) => Some(Levels::Set {
                center: c.as_f64().unwrap_or(0.0),
                width: w.as_f64().unwrap_or(1.0),
            }),
            _ => None,
        };
        ViewSet {
            zoom: Some(if fit {
                Zoom::Fit
            } else {
                Zoom::Percent(v["zoom"].as_f64().unwrap_or(100.0))
            }),
            center: (!fit).then(|| centre_of(&self.view)),
            levels,
            colormap: v["colormap"].as_str().map(str::to_owned),
            channel: v["channel"].as_u64().map(|k| Channel::Index(k as u32)),
        }
    }

    pub(super) fn script_view_command(&self, lang: Lang) -> Result<String, Fail> {
        let file = self.script_need_file()?;
        Ok(command_text(lang, &file, &self.script_current_view()))
    }

    /// "Copy view as command" (palette): the snippet goes to the clipboard.
    pub(super) fn copy_view_command(&mut self, lang: Lang) {
        match self.script_view_command(lang) {
            Ok(text) => {
                self.clipboard
                    .copy_text(&self.loader, text, crate::loader::Copied::ViewCommand);
            }
            Err(_) => {
                let now = self.now();
                self.chrome
                    .toast(specular_i18n::t!("script-copy-no-image"), None, now);
            }
        }
    }

    /// The palette's scripting actions.
    pub(super) fn script_action(&mut self, a: specular_input::Action) -> bool {
        match a {
            specular_input::Action::CopyViewCommand => self.copy_view_command(Lang::Shell),
            specular_input::Action::CopyViewPython => self.copy_view_command(Lang::Python),
            _ => return false,
        }
        true
    }

    /// `open` with a view: applied once that file shows.
    pub(super) fn script_expect_view(&mut self, file: PathBuf, view: ViewSet) {
        self.remote.script.pending_view = Some((file, view, Instant::now()));
    }

    /// Once per turn: the view an `open` asked for, once its image is in (and, for window
    /// and level, its values), or what can be applied after a while.
    pub(super) fn script_apply_pending(&mut self) {
        let Some((file, view, since)) = self.remote.script.pending_view.as_ref() else {
            return;
        };
        let late = since.elapsed() > VIEW_WAIT;
        let shown = self.script_file().as_deref() == Some(file.as_path());
        let sized = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.info.as_ref())
            .is_some_and(|_| self.view.stored_size() != (0, 0));
        let values = view.levels.is_none()
            || self.medical_script_window().is_some()
            || self.science_range().is_some();
        if late && !shown {
            self.remote.script.pending_view = None;
            return;
        }
        if !(shown && sized && (values || late)) {
            return;
        }
        let Some((_, mut view, _)) = self.remote.script.pending_view.take() else {
            return;
        };
        if !values {
            view.levels = None;
        }
        if let Err(e) = self.script_view_set(&view) {
            specular_log::warn!(target: specular_log::SETTINGS, "remote control: the view asked for: {}", e.message);
        }
    }

    /// The overlays of the current file, placed on screen this frame.
    pub(super) fn script_placed(&self) -> Vec<Placed> {
        let Some(file) = self.script_file() else {
            return Vec::new();
        };
        let layers = self.remote.script.layers(&file);
        if layers.is_empty() {
            return Vec::new();
        }
        let ls = self.dl.scale.max(0.1);
        let vp = self.view.viewport;
        let to_screen = self.view.stored_from_screen().invert();
        place(
            layers,
            |p| {
                let q = to_screen.apply(p);
                (q.0 / ls, q.1 / ls)
            },
            Rect::new(vp.x / ls, vp.y / ls, vp.w / ls, vp.h / ls),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(kind: ShapeKind, pts: &[[f64; 2]]) -> Shape {
        Shape::new(kind, pts.to_vec())
    }

    /// A screenshot that gets no frame (a minimised window) fails after the wait instead
    /// of leaving the script to time out; the event loop wakes for it.
    #[test]
    fn screenshot_without_a_frame_fails_in_time() {
        let mut s = Script::default();
        let t0 = Instant::now();
        assert_eq!(s.screenshot_wake(), None);
        let (call, answers) = crate::remote::server::Call::new(Request::Screenshot);
        s.screenshot = Some(call);
        s.screenshot_at = Some(t0);
        assert_eq!(s.screenshot_wake(), Some(t0 + SCREENSHOT_WAIT));
        assert!(
            s.screenshot_overdue(t0 + Duration::from_millis(500))
                .is_none()
        );
        let late = s.screenshot_overdue(t0 + SCREENSHOT_WAIT).expect("overdue");
        late.answer(Err(Fail::new(Code::Mode, NO_FRAME)));
        let a = answers.try_recv().expect("answered");
        assert!(a.reply.is_err());
        assert!(s.screenshot.is_none() && s.screenshot_wake().is_none());
        assert!(s.screenshot_overdue(t0 + SCREENSHOT_WAIT * 2).is_none());
    }

    #[test]
    fn layers_are_kept_per_file_and_bounded() {
        let mut s = Script::default();
        let a = Path::new("/a.png");
        let b = Path::new("/b.png");
        s.add(a, "x", vec![shape(ShapeKind::Point, &[[1.0, 1.0]])])
            .unwrap();
        s.add(a, "x", vec![shape(ShapeKind::Point, &[[2.0, 1.0]])])
            .unwrap();
        s.add(
            a,
            "y",
            vec![shape(ShapeKind::Line, &[[0.0, 0.0], [1.0, 1.0]])],
        )
        .unwrap();
        s.add(b, "x", vec![shape(ShapeKind::Point, &[[1.0, 1.0]])])
            .unwrap();
        assert_eq!(s.layers(a).len(), 2);
        assert_eq!(s.layers(a)[0].shapes.len(), 2);
        assert_eq!(s.clear(a, Some("x")), 2);
        assert_eq!(s.clear(a, None), 1);
        assert!(s.layers(a).is_empty());
        assert_eq!(s.layers(b).len(), 1);
        let many = vec![shape(ShapeKind::Point, &[[0.0, 0.0]]); MAX_KEPT];
        assert_eq!(s.add(a, "z", many).unwrap_err().code, Code::Busy);
        assert_eq!(s.clear(b, None), 1);
        assert_eq!(s.kept, 0);
    }

    #[test]
    fn shapes_are_placed_and_culled() {
        let mut text = shape(ShapeKind::Text, &[[10.0, 10.0]]);
        text.text = "hi".into();
        let layers = vec![Layer {
            name: "l".into(),
            shapes: vec![
                shape(ShapeKind::Rect, &[[0.0, 0.0], [10.0, 20.0]]),
                shape(ShapeKind::Point, &[[5000.0, 5000.0]]),
                shape(
                    ShapeKind::Polyline,
                    &[[0.0, 0.0], [0.1, 0.1], [0.2, 0.2], [30.0, 30.0]],
                ),
                text,
            ],
        }];
        let placed = place(
            &layers,
            |p| (p.0 * 2.0, p.1 * 2.0),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );
        assert_eq!(placed.len(), 3, "the far point is culled");
        assert_eq!(
            placed[0].pts,
            vec![(0.0, 0.0), (20.0, 0.0), (20.0, 40.0), (0.0, 40.0)]
        );
        assert_eq!(
            placed[1].pts,
            vec![(0.0, 0.0), (60.0, 60.0)],
            "sub-pixel steps go"
        );
        assert_eq!(placed[2].text, "hi");
    }

    #[test]
    fn view_commands() {
        let view = ViewSet {
            zoom: Some(Zoom::Percent(200.0)),
            center: Some([10.0, 20.5]),
            levels: Some(Levels::Set {
                center: 40.0,
                width: 400.0,
            }),
            colormap: Some("viridis".into()),
            channel: None,
        };
        let file = crate::remote::test_abs("/data/it's a scan.dcm");
        let file = file.as_path();
        let sh = command_text(Lang::Shell, file, &view);
        assert!(sh.starts_with("printf '%s\\n' '{"), "{sh}");
        assert!(sh.ends_with("| specular --remote raw\n"), "{sh}");
        // The quoted JSON is the open request, with the view.
        let quoted = sh
            .strip_prefix("printf '%s\\n' ")
            .and_then(|s| s.strip_suffix(" | specular --remote raw\n"))
            .unwrap();
        let json_text = quoted[1..quoted.len() - 1].replace("'\\''", "'");
        let e = crate::remote::protocol::parse_request(json_text.as_bytes()).unwrap();
        assert_eq!(
            e.req,
            Request::Open {
                files: vec![file.to_path_buf()],
                window: Window::Current,
                view: Some(view.clone()),
                compare: None,
            }
        );
        let py = command_text(Lang::Python, file, &view);
        assert!(py.contains("import specular"), "{py}");
        assert!(
            py.contains(r#"viewer.open("/data/it's a scan.dcm", view={"#),
            "{py}"
        );
        assert!(py.contains(r#""colormap":"viridis""#), "{py}");
        let plain = command_text(Lang::Python, file, &ViewSet::default());
        assert!(
            plain.ends_with("viewer.open(\"/data/it's a scan.dcm\")\n"),
            "{plain}"
        );
    }
}
