//! A book's table of contents (EPUB) in a side panel: its entries, indented by depth, the
//! chapter of the page on screen marked. A click on an entry turns to its page. Alt+T
//! ([`Action::ToggleContents`]) shows and hides the panel; it stays open across books and
//! opens by itself the first time a book with contents is shown in a session.

#[cfg(test)]
mod shots;

use std::sync::Arc;

use specular_decode::ContentsEntry;
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::Action;
use specular_theme::Palette;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, TOOLBAR_MARGIN};
use specular_widgets::accesskit;

use super::App;

/// AccessKit ids: the panel, then a button per entry (the font specimen uses 0x6200...).
const A11Y_ID: u64 = 0x6100_0000_0000_0000;
const A11Y_ENTRY: u64 = A11Y_ID + 0x100;
/// Entries past this many have no node of their own.
const A11Y_MAX: u64 = 0xF000;

/// Logical width of the panel.
const PANEL_W: f32 = 280.0;
/// Indent per level (logical px).
const INDENT: f32 = 14.0;
/// Levels indented at most.
const MAX_INDENT: u32 = 4;
/// Characters of a title shown before it is cut.
const TITLE_CHARS: usize = 60;

/// The panel's state.
#[derive(Default)]
pub(super) struct Contents {
    /// Shown or hidden by hand (`None`: not yet, it then opens with a book's contents).
    open: Option<bool>,
    /// First entry listed.
    scroll: usize,
    /// The panel and its rows as last drawn (logical px): entry index and rect.
    panel_rect: Option<Rect>,
    rows: Vec<(usize, Rect)>,
}

/// The panel to draw.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ContentsPanel {
    pub entries: Arc<[ContentsEntry]>,
    /// The entry of the chapter on screen.
    pub current: Option<usize>,
    pub scroll: usize,
}

/// The panel as drawn: its rect, each row's entry and rect, and the first entry listed.
pub(super) type DrawnContents = (Rect, Vec<(usize, Rect)>, usize);

/// The entry whose chapter `page` is in: the last one starting at or before it.
pub(super) fn current_entry(entries: &[ContentsEntry], page: u32) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for (i, e) in entries.iter().enumerate() {
        if let Some(p) = e.page
            && p <= page
            && best.is_none_or(|(_, bp)| p >= bp)
        {
            best = Some((i, p));
        }
    }
    best.map(|b| b.0)
}

fn short(title: &str) -> String {
    let t = title.trim();
    if t.chars().count() <= TITLE_CHARS {
        return t.to_owned();
    }
    let mut s: String = t.chars().take(TITLE_CHARS - 1).collect();
    s.push('…');
    s
}

impl Contents {
    pub(super) fn set_drawn(&mut self, drawn: Option<DrawnContents>) {
        match drawn {
            Some((panel, rows, scroll)) => {
                self.panel_rect = Some(panel);
                self.rows = rows;
                self.scroll = scroll;
            }
            None => {
                self.panel_rect = None;
                self.rows.clear();
            }
        }
    }
}

impl App {
    /// The current image's table of contents, when it has one.
    fn contents_entries(&self) -> Option<Arc<[ContentsEntry]>> {
        let id = self.current_id()?;
        let s = self.images.get(&id)?.session.as_ref()?;
        s.contents().filter(|c| !c.is_empty())
    }

    /// The page shown of the current image.
    fn contents_page(&self) -> u32 {
        self.current_id()
            .and_then(|id| self.images.get(&id))
            .map_or(0, |i| i.page)
    }

    /// Alt+T: shows or hides the panel. True when `a` was that action.
    pub(super) fn contents_action(&mut self, a: Action) -> bool {
        if a != Action::ToggleContents {
            return false;
        }
        if self.contents_entries().is_none() {
            let now = self.now();
            self.chrome.toast(t!("contents-none"), None, now);
            return true;
        }
        let open = !self.contents.open.unwrap_or(true);
        self.contents.open = Some(open);
        self.redraw = true;
        true
    }

