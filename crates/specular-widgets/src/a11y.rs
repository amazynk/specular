//! The AccessKit tree of the chrome (DESIGN.md §2, "Accessibility from day one").
//!
//! The tree is derived from what the last [`Chrome::draw`] laid out: every hit target becomes a
//! node with its tooltip label as its name, the open modal becomes a dialog, toasts sit in a
//! live region so they are announced, and the image is an image node named after its file and
//! size. Building it costs a few hundred allocations, so the application only asks for it while
//! an assistive technology is active.

use std::hash::{DefaultHasher, Hash, Hasher};

use accesskit::{
    Action as AkAction, ActionData, ActionRequest, CustomAction, HasPopup, Live, Node, NodeId,
    Rect as AkRect, Role, TextPosition, TextSelection, Toggled, TreeId, TreeInfo, TreeUpdate,
};
use specular_gfx::Rect;
use specular_i18n::{I18n, t};
use specular_input::Action;
use specular_settings::{Kind, ToolbarItem, Value};

use crate::ctx::Target;
use crate::settings::SettingsTarget;
use crate::{Chrome, Modal, Response, ViewerState};

const ROOT: NodeId = NodeId(1);
const IMAGE: NodeId = NodeId(2);
const TOOLBAR: NodeId = NodeId(3);
const INFO: NodeId = NodeId(4);
const TOASTS: NodeId = NodeId(5);
const DIALOG: NodeId = NodeId(6);
/// The focused text field (go-to, palette, settings or language search).
const FIELD: NodeId = NodeId(7);
const MESSAGE: NodeId = NodeId(8);
/// The rows of the palette and the language picker.
const LIST: NodeId = NodeId(9);
/// The toolbar's More menu (the items that don't fit on the bar), while it is open.
const OVERFLOW: NodeId = NodeId(11);
/// The open menu of a row of the toolbar list in Settings.
const ROW_MENU: NodeId = NodeId(12);
/// The text of the focused field ([`text_run`]).
const FIELD_TEXT: NodeId = NodeId(13);
/// The zoom presets, while they are open.
const ZOOM_MENU: NodeId = NodeId(14);
/// The start screen, its recent folders and its "Things to try" cards.
const WELCOME: NodeId = NodeId(15);
const WELCOME_RECENT: NodeId = NodeId(16);
const WELCOME_TIPS: NodeId = NodeId(17);
/// The playback or page controls over the image (a bar of their own above the toolbar).
const TRANSPORT: NodeId = NodeId(18);
/// The choices of the open drop-down in Settings.
const DROPDOWN: NodeId = NodeId(19);
/// The sections of Settings (its sidebar), and the key editor's modes: tab lists.
const SECTIONS: NodeId = NodeId(20);
const KEY_MODES: NodeId = NodeId(21);
/// Settings' sidebar: its search field and the sections.
const SETTINGS_NAV: NodeId = NodeId(22);
/// Settings' section: the controls under its header, where they scroll.
const SETTINGS_BODY: NodeId = NodeId(23);
/// A movie's open player menu (subtitles, sound tracks or chapters).
const PLAYER_MENU: NodeId = NodeId(24);
/// The inspection panels (histogram, pixel inspector).
const HISTOGRAM: NodeId = NodeId(40);
const PIXEL: NodeId = NodeId(41);
/// What the channel view shows (a polite live region: switching channels is announced).
const CHANNEL: NodeId = NodeId(42);
/// The false-colour bar: its map, range and unit.
const COLOUR_BAR: NodeId = NodeId(43);
/// The display aids on (colour-vision simulation, HiLo): a polite live region.
const DISPLAY_AIDS: NodeId = NodeId(44);
/// Static text lines of a container (`TEXT + container * 4096 + line`).
const TEXT: u64 = 1 << 32;
/// The lines of the config editor's text (`EDITOR_LINE + line`, [`editor_line_id`]).
const EDITOR_LINE: u64 = 1 << 34;
/// Hit targets: a hash of the target with the top bit set, stable while it exists.
const TARGET_BIT: u64 = 1 << 63;
/// Announcements (`ANNOUNCE | n`).
const ANNOUNCE: u64 = 1 << 40;
/// The hardware report's headings, rows and table cells (`REPORT + n`).
const REPORT: u64 = 1 << 36;
/// The background-work panel's jobs, list items (`WORK_ITEM + row`).
const WORK_ITEM: u64 = 1 << 38;
/// The "Downloads" panel's rows, list items (`NET_ITEM + row`).
const NET_ITEM: u64 = 1 << 39;
/// The toolbar's custom action that opens its list in Settings (toolbar_edit.rs).
const CUSTOMISE: i32 = 10;

/// Drops the bidi controls of translated text from every name and description in `tree`:
/// Fluent's isolation marks (U+2068 FSI, U+2069 PDI), the LRI / RLI that right-to-left
/// languages resolve FSI to, and the RLM that starts their paragraphs (the set of
/// [`specular_i18n::strip_isolates`]). They keep placeables in order on screen, but a screen
/// reader gets them as characters: braille displays show them, and some synthesizers pause or
/// spell them. Values are left alone (a field's value is what the user typed, matched by its
/// text run).
pub fn strip_isolates(tree: &mut TreeUpdate) {
    let isolate = |c: char| {
        matches!(
            c,
            '\u{2066}'..='\u{2069}' | '\u{202A}'..='\u{202E}' | '\u{200E}' | '\u{200F}' | '\u{061C}'
        )
    };
    for (_, node) in &mut tree.nodes {
        if let Some(l) = node.label().filter(|l| l.contains(isolate)) {
            let l = l.replace(isolate, "");
            node.set_label(l);
        }
        if let Some(d) = node.description().filter(|d| d.contains(isolate)) {
            let d = d.replace(isolate, "");
            node.set_description(d);
        }
    }
}

/// Makes `field` (a text or search input) expose `value` as text with the caret at its end,
/// and returns the text run node to add as its only child, `run`. A value alone reaches
/// Windows and macOS, but AT-SPI reads a field's text and caret only from text runs: without
/// one Orca hears an empty field and cannot echo what is typed.
pub fn text_run(field: &mut Node, run: NodeId, value: &str) -> Node {
    text_run_sel(field, run, value, None)
}

/// [`text_run`] with the selection (anchor, caret) in characters; `None`: the caret at the
/// end.
pub fn text_run_sel(
    field: &mut Node,
    run: NodeId,
    value: &str,
    sel: Option<(usize, usize)>,
) -> Node {
    let mut n = Node::new(Role::TextRun);
    n.set_value(value);
    let lengths: Vec<u8> = value.chars().map(|c| c.len_utf8() as u8).collect();
    let at = |character_index| TextPosition {
        node: run,
        character_index,
    };
    let (anchor, focus) = sel.unwrap_or((lengths.len(), lengths.len()));
    n.set_character_lengths(lengths);
    field.set_value(value);
    field.set_text_selection(TextSelection {
        anchor: at(anchor),
        focus: at(focus),
    });
    n
}

/// The text run of line `l` of the config editor's text.
pub(crate) fn editor_line_id(l: usize) -> NodeId {
    NodeId(EDITOR_LINE + l as u64)
}

/// The line whose text run `id` is, for the config editor's text.
fn editor_line_of(id: NodeId) -> Option<usize> {
    id.0.checked_sub(EDITOR_LINE)
        .filter(|l| *l < EDITOR_LINE)
        .map(|l| l as usize)
}

/// [`text_run_sel`] for multi-line text: one run per line, each ending with its line break
/// (AccessKit's runs are lines, a paragraph ends at a run ending with one), with its bounds
/// where it is drawn; `sel`: anchor and caret as (line, character within the line). Returns
/// the runs, in order, to add as `field`'s children.
fn text_runs_lines(
    field: &mut Node,
    value: &str,
    lines: &[(&str, Option<AkRect>)],
    sel: ((usize, usize), (usize, usize)),
) -> Vec<(NodeId, Node)> {
    let at = |(l, character_index): (usize, usize)| TextPosition {
        node: editor_line_id(l),
        character_index,
    };
    field.set_value(value);
    field.set_text_selection(TextSelection {
        anchor: at(sel.0),
        focus: at(sel.1),
    });
    lines
        .iter()
        .enumerate()
        .map(|(l, (text, bounds))| {
            let mut n = Node::new(Role::TextRun);
            n.set_value(*text);
            let lengths: Vec<u8> = text.chars().map(|c| c.len_utf8() as u8).collect();
            n.set_character_lengths(lengths);
            if let Some(b) = bounds {
                n.set_bounds(*b);
            }
            (editor_line_id(l), n)
        })
        .collect()
}

fn target_id(t: &Target) -> NodeId {
    let mut h = DefaultHasher::new();
    format!("{t:?}").hash(&mut h);
    NodeId(h.finish() | TARGET_BIT)
}

/// The text run of a field of Settings (`t`: the field).
fn field_run_id(t: &Target) -> NodeId {
    NodeId(target_id(t).0 ^ 0x5eed)
}

fn custom(n: &mut Node, id: i32, description: String) {
    n.push_custom_action(CustomAction { id, description });
}

fn text_id(container: NodeId, line: usize) -> NodeId {
    NodeId(TEXT + (container.0 << 12) + line as u64)
}

/// Collects nodes; children are attached to their container as they are added.
struct Builder {
    nodes: Vec<(NodeId, Node)>,
    scale: f64,
}

impl Builder {
    fn bounds(&self, r: Rect) -> AkRect {
        let s = self.scale;
        AkRect::new(
            r.x as f64 * s,
            r.y as f64 * s,
            r.right() as f64 * s,
            r.bottom() as f64 * s,
        )
    }

