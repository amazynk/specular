//! Client galleries (`specular_webgallery`): "Client gallery…" writes the selection, the
//! album or search the gallery shows, the folder, or the picks as a static web page folder,
//! and "Import client picks…" reads back what the client picked.
//!
//! - The gallery dialog: title and watermark fields, which photos, theme, long edge, format,
//!   what shows under each photo, the camera summary, and whether GPS stays (off by default).
//!   It says plainly that a static gallery cannot be protected by a password. Write gallery…
//!   asks for a folder, then renders on a background task through Develop's export (each
//!   photo with its edit); the dialog shows the progress and Stop.
//! - The picks dialog: a field for the client's pasted list and Choose picks file… for their
//!   `picks.txt` / `picks.json`; it shows how many names match the photos listed and which
//!   don't, and marks the matched ones (a pick, stars or a colour label) only on Apply. The
//!   change is one undo step.
//!
//! The choices last for the session.

#[cfg(test)]
mod shots;
pub(in crate::app) mod view;

use std::path::PathBuf;
use std::time::Instant;

use crossbeam_channel::Receiver;
use specular_core::CancelToken;
use specular_develop::export::batch::{self, Job};
use specular_develop::gpu::pool::Budget;
use specular_i18n::{I18n, t};
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_webgallery::build::{self, Built, Progress};
use specular_webgallery::picks::{self, Matched, Picks};
use specular_webgallery::{Captions, ImageFormat, Options, SIZES, Theme, Words};
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use self::view::{FieldView, FormView, Hit, Part, Row};
use super::App;
use super::gallery::Mark;

/// A chosen picks file: its name and text, nothing (the dialog was closed), or why it
/// could not be read.
type Chosen = Result<Option<(String, String)>, String>;
/// Names listed in the picks summary before "and N more".
const LIST_NAMES: usize = 6;
/// The largest picks file read.
const MAX_PICKS_FILE: u64 = 4 << 20;

/// Which photos a gallery holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Source {
    Current,
    Selection,
    /// What the gallery shows: a folder, an album or a search.
    Shown,
    Picks,
}

/// How matched photos are marked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum ApplyAs {
    Pick,
    Stars(u8),
    Label(u8),
}

impl ApplyAs {
    const ALL: [ApplyAs; 10] = [
        ApplyAs::Pick,
        ApplyAs::Stars(5),
        ApplyAs::Stars(4),
        ApplyAs::Stars(3),
        ApplyAs::Stars(2),
        ApplyAs::Stars(1),
        ApplyAs::Label(1),
        ApplyAs::Label(2),
        ApplyAs::Label(3),
        ApplyAs::Label(4),
    ];

    fn label(self) -> String {
        match self {
            ApplyAs::Pick => t!("clientpicks-as-pick"),
            ApplyAs::Stars(n) => t!("clientpicks-as-stars", stars = i64::from(n)),
            ApplyAs::Label(l) => t!(
                "clientpicks-as-label",
                label = super::gallery::mark_label_name(l)
            ),
        }
    }

    fn apply(self, m: Mark) -> Mark {
        match self {
            ApplyAs::Pick => Mark { flag: 1, ..m },
            ApplyAs::Stars(n) => Mark { rating: n, ..m },
            ApplyAs::Label(l) => Mark { label: l, ..m },
        }
    }
}

/// The dialog's choices, kept for the session (the title is the photos').
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Choices {
    pub theme: Theme,
    /// Index into [`SIZES`].
    pub size: usize,
    pub format: ImageFormat,
    pub captions: Captions,
    pub exif: bool,
    pub keep_location: bool,
    pub watermark: String,
    /// Index into [`ApplyAs::ALL`].
    pub apply: usize,
}

impl Default for Choices {
    fn default() -> Choices {
        Choices {
            theme: Theme::Light,
            size: 2,
            format: ImageFormat::Jpeg,
            captions: Captions::FileName,
            exif: false,
            keep_location: false,
            watermark: String::new(),
            apply: 0,
        }
    }
}

fn cycle<T: Copy + PartialEq>(all: &[T], v: T, d: i32) -> T {
    let i = all.iter().position(|x| *x == v).unwrap_or(0) as i32;
    all[(i + d).rem_euclid(all.len() as i32) as usize]
}

/// The open gallery dialog.
pub(in crate::app) struct GalleryDialog {
    /// Each source with what it is called ("Album “Smith”") and its photos.
    pub sources: Vec<(Source, String, Vec<PathBuf>)>,
    pub source: usize,
    pub title: LineEdit,
    pub watermark: LineEdit,
    pub focus: Part,
    pub preedit: String,
    /// The image open in Develop, with its edit as it stands.
    pub current: Option<(PathBuf, specular_develop::model::Edit)>,
}

