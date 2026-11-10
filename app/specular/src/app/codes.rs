//! "Read codes" ([`Action::ReadCodes`]): the QR codes in the picture shown (`specular-codes`),
//! listed in the palette's sheet with their outlines drawn on the image. Choosing a row
//! copies its text; a link has a second row that opens it in the system browser, only
//! after a confirmation naming its host, and never while the network is switched off.
//!
//! [`Action::ReadCodes`]: specular_input::Action::ReadCodes

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use specular_core::CancelToken;
use specular_decode::{DecodeRequest, Session};
use specular_gfx::Icon;
use specular_i18n::t;
use specular_widgets::PaletteItem;

use super::App;
use super::script::Placed;
use crate::loader::{Copied, ImageId};

/// Rows: copy is `ITEM_BASE + 2k`, open is `ITEM_BASE + 2k + 1`.
pub(super) const ITEM_BASE: u64 = 1 << 44;
/// The open-link confirmation.
pub(super) const OPEN_ITEM: u64 = super::items::CODES_OPEN;
/// Pictures are searched at most this big (larger ones at half, quarter… size).
const MAX_PIXELS: u64 = 40_000_000;
const MAX_ROWS: usize = 200;

struct Found {
    req: u64,
    image: ImageId,
    codes: Result<Vec<specular_codes::Code>, String>,
}

pub(super) struct Codes {
    tx: Sender<Found>,
    rx: Receiver<Found>,
    req: u64,
    /// The image the codes were read from, and the codes (outlines in stored pixels).
    image: Option<ImageId>,
    found: Vec<specular_codes::Code>,
    /// The link waiting for its confirmation.
    confirm: Option<String>,
}

impl Codes {
    pub(super) fn new() -> Codes {
        let (tx, rx) = channel();
        Codes {
            tx,
            rx,
            req: 0,
            image: None,
            found: Vec::new(),
            confirm: None,
        }
    }
}

/// A code's text for one row: one line, at most 80 characters.
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

impl App {
    /// Reads the codes in the picture shown; the list opens at once and fills when done.
    pub(super) fn codes_read(&mut self) {
        let Some(id) = self.current_id() else { return };
        let Some(session) = self.images.get(&id).and_then(|i| i.session.clone()) else {
            return;
        };
        self.codes.req += 1;
        self.codes.image = Some(id);
        self.codes.found.clear();
        let req = self.codes.req;
        self.chrome.open_list(
            t!("codes-placeholder"),
            Vec::new(),
            Some(t!("codes-looking")),
        );
        let tx = self.codes.tx.clone();
        let wake = self.loader.waker();
        self.loader.background("codes-read", move |_| {
            let codes = read(&session);
            let _ = tx.send(Found {
                req,
                image: id,
                codes,
            });
            wake();
        });
        self.redraw = true;
    }

    /// Once per event-loop turn: the codes found.
    pub(super) fn drain_codes(&mut self) {
        while let Ok(f) = self.codes.rx.try_recv() {
            if f.req != self.codes.req {
                continue;
            }
            match f.codes {
                Ok(c) => {
                    self.codes.image = Some(f.image);
                    self.codes.found = c;
                    let items = self.codes_items();
                    let empty = items.is_empty().then(|| t!("codes-none"));
                    self.chrome.update_list(items, empty);
                }
                Err(e) => {
                    self.codes.found.clear();
                    self.chrome
                        .update_list(Vec::new(), Some(t!("codes-failed", reason = e)));
                }
            }
            self.redraw = true;
        }
    }

