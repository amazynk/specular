//! The help panel: the bundled manual ([`specular_help`]), searched as you type and drawn
//! here (no browser, no web view).
//!
//! Three views share the sheet: the contents (no page, no search), the results of a search
//! (one row per section found) and a page. A page is drawn block by block with clear
//! breaks: a gap between paragraphs, a larger one above headings, list items on lines of
//! their own, notes on a tinted band. Links are rows of their own under the paragraph that
//! names them (one style per text block), reached with Up and Down and opened with Enter.
//! Key tables come from the live keymap, as the cheat sheet's do.

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock, TextStyle, draw_icon};
use specular_help::{Block, Hit, Manual, Section, Span, Topic};
use specular_i18n::{I18n, t};
use specular_input::{KeyCombo, Keymap, Mode, NamedKey};
use specular_theme::metrics;

use crate::cheatsheet::SheetMode;
use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Modal, Response, ViewerState};

/// A place in the manual: a page (none: the contents) and a section of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Loc {
    pub page: Option<usize>,
    pub anchor: String,
}

/// What the sheet shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum View {
    Contents,
    Results,
    Page(usize),
}

#[derive(Default)]
pub(crate) struct HelpState {
    pub query: String,
    /// The search field's caret, selection and undo (the truth is `query`).
    pub query_edit: crate::line_edit::LineEdit,
    pub at: Loc,
    pub back: Vec<Loc>,
    pub fwd: Vec<Loc>,
    pub hits: Vec<Hit>,
    /// The row with the keyboard: a result, a page of the contents, a link of the page.
    pub selected: Option<usize>,
    pub scroll: f32,
    pub last_scroll: f32,
    /// Scroll to this section at the next draw.
    pub jump: Option<String>,
    /// The page's links (page id, empty: this page; anchor), in reading order.
    pub links: Vec<(String, String)>,
}

/// Results listed at most.
const MAX_HITS: usize = 40;

fn manual() -> Arc<Manual> {
    Manual::current()
}

impl HelpState {
    pub fn view(&self) -> View {
        if !self.query.trim().is_empty() {
            View::Results
        } else {
            match self.at.page {
                Some(p) => View::Page(p),
                None => View::Contents,
            }
        }
    }

    /// Rows the keyboard moves through in the current view.
    pub fn rows(&self) -> usize {
        match self.view() {
            View::Results => self.hits.len(),
            View::Contents => manual().pages().len(),
            View::Page(_) => self.links.len(),
        }
    }

    /// Shows `loc`, the place left going on the Back list.
    pub fn go(&mut self, loc: Loc) {
        if loc != self.at || !self.query.is_empty() {
            let from = std::mem::replace(&mut self.at, loc);
            if self.back.last() != Some(&from) && from != self.at {
                self.back.push(from);
            }
            self.fwd.clear();
        }
        self.arrive();
    }

    fn arrive(&mut self) {
        self.query.clear();
        self.query_edit.sync(&self.query);
        self.hits.clear();
        self.selected = None;
        self.scroll = 0.0;
        self.last_scroll = 0.0;
        self.jump = (!self.at.anchor.is_empty()).then(|| self.at.anchor.clone());
        self.links = match self.at.page {
            Some(p) => links_of(&manual(), p),
            None => Vec::new(),
        };
    }

    pub fn go_back(&mut self) -> bool {
        if !self.query.is_empty() {
            // Out of a search: back where it started.
            self.arrive_keep();
            return true;
        }
        let Some(l) = self.back.pop() else {
            return false;
        };
        self.fwd.push(std::mem::replace(&mut self.at, l));
        self.arrive();
        true
    }

    pub fn go_forward(&mut self) -> bool {
        let Some(l) = self.fwd.pop() else {
            return false;
        };
        self.back.push(std::mem::replace(&mut self.at, l));
        self.arrive();
        true
    }

    fn arrive_keep(&mut self) {
        let at = self.at.clone();
        self.at = at;
        self.arrive();
    }

    pub fn type_text(&mut self, s: &str) {
        self.edit().insert(s);
        self.edited();
    }

    pub fn backspace(&mut self) {
        let mut clip = crate::line_edit::MemoryClipboard::default();
        let k = specular_input::Key::Named(NamedKey::Backspace);
        self.edit()
            .key(k, specular_input::Mods::NONE, false, &mut clip);
        self.edited();
    }