/// The open picks dialog.
pub(in crate::app) struct PicksDialog {
    pub paste: LineEdit,
    /// Names read from a chosen file (the field shows them; editing it drops the file).
    pub from: Option<(String, Picks)>,
    pub photos: Vec<PathBuf>,
    pub matched: Matched,
    /// Names read.
    pub names: usize,
    pub focus: Part,
    pub preedit: String,
    pub error: Option<String>,
    choosing: Option<Receiver<Chosen>>,
}

impl PicksDialog {
    fn picks(&self) -> Picks {
        match &self.from {
            Some((_, p)) => p.clone(),
            None => picks::parse(self.paste.text()),
        }
    }

    fn rematch(&mut self) {
        let p = self.picks();
        self.names = p.names.len();
        self.matched = picks::match_photos(&p, &self.photos);
    }
}

/// A gallery being written.
struct Run {
    cancel: CancelToken,
    progress: Receiver<Progress>,
    done: Receiver<Result<Option<Built>, String>>,
    last: Progress,
}

/// The dialogs and a gallery being written.
#[derive(Default)]
pub(in crate::app) struct WebGallery {
    pub gallery: Option<GalleryDialog>,
    pub picks: Option<PicksDialog>,
    pub choices: Choices,
    pub drawn: view::Drawn,
    run: Option<Run>,
}

impl WebGallery {
    pub(in crate::app) fn is_open(&self) -> bool {
        self.gallery.is_some() || self.picks.is_some()
    }

    fn focus(&self) -> Option<Part> {
        self.gallery
            .as_ref()
            .map(|d| d.focus)
            .or_else(|| self.picks.as_ref().map(|d| d.focus))
    }

    /// An input method's composition for the focused field (empty: none).
    pub(in crate::app) fn set_preedit(&mut self, s: &str) {
        let field = self.focus().is_some_and(Part::is_field);
        let s = if field { s.to_owned() } else { String::new() };
        if let Some(d) = self.gallery.as_mut() {
            d.preedit = s;
        } else if let Some(d) = self.picks.as_mut() {
            d.preedit = s;
        }
    }

    pub(in crate::app) fn wants_text(&self) -> bool {
        self.run.is_none() && self.focus().is_some_and(Part::is_field)
    }

    pub(in crate::app) fn ime_area(&self) -> Option<specular_gfx::Rect> {
        self.focus()
            .is_some_and(Part::is_field)
            .then_some(self.drawn.ime)
            .flatten()
    }

    /// The focused field.
    fn field_mut(&mut self) -> Option<&mut LineEdit> {
        if let Some(d) = self.gallery.as_mut() {
            return match d.focus {
                Part::Title => Some(&mut d.title),
                Part::Watermark => Some(&mut d.watermark),
                _ => None,
            };
        }
        let d = self.picks.as_mut()?;
        (d.focus == Part::Paste).then_some(&mut d.paste)
    }

    /// After the picks field changed: its names, not a chosen file's.
    fn field_changed(&mut self) {
        if let Some(d) = self.picks.as_mut() {
            d.from = None;
            d.rematch();
        }
    }
}

fn field(label: String, f: &LineEdit, preedit: &str) -> FieldView {
    FieldView {
        label,
        text: f.text().to_owned(),
        caret: f.caret(),
        selection: f.selection(),
        preedit: preedit.to_owned(),
    }
}

fn theme_label(t: Theme) -> String {
    match t {
        Theme::Light => t!("webgallery-theme-light"),
        Theme::Dark => t!("webgallery-theme-dark"),
        Theme::Minimal => t!("webgallery-theme-minimal"),
    }
}

fn captions_label(c: Captions) -> String {
    match c {
        Captions::Off => t!("webgallery-captions-off"),
        Captions::FileName => t!("webgallery-captions-name"),
        Captions::Title => t!("webgallery-captions-title"),
        Captions::All => t!("webgallery-captions-all"),
    }
}

