//! The command palette (Ctrl+K): fuzzy search over every action, the user's commands and
//! settings ("Set: Theme → Dark"). The same sheet also shows lists the application fills
//! ("Open with…", [`Chrome::open_list`](crate::Chrome::open_list)).

use specular_gfx::{Icon, Rect, draw_icon};
use specular_i18n::{I18n, fold_for_search, t};
use specular_input::{Action, Capability, Disabled};
use specular_settings::{Kind, Value};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, PaletteItem, ViewerState};

#[derive(Clone, Debug)]
pub(crate) enum Run {
    Action(Action),
    Set(&'static str, Value),
    /// An application item ([`PaletteItem::id`]).
    Item(u64, Icon),
    /// Opens the settings.toml editor.
    EditSettingsFile,
    /// Opens the keymap.toml editor.
    EditKeymapFile,
    /// Opens the manual at a page (index in [`specular_help::Manual::pages`]) and section.
    Help(usize, String),
    /// Turns a capability on, for this session or always ([`Command::Capability`]).
    ///
    /// [`Command::Capability`]: crate::Command::Capability
    Capability(Capability, bool),
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub label: String,
    pub key: Option<String>,
    pub run: Run,
    /// Why the action is unavailable; the entry is listed greyed out with this reason.
    pub disabled: Option<Disabled>,
    /// An application item that cannot run now, and why.
    pub unavailable: Option<String>,
    /// The action's capability is off ("Medical tools are off"): listed greyed, and
    /// choosing it turns the capability on for the session, then runs it.
    pub hidden: Option<Capability>,
}

impl Entry {
    pub fn greyed(&self) -> bool {
        self.disabled.is_some() || self.unavailable.is_some() || self.hidden.is_some()
    }

    /// Why the entry cannot run, in the UI language.
    pub fn reason(&self) -> Option<String> {
        match (&self.hidden, &self.disabled, &self.unavailable) {
            (Some(c), ..) => Some(tools_off(*c)),
            (None, Some(why), _) => Some(I18n::global().tr(why.message_id())),
            (None, None, u) => u.clone(),
        }
    }

    /// What the entry does, in one sentence: an action's description (shown under the list
    /// for the selected entry). Settings and application items have none.
    pub fn help(&self) -> Option<String> {
        if matches!(self.run, Run::Help(..)) {
            return Some(t!("palette-help-topic-help"));
        }
        let Run::Action(a) = self.run else {
            return None;
        };
        // Unavailable: Enter opens the manual where it says why.
        if self.disabled.is_some() {
            return Some(t!("palette-disabled-help"));
        }
        let i18n = I18n::global();
        let id = a.help_id();
        i18n.localizer().has_message(&id).then(|| i18n.tr(&id))
    }
}

/// "Medical tools": a capability's tools, as the toolbar chip names them.
pub(crate) fn tools(c: Capability) -> String {
    t!("profile-tools", capability = c.id())
}

/// "Medical tools are off".
/// "Turn on Medical tools for this session" (or "always").
pub(crate) fn turn_on(c: Capability, always: bool) -> String {
    t!(
        "profile-turn-on",
        tools = tools(c),
        always = if always { "yes" } else { "no" }
    )
}

pub(crate) fn tools_off(c: Capability) -> String {
    t!("profile-tools-off", capability = c.id())
}

/// A list the application filled ("Open with…") shown in the palette's sheet instead of the
/// actions.
#[derive(Clone, Debug, Default)]
pub(crate) struct ListMode {
    pub placeholder: String,
    pub items: Vec<PaletteItem>,
    /// Still being filled: an empty list says so instead of "no results".
    pub loading: Option<String>,
}

#[derive(Default)]
pub(crate) struct PaletteState {
    pub query: String,
    /// The field's caret, selection and undo history (`query` is the truth).
    pub query_edit: crate::line_edit::LineEdit,
    pub items: Vec<Entry>,
    pub selected: usize,
    pub scroll: f32,
    pub last_scroll: f32,
    pub dirty: bool,
    /// Application entries listed with the actions (user commands).
    pub extra: Vec<PaletteItem>,
    /// A list shown instead of the actions.
    pub list: Option<ListMode>,
}

impl PaletteState {
    pub fn reset(&mut self) {
        self.list = None;
        self.query.clear();
        self.selected = 0;
        self.scroll = 0.0;
        self.dirty = true;
    }
    pub fn type_text(&mut self, s: &str) {
        self.edit().insert(s);
        self.edited();
    }
    pub fn backspace(&mut self) {
        let mut clip = crate::line_edit::MemoryClipboard::default();
        let k = specular_input::Key::Named(specular_input::NamedKey::Backspace);
        self.edit()
            .key(k, specular_input::Mods::NONE, false, &mut clip);
        self.edited();
    }
    /// The field, caught up with `query`.
    pub fn edit(&mut self) -> &mut crate::line_edit::LineEdit {
        self.query_edit.sync(&self.query);
        &mut self.query_edit
    }
    /// The field edited: the list follows it, from its top.
    pub fn edited(&mut self) {
        if self.query != self.query_edit.text() {
            self.query = self.query_edit.text().to_owned();
            self.selected = 0;
            self.scroll = 0.0;
            self.dirty = true;
        }
    }
    pub fn move_sel(&mut self, d: i32) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len() as i32;
        self.selected = (self.selected as i32 + d).clamp(0, n - 1) as usize;
        self.scroll = f32::NAN; // scroll the selection into view
    }
}

