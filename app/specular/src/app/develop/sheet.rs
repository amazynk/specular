//! Develop's small sheets, shown in the file-operations sheet ([`crate::app::fileops`]
//! draws, focuses and reads them): a name to type (a style, a preset, a snapshot, a
//! version), and the modules to take (selective copy, a style from the current edit), with
//! Append or Replace for a paste.

use specular_develop::model::ModuleKind;
use specular_i18n::t;
use specular_widgets::line_edit::LineEdit;

use crate::app::fileops::{ButtonView, ChoiceView, FieldView, Part, RowView, SheetView};

/// The most rows shown at once (the sheet's `MAX_ROWS`; it scrolls the rest).
const ROWS: usize = 9;

/// What the sheet is for; what its OK button does.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum Purpose {
    /// Copy the chosen modules of the open edit.
    CopySelective,
    /// Paste the copied edit's chosen modules to the photos named.
    PasteSelective,
    /// A new style from the chosen modules, named.
    NewStyle,
    /// Rename the user's style at this index.
    RenameStyle(usize),
    /// Save a module's current values as a preset.
    SavePreset(ModuleKind),
    /// Take a snapshot, named.
    NewSnapshot,
    RenameSnapshot(usize),
    /// A new version: empty, or a copy of the one open.
    NewVersion {
        duplicate: bool,
    },
    RenameVersion(u32),
    /// Save a module instance's mask as a preset.
    SaveMaskPreset(ModuleKind, u16),
    /// The watermark's text.
    WatermarkText,
    /// The Crop card's typed aspect (`W:H`).
    CropRatio,
    /// An auto-apply rule's maker, model, lens or ISO range (empty: any).
    RuleText(usize, super::batch::RuleField),
    /// Merge these raws into one HDR raw: rows are the photos, then the options.
    MergeHdr(Vec<std::path::PathBuf>),
    /// Save the chart profile just measured, named.
    ChartProfile,
    /// Stitch these photos into a panorama: rows are the photos, then the options.
    MergePano(Vec<std::path::PathBuf>),
    /// Merge these raws into one all-in-focus raw: rows are the photos, then the options.
    FocusStack(Vec<std::path::PathBuf>),
    /// Fuse these JPEG, HEIC or TIFF photos into one HDR picture.
    MergeFusion(Vec<std::path::PathBuf>),
}

/// One of Develop's sheets.
#[derive(Clone, Debug)]
pub(in crate::app) struct DevelopSheet {
    pub purpose: Purpose,
    pub title: String,
    /// The name field and its label, for sheets that name something.
    pub field: Option<(String, LineEdit)>,
    /// Modules to choose from, and whether each is chosen.
    pub rows: Vec<(String, bool)>,
    pub kinds: Vec<ModuleKind>,
    /// Append (0) or Replace (1), for pastes.
    pub mode: Option<usize>,
    pub ok: String,
    /// The first row in view.
    pub top: usize,
    /// A note under the title (what happens).
    pub note: Option<String>,
    /// Per row, what its end says when on and when off (empty: included / left out).
    pub ends: Vec<(String, String)>,
    /// The choice row's label and options (None: a paste's Append / Replace).
    pub choice_text: Option<(String, Vec<String>)>,
}

impl DevelopSheet {
    /// A sheet asking for a name, starting with `name` selected.
    pub fn name(purpose: Purpose, title: String, label: String, name: &str, ok: String) -> Self {
        let mut f = LineEdit::new(name);
        f.select(0..name.len());
        DevelopSheet {
            purpose,
            title,
            field: Some((label, f)),
            rows: Vec::new(),
            kinds: Vec::new(),
            mode: None,
            ok,
            top: 0,
            note: None,
            ends: Vec::new(),
            choice_text: None,
        }
    }

    /// A sheet listing `modules` (name, kind, chosen) to choose from.
    pub fn modules(
        purpose: Purpose,
        title: String,
        modules: Vec<(String, ModuleKind, bool)>,
        ok: String,
    ) -> Self {
        DevelopSheet {
            purpose,
            title,
            field: None,
            rows: modules.iter().map(|m| (m.0.clone(), m.2)).collect(),
            kinds: modules.iter().map(|m| m.1).collect(),
            mode: None,
            ok,
            top: 0,
            note: None,
            ends: Vec::new(),
            choice_text: None,
        }
    }

    /// The kinds chosen.
    pub fn chosen(&self) -> Vec<ModuleKind> {
        self.rows
            .iter()
            .zip(&self.kinds)
            .filter(|(r, _)| r.1)
            .map(|(_, k)| *k)
            .collect()
    }