    pub fn edit(&mut self) -> &mut crate::line_edit::LineEdit {
        self.query_edit.sync(&self.query);
        &mut self.query_edit
    }

    /// The field edited: the results follow it.
    pub fn edited(&mut self) {
        if self.query != self.query_edit.text() {
            self.query = self.query_edit.text().to_owned();
            self.hits = manual().search(&self.query, MAX_HITS);
            self.selected = (!self.hits.is_empty()).then_some(0);
            self.scroll = 0.0;
        }
    }

    pub fn move_sel(&mut self, d: i32) {
        let n = self.rows();
        if n == 0 {
            return;
        }
        let at = match self.selected {
            Some(s) => (s as i64 + d as i64).clamp(0, n as i64 - 1) as usize,
            None if d >= 0 => 0,
            None => n - 1,
        };
        self.selected = Some(at);
        self.scroll = f32::NAN;
    }

    /// Opens row `i` of the current view.
    pub fn open_row(&mut self, i: usize) {
        let m = manual();
        let loc = match self.view() {
            View::Results => self.hits.get(i).map(|h| Loc {
                page: Some(h.page),
                anchor: h.anchor.clone(),
            }),
            View::Contents => (i < m.pages().len()).then_some(Loc {
                page: Some(i),
                anchor: String::new(),
            }),
            View::Page(p) => self.links.get(i).map(|(page, anchor)| Loc {
                page: if page.is_empty() {
                    Some(p)
                } else {
                    m.index_of(page)
                },
                anchor: anchor.clone(),
            }),
        };
        if let Some(loc) = loc {
            self.go(loc);
        }
    }
}

/// The links of page `p`, in reading order.
fn links_of(m: &Manual, p: usize) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for b in &m.pages()[p].blocks {
        for s in specular_help::doc::spans_of(b) {
            if let Span::Link { page, anchor, .. } = s {
                v.push((page.clone(), anchor.clone()));
            }
        }
    }
    v
}

/// A link's row: "Go to Convert".
fn link_title(text: &str) -> String {
    t!("help-link", title = text.to_owned())
}

/// The sheet's tab for a `::: keys` table.
fn sheet_mode(table: &str) -> Option<SheetMode> {
    Some(match table {
        "viewer" => SheetMode::Viewer,
        "gallery" => SheetMode::Gallery,
        "compare" => SheetMode::Compare,
        "cull" => SheetMode::Cull,
        "medical" => SheetMode::Medical,
        "science" => SheetMode::Science,
        "drawing" => SheetMode::Drawing,
        "mesh" => SheetMode::Mesh,
        "relight" => SheetMode::Relight,
        "maps" => SheetMode::Map,
        "develop" => SheetMode::Develop,
        _ => return None,
    })
}

/// The key bound to an action now ("Right"); unbound, its name and "Not set".
pub(crate) fn key_ref(km: &Keymap, id: &str) -> String {
    let Some(a) = specular_input::Action::from_id(id) else {
        return id.to_owned();
    };
    let name = a.name();
    // The pages say what the key does next to it ("[[key:next]] next image"): its first key
    // alone; without one, the command's name (run from the palette).
    match Mode::ALL
        .iter()
        .find_map(|m| km.keys_in(*m, a).into_iter().next())
    {
        Some(k) => k.to_string(),
        None => t!("help-key-unbound", name = name),
    }
}

/// The page's screen-reader text: everything as drawn, one block per line.
pub(crate) fn page_text(m: &Manual, p: usize, km: &Keymap) -> String {
    let mut s = String::new();
    for b in &m.pages()[p].blocks {
        let line = match b {
            Block::Keys(table) => match sheet_mode(table) {
                Some(mode) => crate::cheatsheet::groups_for(mode, km)
                    .into_iter()
                    .flat_map(|(_, lines)| lines)
                    .map(|(name, keys)| format!("{name}: {}", keys.join(", ")))
                    .collect::<Vec<_>>()
                    .join("\n"),
                None => String::new(),
            },
            Block::Commands => commands_text(km),
            b => specular_help::doc::plain(specular_help::doc::spans_of(b), &|id| key_ref(km, id)),
        };
        s.push_str(&line);
        s.push('\n');
    }
    s
}