    /// Puts the children of `parent` that are in `order` in that order, in the places they
    /// hold; the others stay where they are.
    fn reorder(&mut self, parent: NodeId, order: &[NodeId], last_to_end: bool) {
        let Some((_, p)) = self.nodes.iter_mut().find(|(i, _)| *i == parent) else {
            return;
        };
        let mut kids = p.children().to_vec();
        let slots: Vec<usize> = (0..kids.len())
            .filter(|i| order.contains(&kids[*i]))
            .collect();
        let mut moved: Vec<NodeId> = slots.iter().map(|i| kids[*i]).collect();
        moved.sort_by_key(|k| order.iter().position(|o| o == k));
        for (i, k) in slots.into_iter().zip(moved) {
            kids[i] = k;
        }
        // Tab reaches the last stop after everything: it is read last too.
        if let Some(last) = order.last().filter(|_| last_to_end)
            && let Some(at) = kids.iter().position(|k| k == last)
        {
            let k = kids.remove(at);
            kids.push(k);
        }
        p.set_children(kids);
    }

    fn add(&mut self, parent: NodeId, id: NodeId, node: Node) {
        if self.nodes.iter().any(|(i, _)| *i == id) {
            return;
        }
        // Controls of a container that is not in the tree (the info panel's tabs while it
        // slides away, say) are left out: an orphan node makes AccessKit's consumer panic on
        // the next update, taking the app down while a screen reader is running.
        let Some((_, p)) = self.nodes.iter_mut().find(|(i, _)| *i == parent) else {
            return;
        };
        p.push_child(id);
        self.nodes.push((id, node));
    }

    fn container(&mut self, parent: NodeId, id: NodeId, role: Role, name: &str) {
        let mut n = Node::new(role);
        if !name.is_empty() {
            n.set_label(name);
        }
        self.add(parent, id, n);
    }

    fn text(&mut self, parent: NodeId, line: usize, text: &str) {
        let mut n = Node::new(Role::Label);
        n.set_value(text);
        self.add(parent, text_id(parent, line), n);
    }
}

/// The hardware report under its dialog: a heading per section, each row as "label: value",
/// the path table as a table (a row of column headers, then one row per path with its task as
/// the row header), the notes as text.
fn report_nodes(b: &mut Builder, r: &crate::HwReport) {
    let mut next = REPORT;
    let mut id = || {
        next += 1;
        NodeId(next)
    };
    for s in &r.sections {
        let mut h = Node::new(Role::Heading);
        h.set_level(2);
        h.set_label(s.title.as_str());
        b.add(DIALOG, id(), h);
        for row in &s.rows {
            let value = row.value_text_spoken();
            let mut n = Node::new(Role::Label);
            n.set_label(format!("{}: {value}", row.label));
            b.add(DIALOG, id(), n);
        }
        if let Some(tb) = &s.table {
            let table = id();
            let mut t = Node::new(Role::Table);
            t.set_label(tb.caption.as_str());
            let rows = tb.tasks.iter().map(|t| t.paths.len()).sum::<usize>() + 1;
            t.set_row_count(rows);
            t.set_column_count(tb.columns.len());
            b.add(DIALOG, table, t);
            let mut row_index = 0;
            let mut add_row = |b: &mut Builder, cells: Vec<(Role, String)>| {
                let row = id();
                let mut n = Node::new(Role::Row);
                n.set_row_index(row_index);
                b.add(table, row, n);
                for (c, (role, text)) in cells.into_iter().enumerate() {
                    let mut n = Node::new(role);
                    n.set_label(text);
                    n.set_row_index(row_index);
                    n.set_column_index(c);
                    b.add(row, id(), n);
                }
                row_index += 1;
            };
            add_row(
                b,
                tb.columns
                    .iter()
                    .map(|c| (Role::ColumnHeader, c.clone()))
                    .collect(),
            );
            for task in &tb.tasks {
                for p in &task.paths {
                    let mut note = p.uses.join(", ");
                    if let Some((s, _)) = &p.state {
                        if !note.is_empty() {
                            note.push_str(", ");
                        }
                        note.push_str(s);
                    }
                    add_row(
                        b,
                        vec![
                            (Role::RowHeader, task.name.clone()),
                            (Role::Cell, p.engine.clone()),
                            (Role::Cell, p.time.clone()),
                            (Role::Cell, p.cpu.clone()),
                            (Role::Cell, p.runs.clone()),
                            (Role::Cell, note),
                        ],
                    );
                }
            }
        }
        for note in &s.notes {
            let mut n = Node::new(Role::Label);
            n.set_label(note.as_str());
            b.add(DIALOG, id(), n);
        }
    }
}

/// Name, role, value and state of one hit target; `None` for backgrounds (scrims, panel
/// bodies) that only exist to catch the pointer.
/// Whether `item` is a row of the open More menu.
fn in_overflow(ch: &Chrome, item: ToolbarItem) -> bool {
    ch.overflow_menu.is_some() && ch.toolbar_overflow.contains(&item)
}