/// The gallery dialog as drawn; `busy` the progress of a run.
pub(in crate::app) fn gallery_view(
    d: &GalleryDialog,
    c: &Choices,
    busy: Option<&Progress>,
) -> FormView {
    let (_, what, photos) = &d.sources[d.source.min(d.sources.len() - 1)];
    let pre = |p: Part| if d.focus == p { d.preedit.as_str() } else { "" };
    let rows = vec![
        Row::Field(
            Part::Title,
            field(t!("webgallery-title-label"), &d.title, pre(Part::Title)),
        ),
        Row::Choice(
            Part::Source,
            t!("webgallery-source"),
            t!(
                "webgallery-source-value",
                what = what.clone(),
                count = photos.len() as i64
            ),
        ),
        Row::Choice(Part::Theme, t!("webgallery-theme"), theme_label(c.theme)),
        Row::Choice(
            Part::Size,
            t!("webgallery-size"),
            t!(
                "webgallery-size-value",
                px = i64::from(SIZES[c.size % SIZES.len()])
            ),
        ),
        Row::Choice(
            Part::Format,
            t!("webgallery-format"),
            match c.format {
                ImageFormat::Jpeg => "JPEG".to_owned(),
                ImageFormat::Webp => "WebP".to_owned(),
            },
        ),
        Row::Choice(
            Part::Captions,
            t!("webgallery-captions"),
            captions_label(c.captions),
        ),
        Row::Field(
            Part::Watermark,
            field(
                t!("webgallery-watermark-label"),
                &d.watermark,
                pre(Part::Watermark),
            ),
        ),
        Row::Check(Part::Exif, t!("webgallery-exif"), c.exif),
        Row::Check(Part::Location, t!("webgallery-location"), c.keep_location),
        Row::Note(
            format!(
                "{}\n{}",
                t!("webgallery-note-private"),
                t!("webgallery-note-share")
            ),
            true,
        ),
        Row::Note(t!("webgallery-note-picks"), false),
    ];
    FormView {
        heading: t!("webgallery-heading"),
        subtitle: t!("webgallery-subtitle", count = photos.len() as i64),
        rows,
        busy: busy.map(|p| {
            t!(
                "webgallery-busy",
                done = p.done as i64,
                total = p.total as i64
            )
        }),
        cancel: if busy.is_some() {
            t!("webgallery-stop")
        } else {
            t!("app-cancel")
        },
        ok: t!("webgallery-write"),
        ok_enabled: !photos.is_empty(),
        focus: d.focus,
    }
}

/// `names` as a short list: the first few, then "and N more".
fn name_list(names: &[String]) -> String {
    let shown = names
        .iter()
        .take(LIST_NAMES)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > LIST_NAMES {
        t!(
            "clientpicks-more",
            names = shown,
            more = (names.len() - LIST_NAMES) as i64
        )
    } else {
        shown
    }
}

/// The picks dialog as drawn.
pub(in crate::app) fn picks_view(d: &PicksDialog, c: &Choices) -> FormView {
    let mut rows = vec![
        Row::Field(
            Part::Paste,
            field(
                t!("clientpicks-paste-label"),
                &d.paste,
                if d.focus == Part::Paste {
                    &d.preedit
                } else {
                    ""
                },
            ),
        ),
        Row::Button(Part::Choose, t!("clientpicks-choose")),
    ];
    if let Some((name, _)) = &d.from {
        rows.push(Row::Note(
            t!("clientpicks-from-file", name = name.clone()),
            false,
        ));
    }
    rows.push(Row::Choice(
        Part::ApplyAs,
        t!("clientpicks-apply-as"),
        ApplyAs::ALL[c.apply % ApplyAs::ALL.len()].label(),
    ));
    if let Some(e) = &d.error {
        rows.push(Row::Note(e.clone(), true));
    } else if d.names == 0 {
        rows.push(Row::Note(t!("clientpicks-empty"), true));
    } else {
        let m = &d.matched;
        let mut lines = vec![t!(
            "clientpicks-matched",
            matched = m.found.len() as i64,
            total = d.names as i64
        )];
        if !m.missing.is_empty() {
            lines.push(t!("clientpicks-missing", names = name_list(&m.missing)));
        }
        if !m.ambiguous.is_empty() {
            lines.push(t!("clientpicks-ambiguous", names = name_list(&m.ambiguous)));
        }
        rows.push(Row::Note(lines.join("\n"), true));
    }
    FormView {
        heading: t!("clientpicks-heading"),
        subtitle: t!("clientpicks-subtitle", count = d.photos.len() as i64),
        rows,
        busy: None,
        cancel: t!("app-cancel"),
        ok: t!("clientpicks-apply"),
        ok_enabled: !d.matched.found.is_empty(),
        focus: d.focus,
    }
}

