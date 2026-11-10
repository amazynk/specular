//! The app side of figures and key images: what the image offers, the panels read back from
//! the GPU through the display path, and the exports run off the UI thread.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crossbeam_channel::Receiver;
use specular_dicom::mask::Rect as Area;
use specular_figure::compose::{InsetSpec, LegendSpec, Panel, Spec, compose, inset_size};
use specular_figure::render::Shot;
use specular_figure::{Rgb16, nice};
use specular_gfx::Text;
use specular_i18n::t;
use specular_inspect::ChannelView;
use specular_render::RenderOptions;
use specular_settings::HighZoomSmoothing;

use super::build::{self, KeyImage, Map, Measured};
use super::{Done, Format, KeyFormat, Region, Size};
use crate::app::App;
use crate::loader::ImageId;

/// The largest figure panel: a side, and pixels.
const MAX_SIDE: u32 = 20_000;
const MAX_PIXELS: u64 = 200_000_000;
/// Key images are kept at most this long on their longest side.
const KEY_SIDE: f32 = 2048.0;
/// Figures are written at this resolution in PDF (px per inch).
const PDF_DPI: f64 = 300.0;

/// What a finished job hands back: its result and the text system it used.
pub(super) type Reply = (Done, Option<Text>);

/// The user's masks when they fit the key image's stored size.
pub(super) fn masks_for(k: &KeyImage, user: &((u32, u32), Vec<Area>)) -> Vec<Area> {
    if user.0 == k.stored {
        user.1.clone()
    } else {
        Vec::new()
    }
}

/// A rectangle of captured pixels as stored pixels (rounded outward).
fn to_stored(k: &KeyImage, r: &Area) -> Area {
    let inv = build::invert(&k.from_stored);
    let pts = [
        (f64::from(r.x), f64::from(r.y)),
        (f64::from(r.x + r.w), f64::from(r.y)),
        (f64::from(r.x), f64::from(r.y + r.h)),
        (f64::from(r.x + r.w), f64::from(r.y + r.h)),
    ]
    .map(|p| build::apply(&inv, p));
    let x0 = pts
        .iter()
        .map(|p| p.0)
        .fold(f64::MAX, f64::min)
        .floor()
        .max(0.0);
    let y0 = pts
        .iter()
        .map(|p| p.1)
        .fold(f64::MAX, f64::min)
        .floor()
        .max(0.0);
    let x1 = pts.iter().map(|p| p.0).fold(f64::MIN, f64::max).ceil();
    let y1 = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil();
    Area::new(
        x0 as u32,
        y0 as u32,
        (x1 - x0).max(1.0) as u32,
        (y1 - y0).max(1.0) as u32,
    )
}

/// Text the detector finds in the captured pixels, as stored-pixel areas.
pub(super) fn detected(k: &KeyImage) -> Vec<Area> {
    let luma: Vec<u8> =
        k.px.data
            .chunks_exact(3)
            .map(|p| {
                let y = (u32::from(p[0]) * 54 + u32::from(p[1]) * 183 + u32::from(p[2]) * 19) >> 8;
                specular_figure::to8(y.min(65_535) as u16)
            })
            .collect();
    specular_dicom::detect::suggest_luma(&luma, k.px.width as usize, k.px.height as usize)
        .iter()
        .map(|r| to_stored(k, r))
        .collect()
}

/// One key image of a teaching file: its masks (the user's, else the detector's) and, for a
/// DICOM file, its anonymised copy; `None` when it must be left out (burned-in text is
/// declared and none could be found, or the file cannot be read).
pub(super) fn teaching_one(
    k: &KeyImage,
    user: &((u32, u32), Vec<Area>),
    session: &specular_dicom::anon::Session,
    want_copy: bool,
) -> Option<(Vec<Area>, Option<Vec<u8>>)> {
    use specular_dicom::anon::{self, AnonError, Options, Request, Risk};
    let mut areas = masks_for(k, user);
    if areas.is_empty() {
        areas = detected(k);
    }
    if !k.dicom {
        return Some((areas, None));
    }
    let bytes = std::fs::read(&k.path).ok()?;
    let d = specular_dicom::Dicom::parse(&bytes).ok()?;
    match anon::risk(&d, &bytes) {
        Risk::Document => return None,
        Risk::Present if areas.is_empty() => return None,
        _ => {}
    }
    if !want_copy {
        return Some((areas, None));
    }
    let fits = d
        .pixel
        .as_ref()
        .is_some_and(|p| (p.columns, p.rows) == k.stored);
    let masks: &[Area] = if fits { &areas } else { &[] };
    let opts = Options::default();
    match anon::anonymise(
        &bytes,
        &Request {
            options: &opts,
            session,
            masks,
            allow_burned_in: false,
            cancel: None,
        },
    ) {
        Ok(a) => Some((areas, Some(a.bytes))),
        Err(AnonError::BurnedIn) => None,
        Err(e) => {
            specular_log::warn!("teaching copy {}: {e}", k.path.display());
            Some((areas, None))
        }
    }
}

