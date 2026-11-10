//! "Find text" ([`Action::FindText`]): the text in the picture shown, read by the Local AI
//! pack's Text models (`specular_ai::ocr`), listed in the palette's sheet with each line's
//! outline and words drawn over the image. Choosing a row copies its line; the first row
//! copies all of it, in reading order. Right-to-left lines (Hebrew, Arabic) come in logical
//! order and are drawn by the text renderer's bidi.
//!
//! The models load on the first use (on the background thread that reads, and stay loaded
//! for the next picture); the detector runs on the app's GPU when it matched the CPU there.
//! Nothing is downloaded here: without the Text models the sheet says where to get them.
//!
//! [`Action::FindText`]: specular_input::Action::FindText

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use specular_ai::ocr::{Ocr, OcrFiles, TextLine};
use specular_core::CancelToken;
use specular_decode::{DecodeRequest, Session};
use specular_gfx::{Icon, wgpu};
use specular_i18n::t;
use specular_widgets::PaletteItem;

use super::App;
use super::ai::device::Session as AiSession;
use super::script::Placed;
use crate::loader::{Copied, ImageId};

/// Rows: "Copy all" is `ITEM_BASE`, line k is `ITEM_BASE + 1 + k`.
pub(super) const ITEM_BASE: u64 = (1 << 44) + (1 << 20);
/// Pictures are read at most this big (larger ones at half, quarter… size).
const MAX_PIXELS: u64 = 24_000_000;
const MAX_ROWS: usize = 500;
/// The outline colour (cyan: apart from codes' amber).
const COLOUR: [u8; 4] = [0, 200, 255, 255];

struct Found {
    req: u64,
    image: ImageId,
    lines: Result<Vec<TextLine>, String>,
}

pub(super) struct FindText {
    tx: Sender<Found>,
    rx: Receiver<Found>,
    req: u64,
    /// The image the text was read from, and its lines (quads in stored pixels).
    image: Option<ImageId>,
    lines: Vec<TextLine>,
    /// A reading is under way (screenshots wait for it).
    pub(super) pending: bool,
}

impl FindText {
    pub(super) fn new() -> FindText {
        let (tx, rx) = channel();
        FindText {
            tx,
            rx,
            req: 0,
            image: None,
            lines: Vec::new(),
            pending: false,
        }
    }
}

/// The loaded models, the files they came from (reloaded when those change) and the GPU
/// session they were loaded in (reloaded on the new device after a GPU reset).
type Loaded = Option<(OcrFiles, Ocr, AiSession)>;

fn engine() -> &'static Mutex<Loaded> {
    static E: OnceLock<Mutex<Loaded>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(None))
}

/// The Text feature's installed files (read from the pack's store on this thread).
fn installed_files() -> Option<OcrFiles> {
    let data = specular_settings::Locations::detect().data_dir;
    let store = specular_ai::install::Store::new(&data);
    OcrFiles::find(
        &specular_ai::Manifest::load(),
        store.root(),
        &store.installed(),
    )
}

/// A line for a row and an outline label: one line, at most 80 characters.
/// The "95% sure" chip: the locale's percent in its own isolate, the whole chip laid out in
/// the UI's direction (a right-to-left chip starts with an RLM, so in Hebrew the percent
/// reads after the word, on its left).
pub(crate) fn confidence_chip(percent: &str, dir: specular_i18n::Direction) -> String {
    let p = specular_i18n::isolate_label(percent, specular_i18n::Direction::Ltr);
    specular_i18n::isolate_label(&t!("ocr-confidence", percent = p), dir)
}

fn row_text(text: &str) -> String {
    let one: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let one = one.trim();
    if one.chars().count() > 80 {
        let cut: String = one.chars().take(79).collect();
        format!("{cut}…")
    } else {
        one.to_string()
    }
}