/// The page's words in the language shown now.
fn page_words(rtl: bool) -> Words {
    Words {
        lang: I18n::global().localizer().language().to_string(),
        rtl,
        count: String::new(),
        hint: t!("webgallery-page-hint"),
        picked: t!("webgallery-page-picked"),
        pick: t!("webgallery-page-pick"),
        copy: t!("webgallery-page-copy"),
        copied: t!("webgallery-page-copied"),
        copy_by_hand: t!("webgallery-page-copy-by-hand"),
        download_txt: t!("webgallery-page-download-txt"),
        download_json: t!("webgallery-page-download-json"),
        clear: t!("webgallery-page-clear"),
        clear_ask: t!("webgallery-page-clear-ask"),
        no_storage: t!("webgallery-page-no-storage"),
        noscript: t!("webgallery-page-noscript"),
        previous: t!("webgallery-page-previous"),
        next: t!("webgallery-page-next"),
        close: t!("webgallery-page-close"),
        position: t!("webgallery-page-position"),
        viewer: t!("webgallery-page-viewer"),
        picks: t!("webgallery-page-picks"),
    }
}

impl App {
    /// The photos a gallery or the picks can come from, the default first.
    fn web_gallery_sources(&mut self) -> (Vec<(Source, String, Vec<PathBuf>)>, usize) {
        let mut sources = Vec::new();
        let mut first = 0;
        if self.gallery_active() {
            let sel: Vec<PathBuf> = self
                .gallery
                .targets()
                .into_iter()
                .filter(|p| !p.is_dir())
                .collect();
            if sel.len() > 1 {
                sources.push((Source::Selection, t!("webgallery-source-selection"), sel));
            }
            let shown = self.gallery.image_order();
            if !shown.is_empty() {
                let what = match self.gallery.shown_scope() {
                    (Some(name), _) => t!("webgallery-source-album", name = name),
                    (None, true) => t!("webgallery-source-search"),
                    (None, false) => t!("webgallery-source-folder"),
                };
                sources.push((Source::Shown, what, shown));
            }
        } else {
            if let Some(p) = self.current.and_then(|i| self.entries.get(i)) {
                sources.push((
                    Source::Current,
                    t!("webgallery-source-current"),
                    vec![p.clone()],
                ));
            }
            if self.entries.len() > 1 {
                sources.push((
                    Source::Shown,
                    t!("webgallery-source-folder"),
                    self.entries.clone(),
                ));
                first = sources.len() - 1;
            }
        }
        let picks = self.marked_files();
        if !picks.is_empty() {
            sources.push((Source::Picks, t!("webgallery-source-picks"), picks));
        }
        (sources, first)
    }

    /// "Client gallery…": opens the dialog (or shows the gallery being written).
    pub(super) fn web_gallery_open(&mut self) {
        if self.web_gallery.picks.is_some() {
            return;
        }
        if self.web_gallery.gallery.is_some() {
            self.redraw = true;
            return;
        }
        let (sources, first) = self.web_gallery_sources();
        if sources.is_empty() {
            return;
        }
        let title = match self.gallery_active().then(|| self.gallery.shown_scope()) {
            Some((Some(album), _)) => album.rsplit('/').next().unwrap_or(&album).to_owned(),
            _ => sources[first]
                .2
                .first()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .map_or_else(
                    || t!("webgallery-title-default"),
                    |n| n.to_string_lossy().into_owned(),
                ),
        };
        let mut title = LineEdit::new(&title);
        title.select_all();
        let current = self.develop_open_edit();
        self.web_gallery.gallery = Some(GalleryDialog {
            sources,
            source: first,
            title,
            watermark: LineEdit::new(&self.web_gallery.choices.watermark),
            focus: Part::Title,
            preedit: String::new(),
            current,
        });
        self.chrome.announce_status(t!("webgallery-heading"));
        self.redraw = true;
    }

    /// "Import client picks…": opens the dialog for the photos listed.
    pub(super) fn client_picks_open(&mut self) {
        if self.web_gallery.is_open() {
            return;
        }
        let photos = if self.gallery_active() {
            self.gallery_images()
        } else {
            self.entries.clone()
        };
        if photos.is_empty() {
            return;
        }
        let mut d = PicksDialog {
            paste: LineEdit::new(""),
            from: None,
            photos,
            matched: Matched::default(),
            names: 0,
            focus: Part::Paste,
            preedit: String::new(),
            error: None,
            choosing: None,
        };
        d.rematch();
        self.web_gallery.picks = Some(d);
        self.chrome.announce_status(t!("clientpicks-heading"));
        self.redraw = true;
    }

    pub(in crate::app) fn web_gallery_view(&self) -> Option<FormView> {
        let w = &self.web_gallery;
        if let Some(d) = &w.gallery {
            return Some(gallery_view(d, &w.choices, w.run.as_ref().map(|r| &r.last)));
        }
        w.picks.as_ref().map(|d| picks_view(d, &w.choices))
    }