/// The whole-pixel size of the view's display image.
pub(super) fn display_size(view: &specular_render::View) -> (u32, u32) {
    let (w, h) = view.display_size();
    (w.round().max(1.0) as u32, h.round().max(1.0) as u32)
}

/// The main panel of a figure from the viewer's `view`: the visible part or the whole image,
/// at 1:1 or a chosen width.
pub(super) fn main_shot(
    view: &specular_render::View,
    region: Region,
    size: Size,
) -> Result<Shot, String> {
    let (dw, dh) = display_size(view);
    let r = match region {
        Region::Whole => [0, 0, dw, dh],
        Region::Visible => {
            let (x0, y0, x1, y1) = view.visible_display();
            let c = |v: f32, max: u32| v.clamp(0.0, max as f32);
            [
                c(x0.floor(), dw) as u32,
                c(y0.floor(), dh) as u32,
                c(x1.ceil(), dw) as u32,
                c(y1.ceil(), dh) as u32,
            ]
        }
    };
    let mut shot = Shot::region(view.stored_size(), view.orientation(), r);
    if let Size::Width(w) = size {
        let z = w as f32 / shot.size.0.max(1) as f32;
        shot.zoom = z;
        shot.size = (w, ((shot.size.1 as f32 * z).round() as u32).max(1));
    }
    let (w, h) = shot.size;
    if w > MAX_SIDE || h > MAX_SIDE || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(t!(
            "figure-too-large",
            width = i64::from(w),
            height = i64::from(h)
        ));
    }
    Ok(shot)
}

/// An inset of the stored rectangle `r` (x0, y0, x1, y1): its region on the main panel
/// (`panel_from_stored` maps into it) and the shot that draws the region at the inset's size.
pub(super) fn inset_shot(
    main: &Shot,
    panel_from_stored: &Map,
    r: (f64, f64, f64, f64),
) -> ([f32; 4], Shot) {
    let a = build::apply(panel_from_stored, (r.0, r.1));
    let b = build::apply(panel_from_stored, (r.2, r.3));
    let (px0, py0) = (a.0.min(b.0), a.1.min(b.1));
    let (pw, ph) = ((a.0 - b.0).abs().max(1.0), (a.1 - b.1).abs().max(1.0));
    let region = [px0 as f32, py0 as f32, pw as f32, ph as f32];
    let (iw, ih) = inset_size(main.size, region);
    // The panel's centre is the display point `centre`; panel px are 1 / zoom display px.
    let cx = main.centre.0 + ((px0 + pw * 0.5) as f32 - main.size.0 as f32 * 0.5) / main.zoom;
    let cy = main.centre.1 + ((py0 + ph * 0.5) as f32 - main.size.1 as f32 * 0.5) / main.zoom;
    let shot = Shot {
        stored: main.stored,
        orient: main.orient,
        zoom: main.zoom * iw as f32 / pw as f32,
        centre: (cx, cy),
        size: (iw, ih),
    };
    (region, shot)
}

/// Writes `files` into `dir`, each a new file (an existing one stops the export).
fn write_new(dir: &Path, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| crate::error_text::io_reason(&e))?;
    for (name, data) in files {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::error_text::io_reason(&e))?;
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&p)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::AlreadyExists => t!("figure-folder-taken", name = name.clone()),
                _ => crate::error_text::io_reason(&e),
            })?;
        f.write_all(data)
            .and_then(|_| f.sync_all())
            .map_err(|e| crate::error_text::io_reason(&e))?;
    }
    Ok(())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The figure text system, waiting a little for the system fonts.