/// The screen-reader name of row `i` and whether it is a link.
pub(crate) fn row_label(s: &HelpState, i: usize) -> Option<(String, bool)> {
    let m = manual();
    match s.view() {
        View::Results => {
            let h = s.hits.get(i)?;
            let page = &m.pages()[h.page].title;
            let title = if h.heading.is_empty() {
                page.clone()
            } else {
                format!("{page} › {}", h.heading)
            };
            Some((format!("{title}. {}", h.snippet), false))
        }
        View::Contents => Some((m.pages().get(i)?.title.clone(), false)),
        View::Page(p) => {
            let mut k = 0;
            for b in &m.pages()[p].blocks {
                for sp in specular_help::doc::spans_of(b) {
                    if let Span::Link { text, .. } = sp {
                        if k == i {
                            return Some((link_title(text), true));
                        }
                        k += 1;
                    }
                }
            }
            None
        }
    }
}

/// What the panel shows as text (screen readers): the page, or the search's summary.
pub(crate) fn screen_text(s: &HelpState, km: &Keymap) -> String {
    match s.view() {
        View::Page(p) => page_text(&manual(), p, km),
        View::Results if s.hits.is_empty() => {
            t!("help-no-results", query = s.query.trim().to_owned())
        }
        View::Results => t!("help-results", count = s.hits.len() as i64),
        View::Contents => t!("help-contents"),
    }
}

impl Chrome {
    /// The title of the page the help panel shows.
    pub(crate) fn help_page_title(&self) -> Option<String> {
        let p = self.help.at.page?;
        Some(manual().pages().get(p)?.title.clone())
    }
}

fn commands_text(km: &Keymap) -> String {
    match km.primary_key(specular_input::Action::Palette) {
        Some(k) => t!("help-commands", key = k.to_string()),
        None => t!("help-commands-no-key"),
    }
}

impl Chrome {
    /// Opens the manual at the page for `context` (one of [`specular_help::CONTEXTS`]).
    pub fn show_help(&mut self, context: &str) {
        let page = manual().for_context(context);
        self.show_help_at(Some(page), "");
    }

    /// Opens the manual at a topic (the help of a disabled command, a link of another dialog).
    pub fn show_help_topic(&mut self, topic: Topic) {
        let page = manual().index_of(topic.page);
        self.show_help_at(page, topic.anchor);
    }

    pub(crate) fn show_help_at(&mut self, page: Option<usize>, anchor: &str) {
        let open = self.modal == Some(Modal::Help);
        if !open {
            self.help = HelpState::default();
        }
        self.open_modal(Modal::Help);
        self.help.go(Loc {
            page,
            anchor: anchor.to_owned(),
        });
        if !open {
            // A fresh panel has nothing to go back to.
            self.help.back.clear();
        }
    }

    /// Opens the manual at its contents.
    pub fn show_help_contents(&mut self) {
        self.show_help_at(None, "");
    }

    /// The page the help panel shows (its id), if any.
    pub fn help_page(&self) -> Option<String> {
        if self.modal != Some(Modal::Help) {
            return None;
        }
        let p = self.help.at.page?;
        Some(manual().pages()[p].id.clone())
    }