    fn web_gallery_close(&mut self) {
        let w = &mut self.web_gallery;
        if let Some(d) = &w.gallery {
            w.choices.watermark = d.watermark.text().to_owned();
        }
        w.gallery = None;
        w.picks = None;
        w.drawn = view::Drawn::default();
        self.redraw = true;
    }

    fn web_gallery_step(&mut self, part: Part, d: i32) {
        let w = &mut self.web_gallery;
        if w.run.is_some() {
            return;
        }
        let c = &mut w.choices;
        match part {
            Part::Source => {
                if let Some(g) = w.gallery.as_mut() {
                    let n = g.sources.len() as i32;
                    g.source = (g.source as i32 + d).rem_euclid(n) as usize;
                }
            }
            Part::Theme => c.theme = cycle(&Theme::ALL, c.theme, d),
            Part::Size => c.size = (c.size as i32 + d).rem_euclid(SIZES.len() as i32) as usize,
            Part::Format => c.format = cycle(&ImageFormat::ALL, c.format, d),
            Part::Captions => c.captions = cycle(&Captions::ALL, c.captions, d),
            Part::ApplyAs => {
                c.apply = (c.apply as i32 + d).rem_euclid(ApplyAs::ALL.len() as i32) as usize
            }
            _ => return,
        }
        self.redraw = true;
    }

    /// A button, a checkbox or a choice (a click steps it on).
    fn web_gallery_hit(&mut self, part: Part) {
        let w = &mut self.web_gallery;
        if let Some(d) = w.gallery.as_mut() {
            d.focus = part;
            d.preedit.clear();
        } else if let Some(d) = w.picks.as_mut() {
            d.focus = part;
            d.preedit.clear();
        }
        let busy = w.run.is_some();
        let gallery = w.gallery.is_some();
        match part {
            Part::Cancel if busy => {
                if let Some(r) = &self.web_gallery.run {
                    r.cancel.cancel();
                }
            }
            Part::Cancel => self.web_gallery_close(),
            _ if busy => {}
            Part::Exif => self.web_gallery.choices.exif ^= true,
            Part::Location => self.web_gallery.choices.keep_location ^= true,
            Part::Choose => self.client_picks_choose(),
            Part::Ok if gallery => self.web_gallery_write(),
            Part::Ok => self.client_picks_apply(),
            p => self.web_gallery_step(p, 1),
        }
        self.redraw = true;
    }

    /// Asks for a folder, then writes the gallery on a background task.
    fn web_gallery_write(&mut self) {
        let Some(d) = self.web_gallery.gallery.as_ref() else {
            return;
        };
        let (_, _, paths) = &d.sources[d.source.min(d.sources.len() - 1)];
        if paths.is_empty() || self.web_gallery.run.is_some() {
            return;
        }
        let paths = paths.clone();
        let current = d.current.clone();
        let c = &self.web_gallery.choices;
        let title = d.title.text().trim();
        let opts = Options {
            title: if title.is_empty() {
                t!("webgallery-title-default")
            } else {
                title.to_owned()
            },
            theme: c.theme,
            long_edge: SIZES[c.size % SIZES.len()],
            format: c.format,
            captions: c.captions,
            exif: c.exif,
            keep_location: c.keep_location,
            watermark: d.watermark.text().trim().to_owned(),
            words: page_words(self.rtl()),
        };
        let start_dir = paths.first().and_then(|p| p.parent()).map(PathBuf::from);
        let gpu = self.gfx.as_ref().map(|g| {
            (
                g.gpu.device.clone(),
                g.gpu.queue.clone(),
                g.gpu.info.device_type != specular_gfx::wgpu::DeviceType::DiscreteGpu,
            )
        });
        super::develop::ready_export(gpu.as_ref().map(|g| (g.0.clone(), g.1.clone())));
        let registry = self.loader.registry();
        let lang = I18n::global().localizer().format_language().clone();
        let cancel = CancelToken::new();
        let c2 = cancel.clone();
        let (ptx, prx) = crossbeam_channel::unbounded();
        let (rtx, rrx) = crossbeam_channel::bounded(1);
        let total = paths.len();
        self.loader.background("webgallery", move |l| {
            let wake = l.waker();
            let mut dlg = rfd::FileDialog::new();
            if let Some(dir) = &start_dir {
                dlg = dlg.set_directory(dir);
            }
            let Some(parent) = dlg.pick_folder() else {
                let _ = rtx.send(Ok(None));
                wake();
                return;
            };
            let jobs: Vec<Job> = paths
                .iter()
                .map(|p| Job {
                    source: p.clone(),
                    edit: match &current {
                        Some((cp, e)) if cp == p => e.clone(),
                        _ => super::develop::saved_edit(p),
                    },
                    orient: None,
                })
                .collect();
            let opened = match gpu {
                Some(g) => Ok(g),
                None => batch::open_device(),
            };
            let r = match opened {
                Ok((device, queue, integrated)) => {
                    let loc = crate::loader::meta_localizer(lang);
                    let w = wake.clone();
                    let t0 = Instant::now();
                    let r = build::build(
                        &device,
                        &queue,
                        Budget::for_device(integrated, None, None),
                        jobs,
                        &opts,
                        &registry,
                        &parent,
                        &|n| t!("webgallery-page-count", count = n as i64),
                        &loc,
                        &c2,
                        &move |p| {
                            let _ = ptx.send(p);
                            w();
                        },
                    );
                    specular_log::info!(
                        "client gallery: {total} photo(s) in {:.1} s",
                        t0.elapsed().as_secs_f64()
                    );
                    r.map(Some)
                }
                Err(e) => Err(e),
            };
            let _ = rtx.send(r);
            wake();
        });
        self.web_gallery.run = Some(Run {
            cancel,
            progress: prx,
            done: rrx,
            last: Progress {
                done: 0,
                total,
                fraction: 0.0,
            },
        });
        if let Some(d) = self.web_gallery.gallery.as_mut() {
            d.focus = Part::Cancel;
        }
        self.redraw = true;
    }