    /// The typed name, trimmed.
    pub fn typed(&self) -> String {
        self.field
            .as_ref()
            .map(|f| f.1.text().trim().to_owned())
            .unwrap_or_default()
    }

    /// Whether OK can be pressed: a name when there is a field, a module when there are
    /// rows.
    pub fn ready(&self) -> bool {
        // A rule's part may be left empty: it then matches any.
        (self.field.is_none()
            || !self.typed().is_empty()
            || matches!(self.purpose, Purpose::RuleText(..)))
            && (self.rows.is_empty() || self.rows.iter().any(|r| r.1))
    }

    /// Row `i` in view pressed: chosen or left out.
    pub fn toggle(&mut self, i: usize) {
        if let Some(r) = self.rows.get_mut(self.top + i) {
            r.1 = !r.1;
        }
    }

    /// Scrolls the rows by `by` (the sheet itself uses [`Self::row_window`]).
    #[cfg(test)]
    pub fn scroll(&mut self, by: isize) {
        let max = self.rows.len().saturating_sub(ROWS);
        self.top = (self.top as isize + by).clamp(0, max as isize) as usize;
    }

    /// The modules' rows: (top, total).
    pub fn row_window(&mut self) -> (&mut usize, usize) {
        let n = self.rows.len();
        (&mut self.top, n)
    }

    pub fn view(&self, focus: Part) -> SheetView {
        let rows: Vec<RowView> = self
            .rows
            .iter()
            .skip(self.top)
            .take(ROWS)
            .enumerate()
            .map(|(i, (name, on))| RowView {
                start: name.clone(),
                end: Some(match (self.ends.get(self.top + i), *on) {
                    (Some(e), true) => e.0.clone(),
                    (Some(e), false) => e.1.clone(),
                    (None, true) => t!("develop-sheet-included"),
                    (None, false) => t!("develop-sheet-left-out"),
                }),
                problem: None,
                quiet: !*on,
                choosable: true,
                chosen: *on,
            })
            .collect();
        let below = self.rows.len().saturating_sub(self.top + rows.len());
        let field = self.field.as_ref().map(|(label, f)| {
            let sel = f.selection();
            FieldView {
                label: label.clone(),
                text: f.text().to_owned(),
                caret: f.caret(),
                selection: (sel.start, sel.end),
                preedit: String::new(),
            }
        });
        SheetView {
            title: self.title.clone(),
            subtitle: None,
            field,
            error: None,
            notes: self.note.iter().cloned().collect(),
            list_label: (!self.rows.is_empty()).then(|| match self.purpose {
                Purpose::MergeHdr(_)
                | Purpose::MergePano(_)
                | Purpose::FocusStack(_)
                | Purpose::MergeFusion(_) => t!("develop-merge-list"),
                _ => t!("develop-sheet-modules"),
            }),
            rows,
            multi: true,
            more: (below > 0).then(|| t!("develop-sheet-more", count = below as i64)),
            choice: self.mode.map(|m| match &self.choice_text {
                Some((label, options)) => ChoiceView {
                    label: label.clone(),
                    options: options.clone(),
                    selected: m,
                },
                None => ChoiceView {
                    label: t!("develop-paste-mode"),
                    options: vec![t!("develop-paste-append"), t!("develop-paste-replace")],
                    selected: m,
                },
            }),
            buttons: vec![
                ButtonView {
                    label: self.ok.clone(),
                    enabled: self.ready(),
                    default: true,
                },
                ButtonView {
                    label: t!("fileop-cancel"),
                    enabled: true,
                    default: false,
                },
            ],
            focus,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_toggle_scroll_and_gate_ok() {
        let mods: Vec<(String, ModuleKind, bool)> = (0..12)
            .map(|i| (format!("m{i}"), ModuleKind(i), i % 2 == 0))
            .collect();
        let mut s = DevelopSheet::modules(Purpose::CopySelective, "t".into(), mods, "ok".into());
        assert_eq!(s.chosen().len(), 6);
        s.scroll(100);
        assert_eq!(s.top, 12 - ROWS);
        s.toggle(0);
        assert!(s.rows[12 - ROWS].1 != (12 - ROWS).is_multiple_of(2));
        for r in &mut s.rows {
            r.1 = false;
        }
        assert!(!s.ready());
        let v = s.view(Part::Row(0));
        assert_eq!(v.rows.len(), ROWS);
        assert!(!v.buttons[0].enabled);
        let mut n = DevelopSheet::name(
            Purpose::NewStyle,
            "t".into(),
            "Name".into(),
            "  ",
            "ok".into(),
        );
        assert!(!n.ready());
        if let Some((_, f)) = n.field.as_mut() {
            f.set_text("Warm");
        }
        assert!(n.ready());
        assert_eq!(n.typed(), "Warm");
    }
}