    /// The panel to draw, when it is open over a book with contents.
    pub(super) fn contents_panel(&self) -> Option<ContentsPanel> {
        if !self.contents.open.unwrap_or(true) || self.gallery_active() {
            return None;
        }
        let entries = self.contents_entries()?;
        let current = current_entry(&entries, self.contents_page());
        Some(ContentsPanel {
            entries,
            current,
            scroll: self.contents.scroll,
        })
    }

    /// Turns to the page of entry `index`.
    fn contents_go(&mut self, index: usize) {
        let Some(entries) = self.contents_entries() else {
            return;
        };
        let Some(page) = entries.get(index).and_then(|e| e.page) else {
            return;
        };
        let Some(id) = self.current_id() else { return };
        if self.contents_page() != page {
            self.page_switch(id, page);
        }
        self.redraw = true;
    }

    /// A press at logical `lp`: an entry's row turns to its page; the rest of the panel takes
    /// the press. True when taken.
    pub(super) fn contents_press(&mut self, lp: (f32, f32)) -> bool {
        let Some(panel) = self.contents.panel_rect else {
            return false;
        };
        if !panel.contains(lp) {
            return false;
        }
        if let Some(&(i, _)) = self.contents.rows.iter().find(|(_, r)| r.contains(lp)) {
            self.contents_go(i);
        }
        true
    }

    /// The wheel over the panel scrolls it. True when taken.
    pub(super) fn contents_wheel(&mut self, dy: f32, lp: (f32, f32)) -> bool {
        if !self.contents.panel_rect.is_some_and(|r| r.contains(lp)) {
            return false;
        }
        let steps = (dy.abs().ceil() as usize).max(1) * 3;
        self.contents.scroll = if dy > 0.0 {
            self.contents.scroll.saturating_sub(steps)
        } else {
            self.contents.scroll + steps
        };
        self.redraw = true;
        true
    }

    /// A screen reader's click on an entry; true when the node is the panel's.
    pub(super) fn contents_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let id = req.target_node.0;
        if !(A11Y_ID..A11Y_ENTRY + A11Y_MAX).contains(&id) {
            return false;
        }
        if id >= A11Y_ENTRY && req.action == accesskit::Action::Click {
            self.contents_go((id - A11Y_ENTRY) as usize);
        }
        self.redraw = true;
        true
    }
}