    /// Choose picks file…: the platform's file dialog, off the UI thread.
    fn client_picks_choose(&mut self) {
        let Some(d) = self.web_gallery.picks.as_mut() else {
            return;
        };
        if d.choosing.is_some() {
            return;
        }
        let start = d.photos.first().and_then(|p| p.parent()).map(PathBuf::from);
        let (tx, rx) = crossbeam_channel::bounded(1);
        d.choosing = Some(rx);
        self.loader.background("dlg-picks", move |l| {
            let mut dlg = rfd::FileDialog::new().add_filter("picks", &["txt", "json"]);
            if let Some(s) = &start {
                dlg = dlg.set_directory(s);
            }
            let r = match dlg.pick_file() {
                None => Ok(None),
                Some(p) => {
                    let name = p
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    let read = std::fs::metadata(&p)
                        .and_then(|m| {
                            if m.len() > MAX_PICKS_FILE {
                                Err(std::io::Error::other("larger than 4 MB"))
                            } else {
                                std::fs::read(&p)
                            }
                        })
                        .map_err(|e| crate::error_text::io_reason(&e));
                    match read {
                        Ok(b) => Ok(Some((name, String::from_utf8_lossy(&b).into_owned()))),
                        Err(e) => Err(t!("clientpicks-read-failed", name = name, reason = e)),
                    }
                }
            };
            let _ = tx.send(r);
            l.waker()();
        });
    }

    /// Apply: marks the matched photos (one undo step).
    fn client_picks_apply(&mut self) {
        let Some(d) = self.web_gallery.picks.as_ref() else {
            return;
        };
        let photos = d.matched.photos();
        if photos.is_empty() {
            return;
        }
        let how = ApplyAs::ALL[self.web_gallery.choices.apply % ApplyAs::ALL.len()];
        self.gallery_marks_ready();
        let undo = self.gallery_set_marks(&photos, |m| how.apply(m));
        self.web_gallery_close();
        let now = self.now();
        let msg = if undo.is_some() {
            t!("clientpicks-applied", count = photos.len() as i64)
        } else {
            t!("clientpicks-unchanged")
        };
        self.chrome.announce_status(msg.clone());
        self.chrome.toast(msg, undo, now);
        self.redraw = true;
    }