fn fold(s: &str) -> String {
    fold_for_search(s).to_lowercase()
}

/// Fuzzy match score of `needle` in `hay` (both folded); `None` when it does not match.
/// Substrings beat scattered letters; matches at word starts beat matches inside words.
pub(crate) fn fuzzy(hay: &str, needle: &str) -> Option<f32> {
    if needle.is_empty() {
        return Some(0.0);
    }
    if let Some(pos) = hay.find(needle) {
        let word_start = pos == 0 || hay[..pos].ends_with(|c: char| !c.is_alphanumeric());
        return Some(
            100.0 + if word_start { 30.0 } else { 0.0 }
                - pos as f32 * 0.3
                - (hay.len() as f32 * 0.05),
        );
    }
    // Every word of the needle as a substring somewhere.
    let words: Vec<&str> = needle.split_whitespace().collect();
    if words.len() > 1 && words.iter().all(|w| hay.contains(w)) {
        return Some(80.0);
    }
    // Letters in order.
    let mut score = 50.0f32;
    let mut it = hay.chars().peekable();
    let mut prev_matched = false;
    let mut prev_char = ' ';
    for n in needle.chars().filter(|c| !c.is_whitespace()) {
        let mut found = false;
        for c in it.by_ref() {
            if c == n {
                if prev_matched {
                    score += 2.0;
                }
                if !prev_char.is_alphanumeric() {
                    score += 4.0;
                }
                prev_char = c;
                prev_matched = true;
                found = true;
                break;
            }
            prev_char = c;
            prev_matched = false;
            score -= 0.5;
        }
        if !found {
            return None;
        }
    }
    Some(score.max(1.0))
}

/// Sections of the manual the palette lists for a search.
const HELP_HITS: usize = 3;