fn describe(ch: &Chrome, t: &Target, st: &ViewerState) -> Option<Node> {
    let i18n = I18n::global();
    let mut n = Node::new(Role::Button);
    let mut clickable = true;
    match t {
        Target::Toolbar(item) => {
            let (label, _) = crate::toolbar::item_label(*item, st);
            n.set_label(label);
            match item {
                ToolbarItem::ZoomLevel => {
                    n.set_role(Role::ComboBox);
                    let zoom = crate::toolbar::zoom_label(st.zoom, st.at_fit);
                    // AT-SPI drops a combo box's value: the level is its description too.
                    n.set_description(zoom.as_str());
                    n.set_value(zoom);
                    n.set_expanded(ch.zoom_menu);
                }
                ToolbarItem::Info => n.set_toggled(toggled(ch.info_open)),
                ToolbarItem::Fullscreen => n.set_toggled(toggled(st.fullscreen)),
                ToolbarItem::Mpr => n.set_toggled(toggled(st.actions.mpr)),
                _ => {}
            }
            // A row of the More menu.
            if in_overflow(ch, *item) {
                n.set_role(match item {
                    ToolbarItem::Info | ToolbarItem::Fullscreen | ToolbarItem::Mpr => {
                        Role::MenuItemCheckBox
                    }
                    _ => Role::MenuItem,
                });
                n.add_action(AkAction::Focus);
            }
        }
        Target::ProfileChip(c) => {
            n.set_label(crate::palette::tools(*c));
            n.set_description(crate::palette::turn_on(*c, false));
        }
        Target::ToolbarMore => {
            n.set_label(t!("toolbar-more"));
            n.set_has_popup(HasPopup::Menu);
            n.set_expanded(ch.overflow_menu.is_some());
        }
        Target::Position => {
            // Named after the position ("Image 3 of 40"): AT-SPI drops a button's value. The
            // description (its tooltip) says it opens the go-to field.
            n.set_label(position_text(st));
            n.set_value(position_text(st));
        }
        Target::ZoomPreset(a) => {
            if *a == Action::ToggleFullscreen {
                // Ticked while the window is full screen.
                n.set_role(Role::MenuItemCheckBox);
                n.set_toggled(toggled(st.fullscreen));
            } else {
                n.set_role(Role::MenuItem);
            }
            let label = crate::toolbar::zoom_entries()
                .into_iter()
                .find(|(_, e)| e == a)
                .map(|(l, _)| l)?;
            n.set_label(label);
        }
        Target::Transport(a) => {
            n.set_label(crate::transport::label(*a, st));
            if matches!(a, Action::PrevPage | Action::NextPage)
                && let Some(p) = &st.pages
            {
                n.set_value(crate::pages::spoken(p));
            }
            if let (Action::PlayPause, Some(pb)) = (a, &st.playback) {
                n.set_value(crate::transport::readout(pb));
            }
            if matches!(a, Action::SlowerPlayback | Action::FasterPlayback)
                && let Some(pb) = &st.playback
            {
                n.set_value(t!(
                    "transport-speed-name",
                    speed = crate::transport::speed_number(pb.speed)
                ));
            }
        }
        Target::SeekBar => {
            let v = st.playback.as_ref()?.video?;
            n.set_role(Role::Slider);
            n.set_label(t!("transport-seek-bar"));
            n.set_value(crate::transport::time_text(&v));
            n.set_numeric_value(v.position);
            n.set_min_numeric_value(0.0);
            n.set_max_numeric_value(v.duration.max(0.0));
        }
        Target::Volume => {
            ch.volume_node(&mut n, st)?;
            clickable = false;
        }
        Target::PlayerButton(m) => {
            n.set_label(crate::player::label(*m));
            n.set_has_popup(HasPopup::Menu);
            n.set_expanded(ch.player_menu() == Some(*m));
        }
        Target::PlayerItem(m, i) => {
            let v = ch.player.as_ref()?;
            let (label, ticked) = crate::player::row_label(*m, v, *i)?;
            // A track, chapter, speed, device or speakers is one of several (radio); the
            // rest turn on and off on their own.
            use crate::player::PlayerCmd as P;
            let one_of = matches!(
                crate::player::stop_cmd(*m, v, *i),
                Some(
                    P::Subtitle(_)
                        | P::Sound(_)
                        | P::Chapter(_)
                        | P::Speed(_)
                        | P::Output(_)
                        | P::Speakers(_)
                        | P::CastTo(_)
                )
            );
            n.set_role(if one_of {
                Role::MenuItemRadio
            } else {
                Role::MenuItemCheckBox
            });
            n.set_label(label);
            n.set_toggled(toggled(ticked));
        }
        Target::PlayerDelay(m, row, step) => {
            let v = ch.player.as_ref()?;
            let steps = crate::player::steppers(*m, v);
            let s = steps.get(*row)?;
            n.set_label(match step {
                -1 => s.less.clone(),
                1 => s.more.clone(),
                _ => s.reset.clone(),
            });
            n.set_value(s.value.clone());
        }
        Target::Motion => {
            let m = st.motion.as_ref()?;
            n.set_label(crate::motion::label(m));
            n.set_toggled(toggled(m.playing));
        }
        Target::PresentingPause(paused) => {
            n.set_label(crate::presenting::pause_label(*paused));
            n.set_toggled(toggled(*paused));
            if let Some(p) = &ch.presenting_on {
                n.set_description(crate::presenting::text(p));
            }
        }
        Target::PresentingStop(show) => {
            n.set_label(crate::presenting::stop_label(*show));
            if let Some(p) = &ch.presenting_on {
                n.set_description(crate::presenting::text(p));
            }
        }
        Target::CastStop => {
            n.set_label(t!("cast-stop"));
            if let Some(d) = ch.player.as_ref().and_then(|p| crate::cast::title(&p.cast)) {
                n.set_description(d);
            }
        }
        Target::EdgePrev => n.set_label(t!("toolbar-prev")),
        Target::EdgeNext => n.set_label(t!("toolbar-next")),
        Target::ViewerBack => n.set_label(t!("toolbar-back-gallery")),
        Target::Welcome(w) => {
            use crate::welcome::WelcomeTarget as W;
            match w {
                W::Open | W::Browse | W::Pictures => {
                    let (label, about) = crate::welcome::entry_texts(ch, *w);
                    n.set_label(label);
                    n.set_description(about);
                    if *w == W::Open
                        && let Some(k) = st.keymap.primary_key(Action::Open)
                    {
                        n.set_keyboard_shortcut(k.to_string());
                    }
                }
                W::Recent(i) => {
                    n.set_role(Role::Link);
                    n.set_label(crate::welcome::recent_name(ch.welcome.recents.get(*i)?));
                }
                W::Tip(i) => {
                    let tip = ch.welcome_tips(st).into_iter().nth(*i)?;
                    n.set_label(tip.kind.title());
                    n.set_description(tip.kind.about());
                    n.set_keyboard_shortcut(tip.key.clone());
                }
                W::HideTips => {
                    n.set_role(Role::Link);
                    n.set_label(t!("welcome-tips-hide"));
                }
            }
        }
        Target::InfoTab(all) => {
            n.set_role(Role::Tab);
            n.set_label(if *all {
                t!("app-info-all-fields")
            } else {
                t!("app-info-summary")
            });
            n.set_selected(ch.info_all == *all);
        }
        Target::CheatTab(m) => {
            n.set_role(Role::Tab);
            n.set_label(m.label());
            n.set_selected(ch.cheat_shown(st.keymap) == *m);
        }
        Target::InfoClose | Target::ModalClose => n.set_label(t!("app-close")),
        Target::InfoShowOnMap => n.set_label(t!("app-info-show-on-map")),
        Target::InfoEditMetadata => n.set_label(t!("app-info-edit-metadata")),
        Target::Editor(e) => {
            use crate::config_editor::EditorTarget as E;
            match e {
                // The text is the focused field (or nothing: the find field has the keyboard).
                E::Text | E::FindField => return None,
                E::Problem(i) => {
                    n.set_role(Role::ListBoxOption);
                    n.set_label(ch.editor_problem_label(*i)?);
                    n.set_selected(ch.editor.problem_sel == *i);
                }
                E::Save => n.set_label(t!("config-editor-save")),
                E::Revert => n.set_label(t!("config-editor-revert")),
                E::Reload => n.set_label(t!("config-editor-reload")),
                E::KeepMine => n.set_label(t!("config-editor-keep-mine")),
                E::Discard => n.set_label(t!("config-editor-discard")),
                E::KeepEditing => n.set_label(t!("config-editor-keep-editing")),
                E::FindPrev => n.set_label(t!("config-editor-find-previous")),
                E::FindNext => n.set_label(t!("config-editor-find-next")),
                E::FindClose => n.set_label(t!("config-editor-find-close")),
            }
        }
        Target::InfoPerf => {
            n.set_label(t!("open-report-title"));
            n.set_expanded(ch.info_perf);
        }
        Target::InfoCopy(g, f) => {
            let field = st.metadata?.all_fields.get(*g)?.fields.get(*f)?;
            n.set_label(t!("app-info-copy-field", field = field.tag.clone()));
            // What is shown (the copy holds the whole value).
            n.set_description(format!("{}: {}", field.tag, field.display));
        }
        Target::InfoSummary(g, i) => {
            let item = st.metadata?.summary.get(*g)?.items.get(*i)?;
            n.set_label(t!("app-info-copy-field", field = item.label.clone()));
            n.set_description(format!("{}: {}", item.label, item.value));
        }
        Target::ToastUndo(id, _) => n.set_label(ch.toast_button(*id)),
        Target::ToastClose(_) => n.set_label(t!("app-close")),
        Target::PaletteRow(i) => {
            let e = ch.palette.items.get(*i)?;
            n.set_role(Role::ListBoxOption);
            n.set_label(e.label.clone());
            if let Some(k) = &e.key {
                n.set_keyboard_shortcut(k.clone());
            }
            // Why it is greyed out, else what it does.
            if let Some(why) = e.reason() {
                n.set_disabled();
                n.set_description(why);
            } else if let Some(help) = e.help() {
                n.set_description(help);
            }
            n.set_selected(ch.palette.selected == *i);
        }
        Target::LanguageRow(i) => {
            let (name, sub, _) = ch.language.rows.get(*i)?;
            n.set_role(Role::ListBoxOption);
            n.set_label(name.clone());
            if !sub.is_empty() {
                n.set_description(sub.clone());
            }
            n.set_selected(ch.language.selected == *i);
        }
        Target::Settings(s) if s.is_keys() => {
            clickable = crate::keys_edit::describe(&mut n, s, ch, st)?;
        }
        Target::TagRow(i) => {
            let s = &ch.dicom_tags;
            let row = s.sheet.rows.get(*s.visible.get(*i)?)?;
            n.set_role(Role::TreeItem);
            n.set_label(crate::dicom_tags::row_label(s, row));
            n.set_level(row.depth as usize + 1);
            if row.descendants > 0 {
                n.set_expanded(s.filtering() || s.expanded.contains(&s.visible[*i]));
            }
            n.set_selected(s.selected == *i);
        }
        Target::TagJump(_) => {
            n.set_role(Role::Button);
            n.set_label(t!("sr-show-image"));
        }
        Target::PlotArea => {
            n.set_role(Role::Figure);
            if let Some(s) = &ch.plot.sheet {
                n.set_label(s.title.clone());
            }
        }
        Target::TagPatient => {
            n.set_role(Role::Switch);
            n.set_label(t!("dicom-tags-show-patient"));
            n.set_toggled(toggled(ch.dicom_tags.patient));
        }
        Target::StudyRow(i) => {
            let s = &ch.studies;
            let row = s.sheet.rows.get(*i)?;
            n.set_role(Role::TreeItem);
            n.set_label(crate::studies::row_label(s, row));
            n.set_level(match row.kind {
                crate::StudyKind::Patient => 1,
                crate::StudyKind::Study => 2,
                crate::StudyKind::Series => 3,
            });
            if row.kind == crate::StudyKind::Series {
                n.set_selected(s.selected == *i);
                if row.missing {
                    n.set_disabled();
                }
            } else {
                n.set_expanded(true);
            }
        }
        Target::StudiesCancel => {
            n.set_label(t!("app-cancel"));
            n.set_description(t!("studies-cancel-description"));
        }
        Target::StudiesSend => {
            n.set_label(t!("studies-send"));
            n.set_description(t!("studies-send-description"));
        }
        Target::StudiesPatient => {
            n.set_role(Role::Switch);
            n.set_label(t!("studies-show-patient"));
            n.set_toggled(toggled(ch.studies.patient));
        }
        Target::Settings(s) if s.is_panel() => {
            clickable = crate::settings_panel::describe(&mut n, s, ch)?;
        }
        Target::Settings(s) => {
            describe_setting(&mut n, s, ch, st, i18n)?;
            clickable = !n.is_disabled();
        }
        Target::ConfirmOk => n.set_label(ch.confirm_texts().1),
        Target::ConfirmCancel => n.set_label(t!("app-cancel")),
        Target::ConfirmAlt => n.set_label(ch.confirm_alt.clone().unwrap_or_default().0),
        Target::ConfirmAlt2 => n.set_label(ch.confirm_alt2.clone().unwrap_or_default().0),
        Target::ProfileCheck(c) => {
            n.set_role(Role::CheckBox);
            n.set_label(crate::profile_sheet::label(*c));
            n.set_description(crate::profile_sheet::about(*c));
            n.set_toggled(toggled(ch.profile_pick.contains(*c)));
            if *c == specular_input::Capability::General {
                n.set_disabled();
                clickable = false;
            }
        }
        Target::ProfileDone => n.set_label(t!("profile-done")),
        Target::ProfileStart(o) => {
            n.set_role(Role::RadioButton);
            n.set_label(crate::profile_sheet::start_label(*o));
            n.set_toggled(toggled(ch.profile_start == *o));
        }
        Target::ProfileSkip => n.set_label(t!("profile-skip")),
        Target::ReportCopy => n.set_label(t!("hw-report-copy")),
        Target::DecoderCard(i) => {
            n.set_label(ch.decoder_card_button(*i)?);
            if let Some(c) = ch.decoder_card() {
                n.set_description(c.title.as_str());
            }
        }
        Target::Insight => {
            n.set_label(ch.insight_indicator.as_ref()?.label.as_str());
            n.set_description(t!("open-report-insights-title"));
            n.set_has_popup(HasPopup::Dialog);
        }
        Target::InsightHide => n.set_label(t!("open-report-insights-hide")),
        Target::InsightAction(i) => {
            let it = ch.insights.now.get(*i)?;
            n.set_label(it.action.as_ref()?.0.as_str());
            n.set_description(it.text.as_str());
        }
        Target::InsightDismiss => n.set_label(ch.insights.dismiss.as_ref()?.0.as_str()),
        Target::InsightSwitch => n.set_label(ch.insights.switch.as_ref()?.0.as_str()),
        Target::InsightRestore => n.set_label(ch.insights.restore.as_ref()?.0.as_str()),
        Target::Work => {
            n.set_label(ch.work_chip.as_ref()?.name.as_str());
            n.set_description(t!("work-show"));
            n.set_has_popup(HasPopup::Dialog);
        }
        // A progress bar: its value as a percentage, read out as the row's count.
        Target::WorkBar(i) => {
            let r = ch.work.rows.get(*i)?;
            n.set_role(Role::ProgressIndicator);
            n.set_label(r.title.as_str());
            n.set_value(r.count.as_str());
            if let Some(f) = r.fraction {
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(100.0);
                n.set_numeric_value((f64::from(f.clamp(0.0, 1.0)) * 100.0).round());
            }
            clickable = false;
        }
        Target::WorkPause(i) => {
            let r = ch.work.rows.get(*i)?;
            n.set_label(r.pause.as_ref()?.0.as_str());
            n.set_description(r.title.as_str());
        }
        Target::WorkSettings(i) => {
            let r = ch.work.rows.get(*i)?;
            n.set_label(r.settings.as_ref()?.0.as_str());
            n.set_description(r.title.as_str());
        }
        Target::Downloads => {
            if !ch.downloads_on {
                return None;
            }
            n.set_label(ch.downloads.name.as_str());
            n.set_description(t!("net-show"));
            n.set_has_popup(HasPopup::Dialog);
        }
        // A progress bar: its value as a percentage (none while the total is unknown),
        // read out as the row's figures.
        Target::DownloadBar(i) => {
            let r = ch.downloads.rows.get(*i)?;
            n.set_role(Role::ProgressIndicator);
            n.set_label(r.title.as_str());
            n.set_value(r.detail.as_str());
            if let Some(f) = r.fraction {
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(100.0);
                n.set_numeric_value((f64::from(f.clamp(0.0, 1.0)) * 100.0).round());
            }
            clickable = false;
        }
        Target::DownloadStop(i) => {
            let r = ch.downloads.rows.get(*i)?;
            n.set_label(r.stop.as_ref()?.0.as_str());
            n.set_description(r.title.as_str());
        }
        Target::Queue(q) => crate::queue_panel::describe(ch, *q, &mut n)?,
        Target::ToolbarMenu => {
            n.set_role(Role::MenuItem);
            n.set_label(t!("toolbar-customise"));
        }
        Target::MenuItem(i) => {
            let m = ch.menu.as_ref()?;
            let row = m.rows.get(*i)?;
            n.set_role(Role::MenuItem);
            n.set_label(row.label.as_str());
            if let Some(why) = &row.unavailable {
                n.set_description(why.as_str());
                n.set_disabled();
            }
            if m.sel == Some(*i) {
                n.set_selected(true);
            }
            if m.leads_on(*i) {
                n.set_expanded(m.sub.as_ref().is_some_and(|s| s.parent == *i));
            }
        }
        Target::MenuSubItem(j) => {
            let s = ch.menu.as_ref()?.sub.as_ref()?;
            let row = s.rows.get(*j)?;
            n.set_role(Role::MenuItem);
            n.set_label(row.label.as_str());
            if let Some(why) = &row.unavailable {
                n.set_description(why.as_str());
                n.set_disabled();
            }
            if s.sel == Some(*j) {
                n.set_selected(true);
            }
        }
        Target::HelpRow(i) => {
            let (label, link) = crate::help::row_label(&ch.help, *i)?;
            n.set_role(if link { Role::Link } else { Role::ListItem });
            n.set_label(label);
            if ch.help.selected == Some(*i) {
                n.set_selected(true);
            }
        }
        Target::AboutHelp => n.set_label(t!("about-help")),
        Target::AboutDiagnostics => n.set_label(t!("about-diagnostics")),
        Target::AboutRenderer => n.set_label(t!("about-renderer")),
        Target::AboutDiscord => n.set_label(t!("toolbar-discord")),
        Target::HelpBack => n.set_label(t!("help-back")),
        Target::HelpContents => n.set_label(t!("help-contents")),
        Target::CrashView => n.set_label(if ch.crash.show_report {
            t!("crash-hide")
        } else {
            t!("crash-view")
        }),
        Target::CrashCopy => n.set_label(t!("crash-copy")),
        Target::CrashSave => n.set_label(t!("crash-save-bundle")),
        Target::CrashDismiss => n.set_label(t!("crash-dismiss")),
        Target::DiagEntry(i) => {
            let e = ch.diag.sheet.entries.get(*i)?;
            n.set_role(Role::ListItem);
            n.set_label(format!("{}: {}", e.name, e.description));
            if ch.diag.selected == *i {
                n.set_selected(true);
            }
        }
        Target::DiagNames => {
            n.set_role(Role::CheckBox);
            n.set_label(t!("diag-include-names"));
            n.set_description(t!("diag-include-names-help"));
            n.set_toggled(toggled(ch.diag.sheet.include_names));
        }
        Target::DiagSave => n.set_label(t!("diag-save")),
        Target::DiagCancel => n.set_label(t!("diag-cancel")),
        Target::Toast
        | Target::MenuBg
        | Target::ToolbarBg
        | Target::BarSlot(_)
        | Target::InfoPanel
        | Target::InfoTitle
        // The sheet's lines are in the tree whole, as text.
        | Target::CheatLine(..)
        | Target::Scrim
        | Target::ModalBody
        | Target::Scrollbar(_) => return None,
    }
    // Disabled controls stay in the tree, announced as unavailable with the reason.
    if let Some(why) = t.disabled(st) {
        n.set_disabled();
        n.set_description(i18n.tr(why.message_id()));
        clickable = false;
    } else if let Some(tip) = t.tooltip(st)
        && n.label().is_some_and(|l| l != tip)
        && n.description().is_none()
    {
        // The tooltip adds the shortcut ("Next image (→)").
        n.set_description(tip);
    }
    if let Some(k) = t.action(st).and_then(|a| st.keymap.primary_key(a)) {
        n.set_keyboard_shortcut(k.to_string());
    }
    if clickable {
        n.add_action(AkAction::Click);
    }
    Some(n)
}