    /// Progress, the end of a run, and a chosen picks file.
    pub(in crate::app) fn web_gallery_tick(&mut self) {
        if let Some(d) = self.web_gallery.picks.as_mut()
            && let Some(rx) = &d.choosing
            && let Ok(r) = rx.try_recv()
        {
            d.choosing = None;
            match r {
                Ok(Some((name, text))) => {
                    let p = picks::parse(&text);
                    d.paste.set_text(&p.names.join(", "));
                    d.from = Some((name, p));
                    d.error = None;
                    d.rematch();
                }
                Ok(None) => {}
                Err(e) => d.error = Some(e),
            }
            self.redraw = true;
        }
        let Some(run) = self.web_gallery.run.as_mut() else {
            return;
        };
        while let Ok(p) = run.progress.try_recv() {
            run.last = p;
            self.redraw = true;
        }
        let Ok(r) = run.done.try_recv() else { return };
        self.web_gallery.run = None;
        self.redraw = true;
        let now = self.now();
        let msg = match r {
            // The folder dialog was closed: the gallery dialog stays.
            Ok(None) => {
                if let Some(d) = self.web_gallery.gallery.as_mut() {
                    d.focus = Part::Ok;
                }
                return;
            }
            Ok(Some(b)) if b.cancelled => t!("webgallery-stopped"),
            Ok(Some(b)) if b.written == 0 => {
                for (p, e) in &b.failed {
                    specular_log::warn!("client gallery: {}: {e}", p.display());
                }
                t!("webgallery-nothing")
            }
            Ok(Some(b)) => {
                for (p, e) in &b.failed {
                    specular_log::warn!("client gallery: {}: {e}", p.display());
                }
                let mut m = t!(
                    "webgallery-done",
                    count = b.written as i64,
                    folder = b.dir.display().to_string()
                );
                if !b.failed.is_empty() {
                    m = format!(
                        "{m} {}",
                        t!("webgallery-done-failed", failed = b.failed.len() as i64)
                    );
                }
                m
            }
            Err(e) => t!("webgallery-failed", reason = e),
        };
        self.web_gallery_close();
        self.chrome.announce_status(msg.clone());
        self.chrome.toast(msg, None, now);
    }