    fn codes_items(&self) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        for (k, c) in self.codes.found.iter().enumerate().take(MAX_ROWS) {
            let base = ITEM_BASE + 2 * k as u64;
            items.push(PaletteItem {
                id: base,
                label: t!("codes-copy", text = row_text(&c.text)),
                key: Some(match c.size {
                    Some((rows, cols)) => t!(
                        "codes-kind-size",
                        kind = c.kind,
                        rows = rows as i64,
                        cols = cols as i64
                    ),
                    None => t!("codes-kind", kind = c.kind, version = c.version as i64),
                }),
                unavailable: None,
                icon: Icon::Copy,
            });
            if let Some(host) = c.host() {
                items.push(PaletteItem {
                    id: base + 1,
                    label: t!("codes-open-row", host = host.to_string()),
                    key: None,
                    unavailable: specular_core::net::off::is_off().then(|| t!("codes-network-off")),
                    icon: Icon::Globe,
                });
            }
        }
        items
    }

    /// A row of ours was chosen.
    pub(super) fn codes_item(&mut self, id: u64) -> bool {
        if !(ITEM_BASE..ITEM_BASE + 2 * MAX_ROWS as u64).contains(&id) {
            return false;
        }
        let k = ((id - ITEM_BASE) / 2) as usize;
        let Some(c) = self.codes.found.get(k).cloned() else {
            return true;
        };
        if (id - ITEM_BASE).is_multiple_of(2) {
            self.clipboard
                .copy_text(&self.loader, c.text.clone(), Copied::Value);
            return true;
        }
        let (Some(url), Some(host)) = (c.url(), c.host()) else {
            return true;
        };
        if specular_core::net::off::is_off() {
            self.integration_toast(t!("codes-network-off"));
            return true;
        }
        self.codes.confirm = Some(url.to_string());
        self.chrome.confirm_with_detail(
            t!("codes-open-title", host = host.to_string()),
            t!("codes-open-detail"),
            t!("codes-open"),
            OPEN_ITEM,
        );
        self.redraw = true;
        true
    }

    /// The user accepted opening the link.
    pub(super) fn codes_open_confirmed(&mut self) {
        let Some(url) = self.codes.confirm.take() else {
            return;
        };
        // the switch may have been turned on meanwhile
        if specular_core::net::off::is_off() {
            self.integration_toast(t!("codes-network-off"));
            return;
        }
        super::ai::open_url(url);
    }

    /// The codes' outlines on screen (logical px) while their list is open on their image.
    pub(super) fn codes_placed(&self) -> Vec<Placed> {
        if self.codes.found.is_empty()
            || !self.chrome.list_open()
            || self.codes.image != self.current_id()
        {
            return Vec::new();
        }
        let ls = self.dl.scale.max(0.1);
        let to_screen = self.view.stored_from_screen().invert();
        self.codes
            .found
            .iter()
            .map(|c| {
                let mut pts: Vec<(f32, f32)> = c
                    .corners
                    .iter()
                    .map(|&p| {
                        let q = to_screen.apply(p);
                        (q.0 / ls, q.1 / ls)
                    })
                    .collect();
                pts.push(pts[0]);
                Placed {
                    kind: crate::remote::protocol::ShapeKind::Polyline,
                    pts,
                    text: row_text(&c.text).chars().take(32).collect(),
                    color: [255, 196, 0, 255],
                    width: 2.0,
                }
            })
            .collect()
    }
}

/// Decodes the picture (reduced when very large) and reads its codes; outlines in stored
/// pixels.
fn read(session: &Arc<dyn Session>) -> Result<Vec<specular_codes::Code>, String> {
    let info = session.info();
    let px = u64::from(info.width) * u64::from(info.height);
    let mut scale = 1u32;
    while px / u64::from(scale * scale) > MAX_PIXELS && scale < 16 {
        scale *= 2;
    }
    let r = session
        .decode(&DecodeRequest::full(scale), &CancelToken::new())
        .map_err(|e| crate::error_text::reason(&e))?;
    let rgba = crate::clipboard::to_rgba(&r);
    let (w, h) = (r.width as usize, r.height as usize);
    let sx = info.width as f32 / w.max(1) as f32;
    let sy = info.height as f32 / h.max(1) as f32;
    let mut codes = specular_codes::scan_rgba(w, h, &rgba);
    for c in &mut codes {
        for p in &mut c.corners {
            *p = (p.0 * sx, p.1 * sy);
        }
    }
    Ok(codes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_one_short_line() {
        assert_eq!(row_text(" a\nb\t"), "a b");
        let long = "x".repeat(200);
        assert_eq!(row_text(&long).chars().count(), 80);
    }
}