pub(crate) fn rebuild(p: &mut PaletteState, st: &ViewerState) {
    p.dirty = false;
    let i18n = I18n::global();
    let q = fold(p.query.trim());
    let mut scored: Vec<(f32, usize, Entry)> = Vec::new();
    if let Some(list) = &p.list {
        for (i, it) in list.items.iter().enumerate() {
            if let Some(s) = fuzzy(&fold(&it.label), &q) {
                scored.push((s, i, item_entry(it)));
            }
        }
        if !q.is_empty() {
            scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        }
        p.items = scored.into_iter().map(|e| e.2).collect();
        p.selected = p.selected.min(p.items.len().saturating_sub(1));
        return;
    }
    for (i, a) in Action::LISTED.iter().enumerate() {
        let label = a.name_in(&st.actions);
        let s = fuzzy(&fold(&label), &q).or_else(|| fuzzy(&a.id(), &q).map(|s| s * 0.6));
        if let Some(s) = s {
            // Hidden by the profile: found by a search only (greyed, offering its tools).
            let hidden = a.hidden(&st.actions);
            if hidden.is_some() && q.is_empty() {
                continue;
            }
            let key = st.keymap.primary_key(*a).map(|k| k.to_string());
            let disabled = a.disabled(&st.actions);
            scored.push((
                s,
                i,
                Entry {
                    label,
                    key,
                    run: Run::Action(*a),
                    disabled,
                    unavailable: None,
                    hidden,
                },
            ));
        }
    }
    let n = Action::LISTED.len();
    for (i, it) in p.extra.iter().enumerate() {
        if let Some(s) = fuzzy(&fold(&it.label), &q) {
            scored.push((s, n + i, item_entry(it)));
        }
    }
    if !q.is_empty() {
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    }
    if q.is_empty() {
        // Browsing without a query: what can run now comes first (and gets the selection);
        // the rest follow, greyed out with their reason.
        scored.sort_by_key(|e| e.2.greyed());
    }
    let mut items: Vec<Entry> = scored.into_iter().map(|e| e.2).collect();
    if !q.is_empty() {
        // Capabilities that are off: offered when a command of theirs was found, or when
        // the search names them ("medical").
        let profile = st.actions.content_profile();
        for c in Capability::ALL {
            if c == Capability::General || profile.is_on(c) {
                continue;
            }
            let found = items.iter().any(|e| e.hidden == Some(c));
            for always in [false, true] {
                let label = turn_on(c, always);
                if found || fuzzy(&fold(&tools(c)), &q).is_some_and(|s| s >= 100.0) {
                    items.push(Entry {
                        label,
                        key: None,
                        run: Run::Capability(c, always),
                        disabled: None,
                        unavailable: None,
                        hidden: None,
                    });
                }
            }
        }
        for (label, run) in [
            (
                i18n.tr("settings-advanced-edit-settings-file"),
                Run::EditSettingsFile,
            ),
            (i18n.tr("keys-edit-file"), Run::EditKeymapFile),
        ] {
            if fuzzy(&fold(&label), &q).is_some_and(|s| s > 0.0) {
                items.push(Entry {
                    label,
                    key: None,
                    run,
                    disabled: None,
                    unavailable: None,
                    hidden: None,
                });
            }
        }
        // Settings: toggles and enum options, best matches first.
        for m in specular_settings::search(&p.query, i18n)
            .into_iter()
            .take(10)
        {
            let current = st.store.get(m.key).ok();
            let label = m.label(i18n);
            match &m.kind {
                Kind::Bool => {
                    let on = matches!(current, Some(Value::Bool(true)));
                    let v = Value::Bool(!on);
                    let text = t!(
                        "palette-set-setting",
                        setting = label.clone(),
                        value = m.display_value(&v, i18n)
                    );
                    items.push(Entry {
                        label: text,
                        key: None,
                        run: Run::Set(m.key, v),
                        disabled: None,
                        unavailable: None,
                        hidden: None,
                    });
                }
                Kind::Enum { options } => {
                    for o in options.iter() {
                        let v = Value::Text((*o).to_owned());
                        if current.as_ref() == Some(&v) {
                            continue;
                        }
                        let text = t!(
                            "palette-set-setting",
                            setting = label.clone(),
                            value = m.display_value(&v, i18n)
                        );
                        items.push(Entry {
                            label: text,
                            key: None,
                            run: Run::Set(m.key, v),
                            disabled: None,
                            unavailable: None,
                            hidden: None,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    // The manual: its best sections for three letters or more, after the commands.
    if q.chars().count() >= 3 {
        let m = specular_help::Manual::current();
        for h in m.search(&p.query, HELP_HITS) {
            let page = &m.pages()[h.page].title;
            let title = if h.heading.is_empty() {
                page.clone()
            } else {
                format!("{page} › {}", h.heading)
            };
            items.push(Entry {
                label: t!("palette-help-topic", title = title),
                key: None,
                run: Run::Help(h.page, h.anchor),
                disabled: None,
                unavailable: None,
                hidden: None,
            });
        }
    }
    items.truncate(80);
    p.items = items;
    p.selected = p.selected.min(p.items.len().saturating_sub(1));
}

fn item_entry(it: &PaletteItem) -> Entry {
    Entry {
        label: it.label.clone(),
        key: it.key.clone(),
        run: Run::Item(it.id, it.icon),
        disabled: None,
        unavailable: it.unavailable.clone(),
        hidden: None,
    }
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState) {
    // A Tab away from the field puts the keyboard on Close (the field still takes typing).
    let on_field = ch.modal_focus.is_none();
    let p = &mut ch.palette;
    // The enabled state can change while the palette is open (an image finishes loading).
    let state_changed = p.items.iter().any(|e| match e.run {
        Run::Action(a) => {
            a.disabled(&st.actions) != e.disabled || a.hidden(&st.actions) != e.hidden
        }
        Run::Set(..)
        | Run::Item(..)
        | Run::EditSettingsFile
        | Run::EditKeymapFile
        | Run::Capability(..)
        | Run::Help(..) => false,
    });
    if p.dirty || state_changed {
        rebuild(p, st);
    }
    let w = (cx.w - 48.0).min(cx.u(620.0));
    let row = cx.u(38.0);
    let field_h = cx.u(38.0);
    let visible = (((cx.h * 0.62 - 60.0) / row).floor() as usize).clamp(3, 10);
    let list_h = row * visible.min(p.items.len().max(1)) as f32;
    // Under the list: what the selected command does, two lines at most. Only the actions
    // have descriptions, so lists the application fills have no footer.
    let help_style = cx.style(metrics::FONT_SMALL);
    let help_h = if p.list.is_none() && !p.items.is_empty() {
        let line = cx.layout("Ag", help_style, None).height;
        (line * 2.0).ceil() + 14.0
    } else {
        0.0
    };
    let h = field_h + 18.0 + list_h + 12.0 + help_h;
    let r = Rect::new(((cx.w - w) * 0.5).round(), (cx.h * 0.14).round(), w, h);
    cx.sheet(r);

    // Query field, and Close at its end (Tab reaches it; the field keeps what is typed).
    let close_w = 32.0;
    let field = cx.m_in(
        Rect::new(r.x + 12.0, r.y + 10.0, r.w - 24.0 - close_w - 8.0, field_h),
        r,
    );
    let close = cx.m_in(
        Rect::new(
            r.right() - 12.0 - close_w,
            r.y + 10.0 + (field_h - close_w) * 0.5,
            close_w,
            close_w,
        ),
        r,
    );
    let placeholder = match &p.list {
        Some(l) => l.placeholder.clone(),
        None => t!("palette-placeholder"),
    };
    cx.field(
        field,
        &p.query,
        Some(&p.query_edit),
        &placeholder,
        on_field,
        true,
    );
    cx.close_button(close);

    let list = Rect::new(r.x + 6.0, field.bottom() + 8.0, r.w - 12.0, list_h);
    if p.items.is_empty() {
        let style = cx.style(metrics::FONT_BODY);
        let empty = match p.list.as_ref().and_then(|l| l.loading.clone()) {
            Some(loading) => loading,
            None => t!("palette-no-results"),
        };
        cx.label(&empty, style, list, HAlign::Center, cx.pal.text_muted);
        return;
    }
    let content = p.items.len() as f32 * row;
    let max = (content - list.h).max(0.0);
    if p.scroll.is_nan() {
        // Bring the selection into view.
        let top = p.selected as f32 * row;
        let cur = p.last_scroll.clamp(0.0, max);
        p.scroll = if top < cur {
            top
        } else if top + row > cur + list.h {
            top + row - list.h
        } else {
            cur
        };
    }
    p.scroll = p.scroll.clamp(0.0, max);
    p.last_scroll = p.scroll;
    cx.dl.push_clip(list);
    let style = cx.style(metrics::FONT_BODY);
    let first = (p.scroll / row).floor() as usize;
    for i in first..p.items.len().min(first + visible + 1) {
        let e = &p.items[i];
        let rr = Rect::new(list.x, list.y + i as f32 * row - p.scroll, list.w, row);
        let t = Target::PaletteRow(i);
        if i == p.selected {
            cx.dl.rect(
                rr.inset_xy(0.0, 1.0),
                metrics::RADIUS_SM + 1.0,
                cx.pal.accent_soft,
            );
        } else if cx.is_hover(&t) {
            cx.dl.rect(
                rr.inset_xy(0.0, 1.0),
                metrics::RADIUS_SM + 1.0,
                cx.pal.surface_hover,
            );
        }
        let icon = match e.run {
            Run::Action(_) => Icon::Keyboard,
            Run::Help(..) => Icon::Info,
            Run::Set(..) | Run::EditSettingsFile | Run::EditKeymapFile | Run::Capability(..) => {
                Icon::Settings
            }
            Run::Item(_, icon) => icon,
        };
        let faint = cx.pal.text_faint.with_alpha(0.6);
        // Icons grow with the text, less than it (a 2× text gets 1.5× icons).
        let (isz, lead) = (cx.u(16.0 * 0.75) + 4.0, cx.u(28.0) + 10.0);
        let ir = cx
            .m_in(Rect::new(rr.x + 10.0, rr.y, isz + 2.0, rr.h), list)
            .centered(isz, isz);
        draw_icon(
            cx.dl,
            icon,
            ir,
            if e.greyed() { faint } else { cx.pal.text_faint },
            1.4,
            false,
        );
        // Disabled entries show why instead of their shortcut.
        let key_w = match (e.reason(), &e.key) {
            (Some(reason), _) => {
                let rs = cx.style(metrics::FONT_SMALL);
                let reason = cx.ellipsize(&reason, rs, rr.w * 0.45, false);
                let b = cx.layout(&reason, rs, None);
                let w = b.width;
                let r = cx.m_in(Rect::new(rr.right() - 10.0 - w, rr.y, w, rr.h), rr);
                cx.block_in(&b, r, HAlign::End, cx.pal.text_faint);
                w
            }
            (None, Some(k)) => cx.keycap_end(k, rr, 10.0),
            (None, None) => 0.0,
        };
        let tr = cx.m_in(
            Rect::new(rr.x + lead, rr.y, rr.w - lead - 12.0 - key_w - 12.0, rr.h),
            list,
        );
        // Long labels end in "…" (the row's accessible name has them in full).
        cx.label_fit(
            &e.label,
            style,
            tr,
            HAlign::Start,
            if e.greyed() {
                cx.pal.text_faint
            } else {
                cx.pal.text
            },
            false,
        );
        cx.hit(rr.intersect(&list), t);
    }
    cx.dl.pop_clip();
    if help_h > 0.0 {
        let line = Rect::new(r.x + 12.0, list.bottom() + 6.0, r.w - 24.0, 1.0);
        cx.dl.rect(line, 0.0, cx.pal.separator);
        let hr = Rect::new(r.x + 18.0, line.bottom() + 6.0, r.w - 36.0, help_h - 14.0);
        if let Some(help) = p.items.get(p.selected).and_then(Entry::help) {
            let b = cx.layout(&help, help_style, Some(hr.w));
            if b.height <= hr.h + 0.5 {
                cx.block_top(&b, hr, HAlign::Start, cx.pal.text_muted);
            } else {
                // A long translation: one line, cut short (the row's description has it all).
                let top = Rect::new(hr.x, hr.y, hr.w, hr.h * 0.5);
                cx.label_fit(
                    &help,
                    help_style,
                    top,
                    HAlign::Start,
                    cx.pal.text_muted,
                    false,
                );
            }
        }
    }
    crate::scrollbar::draw(ch, cx, crate::scrollbar::Area::Palette, list, r, max);
}