    /// Keys while a dialog is open (it takes them all); `text` is what the key types.
    pub(in crate::app) fn web_gallery_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) {
        let Some(v) = self.web_gallery_view() else {
            return;
        };
        let parts = v.parts();
        let focus = v.focus;
        if focus.is_field()
            && self.web_gallery.run.is_none()
            && self.web_gallery_field_key(combo, text)
        {
            return;
        }
        let Some(combo) = combo else { return };
        let rtl = self.rtl();
        let set = |me: &mut App, p: Part| {
            let w = &mut me.web_gallery;
            if let Some(d) = w.gallery.as_mut() {
                d.focus = p;
                d.preedit.clear();
            } else if let Some(d) = w.picks.as_mut() {
                d.focus = p;
                d.preedit.clear();
            }
            me.redraw = true;
        };
        let step = |me: &mut App, k: i32| {
            let i = parts.iter().position(|p| *p == focus).unwrap_or(0) as i32;
            set(me, parts[(i + k).rem_euclid(parts.len() as i32) as usize]);
        };
        let choice = matches!(
            focus,
            Part::Source | Part::Theme | Part::Size | Part::Format | Part::Captions | Part::ApplyAs
        );
        match combo.key {
            Key::Named(NamedKey::Escape) => self.web_gallery_hit(Part::Cancel),
            Key::Named(NamedKey::Tab) if combo.mods.shift => step(self, -1),
            Key::Named(NamedKey::Tab) => step(self, 1),
            Key::Named(NamedKey::ArrowUp) if combo.mods == Mods::NONE => step(self, -1),
            Key::Named(NamedKey::ArrowDown) if combo.mods == Mods::NONE => step(self, 1),
            Key::Named(n @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if choice => {
                let right = n == NamedKey::ArrowRight;
                self.web_gallery_step(focus, if right != rtl { 1 } else { -1 });
            }
            Key::Named(NamedKey::Enter) => {
                self.web_gallery_hit(if focus.is_field() { Part::Ok } else { focus })
            }
            Key::Named(NamedKey::Space) if !focus.is_field() => self.web_gallery_hit(focus),
            _ => {}
        }
    }

    /// Editing keys in the focused field; false when the key is not one.
    fn web_gallery_field_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let rtl = {
            let w = &self.web_gallery;
            let focus = w.focus();
            w.drawn
                .fields
                .iter()
                .find(|(p, ..)| Some(*p) == focus)
                .is_some_and(|(_, b, _)| b.rtl)
        };
        let Some(f) = self.web_gallery.field_mut() else {
            return false;
        };
        if let Some(c) = combo {
            let mut clip = PendingClipboard::default();
            let before = f.text().to_owned();
            if f.key(c.key, c.mods, rtl, &mut clip).used() {
                let changed = f.text() != before;
                if changed {
                    self.web_gallery.field_changed();
                }
                self.field_clipboard(clip, super::integration::PasteTo::WebGallery);
                self.redraw = true;
                return true;
            }
            if c.mods.ctrl || c.mods.alt || c.mods.meta {
                return false;
            }
        }
        let Some(t) = text else { return false };
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return false;
        }
        self.last_key_text = Some((s.clone(), Instant::now()));
        self.web_gallery_text(&s);
        true
    }

    /// Text into the focused field (typed keys, input-method commits).
    pub(in crate::app) fn web_gallery_text(&mut self, s: &str) {
        let w = &mut self.web_gallery;
        if let Some(d) = w.gallery.as_mut() {
            d.preedit.clear();
        } else if let Some(d) = w.picks.as_mut() {
            d.preedit.clear();
        }
        if let Some(f) = w.field_mut()
            && f.insert(s)
        {
            w.field_changed();
            self.redraw = true;
        }
    }

    /// Text pasted into the focused field: a pasted list's lines become one line of names.
    pub(in crate::app) fn web_gallery_paste(&mut self, s: &str) {
        let w = &mut self.web_gallery;
        let list = w.picks.is_some();
        let s = if list {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            s.to_owned()
        };
        if let Some(f) = w.field_mut()
            && f.paste(&s)
        {
            w.field_changed();
        }
        self.redraw = true;
    }

    /// A click (logical px) while a dialog is open: a press in a field puts the caret.
    pub(in crate::app) fn web_gallery_click(&mut self, p: (f32, f32), down: bool) {
        let now = self.now();
        let shift = self.mods.shift_key();
        let w = &mut self.web_gallery;
        if !down {
            if let Some(d) = w.gallery.as_mut() {
                d.title.release();
                d.watermark.release();
            } else if let Some(d) = w.picks.as_mut() {
                d.paste.release();
            }
            return;
        }
        let Some(&(_, hit)) = w.drawn.hits.iter().rev().find(|(r, _)| r.contains(p)) else {
            if !w.drawn.sheet.contains(p) && w.run.is_none() {
                self.web_gallery_close();
            }
            return;
        };
        match hit {
            Hit::Dec(part) => self.web_gallery_step(part, -1),
            Hit::Inc(part) => self.web_gallery_step(part, 1),
            Hit::Part(part) if part.is_field() => {
                let at = w
                    .drawn
                    .fields
                    .iter()
                    .find(|(q, ..)| *q == part)
                    .map(|(_, b, x0)| b.hit_byte(p.0 - x0));
                if let Some(d) = w.gallery.as_mut() {
                    d.focus = part;
                    d.preedit.clear();
                } else if let Some(d) = w.picks.as_mut() {
                    d.focus = part;
                    d.preedit.clear();
                }
                if let (Some(at), Some(f)) = (at, w.field_mut()) {
                    f.press(at.min(f.text().len()), p.0, now, shift);
                }
                self.redraw = true;
            }
            Hit::Part(part) => self.web_gallery_hit(part),
        }
    }

    /// The pointer moved while a dialog is open: a press in a field drags a selection.
    pub(in crate::app) fn web_gallery_drag(&mut self, p: (f32, f32)) {
        let w = &mut self.web_gallery;
        let x0 = w
            .focus()
            .and_then(|f| w.drawn.fields.iter().find(|(q, ..)| *q == f))
            .map(|(_, b, x0)| (b.clone(), *x0));
        if let (Some((b, x0)), Some(f)) = (x0, w.field_mut())
            && f.dragging()
        {
            let at = b.hit_byte(p.0 - x0).min(f.text().len());
            f.drag_to(at);
            self.redraw = true;
        }
    }

    /// A screen reader's request; true when the node is a dialog's.
    pub(in crate::app) fn web_gallery_a11y_request(
        &mut self,
        req: &accesskit::ActionRequest,
    ) -> bool {
        if !self.web_gallery.is_open() {
            return false;
        }
        let Some(part) = view::a11y_target(req.target_node.0) else {
            return false;
        };
        match req.action {
            accesskit::Action::Focus => {
                let w = &mut self.web_gallery;
                if let Some(d) = w.gallery.as_mut() {
                    d.focus = part;
                } else if let Some(d) = w.picks.as_mut() {
                    d.focus = part;
                }
            }
            accesskit::Action::Click => self.web_gallery_hit(part),
            accesskit::Action::Increment => self.web_gallery_step(part, 1),
            accesskit::Action::Decrement => self.web_gallery_step(part, -1),
            accesskit::Action::SetValue => {
                if let Some(accesskit::ActionData::Value(v)) = req.data.as_ref() {
                    let w = &mut self.web_gallery;
                    if let Some(d) = w.gallery.as_mut() {
                        d.focus = part;
                    } else if let Some(d) = w.picks.as_mut() {
                        d.focus = part;
                    }
                    if let Some(f) = w.field_mut() {
                        f.set_text(v);
                        w.field_changed();
                    }
                }
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// `--show client-gallery` / `--show client-picks` (screenshots).
    pub(in crate::app) fn web_gallery_show(&mut self, what: &str) -> bool {
        match what {
            "client-gallery" => self.web_gallery_open(),
            "client-picks" => self.client_picks_open(),
            _ => return false,
        }
        true
    }
}