    /// The help context of what is on screen: the open dialog, else the mode.
    pub(crate) fn help_context(&self, st: &ViewerState) -> &'static str {
        match self.modal {
            Some(Modal::Settings | Modal::ConfigEditor | Modal::Language) => return "settings",
            Some(Modal::Palette) => return "palette",
            Some(Modal::CheatSheet) => return "cheatsheet",
            Some(Modal::About) => return "about",
            Some(Modal::Profile) => return "profile",
            Some(Modal::DicomTags | Modal::Studies | Modal::Plot) => return "medical",
            Some(Modal::Crash) => return "crash",
            Some(Modal::Diagnostics) => return "diagnostics",
            Some(Modal::HwReport | Modal::Insights) => return "troubleshooting",
            Some(Modal::Downloads) => return "downloads",
            Some(Modal::Queue) => return "video",
            _ => {}
        }
        if self.presenting {
            return "slideshow";
        }
        if !self.gallery {
            if !st.has_image && !st.loading {
                return if st.error.is_some() { "error" } else { "empty" };
            }
            if st.error.is_some() {
                return "error";
            }
            if st.playback.is_some() {
                return "animation";
            }
            if st.pages.is_some() {
                return "reading";
            }
        }
        match self.cheat_context(st.keymap) {
            SheetMode::Viewer => "viewer",
            SheetMode::Gallery => "gallery",
            SheetMode::Compare => "compare",
            SheetMode::Cull => "cull",
            SheetMode::Medical => "medical",
            SheetMode::Science => "science",
            SheetMode::Drawing => "drawing",
            SheetMode::Mesh => "mesh",
            SheetMode::Relight => "relight",
            SheetMode::Video => "video",
            SheetMode::Map => "maps",
            SheetMode::Develop => "develop",
        }
    }

    /// F1: the help for what is on screen; over a control that is unavailable, why it is.
    pub(crate) fn help_for_screen(&mut self, st: &ViewerState) {
        if self.modal == Some(Modal::Help) {
            self.close_modal();
            return;
        }
        if let Some(d) = self.hover.as_ref().and_then(|t| t.disabled(st)) {
            self.show_help_topic(specular_help::disabled_topic(d));
            return;
        }
        let c = self.help_context(st);
        self.show_help(c);
    }

    pub(crate) fn help_row_click(&mut self, i: usize) -> Response {
        self.help.open_row(i);
        Response::used()
    }

    /// The keyboard in the help panel: Tab goes round the field, Back, Contents and Close;
    /// Up and Down move through the rows (results, pages, links), Enter opens one; Alt+Left
    /// and Alt+Right (and Backspace in an empty field) go back and forward; Page Up/Down,
    /// Home and End scroll.
    pub(crate) fn help_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        repeat: bool,
        st: &ViewerState,
    ) -> Response {
        if let Some(back) = crate::focus::is_tab(combo) {
            let order = [
                None,
                Some(Target::HelpBack),
                Some(Target::HelpContents),
                Some(Target::ModalClose),
            ];
            let at = order
                .iter()
                .position(|o| *o == self.modal_focus)
                .unwrap_or(0);
            let n = order.len();
            let to = if back { (at + n - 1) % n } else { (at + 1) % n };
            self.modal_focus = order[to].clone();
            self.reveal_focus = true;
            return Response::used();
        }
        if let Some(t) = self.modal_focus.clone() {
            if matches!(named, Some(NamedKey::Enter | NamedKey::Space)) {
                return if repeat {
                    Response::used()
                } else {
                    self.activate(t, st)
                };
            }
            self.modal_focus = None;
            if let Some(r) = self.field_key(combo) {
                return r;
            }
        }
        // Alt+Left (back) is the dialogs' own (`back_in_dialog`); Alt+Right goes forward.
        let m = combo.mods;
        if m.alt
            && !(m.ctrl || m.shift || m.meta)
            && combo.key == specular_input::Key::Named(NamedKey::ArrowRight)
        {
            self.help.go_forward();
            return Response::used();
        }
        if let Some(k @ (NamedKey::PageDown | NamedKey::PageUp)) = named {
            let now = self.last_now;
            self.bar_key(crate::scrollbar::Area::Help, k, now);
            return Response::used();
        }
        let s = &mut self.help;
        match named {
            Some(NamedKey::ArrowDown) => s.move_sel(1),
            Some(NamedKey::ArrowUp) => s.move_sel(-1),
            Some(NamedKey::Home) => s.scroll = 0.0,
            Some(NamedKey::End) => s.scroll = f32::MAX,
            Some(NamedKey::Enter) if !repeat => {
                if let Some(i) = s.selected {
                    s.open_row(i);
                }
            }
            Some(NamedKey::Backspace) => {
                if s.query.is_empty() {
                    s.go_back();
                } else {
                    s.backspace();
                }
            }
            _ => {}
        }
        Response::used()
    }
}

/// One drawn piece of the body, at `y` (content coordinates).
enum Part {
    /// Text, indented by `x`, with an optional marker ("•", "3.") in the indent.
    Text {
        b: Arc<TextBlock>,
        x: f32,
        marker: Option<Arc<TextBlock>>,
        color: Colour,
    },
    Heading {
        b: Arc<TextBlock>,
        anchor: String,
    },
    /// A note: text on a tinted band.
    Note(Arc<TextBlock>),
    /// A row that opens something: a link, a result, a page of the contents.
    Row {
        i: usize,
        title: Arc<TextBlock>,
        sub: Option<Arc<TextBlock>>,
        link: bool,
    },
    /// A key table line: the command, its keys.
    KeyLine {
        name: Arc<TextBlock>,
        keys: String,
    },
    Rule,
}

#[derive(Clone, Copy)]
enum Colour {
    Text,
    Muted,
    Faint,
}

struct Laid {
    y: f32,
    h: f32,
    part: Part,
}

struct Styles {
    page_title: TextStyle,
    h2: TextStyle,
    h3: TextStyle,
    body: TextStyle,
    small: TextStyle,
    strong: TextStyle,
}