fn ready_text(t: Option<Text>) -> Text {
    let mut t = t.unwrap_or_default();
    let t0 = std::time::Instant::now();
    t.begin_frame(1.0);
    while t.fonts_pending() && t0.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(10));
        t.begin_frame(1.0);
    }
    t
}

/// The figure file: SVG, PDF or a 16-bit raster with the provenance in its metadata.
pub(super) fn figure_bytes(
    f: &specular_figure::Figure,
    format: Format,
    created: u64,
) -> Result<Vec<u8>, String> {
    let producer = format!("Specular {}", env!("CARGO_PKG_VERSION"));
    match format {
        Format::Svg => Ok(specular_figure::svg::write(f)?.into_bytes()),
        Format::Pdf => {
            let icc = specular_print::icc::srgb_v2();
            specular_figure::pdf::write(
                f,
                &specular_figure::pdf::Options {
                    dpi: PDF_DPI,
                    icc: Some(&icc),
                    producer,
                    created: Some(build::pdf_date(created)),
                },
            )
        }
        Format::Png | Format::Tiff => {
            let px = specular_figure::raster::render(f, 1.0);
            let xmp = specular_figure::pdf::xmp(f, &producer);
            if format == Format::Png {
                let text = [("Software", producer.as_str())];
                specular_png::encode_with(
                    px.width,
                    px.height,
                    specular_png::ColorType::Rgb,
                    &px.to_be_bytes(),
                    &specular_png::EncodeOptions {
                        sixteen_bit: true,
                        level: Some(6),
                        srgb: true,
                        xmp: Some(xmp.as_bytes()),
                        text: &text,
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())
            } else {
                let p = specular_convert::pixels::Pixels {
                    width: px.width,
                    height: px.height,
                    channels: 3,
                    samples: specular_convert::pixels::Samples::Sixteen(px.data),
                };
                let meta = specular_convert::encode::Meta {
                    xmp: Some(xmp.as_bytes()),
                    ..Default::default()
                };
                Ok(specular_convert::encode::tiff(&p, &meta, true))
            }
        }
    }
}

impl App {
    /// The image a figure or key image is made of: the current one, on screen with pixels.
    fn figure_image(&mut self) -> Result<ImageId, String> {
        if self.mpr_active() || self.mesh_active() || self.cad_active() || self.gallery_active() {
            return Err(t!("figure-not-image"));
        }
        let id = self.current_id().ok_or_else(|| t!("figure-not-ready"))?;
        // Right after a GPU reset: a lost device is replaced now, and an image the new
        // device lacks is uploaded again from its decoded pixels, rather than "not ready"
        // until a frame has drawn it.
        if self.gfx.as_ref().is_some_and(|g| g.gpu.is_lost()) {
            self.recover_lost_gpu();
        }
        if self.shown == Some(id) && self.gpu_ready() && !self.figure_on_gpu(id) {
            self.upload(id);
        }
        if self.shown != Some(id) || !self.figure_on_gpu(id) {
            return Err(t!("figure-not-ready"));
        }
        Ok(id)
    }

    /// Whether the image's overview is on the device.
    fn figure_on_gpu(&mut self, id: ImageId) -> bool {
        self.gpu_images
            .get(id)
            .is_some_and(|g| g.overview.is_some())
    }

    /// The frame or page shown.
    pub(super) fn figure_frame(&self) -> u32 {
        self.medical_for_figure()
            .map(|(i, _)| i)
            .unwrap_or_else(|| {
                self.current_id()
                    .and_then(|id| self.images.get(&id))
                    .map_or(0, |i| i.page)
            })
    }

    /// Every measurement on the image: the measurement tool's and medical mode's, and the
    /// markup.
    fn figure_measured(&self) -> Vec<Measured> {
        let (_, mut v, _) = self.measure_for_figure();
        if let Some((_, m)) = self.medical_for_figure() {
            v.extend(m);
        }
        // The page's markup (clouds, arrows, text and stamps as outlines with their words).
        v.extend(self.markup_for_figure());
        v
    }

    /// The colour bar of the display: the false-colour map's legend, else the medical window
    /// as a grey ramp; and how it reads in the dialog.
    fn figure_legend(&self) -> Option<(LegendSpec, String)> {
        if let Some(l) = &self.chrome.inspect.legend {
            let ramp = build::ramp(|t| l.map.sample(t));
            let desc = t!(
                "figure-legend-map",
                map = l.map.name(),
                lo = nice::label(l.lo),
                hi = nice::label(l.hi),
                unit = l.unit.clone()
            );
            return Some((
                LegendSpec {
                    ramp,
                    lo: l.lo,
                    hi: l.hi,
                    unit: l.unit.clone(),
                },
                desc,
            ));
        }
        let (lo, hi, unit) = self.medical_legend()?;
        let desc = t!(
            "figure-legend-window",
            lo = nice::label(lo.min(hi)),
            hi = nice::label(lo.max(hi)),
            unit = unit.clone()
        );
        Some((
            LegendSpec {
                ramp: build::ramp(|t| [t, t, t]),
                lo,
                hi,
                unit,
            },
            desc,
        ))
    }

    /// The display settings in words, for the provenance.
    fn figure_display(&self) -> String {
        let mut parts = Vec::new();
        if let Some((_, d)) = self.figure_legend() {
            parts.push(d);
        }
        let tools = self.chrome.inspect.tools;
        if tools.channel_colours != specular_inspect::ChannelColours::RedGreen {
            parts.push(t!(
                "figure-display-channels",
                preset = tools.channel_colours.id()
            ));
        }
        let has_alpha = self.action_state().has_alpha;
        let ch = self.chrome.channel_view(has_alpha);
        if ch != ChannelView::Normal {
            parts.push(t!(
                "figure-display-channel",
                channel = format!("{ch:?}").to_lowercase()
            ));
        }
        if tools.hilo {
            parts.push(t!("figure-display-hilo"));
        }
        if parts.is_empty() {
            t!("figure-display-as-stored")
        } else {
            parts.join("; ")
        }
    }

    /// What the image offers the figure dialog.
    pub(super) fn figure_avail(&mut self) -> Result<super::Avail, String> {
        let id = self.figure_image()?;
        let (cal, measured, rect) = self.measure_for_figure();
        let medical = self.medical_for_figure();
        let marks = measured.len() + medical.as_ref().map_or(0, |m| m.1.len());
        let colour = self
            .images
            .get(&id)
            .and_then(|i| i.overview.as_ref())
            .is_some_and(|(_, r)| !r.layout.is_gray());
        let ov = self.gpu_images.get(id).map_or(0.0, |g| g.overview_scale());
        Ok(super::Avail {
            calibration: cal
                .is_calibrated()
                .then(|| App::measure_source_label(cal.source)),
            legend: self.figure_legend().map(|(_, d)| d),
            inset: rect.is_some(),
            split: colour && medical.is_none(),
            marks,
            full_res: ov >= 0.999,
            zoomed_in: self.view.scale() >= 0.999,
            medical: self.medical_state().is_some(),
            keys: self.figure.keys.len(),
        })
    }

    /// The render options of the display path for a figure: the viewer's channel view and
    /// display aids, transparency on white, nearest-neighbour at 1:1 (`exact`), and nothing
    /// that belongs to this display (its profile, GSDF, colour-vision simulation).
    fn figure_opts(&self, exact: bool) -> RenderOptions {
        let white = [1.0, 1.0, 1.0, 1.0];
        let mut o = RenderOptions {
            hq: true,
            canvas: white,
            checker_a: white,
            checker_b: white,
            checker_cell: 1.0e6,
            ..RenderOptions::default()
        };
        let has_alpha = self.action_state().has_alpha;
        super::super::inspect::apply_channel(&mut o, &self.chrome, &self.settings, has_alpha);
        o.nearest_from = if exact {
            1.0
        } else {
            match self.settings.viewing.smoothing_high_zoom {
                HighZoomSmoothing::Auto => 4.0,
                HighZoomSmoothing::Smooth => f32::INFINITY,
                HighZoomSmoothing::Pixelated => 1.0001,
            }
        };
        o.gsdf = None;
        o
    }

    /// Reads `shot` of the current image back through the display path (once more on the
    /// new device after a GPU reset on the way).
    fn figure_render(&mut self, shot: &Shot, opts: &RenderOptions) -> Result<Rgb16, String> {
        self.with_gpu_retry(|app| app.figure_render_once(shot, opts))
    }

    fn figure_render_once(&mut self, shot: &Shot, opts: &RenderOptions) -> Result<Rgb16, String> {
        let id = self.current_id().ok_or_else(|| t!("figure-not-ready"))?;
        let gfx = self.gfx.as_mut().ok_or_else(|| t!("figure-not-ready"))?;
        let g = self
            .gpu_images
            .get(id)
            .ok_or_else(|| t!("figure-not-ready"))?;
        let r = specular_figure::render::read(&gfx.gpu, &mut gfx.renderer, g, shot, opts);
        // The scene is drawn again with the viewer's own view.
        self.scene = None;
        r
    }

    /// The display image's whole-pixel size.
    fn figure_display_size(&self) -> (u32, u32) {
        display_size(&self.view)
    }

    /// Starts writing the figure to `target`.
    pub(super) fn figure_export_figure(
        &mut self,
        target: PathBuf,
    ) -> Result<Receiver<Reply>, String> {
        let id = self.figure_image()?;
        let c = self.figure.figure.clone();
        let avail = self
            .figure
            .dialog
            .as_ref()
            .map(|d| d.avail.clone())
            .unwrap_or_default();
        let shot = main_shot(&self.view, c.region, c.size)?;
        let exact = c.size == Size::Pixels;
        let opts = self.figure_opts(exact);
        let main = self.figure_render(&shot, &opts)?;
        let view = shot.view();
        let panel_from_stored: Map = view.screen_from_stored_f64();
        let stored_from_panel = build::invert(&panel_from_stored);
        let mut channels = Vec::new();
        if c.split && avail.split {
            for (ch, label) in [
                (ChannelView::Red, t!("figure-channel-red")),
                (ChannelView::Green, t!("figure-channel-green")),
                (ChannelView::Blue, t!("figure-channel-blue")),
            ] {
                let o = RenderOptions {
                    channel: ch,
                    false_colour: None,
                    ..opts
                };
                channels.push(Panel {
                    px: self.figure_render(&shot, &o)?,
                    label: Some(label),
                });
            }
        }
        let (cal, _, rect) = self.measure_for_figure();
        let inset = match rect.filter(|_| c.inset && avail.inset) {
            Some(r) => {
                let (region, ishot) = inset_shot(&shot, &panel_from_stored, r);
                let px = self.figure_render(&ishot, &self.figure_opts(false))?;
                Some(InsetSpec { px, region })
            }
            None => None,
        };
        let marks = if c.marks {
            build::marks(&self.figure_measured(), &panel_from_stored)
        } else {
            Vec::new()
        };
        let scale = if c.scale_bar {
            build::scale_spec(&cal, &stored_from_panel)
        } else {
            None
        };
        let legend = if c.colour_bar {
            self.figure_legend().map(|(l, _)| l)
        } else {
            None
        };

        // Provenance.
        let (prov, hash_path) = self.measure_provenance();
        let dicom = self.medical_state().is_some()
            || self
                .images
                .get(&id)
                .and_then(|i| i.session.as_ref())
                .is_some_and(|s| s.medical().is_some());
        let created = now_unix();
        let mut meta: Vec<(String, String)> = Vec::new();
        if !c.title.trim().is_empty() {
            meta.push(("title".into(), c.title.trim().to_owned()));
        }
        // A DICOM file's name may name the patient: its hash identifies it instead.
        if !dicom {
            meta.push(("file".into(), prov.file.clone()));
        }
        let (sw, sh) = self.view.stored_size();
        meta.push(("size".into(), format!("{sw}x{sh}")));
        let r = shot.size;
        meta.push((
            "region".into(),
            format!(
                "{}, {} px wide{}",
                match c.region {
                    Region::Visible => "visible area",
                    Region::Whole => "whole image",
                },
                r.0,
                if exact { ", 1:1" } else { ", resampled" }
            ),
        ));
        let cal_text = if cal.is_calibrated() {
            let (x, y) = cal.axis_spacing();
            format!(
                "{}: {} × {} {}/px",
                App::measure_source_label(cal.source),
                nice::label(x),
                nice::label(y),
                cal.unit.symbol()
            )
        } else {
            t!("figure-scale-bar-none")
        };
        meta.push(("calibration".into(), cal_text.clone()));
        let display = self.figure_display();
        meta.push(("display".into(), display.clone()));
        if let Some(d) = &prov.decoder {
            meta.push(("decoder".into(), d.clone()));
        }
        meta.push(("software".into(), prov.software.clone()));
        meta.push(("created".into(), build::rfc3339(created)));
        let caption = if c.drawn {
            let mut v = Vec::new();
            if !dicom {
                v.push(prov.file.clone());
            }
            v.push(t!("figure-caption-calibration", value = cal_text));
            v.push(t!("figure-caption-display", value = display));
            v.push(format!("{} · {}", prov.software, build::rfc3339(created)));
            v
        } else {
            Vec::new()
        };
        let spec = Spec {
            title: Some(c.title.trim().to_owned()).filter(|t| !t.is_empty()),
            main: Some(Panel {
                px: main,
                label: (!channels.is_empty()).then(|| t!("figure-channels-merged")),
            }),
            channels,
            inset,
            scale,
            legend,
            marks,
            meta,
            caption,
            deep: true,
        };
        let format = c.format;
        let name = super::file_name(&target);
        let text = self.figure.text.take();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("figure-export", move |l| {
            let mut text = ready_text(text);
            let mut spec = spec;
            if let Some(p) = hash_path
                && let Ok(h) = specular_measure::provenance::hash_file(&p)
            {
                spec.meta.push(("blake3".into(), h));
            }
            let r = compose(&spec, &mut text)
                .ok_or_else(|| t!("figure-not-ready"))
                .and_then(|f| figure_bytes(&f, format, created))
                .and_then(|bytes| {
                    std::fs::write(&target, bytes).map_err(|e| crate::error_text::io_reason(&e))
                });
            let done = match r {
                Ok(()) => Done::Saved { name, skipped: 0 },
                Err(e) => Done::Failed(e),
            };
            let _ = tx.send((done, Some(text)));
            l.waker()();
        });
        Ok(rx)
    }

    /// The masks drawn with the burned-in text tool.
    pub(super) fn figure_user_masks(&self) -> ((u32, u32), Vec<Area>) {
        self.anon.masks()
    }

    /// Captures the current image as a key image.
    pub(super) fn figure_capture_key(
        &mut self,
        path: PathBuf,
        frame: u32,
    ) -> Result<KeyImage, String> {
        let id = self.figure_image()?;
        let (dw, dh) = self.figure_display_size();
        let ov = self.gpu_images.get(id).map_or(1.0, |g| g.overview_scale());
        let mut z = (KEY_SIDE / dw.max(dh) as f32).min(1.0);
        if ov > 0.0 && ov < 1.0 {
            z = z.min(ov);
        }
        let shot = Shot {
            stored: self.view.stored_size(),
            orient: self.view.orientation(),
            zoom: z,
            centre: (dw as f32 * 0.5, dh as f32 * 0.5),
            size: (
                ((dw as f32 * z).round() as u32).max(1),
                ((dh as f32 * z).round() as u32).max(1),
            ),
        };
        let opts = self.figure_opts(z >= 0.999);
        let px = self.figure_render(&shot, &opts)?;
        let from_stored = shot.view().screen_from_stored_f64();
        let summary = self
            .images
            .get(&id)
            .and_then(|i| i.session.as_ref())
            .and_then(|s| s.medical().cloned());
        let dicom = summary.is_some();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let pages = self.images.get(&id).map_or(1, |i| i.pages);
        let (title, anon_title) = match &summary {
            Some(m) => {
                let mut parts: Vec<String> = [m.modality.clone(), m.body_part.clone()]
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                let anon = parts.join(" · ");
                if let Some(s) = m
                    .series_description
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                {
                    parts.push(s);
                }
                parts.push(t!("figure-key-frame", n = i64::from(frame) + 1));
                (parts.join(" · "), anon)
            }
            None if pages > 1 => (
                format!(
                    "{name} · {}",
                    t!("figure-key-frame", n = i64::from(frame) + 1)
                ),
                String::new(),
            ),
            None => (name, String::new()),
        };
        Ok(KeyImage {
            path,
            frame,
            title,
            anon_title,
            px,
            from_stored,
            stored: self.view.stored_size(),
            measured: self.figure_measured(),
            dicom,
        })
    }

    /// Starts writing the key images to `target` (a PPTX file or a folder).
    pub(super) fn figure_export_keys(
        &mut self,
        target: PathBuf,
    ) -> Result<Receiver<Reply>, String> {
        let keys = self.figure.keys.clone();
        if keys.is_empty() {
            return Err(t!("figure-keys-empty"));
        }
        let c = self.figure.keys_choices.clone();
        let medical = self.figure.dialog.as_ref().is_some_and(|d| d.avail.medical);
        // A teaching case is always anonymised.
        let case = c.format == KeyFormat::Case && medical;
        let teaching = (c.teaching && medical) || case;
        let texts = build::CaseTexts {
            readme: t!("figure-case-readme"),
            notes: t!("figure-case-notes"),
            findings: t!("figure-case-findings"),
            none: t!("figure-case-none"),
        };
        let user = self.figure_user_masks();
        let name = super::file_name(&target);
        let title = if teaching {
            t!("figure-teaching-title")
        } else {
            t!("figure-keys-title")
        };
        let numbered: Vec<String> = (1..=keys.len())
            .map(|n| t!("figure-key-n", n = n as i64))
            .collect();
        let audit_on = self.settings.advanced.audit_log;
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("figure-export", move |l| {
            let session = specular_dicom::anon::Session::new();
            let folder = matches!(c.format, KeyFormat::Folder | KeyFormat::Case);
            let mut by_key: Vec<(usize, Vec<u8>)> = Vec::new();
            let mut skipped = 0;
            let mut kept: Vec<(KeyImage, Vec<Area>)> = Vec::new();
            let mut copies = Vec::new();
            for (i, k) in keys.iter().enumerate() {
                if teaching {
                    match teaching_one(k, &user, &session, folder) {
                        Some((areas, copy)) => {
                            let mut k = k.clone();
                            k.anon_title = if k.anon_title.is_empty() {
                                numbered[i].clone()
                            } else {
                                format!("{} · {}", k.anon_title, numbered[i])
                            };
                            if let Some(b) = copy {
                                if case {
                                    by_key.push((kept.len(), b));
                                } else {
                                    copies.push((format!("key-{:02}.dcm", kept.len() + 1), b));
                                }
                            }
                            kept.push((k, areas));
                        }
                        None => skipped += 1,
                    }
                } else {
                    kept.push((k.clone(), masks_for(k, &user)));
                }
            }
            let with: Vec<(&KeyImage, Vec<Area>)> =
                kept.iter().map(|(k, a)| (k, a.clone())).collect();
            let written = with.len();
            let anon_copies = copies.len() + by_key.len();
            let r = if with.is_empty() {
                Err(t!("figure-keys-none-left"))
            } else if case {
                let [y, mo, d, h, mi, s] = specular_convert::civil_from_unix(now_unix() as i64);
                let date = format!("{y:04}{mo:02}{d:02}");
                let time = format!("{h:02}{mi:02}{s:02}");
                build::tce(&with, &by_key, c.overlays, &c.notes, &texts, (&date, &time))
                    .and_then(|files| write_new(&target, &files))
            } else if folder {
                build::folder(&with, teaching, c.overlays, &c.notes).and_then(|mut files| {
                    files.extend(copies);
                    write_new(&target, &files)
                })
            } else {
                build::deck(&with, teaching, c.overlays, &c.notes, &title)
                    .and_then(|d| specular_figure::pptx::write(&d))
                    .and_then(|b| {
                        std::fs::write(&target, b).map_err(|e| crate::error_text::io_reason(&e))
                    })
            };
            if teaching && audit_on && r.is_ok() {
                let format = match c.format {
                    KeyFormat::Pptx => "pptx",
                    KeyFormat::Folder => "folder",
                    KeyFormat::Case => "case",
                };
                let extra = [
                    ("format", serde_json::json!(format)),
                    ("images", serde_json::json!(written)),
                    ("skipped", serde_json::json!(skipped)),
                    ("anonymised_copies", serde_json::json!(anon_copies)),
                    ("anonymised", serde_json::json!(true)),
                ];
                let unix = now_unix();
                let line = crate::app::audit::line(
                    unix,
                    crate::app::audit::Event::Export,
                    "teaching",
                    &[target.as_path()],
                    &extra,
                );
                if let Err(e) = crate::app::audit::append(&crate::app::audit::dir(), &line) {
                    specular_log::warn!(target: specular_log::UI, "audit log not written: {e}");
                }
            }
            let done = match r {
                Ok(()) => Done::Saved { name, skipped },
                Err(e) => Done::Failed(e),
            };
            let _ = tx.send((done, None));
            l.waker()();
        });
        Ok(rx)
    }
}