/// All lines, in reading order, one per line.
fn all_text(lines: &[TextLine]) -> String {
    lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

impl App {
    /// Reads the text in the picture shown; the list opens at once and fills when done.
    pub(super) fn find_text(&mut self) {
        let Some(id) = self.current_id() else { return };
        let Some(session) = self.images.get(&id).and_then(|i| i.session.clone()) else {
            return;
        };
        self.find_text.req += 1;
        self.find_text.image = Some(id);
        self.find_text.lines.clear();
        self.find_text.pending = true;
        let req = self.find_text.req;
        self.chrome
            .open_list(t!("ocr-placeholder"), Vec::new(), Some(t!("ocr-looking")));
        let tx = self.find_text.tx.clone();
        let wake = self.loader.waker();
        let gpu = self
            .gfx
            .as_ref()
            .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone()));
        self.loader.background("ocr-read", move |_| {
            let lines = read(&session, gpu);
            let _ = tx.send(Found {
                req,
                image: id,
                lines,
            });
            wake();
        });
        self.redraw = true;
    }

    /// Once per event-loop turn: the text read.
    pub(super) fn drain_find_text(&mut self) {
        while let Ok(f) = self.find_text.rx.try_recv() {
            if f.req != self.find_text.req {
                continue;
            }
            self.find_text.pending = false;
            match f.lines {
                Ok(l) => {
                    self.find_text.image = Some(f.image);
                    self.find_text.lines = l;
                    let items = self.find_text_items();
                    let empty = items.is_empty().then(|| t!("ocr-none"));
                    self.chrome.update_list(items, empty);
                }
                Err(e) => {
                    self.find_text.lines.clear();
                    self.chrome.update_list(Vec::new(), Some(e));
                }
            }
            self.redraw = true;
        }
    }

    fn find_text_items(&self) -> Vec<PaletteItem> {
        let lines = &self.find_text.lines;
        if lines.is_empty() {
            return Vec::new();
        }
        let i18n = specular_i18n::I18n::global();
        let loc = i18n.localizer();
        let dir = i18n.direction();
        let mut items = vec![PaletteItem {
            id: ITEM_BASE,
            label: t!("ocr-copy-all"),
            key: None,
            unavailable: None,
            icon: Icon::Copy,
        }];
        for (k, l) in lines.iter().enumerate().take(MAX_ROWS) {
            items.push(PaletteItem {
                id: ITEM_BASE + 1 + k as u64,
                label: t!("ocr-copy", text = row_text(&l.text)),
                key: Some(confidence_chip(
                    &loc.percent(f64::from(l.confidence), 0),
                    dir,
                )),
                unavailable: None,
                icon: Icon::Copy,
            });
        }
        items
    }

    /// A row of ours was chosen.
    pub(super) fn find_text_item(&mut self, id: u64) -> bool {
        if !(ITEM_BASE..=ITEM_BASE + MAX_ROWS as u64).contains(&id) {
            return false;
        }
        let text = if id == ITEM_BASE {
            all_text(&self.find_text.lines)
        } else {
            match self.find_text.lines.get((id - ITEM_BASE - 1) as usize) {
                Some(l) => l.text.clone(),
                None => return true,
            }
        };
        if !text.is_empty() {
            self.clipboard.copy_text(&self.loader, text, Copied::Value);
        }
        true
    }

    /// The lines' outlines and words on screen (logical px) while their list is open on
    /// their image.
    pub(super) fn find_text_placed(&self) -> Vec<Placed> {
        if self.find_text.lines.is_empty()
            || !self.chrome.list_open()
            || self.find_text.image != self.current_id()
        {
            return Vec::new();
        }
        let ls = self.dl.scale.max(0.1);
        let to_screen = self.view.stored_from_screen().invert();
        self.find_text
            .lines
            .iter()
            .map(|l| {
                let mut pts: Vec<(f32, f32)> = l
                    .quad
                    .iter()
                    .map(|&p| {
                        let q = to_screen.apply((p[0], p[1]));
                        (q.0 / ls, q.1 / ls)
                    })
                    .collect();
                pts.push(pts[0]);
                Placed {
                    kind: crate::remote::protocol::ShapeKind::Polyline,
                    pts,
                    text: row_text(&l.text).chars().take(48).collect(),
                    color: COLOUR,
                    width: 2.0,
                }
            })
            .collect()
    }
}

/// Decodes the picture (reduced when very large) and reads its text with the installed
/// models (loaded on first use); quads in stored pixels. The error is the sheet's words.
fn read(
    session: &Arc<dyn Session>,
    gpu: Option<(wgpu::Device, wgpu::Queue)>,
) -> Result<Vec<TextLine>, String> {
    let Some(files) = installed_files() else {
        return Err(t!("ocr-needs-pack"));
    };
    let mut slot = engine().lock();
    let info = session.info();
    let px = u64::from(info.width) * u64::from(info.height);
    let mut scale = 1u32;
    while px / u64::from(scale * scale) > MAX_PIXELS && scale < 16 {
        scale *= 2;
    }
    let r = session
        .decode(&DecodeRequest::full(scale), &CancelToken::new())
        .map_err(|e| t!("ocr-failed", reason = crate::error_text::reason(&e)))?;
    let rgba = crate::clipboard::to_rgba(&r);
    let (w, h) = (r.width as usize, r.height as usize);
    let sx = info.width as f32 / w.max(1) as f32;
    let sy = info.height as f32 / h.max(1) as f32;
    // A reading the GPU lost on the way is read once more on the new device (or the CPU).
    let mut tries = 0;
    let mut lines = loop {
        tries += 1;
        let stale = slot
            .as_ref()
            .is_none_or(|(f, _, s)| *f != files || s.stale());
        if stale {
            let mut ai = match slot.take() {
                Some((_, _, s)) => s,
                None => AiSession::new(gpu.clone()),
            };
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
            let ocr = Ocr::load(
                &files,
                ai.context(),
                specular_core::paths::selector(),
                threads,
            )
            .map_err(|e| t!("ocr-failed", reason = e.to_string()))?;
            *slot = Some((files.clone(), ocr, ai));
        }
        let Some((_, ocr, ai)) = slot.as_mut() else {
            return Err(t!("ocr-needs-pack"));
        };
        match ocr.read(w, h, &rgba, &|| false) {
            Ok(lines) => break lines,
            Err(_) if ai.stale() && tries < 2 => {}
            Err(e) => return Err(t!("ocr-failed", reason = e.to_string())),
        }
    };
    for l in &mut lines {
        for p in &mut l.quad {
            *p = [p[0] * sx, p[1] * sy];
        }
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_all_text() {
        assert_eq!(row_text(" a\nb\t"), "a b");
        assert_eq!(row_text(&"x".repeat(200)).chars().count(), 80);
        let l = |t: &str| TextLine {
            quad: [[0.0; 2]; 4],
            text: t.into(),
            confidence: 0.9,
            addon: None,
        };
        assert_eq!(all_text(&[l("שלום"), l("Exit")]), "שלום\nExit");
    }
}