/// Lays out the body of the current view at width `w`.
fn lay_out(s: &HelpState, cx: &mut Ctx, km: &Keymap, w: f32, st: &Styles) -> Vec<Laid> {
    let m = manual();
    let mut out = Vec::new();
    let mut y = 0.0;
    let para_gap = cx.u(12.0);
    let item_gap = cx.u(5.0);
    let head_gap = cx.u(22.0);
    let push = |out: &mut Vec<Laid>, y: &mut f32, h: f32, part: Part, gap: f32| {
        out.push(Laid { y: *y, h, part });
        *y += h + gap;
    };
    match s.view() {
        View::Results => {
            let n = s.hits.len();
            let summary = if n == 0 {
                t!("help-no-results", query = s.query.trim().to_owned())
            } else {
                t!("help-results", count = n as i64)
            };
            let b = cx.layout(&summary, st.small, Some(w));
            push(
                &mut out,
                &mut y,
                b.height,
                Part::Text {
                    b,
                    x: 0.0,
                    marker: None,
                    color: Colour::Muted,
                },
                para_gap,
            );
            for (i, h) in s.hits.iter().enumerate() {
                let page = &m.pages()[h.page];
                let title = if h.heading.is_empty() {
                    page.title.clone()
                } else {
                    format!("{} › {}", page.title, h.heading)
                };
                let pad = cx.u(8.0);
                let tb = cx.layout(&title, st.strong, Some(w - 2.0 * pad));
                let sb = (!h.snippet.is_empty())
                    .then(|| cx.layout(&h.snippet, st.small, Some(w - 2.0 * pad)));
                let hh = tb.height + sb.as_ref().map_or(0.0, |b| b.height + cx.u(3.0)) + 2.0 * pad;
                push(
                    &mut out,
                    &mut y,
                    hh,
                    Part::Row {
                        i,
                        title: tb,
                        sub: sb,
                        link: false,
                    },
                    cx.u(2.0),
                );
            }
        }
        View::Contents => {
            let mut i = 0;
            for sec in Section::ALL {
                let b = cx.layout(&I18n::global().tr(sec.message_id()), st.h3, Some(w));
                if i > 0 {
                    y += head_gap - para_gap;
                }
                push(
                    &mut out,
                    &mut y,
                    b.height,
                    Part::Heading {
                        b,
                        anchor: String::new(),
                    },
                    cx.u(6.0),
                );
                for p in m.pages().iter().filter(|p| p.section == sec) {
                    let idx = m.index_of(&p.id).unwrap_or(i);
                    let pad = cx.u(6.0);
                    let tb = cx.layout(&p.title, st.body, Some(w - 2.0 * pad));
                    push(
                        &mut out,
                        &mut y,
                        tb.height + 2.0 * pad,
                        Part::Row {
                            i: idx,
                            title: tb,
                            sub: None,
                            link: false,
                        },
                        0.0,
                    );
                    i += 1;
                }
                y += para_gap;
            }
        }
        View::Page(p) => {
            let page = &m.pages()[p];
            let b = cx.layout(&page.title, st.page_title, Some(w));
            push(
                &mut out,
                &mut y,
                b.height,
                Part::Heading {
                    b,
                    anchor: String::new(),
                },
                para_gap,
            );
            let blocks = &page.blocks;
            let mut link_i = 0;
            let indent = cx.u(20.0);
            for (k, b) in blocks.iter().enumerate() {
                let next_is_item = matches!(
                    blocks.get(k + 1),
                    Some(Block::Bullet(_) | Block::Numbered(..))
                );
                let key = |id: &str| key_ref(km, id);
                match b {
                    Block::Heading {
                        level,
                        text,
                        anchor,
                    } => {
                        if k > 0 {
                            y += head_gap - para_gap;
                        }
                        let style = if *level == 2 { st.h2 } else { st.h3 };
                        let tb = cx.layout(&specular_help::doc::plain(text, &key), style, Some(w));
                        push(
                            &mut out,
                            &mut y,
                            tb.height,
                            Part::Heading {
                                b: tb,
                                anchor: anchor.clone(),
                            },
                            cx.u(6.0),
                        );
                    }
                    Block::Para(spans)
                    | Block::Note(spans)
                    | Block::Bullet(spans)
                    | Block::Numbered(_, spans) => {
                        let text = specular_help::doc::plain(spans, &key);
                        let item = matches!(b, Block::Bullet(_) | Block::Numbered(..));
                        let gap = if item && next_is_item {
                            item_gap
                        } else {
                            para_gap
                        };
                        let links: Vec<(usize, String, String)> = spans
                            .iter()
                            .filter_map(|sp| match sp {
                                Span::Link { text, page, .. } => {
                                    Some((0, text.clone(), page.clone()))
                                }
                                _ => None,
                            })
                            .collect();
                        let after = if links.is_empty() { gap } else { cx.u(4.0) };
                        match b {
                            Block::Note(_) => {
                                let pad = cx.u(10.0);
                                let tb = cx.layout(&text, st.body, Some(w - 2.0 * pad - cx.u(3.0)));
                                push(
                                    &mut out,
                                    &mut y,
                                    tb.height + 2.0 * pad,
                                    Part::Note(tb),
                                    after,
                                );
                            }
                            Block::Bullet(_) | Block::Numbered(..) => {
                                let marker = match b {
                                    Block::Numbered(n, _) => format!("{n}."),
                                    _ => "•".to_owned(),
                                };
                                let mb = cx.layout(&marker, st.body, None);
                                let tb = cx.layout(&text, st.body, Some(w - indent));
                                push(
                                    &mut out,
                                    &mut y,
                                    tb.height,
                                    Part::Text {
                                        b: tb,
                                        x: indent,
                                        marker: Some(mb),
                                        color: Colour::Text,
                                    },
                                    after,
                                );
                            }
                            _ => {
                                let tb = cx.layout(&text, st.body, Some(w));
                                push(
                                    &mut out,
                                    &mut y,
                                    tb.height,
                                    Part::Text {
                                        b: tb,
                                        x: 0.0,
                                        marker: None,
                                        color: Colour::Text,
                                    },
                                    after,
                                );
                            }
                        }
                        let n_links = links.len();
                        for (j, (_, ltext, _)) in links.into_iter().enumerate() {
                            let pad = cx.u(4.0);
                            let label = link_title(&ltext);
                            let x = if item { indent } else { 0.0 };
                            let tb =
                                cx.layout(&label, st.body, Some(w - x - 2.0 * pad - cx.u(18.0)));
                            let g = if j + 1 == n_links { gap } else { 0.0 };
                            push(
                                &mut out,
                                &mut y,
                                tb.height + 2.0 * pad,
                                Part::Row {
                                    i: link_i,
                                    title: tb,
                                    sub: None,
                                    link: true,
                                },
                                g,
                            );
                            link_i += 1;
                        }
                    }
                    Block::Keys(table) => {
                        let Some(mode) = sheet_mode(table) else {
                            continue;
                        };
                        let head = cx.layout(
                            &t!("help-keys-command"),
                            st.small.weight(650.0),
                            Some(w * 0.6),
                        );
                        push(
                            &mut out,
                            &mut y,
                            head.height,
                            Part::KeyLine {
                                name: head,
                                keys: t!("help-keys-keys"),
                            },
                            cx.u(4.0),
                        );
                        push(&mut out, &mut y, 1.0, Part::Rule, cx.u(4.0));
                        for (group, lines) in crate::cheatsheet::groups_for(mode, km) {
                            let gb = cx.layout(&group, st.small.weight(650.0), Some(w));
                            push(
                                &mut out,
                                &mut y,
                                gb.height,
                                Part::Text {
                                    b: gb,
                                    x: 0.0,
                                    marker: None,
                                    color: Colour::Faint,
                                },
                                cx.u(4.0),
                            );
                            for (name, keys) in lines {
                                let nb = cx.layout(&name, st.body, Some(w * 0.6));
                                let rh = nb.height.max(cx.u(22.0));
                                push(
                                    &mut out,
                                    &mut y,
                                    rh,
                                    Part::KeyLine {
                                        name: nb,
                                        keys: keys.join("  "),
                                    },
                                    cx.u(2.0),
                                );
                            }
                            y += cx.u(8.0);
                        }
                        y += para_gap;
                    }
                    Block::Commands => {
                        let tb = cx.layout(&commands_text(km), st.body, Some(w));
                        push(
                            &mut out,
                            &mut y,
                            tb.height,
                            Part::Text {
                                b: tb,
                                x: 0.0,
                                marker: None,
                                color: Colour::Text,
                            },
                            para_gap,
                        );
                    }
                }
            }
        }
    }
    out
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, st_view: &ViewerState) {
    let on_field = ch.modal_focus.is_none();
    let w = (cx.w - 48.0).min(cx.u(760.0));
    let h = (cx.h - 48.0).min(cx.h * 0.9).max(200.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let pad = 20.0;
    let inner = Rect::new(r.x + pad, r.y, r.w - 2.0 * pad, r.h);

    // Back, the title, Contents and Close.
    let bar_h = cx.u(32.0).max(32.0);
    let can_back = !ch.help.back.is_empty() || !ch.help.query.is_empty();
    let back = cx.m_in(Rect::new(inner.x - 6.0, r.y + 12.0, bar_h, bar_h), r);
    cx.icon_button_e(
        back,
        Icon::ChevronLeft,
        Target::HelpBack,
        cx.pal.text_muted,
        cx.rtl,
        false,
        can_back,
    );
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 12.0, 32.0, 32.0), r);
    cx.close_button(close);
    let label_s = cx.style(metrics::FONT_LABEL);
    let contents = cx.layout(&t!("help-contents"), label_s, None);
    let cw = contents.width + 20.0;
    let cr = cx.m_in(
        Rect::new(r.right() - 46.0 - 8.0 - cw, r.y + 12.0, cw, bar_h),
        r,
    );
    crate::insights::button(cx, cr, &contents, Target::HelpContents, false);
    if cx.is_focused(&Target::HelpContents) {
        cx.button_ring(cr, metrics::RADIUS_SM + 2.0);
    }
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let tr = cx.m_in(
        Rect::new(
            back.right() + 6.0,
            r.y + 12.0,
            (cr.x - back.right() - 12.0).max(10.0),
            bar_h,
        ),
        r,
    );
    cx.label_fit(
        &t!("help-title"),
        title_s,
        tr,
        HAlign::Start,
        cx.pal.text,
        false,
    );

    // The search field.
    let field_h = cx.u(36.0);
    let field = cx.m_in(
        Rect::new(inner.x, r.y + 12.0 + bar_h + 10.0, inner.w, field_h),
        r,
    );
    let s = &mut ch.help;
    cx.field(
        field,
        &s.query,
        Some(&s.query_edit),
        &t!("help-search-placeholder"),
        on_field,
        true,
    );

    // The body.
    let body = Rect::new(
        inner.x,
        field.bottom() + 14.0,
        inner.w,
        r.bottom() - 16.0 - (field.bottom() + 14.0),
    );
    let styles = Styles {
        page_title: cx.style(metrics::FONT_LARGE).weight(650.0),
        h2: cx.style(metrics::FONT_TITLE).weight(650.0),
        h3: cx.style(metrics::FONT_BODY).weight(650.0),
        body: cx.style(metrics::FONT_BODY),
        small: cx.style(metrics::FONT_SMALL),
        strong: cx.style(metrics::FONT_BODY).weight(600.0),
    };
    let parts = lay_out(s, cx, st_view.keymap, body.w - 10.0, &styles);
    let content = parts.last().map_or(0.0, |p| p.y + p.h) + cx.u(12.0);
    let max = (content - body.h).max(0.0);
    if let Some(a) = s.jump.take()
        && let Some(p) = parts
            .iter()
            .find(|p| matches!(&p.part, Part::Heading { anchor, .. } if *anchor == a))
    {
        s.scroll = p.y - cx.u(4.0);
    }
    if s.scroll.is_nan() {
        let cur = s.last_scroll.clamp(0.0, max);
        s.scroll = match s.selected.and_then(|sel| {
            parts
                .iter()
                .find(|p| matches!(p.part, Part::Row { i, .. } if i == sel))
        }) {
            Some(p) if p.y < cur => p.y - cx.u(8.0),
            Some(p) if p.y + p.h > cur + body.h => p.y + p.h - body.h + cx.u(8.0),
            _ => cur,
        };
    }
    s.scroll = s.scroll.clamp(0.0, max);
    s.last_scroll = s.scroll;
    let scroll = s.scroll;
    let selected = s.selected;
    cx.dl.push_clip(body);
    for p in &parts {
        let y = body.y + p.y - scroll;
        if y + p.h < body.y || y > body.bottom() {
            continue;
        }
        let colour = |c: Colour, cx: &Ctx| match c {
            Colour::Text => cx.pal.text,
            Colour::Muted => cx.pal.text_muted,
            Colour::Faint => cx.pal.text_faint,
        };
        match &p.part {
            Part::Text {
                b,
                x,
                marker,
                color,
            } => {
                if let Some(mb) = marker {
                    let mr = cx.m_in(Rect::new(body.x, y, *x - 4.0, mb.height), body);
                    cx.block_top(mb, mr, HAlign::End, cx.pal.text_muted);
                }
                let tr = cx.m_in(Rect::new(body.x + x, y, body.w - x - 10.0, p.h), body);
                let c = colour(*color, cx);
                cx.block_top(b, tr, HAlign::Start, c);
            }
            Part::Heading { b, .. } => {
                let tr = cx.m_in(Rect::new(body.x, y, body.w - 10.0, p.h), body);
                cx.block_top(b, tr, HAlign::Start, cx.pal.text);
            }
            Part::Note(b) => {
                let band = cx.m_in(Rect::new(body.x, y, body.w - 10.0, p.h), body);
                cx.dl
                    .rect(band, metrics::RADIUS_SM + 1.0, cx.pal.accent_soft);
                let edge = cx.m_in(Rect::new(body.x, y, cx.u(3.0), p.h), body);
                cx.dl.rect(edge, 1.5, cx.pal.accent);
                let pad = cx.u(10.0);
                let tr = cx.m_in(
                    Rect::new(
                        body.x + pad + cx.u(3.0),
                        y + pad,
                        body.w - 2.0 * pad - cx.u(3.0) - 10.0,
                        b.height,
                    ),
                    body,
                );
                cx.block_top(b, tr, HAlign::Start, cx.pal.text);
            }
            Part::Row {
                i,
                title,
                sub,
                link,
            } => {
                let rr = Rect::new(body.x, y, body.w - 10.0, p.h);
                let t = Target::HelpRow(*i);
                if selected == Some(*i) {
                    cx.dl.rect(rr, metrics::RADIUS_SM + 1.0, cx.pal.accent_soft);
                } else if cx.is_hover(&t) {
                    cx.dl
                        .rect(rr, metrics::RADIUS_SM + 1.0, cx.pal.surface_hover);
                }
                let pad = if *link {
                    cx.u(4.0)
                } else if sub.is_some() {
                    cx.u(8.0)
                } else {
                    cx.u(6.0)
                };
                let mut x = pad;
                if *link {
                    let ir = cx.m_in(
                        Rect::new(body.x + pad, y + (p.h - 12.0) * 0.5, 12.0, 12.0),
                        rr,
                    );
                    draw_icon(
                        cx.dl,
                        if cx.rtl {
                            Icon::ChevronLeft
                        } else {
                            Icon::ChevronRight
                        },
                        ir,
                        cx.pal.accent,
                        1.6,
                        false,
                    );
                    x += cx.u(18.0);
                }
                let tr = cx.m_in(
                    Rect::new(body.x + x, y + pad, rr.w - x - pad, title.height),
                    rr,
                );
                let tc = if *link { cx.pal.accent } else { cx.pal.text };
                cx.block_top(title, tr, HAlign::Start, tc);
                if let Some(sb) = sub {
                    let sr = cx.m_in(
                        Rect::new(
                            body.x + x,
                            y + pad + title.height + cx.u(3.0),
                            rr.w - x - pad,
                            sb.height,
                        ),
                        rr,
                    );
                    cx.block_top(sb, sr, HAlign::Start, cx.pal.text_muted);
                }
                cx.hit(rr.intersect(&body), t);
            }
            Part::KeyLine { name, keys } => {
                let nr = cx.m_in(Rect::new(body.x, y, body.w * 0.6, p.h), body);
                cx.block_in(name, nr, HAlign::Start, cx.pal.text);
                let kr = cx.m_in(
                    Rect::new(body.x + body.w * 0.6, y, body.w * 0.4 - 10.0, p.h),
                    body,
                );
                let ks = if keys.is_empty() {
                    t!("help-keys-unbound")
                } else {
                    keys.clone()
                };
                let mono = cx.style(metrics::FONT_SMALL).tabular();
                cx.label_fit(&ks, mono, kr, HAlign::End, cx.pal.text_muted, false);
            }
            Part::Rule => {
                let rr = cx.m_in(Rect::new(body.x, y, body.w - 10.0, 1.0), body);
                cx.dl.rect(rr, 0.0, cx.pal.separator);
            }
        }
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::Help,
        body,
        Rect::new(r.x, body.y, r.w, body.h),
        max,
    );
}