fn describe_setting(
    n: &mut Node,
    s: &SettingsTarget,
    ch: &Chrome,
    st: &ViewerState,
    i18n: &I18n,
) -> Option<()> {
    let meta = |k: &str| specular_settings::find(k);
    let value = |k: &str| st.store.get(k).ok();
    match s {
        SettingsTarget::Section(i) => {
            let sec = ch.sections().get(*i).copied()?;
            n.set_role(Role::Tab);
            n.set_label(sec.label(i18n));
            // As the sidebar shows it: no section is current while searching.
            n.set_selected(ch.settings.search.trim().is_empty() && ch.settings.section == *i);
        }
        SettingsTarget::Search | SettingsTarget::Dropdown_ => return None,
        // Described by the key editor (keys_edit::describe).
        SettingsTarget::KeyMode(_)
        | SettingsTarget::KeyPreset(_)
        | SettingsTarget::KeyImport
        | SettingsTarget::KeyExport
        | SettingsTarget::KeyExportMode
        | SettingsTarget::KeyEditFile
        | SettingsTarget::KeyResetMode
        | SettingsTarget::KeyResetAll
        | SettingsTarget::KeyConfirm
        | SettingsTarget::KeyCancel
        | SettingsTarget::KeyConfirmText
        | SettingsTarget::KeyProblem(_)
        | SettingsTarget::KeyNoMatch
        | SettingsTarget::KeyRow(..)
        | SettingsTarget::KeyChip(..)
        | SettingsTarget::KeyRemove(..)
        | SettingsTarget::KeyAdd(..)
        | SettingsTarget::KeyResetAction(..)
        | SettingsTarget::KeyChoice(_)
        | SettingsTarget::KeyPromptText(_)
        | SettingsTarget::KeyRecordCancel => return None,
        // Described from the chrome (settings_panel::describe).
        SettingsTarget::PanelHeading(_)
        | SettingsTarget::PanelRow(..)
        | SettingsTarget::PanelFooter(_)
        | SettingsTarget::PanelButton(..)
        | SettingsTarget::PanelCheck(..)
        | SettingsTarget::PanelExpand(..)
        | SettingsTarget::PanelAction(_)
        | SettingsTarget::PanelConfirm(_)
        | SettingsTarget::PanelCancel(_) => return None,
        SettingsTarget::Toggle(k) => {
            let m = meta(k)?;
            n.set_role(Role::Switch);
            n.set_label(m.label(i18n));
            n.set_toggled(toggled(matches!(value(k), Some(Value::Bool(true)))));
        }
        SettingsTarget::Step(k, d) => {
            let m = meta(k)?;
            let v = value(k)
                .map(|v| m.display_value(&v, i18n))
                .unwrap_or_default();
            n.set_label(format!(
                "{} {}",
                m.label(i18n),
                if *d < 0 { "−" } else { "+" }
            ));
            n.set_value(v);
        }
        SettingsTarget::Dropdown(k) => {
            let m = meta(k)?;
            n.set_role(Role::ComboBox);
            n.set_label(m.label(i18n));
            n.set_expanded(ch.settings.dropdown == Some(*k));
            if let Some(v) = value(k) {
                n.set_value(crate::settings::value_label(m, &v, i18n));
            }
        }
        SettingsTarget::Choose(k, v) => {
            let m = meta(k)?;
            n.set_role(Role::ListBoxOption);
            let label = match &m.kind {
                Kind::Language | Kind::Locale => specular_i18n::languages()
                    .iter()
                    .find(|l| l.id.to_string() == *v)
                    .map(|l| l.native_name.clone())
                    .unwrap_or_else(|| m.display_value(&Value::Text(v.clone()), i18n)),
                _ => crate::settings::value_label(m, &Value::Text(v.clone()), i18n),
            };
            n.set_label(label);
            n.set_selected(matches!(value(k), Some(Value::Text(c)) if c == *v));
        }
        SettingsTarget::ListItem(k, opt) => {
            let m = meta(k)?;
            n.set_role(Role::CheckBox);
            n.set_label(i18n.tr(&m.option_id(opt)));
            let on = matches!(value(k), Some(Value::List(l)) if l.iter().any(|o| o == opt));
            n.set_toggled(toggled(on));
            if crate::settings::locked_item(k, opt) {
                n.set_disabled();
            }
        }
        SettingsTarget::Reset(k) => {
            let m = meta(k)?;
            n.set_label(format!("{} — {}", t!("settings-reset"), m.label(i18n)));
        }
        SettingsTarget::Field(k) => {
            let m = meta(k)?;
            n.set_role(Role::TextInput);
            n.set_label(m.label(i18n));
            let (text, _) = ch.setting_field_a11y(k, st);
            n.set_value(text);
            if let Kind::Path { .. } = m.kind {
                n.set_placeholder(m.display_value(&Value::Path(None), i18n));
            }
            // Refused: the reason (it is shown under the field).
            if let Some(e) = ch.setting_draft(k).and_then(|d| d.error.clone()) {
                n.set_invalid(accesskit::Invalid::True);
                n.set_description(e);
            }
            n.add_action(AkAction::Focus);
            n.add_action(AkAction::SetValue);
            n.add_action(AkAction::ReplaceSelectedText);
            n.add_action(AkAction::SetTextSelection);
        }
        SettingsTarget::PathChoose(k) => {
            let m = meta(k)?;
            n.set_label(format!("{} — {}", t!("settings-choose"), m.label(i18n)));
            n.set_has_popup(HasPopup::Dialog);
        }
        SettingsTarget::PathClear(k) => {
            let m = meta(k)?;
            n.set_label(format!("{} — {}", t!("settings-clear"), m.label(i18n)));
        }
        SettingsTarget::Swatch(k, i) => {
            let m = meta(k)?;
            let (_, id) = crate::setting_field::SWATCHES.get(usize::from(*i))?;
            n.set_role(Role::RadioButton);
            n.set_label(format!("{} — {}", i18n.tr(id), m.label(i18n)));
            n.set_toggled(toggled(crate::setting_field::swatch_on(
                st,
                k,
                usize::from(*i),
            )));
        }
        SettingsTarget::Act(k) => {
            n.set_label(meta(k)?.label(i18n));
            if *k == crate::settings::RESET_ALL {
                // It asks first.
                n.set_has_popup(HasPopup::Dialog);
            }
            if ch.setting_disabled(k) {
                n.set_disabled();
            }
        }
        SettingsTarget::ToolRow(i) => {
            let list = crate::toolbar_edit::current(st.store);
            let hidden = crate::toolbar_edit::hidden(&list);
            n.set_role(Role::ListItem);
            n.add_action(AkAction::Focus);
            if let Some(it) = list.get(*i) {
                n.set_label(crate::toolbar::item_name(*it));
                n.set_description(t!(
                    "settings-toolbar-position",
                    index = (*i + 1) as i64,
                    count = list.len() as i64
                ));
                custom(n, 1, t!("settings-toolbar-move-up"));
                custom(n, 2, t!("settings-toolbar-move-down"));
                custom(n, 3, t!("settings-toolbar-move-start"));
                custom(n, 4, t!("settings-toolbar-move-end"));
                n.add_action(AkAction::CustomAction);
            } else {
                let it = hidden.get(*i - list.len())?;
                n.set_label(crate::toolbar::item_name(*it));
                n.set_description(t!("settings-toolbar-hidden-state"));
            }
        }
        SettingsTarget::ToolShow(i) => {
            let list = crate::toolbar_edit::current(st.store);
            let it = match list.get(*i) {
                Some(it) => *it,
                None => *crate::toolbar_edit::hidden(&list).get(*i - list.len())?,
            };
            n.set_role(Role::Switch);
            n.set_label(crate::toolbar::item_name(it));
            n.set_toggled(toggled(*i < list.len()));
        }
        SettingsTarget::ToolRemove(i) => {
            let it = *crate::toolbar_edit::current(st.store).get(*i)?;
            n.set_label(t!(
                "settings-toolbar-remove",
                item = crate::toolbar::item_name(it)
            ));
        }
        SettingsTarget::ToolUp(i) | SettingsTarget::ToolDown(i) => {
            let list = crate::toolbar_edit::current(st.store);
            let it = *list.get(*i)?;
            let (key, part) = if matches!(s, SettingsTarget::ToolUp(_)) {
                ("settings-toolbar-move-up", crate::toolbar_edit::Part::Up)
            } else {
                (
                    "settings-toolbar-move-down",
                    crate::toolbar_edit::Part::Down,
                )
            };
            n.set_label(i18n.tr(key));
            n.set_description(crate::toolbar::item_name(it));
            n.add_action(AkAction::Focus);
            if !crate::toolbar_edit::part_enabled(list.len(), *i, part) {
                n.set_disabled();
            }
        }
        SettingsTarget::ToolMenu(i) => {
            let list = crate::toolbar_edit::current(st.store);
            let (it, _) = crate::toolbar_edit::row_item(&list, *i)?;
            n.set_label(t!(
                "settings-toolbar-row-menu",
                item = crate::toolbar::item_name(it)
            ));
            n.set_has_popup(HasPopup::Menu);
            n.set_expanded(ch.settings.tools.menu == Some(*i));
            n.add_action(AkAction::Focus);
        }
        SettingsTarget::ToolMenuItem(i, op) => {
            let list = crate::toolbar_edit::current(st.store);
            let (it, shown) = crate::toolbar_edit::row_item(&list, *i)?;
            n.set_role(Role::MenuItem);
            n.set_label(crate::toolbar_edit::op_label(*op, it, shown));
            n.add_action(AkAction::Focus);
            let on = crate::toolbar_edit::row_ops(&list, *i).contains(&(*op, true));
            if !on {
                n.set_disabled();
            }
        }
        SettingsTarget::ToolMenuBg => return None,
        SettingsTarget::ToolAdd(it) => n.set_label(if *it == ToolbarItem::Spacer {
            t!("settings-toolbar-add-spacer")
        } else {
            t!("settings-toolbar-add-separator")
        }),
        SettingsTarget::ToolReset => n.set_label(t!("settings-toolbar-reset")),
        SettingsTarget::ResetSection => n.set_label(t!("settings-reset-section")),
    }
    Some(())
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

fn position_text(st: &ViewerState) -> String {
    if st.count == 0 {
        return String::new();
    }
    t!(
        "readout-position-a11y",
        index = (st.index + 1) as i64,
        count = st.count as i64
    )
}

impl Chrome {
    /// The open dialog's Tab order (`None`: its text field), as a cycle starting where a
    /// reader starts: the top of the dialog.
    fn a11y_tab_order(&self) -> Vec<Option<Target>> {
        match self.modal {
            Some(Modal::Settings) => std::iter::once(None)
                .chain(self.settings_stops().into_iter().map(Some))
                .collect(),
            // With the find bar's field in the tree (as the text field), the text is not.
            Some(Modal::ConfigEditor) => {
                use crate::config_editor::{EditorTarget as E, Region};
                let find = self.editor.region == Region::Find;
                let mut v = Vec::new();
                for t in self.editor_stops() {
                    match t {
                        None if find => {}
                        Some(Target::Editor(E::FindField)) => {
                            v.push(if find { None } else { t });
                            v.extend(
                                [E::FindPrev, E::FindNext, E::FindClose]
                                    .map(|e| Some(Target::Editor(e))),
                            );
                        }
                        t => v.push(t),
                    }
                }
                v
            }
            // The header's Back, Contents and Close, then the search field under them.
            Some(Modal::Help) => vec![
                Some(Target::HelpBack),
                Some(Target::HelpContents),
                Some(Target::ModalClose),
                None,
            ],
            Some(Modal::DicomTags) => {
                vec![None, Some(Target::TagPatient), Some(Target::ModalClose)]
            }
            Some(_) => self.modal_stops().into_iter().map(Some).collect(),
            None => Vec::new(),
        }
    }

    /// Turns on what a screen-reader user needs from the chrome: the toolbar stays shown
    /// (it is how the controls are found), instead of fading out when the pointer is idle.
    pub fn set_accessibility(&mut self, on: bool) {
        self.accessibility = on;
    }

    /// The full accessibility tree for the chrome as last drawn, in physical pixels
    /// (`scale` = physical per logical). `title` names the window.
    pub fn accessibility_tree(&self, st: &ViewerState, title: &str, scale: f64) -> TreeUpdate {
        let mut b = Builder {
            nodes: Vec::new(),
            scale,
        };
        let (w, h) = self.size;
        let mut root = Node::new(Role::Window);
        root.set_label(title);
        root.set_bounds(b.bounds(Rect::new(0.0, 0.0, w, h)));
        b.nodes.push((ROOT, root));

        // The image, or what stands in its place.
        if let Some(err) = st.error {
            // Named after the error (AT-SPI reads a name, not a value) and assertive, so the
            // error is spoken when it appears.
            let mut n = Node::new(Role::Alert);
            n.set_label(err);
            n.set_value(err);
            n.set_live(Live::Assertive);
            b.add(ROOT, MESSAGE, n);
        } else if st.has_image {
            let mut n = Node::new(Role::Image);
            let name = st.file_name.unwrap_or_default();
            n.set_label(match st.dims {
                Some((iw, ih)) => {
                    let loc = I18n::global().localizer();
                    t!(
                        "a11y-image",
                        name = name,
                        width = loc.integer(iw as i64),
                        height = loc.integer(ih as i64)
                    )
                }
                None => name.to_owned(),
            });
            if st.loading {
                n.set_busy();
            }
            n.set_bounds(b.bounds(Rect::new(0.0, 0.0, w, h)));
            b.add(ROOT, IMAGE, n);
        } else if st.loading {
            let mut n = Node::new(Role::ProgressIndicator);
            n.set_label(t!("app-info-loading"));
            b.add(ROOT, MESSAGE, n);
        } else if self.welcome_drawn && !self.gallery {
            // The start screen: its heading, what Specular is, then its parts.
            b.container(ROOT, WELCOME, Role::Group, &t!("welcome-title"));
            b.text(WELCOME, 0, &t!("welcome-tagline"));
            b.text(WELCOME, 1, &t!("welcome-drop"));
            let has = |f: &dyn Fn(&Target) -> bool| self.hits.iter().any(|(_, t)| f(t));
            if has(&|t| matches!(t, Target::Welcome(crate::welcome::WelcomeTarget::Recent(_)))) {
                b.container(WELCOME, WELCOME_RECENT, Role::List, &t!("welcome-recent"));
            }
            if has(&|t| matches!(t, Target::Welcome(crate::welcome::WelcomeTarget::HideTips))) {
                b.container(WELCOME, WELCOME_TIPS, Role::Group, &t!("welcome-tips"));
            } else if !st.settings.general.start_tips
                && let Some(hint) = crate::empty_hint(st.keymap)
            {
                b.text(WELCOME, 2, &hint);
            }
        }

        // The toolbar offers its list in Settings as a custom action (it is also in its
        // right-click menu and the palette).
        let mut bar = Node::new(Role::Toolbar);
        if !self.gallery {
            custom(&mut bar, CUSTOMISE, t!("toolbar-customise"));
            bar.add_action(AkAction::CustomAction);
        }
        b.add(ROOT, TOOLBAR, bar);
        if self.overflow_menu.is_some() {
            b.container(TOOLBAR, OVERFLOW, Role::Menu, &t!("toolbar-more"));
        }
        if self.zoom_menu {
            b.container(TOOLBAR, ZOOM_MENU, Role::Menu, &t!("toolbar-zoom-level"));
        }
        if self.info_open {
            b.container(ROOT, INFO, Role::Group, &t!("toolbar-info"));
            if let Some(m) = st.metadata {
                let mut line = 0;
                for g in &m.summary {
                    b.text(INFO, line, &g.title);
                    line += 1;
                    for it in &g.items {
                        b.text(INFO, line, &format!("{}: {}", it.label, it.value));
                        line += 1;
                    }
                }
            }
        }
        // Inspection panels: a summary line each (the pixel one updates as the pointer moves).
        if st.has_image && st.error.is_none() {
            let ins = &self.inspect;
            if ins.tools.histogram {
                b.container(ROOT, HISTOGRAM, Role::Group, &t!("histogram-title"));
                let s = match ins.histogram.as_deref().filter(|h| !h.is_empty()) {
                    Some(h) => crate::inspect::histogram_summary(h),
                    None => t!("histogram-empty"),
                };
                b.text(HISTOGRAM, 0, &s);
            }
            if ins.tools.pixel {
                b.container(ROOT, PIXEL, Role::Group, &t!("inspector-title"));
                match &ins.pixel {
                    Some(p) => {
                        b.text(PIXEL, 0, &crate::inspect::pixel_summary(p));
                        for (i, (l, v)) in crate::inspect::pixel_rows(p).iter().enumerate() {
                            b.text(PIXEL, i + 1, &format!("{l}: {v}"));
                        }
                    }
                    None => b.text(PIXEL, 0, &t!("inspector-outside")),
                }
            }
            if crate::inspect::colour_bar_shown(ins, st.settings)
                && let Some(l) = &ins.legend
            {
                b.container(
                    ROOT,
                    COLOUR_BAR,
                    Role::Group,
                    &crate::inspect::colour_bar_title(l),
                );
                b.text(COLOUR_BAR, 0, &crate::inspect::colour_bar_summary(l));
            }
        }
        // The display aids on, as their badges say (announced as they change).
        let mut aids = Vec::new();
        aids.extend(crate::inspect::cvd_label(self.inspect.tools.cvd));
        if st.has_image && !self.gallery {
            if self.inspect.tools.hilo {
                aids.push(t!("badge-hilo"));
            }
            if let Some(map) = self.inspect.false_colour() {
                aids.push(t!(
                    "badge-false-colour",
                    map = crate::inspect::colour_map_name(map)
                ));
            }
        }
        // Only while one is on (turning one on or off is also announced).
        if !aids.is_empty() {
            let mut aid = Node::new(Role::Status);
            aid.set_live(Live::Polite);
            aid.set_label(aids.join(", "));
            b.add(ROOT, DISPLAY_AIDS, aid);
        }
        // The channel view is announced as it changes; the image itself has no channel badge.
        let mut channel = Node::new(Role::Status);
        channel.set_live(Live::Polite);
        let view = self.channel_view(st.actions.has_alpha);
        let view = if st.has_image && !self.gallery {
            view
        } else {
            specular_inspect::ChannelView::Normal
        };
        channel.set_label(crate::inspect::channel_label(view));
        b.add(ROOT, CHANNEL, channel);
        // Toasts are announced as they appear (a polite live region).
        let mut toasts = Node::new(Role::Group);
        toasts.set_live(Live::Polite);
        toasts.set_label(t!("a11y-notifications"));
        b.add(ROOT, TOASTS, toasts);
        for toast in &self.toasts {
            let mut n = Node::new(Role::Status);
            n.set_label(toast.text.as_str());
            b.add(TOASTS, text_id(TOASTS, toast.id as usize), n);
        }
        // The last change made in the toolbar editors.
        if let Some((id, text, _)) = &self.announce {
            let mut n = Node::new(Role::Status);
            n.set_label(text.as_str());
            b.add(TOASTS, NodeId(ANNOUNCE | id), n);
        }
        if let Some(m) = self.modal {
            // Resetting every setting asks as an alert: the question is its name, what follows
            // from it its description.
            let alert = self.confirm_reset || self.confirm_destructive.is_some();
            let mut d = Node::new(if alert && m == Modal::ConfirmTrash {
                Role::AlertDialog
            } else {
                Role::Dialog
            });
            d.set_modal();
            if let Some(detail) = self.confirm_detail().filter(|_| m == Modal::ConfirmTrash) {
                d.set_description(detail);
            }
            if m == Modal::Profile {
                d.set_description(format!("{} {}", t!("profile-intro"), t!("profile-later")));
            }
            d.set_label(match m {
                Modal::Palette => t!("toolbar-palette"),
                Modal::CheatSheet => t!("cheatsheet-title"),
                Modal::Settings => t!("settings-title"),
                Modal::Language => t!("language-picker-title"),
                Modal::ConfirmTrash => self.confirm_texts().0,
                Modal::Profile => t!("profile-question"),
                Modal::About => t!("about-title"),
                Modal::HwReport => self.report.title_text(),
                Modal::Insights => t!("open-report-insights-title"),
                Modal::Work => t!("work-title"),
                Modal::Downloads => t!("net-title"),
                Modal::Queue => t!("queue-title"),
                Modal::DicomTags => t!("dicom-tags-title"),
                Modal::Studies => t!("studies-title"),
                Modal::Plot => self
                    .plot
                    .sheet
                    .as_ref()
                    .map(|s| s.title.clone())
                    .unwrap_or_default(),
                Modal::ConfigEditor => self.editor.file.title(),
                Modal::Help => match self.help_page_title() {
                    Some(p) => t!("help-label", page = p),
                    None => t!("help-title"),
                },
                Modal::Crash => t!("crash-title"),
                Modal::Diagnostics => t!("diag-title"),
            });
            b.add(ROOT, DIALOG, d);
            match m {
                Modal::About => {
                    for (i, s) in [
                        t!("about-version", version = env!("CARGO_PKG_VERSION")),
                        t!("about-tagline"),
                        t!("about-license"),
                    ]
                    .iter()
                    .enumerate()
                    {
                        b.text(DIALOG, i, s);
                    }
                }
                Modal::HwReport => report_nodes(&mut b, &self.report),
                Modal::Insights => {
                    for (i, line) in crate::insights::lines(&self.insights).iter().enumerate() {
                        b.text(DIALOG, i, line);
                    }
                }
                // A list of the jobs: each item names its job and reads its step, time left
                // and what follows; its progress bar and buttons are added with the controls.
                Modal::Work => {
                    if self.work.rows.is_empty() {
                        b.text(DIALOG, 0, &t!("work-empty"));
                    } else {
                        b.container(DIALOG, LIST, Role::List, &t!("work-title"));
                    }
                    for (i, r) in self.work.rows.iter().enumerate() {
                        let item = NodeId(WORK_ITEM + i as u64);
                        b.container(LIST, item, Role::ListItem, &r.title);
                        let lines = [Some(&r.step), Some(&r.status), r.then.as_ref()];
                        for (l, s) in lines.into_iter().flatten().enumerate() {
                            b.text(item, l, s);
                        }
                    }
                }
                // The queue's summary, or that it is empty; its rows are controls.
                Modal::Queue => {
                    let v = &self.queue.view;
                    if v.rows.is_empty() {
                        b.text(DIALOG, 0, &t!("queue-empty"));
                    } else {
                        b.text(DIALOG, 0, &v.summary);
                    }
                }
                // A list of the downloads, newest first: each item names it and reads its
                // figures and time left; its bar and Stop are added with the controls.
                Modal::Downloads => {
                    if self.downloads.rows.is_empty() {
                        b.text(DIALOG, 0, &t!("net-empty"));
                    } else {
                        b.container(DIALOG, LIST, Role::List, &t!("net-title"));
                    }
                    for (i, r) in self.downloads.rows.iter().enumerate().rev() {
                        let item = NodeId(NET_ITEM + i as u64);
                        b.container(LIST, item, Role::ListItem, &r.title);
                        for (l, s) in [&r.detail, &r.status].into_iter().enumerate() {
                            b.text(item, l, s);
                        }
                    }
                }
                // Every shortcut as one line of text: "Pick: P", under its group's heading.
                Modal::CheatSheet => {
                    let mut i = 0;
                    for (title, lines) in crate::cheatsheet::groups(self, st) {
                        b.text(DIALOG, i, &title);
                        i += 1;
                        for (label, keys) in lines {
                            let s = crate::cheatsheet::spoken_line(st.keymap, &label, &keys);
                            b.text(DIALOG, i, &s);
                            i += 1;
                        }
                    }
                }
                Modal::Palette | Modal::Language => b.container(DIALOG, LIST, Role::ListBox, ""),
                Modal::DicomTags => {
                    b.container(DIALOG, LIST, Role::Tree, &t!("dicom-tags-title"));
                    // What the panel says outside the tree: still reading, an error, the
                    // count of hidden values.
                    let s = &self.dicom_tags;
                    let mut i = 0;
                    for line in [s.sheet.status.as_ref(), s.sheet.note.as_ref()]
                        .into_iter()
                        .flatten()
                    {
                        b.text(DIALOG, i, line);
                        i += 1;
                    }
                    if s.hidden() > 0 {
                        b.text(
                            DIALOG,
                            i,
                            &t!("dicom-tags-hidden-count", count = s.hidden() as i64),
                        );
                    }
                }
                Modal::Studies => {
                    b.container(DIALOG, LIST, Role::Tree, &t!("studies-title"));
                    let s = &self.studies;
                    let mut i = 0;
                    for line in [s.sheet.status.as_ref(), s.sheet.note.as_ref()]
                        .into_iter()
                        .flatten()
                    {
                        b.text(DIALOG, i, line);
                        i += 1;
                    }
                    if s.sheet.rows.is_empty() && s.sheet.status.is_none() {
                        b.text(DIALOG, i, &t!("studies-none"));
                    }
                }
                // The settings, the caliper's reading and the table, as text.
                Modal::Plot => {
                    for (i, line) in crate::plot::describe(&self.plot).iter().enumerate() {
                        b.text(DIALOG, i, line);
                    }
                }
                // The page as text, block by block; the rows (results, contents, links) are
                // a list.
                Modal::Help => {
                    for (i, line) in crate::help::screen_text(&self.help, st.keymap)
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .enumerate()
                    {
                        b.text(DIALOG, i, line);
                    }
                    b.container(DIALOG, LIST, Role::List, &t!("help-title"));
                }
                Modal::Crash => {
                    b.text(DIALOG, 0, &t!("crash-body"));
                    if self.crash.show_report {
                        b.text(DIALOG, 1, &self.crash.sheet.report);
                    }
                }
                Modal::Diagnostics => {
                    let s = &self.diag;
                    let mut lines = vec![t!("diag-intro")];
                    if let Some(e) = s.sheet.entries.get(s.selected) {
                        lines.push(e.text.clone());
                    }
                    lines.push(t!("diag-privacy"));
                    lines.push(t!(
                        "diag-where",
                        issues = s.sheet.issues.clone(),
                        email = s.sheet.email.clone()
                    ));
                    for (i, l) in lines.iter().enumerate() {
                        b.text(DIALOG, i, l);
                    }
                    b.container(DIALOG, LIST, Role::List, &t!("diag-contents"));
                }
                // The problems pane is a list (its rows are added with the controls).
                Modal::ConfigEditor => {
                    b.container(DIALOG, LIST, Role::ListBox, &t!("config-editor-problems"));
                }
                Modal::Settings => {
                    let name = t!("a11y-settings-sections");
                    b.container(DIALOG, SETTINGS_NAV, Role::Navigation, &name);
                    b.container(SETTINGS_NAV, SECTIONS, Role::TabList, "");
                    let mut body = Node::new(Role::Group);
                    let searching = !self.settings.search.trim().is_empty();
                    match self.sections().get(self.settings.section) {
                        Some(sec) if !searching => body.set_label(sec.label(I18n::global())),
                        _ => body.set_label(t!("settings-title")),
                    }
                    body.set_bounds(b.bounds(self.settings.body));
                    b.add(DIALOG, SETTINGS_BODY, body);
                    if self
                        .hits
                        .iter()
                        .any(|(_, t)| matches!(t, Target::Settings(SettingsTarget::KeyMode(_))))
                    {
                        b.container(SETTINGS_BODY, KEY_MODES, Role::TabList, &t!("keys-modes"));
                    }
                    // The open drop-down's choices: a list of their own, named after it.
                    if let Some(m) = self.settings.dropdown.and_then(specular_settings::find) {
                        let name = m.label(I18n::global());
                        b.container(DIALOG, DROPDOWN, Role::ListBox, &name);
                    }
                    if let Some(row) = self.settings.tools.menu {
                        let list = crate::toolbar_edit::current(st.store);
                        if let Some((it, _)) = crate::toolbar_edit::row_item(&list, row) {
                            let label = t!(
                                "settings-toolbar-row-menu",
                                item = crate::toolbar::item_name(it)
                            );
                            b.container(DIALOG, ROW_MENU, Role::Menu, &label);
                        }
                    }
                }
                _ => {}
            }
        }

        // The focused text field.
        let field = self.text_field.map(|r| {
            let mut sel = None;
            let mut editor_text = false;
            let (role, value, label) = match self.modal {
                Some(Modal::ConfigEditor) => {
                    let (role, value, label, text) = self.editor_a11y_field();
                    editor_text = text;
                    (role, value, label)
                }
                // Named like the placeholder the field shows.
                Some(Modal::Palette) => (
                    Role::SearchInput,
                    self.palette.query.clone(),
                    match &self.palette.list {
                        Some(l) => l.placeholder.clone(),
                        None => t!("palette-placeholder"),
                    },
                ),
                Some(Modal::Settings) => (
                    Role::SearchInput,
                    self.settings.search.clone(),
                    t!("settings-search-placeholder"),
                ),
                Some(Modal::Language) => (
                    Role::SearchInput,
                    self.language.query.clone(),
                    t!("language-picker-search"),
                ),
                Some(Modal::DicomTags) => (
                    Role::SearchInput,
                    self.dicom_tags.query.clone(),
                    t!("dicom-tags-filter"),
                ),
                Some(Modal::Help) => (
                    Role::SearchInput,
                    self.help.query.clone(),
                    t!("help-search-placeholder"),
                ),
                _ => (
                    Role::TextInput,
                    self.goto
                        .as_ref()
                        .map(|g| g.text().to_owned())
                        .unwrap_or_default(),
                    if self.goto_time {
                        t!("player-go-to-time")
                    } else {
                        t!("toolbar-go-to")
                    },
                ),
            };
            let mut n = Node::new(role);
            n.set_label(label);
            let runs = if editor_text {
                // The config editor's text, line by line.
                n.add_action(AkAction::SetTextSelection);
                let lines: Vec<_> = self
                    .editor_a11y_lines()
                    .into_iter()
                    .map(|(t, r)| (t, r.map(|r| b.bounds(r))))
                    .collect();
                text_runs_lines(&mut n, &value, &lines, self.editor_a11y_selection())
            } else {
                // A dialog's search field has a caret and a selection of its own.
                let edit_sel = self.field_a11y_selection();
                if edit_sel.is_some() {
                    sel = edit_sel;
                    n.add_action(AkAction::SetTextSelection);
                }
                vec![(FIELD_TEXT, text_run_sel(&mut n, FIELD_TEXT, &value, sel))]
            };
            n.set_bounds(b.bounds(r));
            n.add_action(AkAction::Focus);
            (n, runs)
        });
        // A field of Settings with the keyboard is its own node (with its text run, below).
        let field = field.filter(|_| self.setting_field_focus().is_none());

        let field_parent = if self.modal == Some(Modal::Settings) {
            SETTINGS_NAV
        } else if self.modal.is_some() {
            DIALOG
        } else {
            TOOLBAR
        };
        let has_field = field.is_some();
        if let Some((n, runs)) = field {
            b.add(field_parent, FIELD, n);
            for (id, run) in runs {
                b.add(FIELD, id, run);
            }
        }

        // The playback or page controls: a bar of their own (they keep left to right in
        // right-to-left languages, as media controls do).
        if self.modal.is_none()
            && self
                .hits
                .iter()
                .any(|(_, t)| matches!(t, Target::Transport(_) | Target::Motion))
        {
            let name = if st.playback.is_some() || st.motion.is_some() {
                t!("a11y-transport-playback")
            } else {
                t!("a11y-transport-pages")
            };
            b.container(ROOT, TRANSPORT, Role::Toolbar, &name);
        }
        if self.modal.is_none()
            && let Some(m) = self.player_menu()
            && self
                .hits
                .iter()
                .any(|(_, t)| matches!(t, Target::PlayerItem(..) | Target::PlayerDelay(..)))
        {
            b.container(ROOT, PLAYER_MENU, Role::Menu, &crate::player::label(m));
        }

        // Every control, in drawing order (which is reading order within each container).
        for (r, t) in &self.hits {
            let Some(mut n) = describe(self, t, st) else {
                continue;
            };
            n.set_bounds(b.bounds(*r));
            let parent = match t {
                _ if self.modal.is_some() => match t {
                    Target::PaletteRow(_)
                    | Target::LanguageRow(_)
                    | Target::TagRow(_)
                    | Target::TagJump(_)
                    | Target::StudyRow(_)
                    | Target::HelpRow(_)
                    | Target::DiagEntry(_)
                    | Target::Editor(crate::config_editor::EditorTarget::Problem(_)) => LIST,
                    Target::Settings(SettingsTarget::Choose(..))
                        if self.settings.dropdown.is_some() =>
                    {
                        DROPDOWN
                    }
                    Target::Settings(SettingsTarget::Section(_)) => SECTIONS,
                    Target::Settings(SettingsTarget::KeyMode(_)) => KEY_MODES,
                    Target::Settings(SettingsTarget::ResetSection) => DIALOG,
                    Target::Settings(s)
                        if self.modal == Some(Modal::Settings)
                            && !matches!(s, SettingsTarget::ToolMenuItem(..)) =>
                    {
                        SETTINGS_BODY
                    }
                    Target::HelpBack
                    | Target::AboutHelp
                    | Target::AboutDiagnostics
                    | Target::AboutRenderer
                    | Target::AboutDiscord
                    | Target::HelpContents
                    | Target::CrashView
                    | Target::CrashCopy
                    | Target::CrashSave
                    | Target::CrashDismiss
                    | Target::DiagNames
                    | Target::DiagSave
                    | Target::DiagCancel => DIALOG,
                    Target::Settings(SettingsTarget::ToolMenuItem(..)) => ROW_MENU,
                    Target::WorkBar(i) | Target::WorkPause(i) | Target::WorkSettings(i) => {
                        NodeId(WORK_ITEM + *i as u64)
                    }
                    Target::DownloadBar(i) | Target::DownloadStop(i) => {
                        NodeId(NET_ITEM + *i as u64)
                    }
                    Target::ConfirmOk
                    | Target::ConfirmCancel
                    | Target::ConfirmAlt
                    | Target::ConfirmAlt2
                    | Target::ProfileCheck(_)
                    | Target::ProfileDone
                    | Target::ProfileStart(_)
                    | Target::ProfileSkip
                    | Target::ModalClose
                    | Target::CheatTab(_)
                    | Target::ReportCopy
                    | Target::InsightAction(_)
                    | Target::InsightDismiss
                    | Target::InsightSwitch
                    | Target::InsightRestore
                    | Target::TagPatient
                    | Target::StudiesPatient
                    | Target::StudiesCancel
                    | Target::StudiesSend
                    | Target::Settings(_)
                    | Target::Editor(_) => DIALOG,
                    // Controls behind the modal are not reachable.
                    _ => continue,
                },
                Target::Toolbar(it) if in_overflow(self, *it) => OVERFLOW,
                Target::ZoomPreset(_) if self.zoom_menu => ZOOM_MENU,
                Target::Toolbar(_)
                | Target::ToolbarMore
                | Target::ProfileChip(_)
                | Target::Position
                | Target::ZoomPreset(_)
                | Target::ToolbarMenu => TOOLBAR,
                Target::Transport(_) | Target::Motion | Target::PlayerButton(_) => TRANSPORT,
                Target::PlayerItem(..) | Target::PlayerDelay(..) => PLAYER_MENU,
                Target::InfoTab(_)
                | Target::InfoClose
                | Target::InfoPerf
                | Target::InfoShowOnMap
                | Target::InfoEditMetadata
                | Target::InfoCopy(..)
                | Target::InfoSummary(..) => INFO,
                Target::ToastUndo(..) | Target::ToastClose(_) => TOASTS,
                Target::Welcome(crate::welcome::WelcomeTarget::Recent(_)) => WELCOME_RECENT,
                Target::Welcome(
                    crate::welcome::WelcomeTarget::Tip(_) | crate::welcome::WelcomeTarget::HideTips,
                ) => WELCOME_TIPS,
                Target::Welcome(_) => WELCOME,
                _ => ROOT,
            };
            // The go-to field replaces the position readout while it is open.
            if *t == Target::Position && has_field && self.modal.is_none() {
                continue;
            }
            // Controls the keyboard reaches can be focused (focus.rs).
            if self.focusable(t) && !n.supports_action(AkAction::Focus) {
                n.add_action(AkAction::Focus);
            }
            // A field of Settings: its text, as a run with the selection.
            if let Target::Settings(SettingsTarget::Field(k)) = t {
                let id = target_id(t);
                let run_id = field_run_id(t);
                let (text, sel) = self.setting_field_a11y(k, st);
                let run = text_run_sel(&mut n, run_id, &text, sel);
                b.add(parent, id, n);
                b.add(id, run_id, run);
                continue;
            }
            b.add(parent, target_id(t), n);
        }

        // A dialog's controls in the order Tab visits them, which is the order a screen
        // reader reads them (they are drawn in another: a title bar's buttons from the end).
        let order: Vec<NodeId> = self
            .a11y_tab_order()
            .iter()
            .map(|t| t.as_ref().map_or(FIELD, target_id))
            .collect();
        if !order.is_empty() {
            b.reorder(DIALOG, &order, true);
            b.reorder(SETTINGS_BODY, &order, false);
        }
        // The bar's controls, and the indicators, as the keyboard visits them.
        if self.modal.is_none() {
            let order: Vec<NodeId> = self.bar_stops.iter().map(target_id).collect();
            b.reorder(TOOLBAR, &order, false);
            let order: Vec<NodeId> = self
                .stops()
                .iter()
                .flat_map(|s| s.targets.iter().map(target_id))
                .collect();
            b.reorder(ROOT, &order, false);
        }

        // The focused part of the toolbar list in Settings, when it is in the tree.
        let tool_focus = self
            .toolbar_focus(st)
            .map(|t| target_id(&t))
            .filter(|id| b.nodes.iter().any(|(i, _)| i == id));
        // The row of the More menu the keyboard is on.
        let menu_focus = self
            .overflow_menu
            .flatten()
            .and_then(|i| self.toolbar_overflow.get(i))
            .map(|it| target_id(&Target::Toolbar(*it)))
            .filter(|id| b.nodes.iter().any(|(i, _)| i == id));
        // The zoom preset the keyboard is on.
        let zoom_focus = self
            .zoom_sel
            .filter(|_| self.zoom_menu)
            .and_then(|i| crate::toolbar::zoom_entries().get(i).map(|e| e.1))
            .map(|a| target_id(&Target::ZoomPreset(a)))
            .filter(|id| b.nodes.iter().any(|(i, _)| i == id));
        // The control with the keyboard focus: in the viewer, or in the open dialog.
        let problem = self.editor_focused_problem();
        let kbd_focus = match self.modal {
            None => self.kbd.as_ref(),
            Some(Modal::ConfigEditor) if problem.is_some() => problem.as_ref(),
            Some(_) => self.modal_focus.as_ref(),
        }
        .map(target_id)
        .filter(|id| b.nodes.iter().any(|(i, _)| i == id));
        let focus = if let Some(id) = tool_focus.filter(|_| self.settings.tools.focus.is_some()) {
            id
        } else if let Some(id) = menu_focus {
            id
        } else if let Some(id) = zoom_focus {
            id
        } else if let Some(id) = kbd_focus.filter(|_| self.modal.is_some()) {
            // A control of the dialog reached with Tab (Settings: off its search field).
            id
        } else if has_field {
            FIELD
        } else if let Some(id) = kbd_focus {
            id
        } else if self.modal.is_some() {
            DIALOG
        } else {
            ROOT
        };
        let mut tree = TreeInfo::new(ROOT);
        tree.toolkit_name = Some("Specular".into());
        tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
        TreeUpdate {
            nodes: b.nodes,
            tree: Some(tree),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    /// Runs an assistive technology's request: a click on a control does what a pointer
    /// click on it does; focusing or the custom actions (move, remove) work the toolbar
    /// editors.
    pub fn accessibility_action(&mut self, req: &ActionRequest, st: &ViewerState) -> Response {
        // A move in Settings is recorded in its history, as from input (history.rs).
        let mark = self.settings_mark();
        let r = self.accessibility_request(req, st);
        self.setting_field_sync(st);
        self.settings_record(mark);
        r
    }

    fn accessibility_request(&mut self, req: &ActionRequest, st: &ViewerState) -> Response {
        if req.action == AkAction::CustomAction {
            let Some(ActionData::CustomAction(id)) = req.data else {
                return Response::default();
            };
            let t = self
                .hits
                .iter()
                .map(|(_, t)| t)
                .find(|t| target_id(t) == req.target_node)
                .cloned();
            if t.is_none() && req.target_node != TOOLBAR {
                return Response::default();
            }
            specular_log::debug!(target: specular_log::UI, "accessibility: action {id} on {t:?}");
            self.toolbar_custom_action(t, id, st);
            return Response::used();
        }
        // The config editor's text: its positions are (line run, character in the line).
        if req.action == AkAction::SetTextSelection
            && self.text_field.is_some()
            && self.config_editor_open()
            && self.editor.region != crate::config_editor::Region::Find
            && (req.target_node == FIELD || editor_line_of(req.target_node).is_some())
        {
            if let Some(ActionData::SetTextSelection(sel)) = &req.data
                && let (Some(a), Some(f)) = (
                    editor_line_of(sel.anchor.node),
                    editor_line_of(sel.focus.node),
                )
                && self.editor_set_a11y_selection(
                    (a, sel.anchor.character_index),
                    (f, sel.focus.character_index),
                )
            {
                let mut r = Response::used();
                self.field_offer_primary(&mut r);
                return r;
            }
            return Response::default();
        }
        // A field of Settings (or its text run): edited, its value set or its text selected.
        if matches!(
            req.action,
            AkAction::SetValue | AkAction::ReplaceSelectedText | AkAction::SetTextSelection
        ) && let Some(k) = self.hits.iter().find_map(|(_, t)| match t {
            Target::Settings(SettingsTarget::Field(k))
                if target_id(t) == req.target_node || field_run_id(t) == req.target_node =>
            {
                Some(*k)
            }
            _ => None,
        }) {
            return self.setting_field_a11y_action(k, req.action, req.data.as_ref(), st);
        }
        // The search field's selection (Settings', the language list's, the palette's).
        if req.action == AkAction::SetTextSelection && matches!(req.target_node, FIELD | FIELD_TEXT)
        {
            if let Some(ActionData::SetTextSelection(sel)) = &req.data
                && self
                    .field_set_a11y_selection(sel.anchor.character_index, sel.focus.character_index)
            {
                let mut r = Response::used();
                self.field_offer_primary(&mut r);
                return r;
            }
            return Response::default();
        }
        // The volume slider: stepped or set.
        if matches!(
            req.action,
            AkAction::Increment | AkAction::Decrement | AkAction::SetValue
        ) && self
            .hits
            .iter()
            .any(|(_, t)| *t == Target::Volume && target_id(t) == req.target_node)
        {
            return self.volume_a11y(req.action, req.data.as_ref(), st);
        }
        if !matches!(req.action, AkAction::Click | AkAction::Focus) {
            return Response::default();
        }
        let Some(t) = self
            .hits
            .iter()
            .map(|(_, t)| t)
            .find(|t| target_id(t) == req.target_node)
            .cloned()
        else {
            return Response::default();
        };
        if req.action == AkAction::Focus {
            // Focusing a part of the toolbar list moves its keyboard focus there; a plain
            // click on a row of the list does just that too.
            if matches!(
                t,
                Target::Settings(
                    SettingsTarget::ToolRow(_)
                        | SettingsTarget::ToolUp(_)
                        | SettingsTarget::ToolDown(_)
                        | SettingsTarget::ToolMenu(_)
                        | SettingsTarget::ToolMenuItem(..)
                )
            ) {
                self.toolbar_focus_on(&t, st);
                return Response::used();
            }
            // Focusing a row of the More menu puts the keyboard on it.
            if let Target::Toolbar(it) = t
                && in_overflow(self, it)
            {
                let i = self.toolbar_overflow.iter().position(|x| *x == it);
                self.overflow_menu = Some(i);
                return Response::used();
            }
            // A zoom preset: the keyboard goes on it.
            if let Target::ZoomPreset(a) = t
                && self.zoom_menu
            {
                let entries = crate::toolbar::zoom_entries();
                self.zoom_sel = entries.iter().position(|e| e.1 == a);
                return Response::used();
            }
            // A control the keyboard reaches: it gets the keyboard focus.
            if self.focusable(&t) {
                if self.modal.is_some() {
                    self.modal_focus = Some(t);
                } else {
                    self.set_kbd(Some(t));
                }
                return Response::used();
            }
            return Response::default();
        }
        specular_log::debug!(target: specular_log::UI, "accessibility: click {t:?}");
        self.activate(t, st)
    }
}