/// Draws the panel at the top of `area` (logical x, y, w, h) on the reading side's start,
/// above `bottom` (the toolbar's height) with as many rows as fit; returns the panel, each
/// row listed and the first entry listed.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_panel(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    p: &ContentsPanel,
    area: (f32, f32, f32, f32),
    bottom: f32,
    rtl: bool,
    ts: f32,
) -> DrawnContents {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts);
    let title = text.layout(&t!("contents-title"), st.weight(600.0), None, Align::Start);
    let row_h = (FONT_SMALL * 2.0 * ts).round();
    let w = (PANEL_W * ts).min(aw - 2.0 * m).max(120.0);
    let max_h = (ah - 2.0 * m - bottom).max(row_h * 3.0);
    let fit = (((max_h - 2.0 * PAD - title.height - 4.0 * ts) / row_h).floor() as usize).max(1);
    let n = p.entries.len().min(fit);
    // Keep the current chapter in view when the list was not scrolled past it by hand.
    let mut scroll = p.scroll.min(p.entries.len() - n);
    if let Some(c) = p.current
        && (c < scroll || c >= scroll + n)
        && p.scroll == 0
    {
        scroll = c.saturating_sub(n / 3).min(p.entries.len() - n);
    }
    let h = 2.0 * PAD + title.height + 4.0 * ts + n as f32 * row_h;
    let x = if rtl { ax + aw - m - w } else { ax + m };
    let y = ay + m;
    let panel = Rect::new(x, y, w, h);
    dl.glass(panel, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let tx = if rtl {
        x + w - PAD - title.width
    } else {
        x + PAD
    };
    text.draw(dl, &title, tx, y + PAD, pal.text_muted);
    let mut rows = Vec::with_capacity(n);
    let mut ry = y + PAD + title.height + 4.0 * ts;
    let inner = w - 2.0 * PAD;
    for (i, e) in p.entries.iter().enumerate().skip(scroll).take(n) {
        let row = Rect::new(x + PAD / 2.0, ry, w - PAD, row_h);
        let current = p.current == Some(i);
        if current {
            dl.rect(row, 6.0 * ts, pal.accent_soft);
        }
        let indent = (e.depth.min(MAX_INDENT) as f32) * INDENT * ts;
        let ink = if e.page.is_none() {
            pal.text_faint
        } else if current {
            pal.text
        } else {
            pal.text_muted
        };
        let style = if e.depth == 0 { st.weight(600.0) } else { st };
        let label = text.layout(&short(&e.title), style, Some(inner - indent), Align::Start);
        let cy = ry + row_h / 2.0;
        // One line: a wrapped title is cut by the clip.
        dl.push_clip(row);
        let lx = if rtl {
            x + w - PAD - indent - label.width.min(inner - indent)
        } else {
            x + PAD + indent
        };
        let line_h = label.height / label.lines.max(1) as f32;
        text.draw(dl, &label, lx, cy - line_h / 2.0, ink);
        dl.pop_clip();
        rows.push((i, row));
        ry += row_h;
    }
    (panel, rows, scroll)
}

/// The AccessKit nodes: the panel as a list of buttons, one per entry listed.
pub(super) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    p: &ContentsPanel,
    panel: Rect,
    rows: &[(usize, Rect)],
    sf: f64,
) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * sf,
        y0: f64::from(r.y) * sf,
        x1: f64::from(r.x + r.w) * sf,
        y1: f64::from(r.y + r.h) * sf,
    };
    let mut g = Node::new(Role::List);
    g.set_label(t!("contents-title"));
    g.set_bounds(bounds(panel));
    let mut children = Vec::new();
    for (index, r) in rows {
        let Some(e) = p.entries.get(*index).filter(|_| (*index as u64) < A11Y_MAX) else {
            continue;
        };
        let id = NodeId(A11Y_ENTRY + *index as u64);
        let mut n = Node::new(Role::Button);
        n.set_label(e.title.to_string());
        if p.current == Some(*index) {
            n.set_selected(true);
        }
        n.set_bounds(bounds(*r));
        if e.page.is_some() {
            n.add_action(accesskit::Action::Click);
        } else {
            n.set_disabled();
        }
        g.push_child(id);
        children.push((id, n));
    }
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), g));
    tree.nodes.extend(children);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, depth: u32, page: Option<u32>) -> ContentsEntry {
        ContentsEntry {
            title: title.into(),
            depth,
            page,
        }
    }

    #[test]
    fn the_current_entry_is_the_last_chapter_started() {
        let e = [
            entry("Cover", 0, Some(0)),
            entry("One", 0, Some(2)),
            entry("One, part a", 1, Some(2)),
            entry("Missing", 0, None),
            entry("Two", 0, Some(9)),
        ];
        assert_eq!(current_entry(&e, 0), Some(0));
        assert_eq!(current_entry(&e, 1), Some(0));
        // Two entries on the same page: the deeper, later one.
        assert_eq!(current_entry(&e, 5), Some(2));
        assert_eq!(current_entry(&e, 9), Some(4));
        assert_eq!(current_entry(&e[1..], 0), None);
    }

    #[test]
    fn long_titles_are_cut() {
        let long = "x".repeat(100);
        assert_eq!(short(&long).chars().count(), TITLE_CHARS);
        assert!(short(&long).ends_with('…'));
        assert_eq!(short("  Short  "), "Short");
    }
}
