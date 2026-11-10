//! Settings → Integration → Default image viewer → "More file types": tick boxes for the types
//! the default viewer leaves out by itself (editor documents, science, medical, CAD and GIS,
//! 3D and molecules, SVG and other images), by group and one by one, all off at first. The
//! choice is the hidden setting `integration.extra_default_types`; while the default viewer
//! is on, a change applies it again in the background (unticked types are given back to the
//! application they had before, if they are still ours).

use specular_i18n::{I18n, t};
use specular_log::{SETTINGS, info};
use specular_platform::integration::mime::{self, ExtraGroup};
use specular_platform::integration::{self as int, OptionId};
use specular_settings::Origin;
use specular_widgets::{CheckState, Modal, PanelEvent, PanelRow, SettingPanel};

use super::Msg;
use crate::app::App;

/// The setting the panel sits under.
pub(super) const UNDER: &str = "integration.default_viewer";

/// What the panel was last built from: the setting, what is open, the UI generation.
pub(super) type Built = (String, bool, Vec<ExtraGroup>, u64);

/// The rows for `picked` with the "More file types" row open or not and `open` groups
/// expanded (collapsed children are left out).
pub(super) fn panel(picked: &[&str], more_open: bool, open: &[ExtraGroup]) -> SettingPanel {
    let chosen = picked.len();
    let mut rows = vec![PanelRow {
        id: "more".into(),
        label: t!("integration-extra-title"),
        value: t!("integration-extra-chosen", count = chosen as i64),
        lines: if more_open {
            vec![t!("integration-extra-intro")]
        } else {
            Vec::new()
        },
        expand: Some(more_open),
        ..Default::default()
    }];
    if more_open {
        for g in ExtraGroup::ALL {
            group_rows(&mut rows, g, picked, open.contains(&g));
        }
    }
    SettingPanel {
        rows,
        keywords: t!("integration-extra-keywords"),
        ..Default::default()
    }
}

fn group_rows(rows: &mut Vec<PanelRow>, g: ExtraGroup, picked: &[&str], open: bool) {
    let types: Vec<_> = g.types().collect();
    let on = types.iter().filter(|(m, _)| picked.contains(m)).count();
    let label = I18n::global().tr(&format!("integration-extra-group-{}", g.key()));
    let help = I18n::global().tr(&format!("integration-extra-group-{}-help", g.key()));
    if types.is_empty() {
        // Videos: shown so the list keeps its shape, nothing to tick yet.
        rows.push(PanelRow {
            id: format!("g:{}", g.key()),
            label,
            value: t!("integration-extra-none-yet"),
            lines: vec![help],
            indent: 1,
            ..Default::default()
        });
        return;
    }
    let check = match on {
        0 => CheckState::Off,
        n if n == types.len() => CheckState::On,
        _ => CheckState::Mixed,
    };
    rows.push(PanelRow {
        id: format!("g:{}", g.key()),
        label,
        value: t!(
            "integration-extra-count",
            on = on as i64,
            total = types.len() as i64
        ),
        lines: vec![help],
        check: Some(check),
        expand: Some(open),
        indent: 1,
        ..Default::default()
    });
    if !open {
        return;
    }
    for (m, exts) in types {
        let exts: Vec<String> = exts.iter().map(|e| format!(".{e}")).collect();
        rows.push(PanelRow {
            id: format!("t:{m}"),
            label: exts.join(", "),
            detail: m.to_owned(),
            check: Some(if picked.contains(&m) {
                CheckState::On
            } else {
                CheckState::Off
            }),
            indent: 3,
            ..Default::default()
        });
    }
}

/// `picked` after a click on the tick box of row `id`: a type flips; a group turns all of
/// its types on, or off when they all were on.
pub(super) fn toggle(picked: &[&'static str], id: &str) -> Vec<&'static str> {
    let mut v = picked.to_vec();
    if let Some(m) = id.strip_prefix("t:") {
        if let Some(i) = v.iter().position(|p| *p == m) {
            v.remove(i);
        } else if let Some((m, _)) = mime::IMAGE_TYPES.iter().find(|(t, _)| *t == m) {
            v.push(m);
        }
    } else if let Some(g) = id
        .strip_prefix("g:")
        .and_then(|k| ExtraGroup::ALL.into_iter().find(|g| g.key() == k))
    {
        let types: Vec<&'static str> = g.types().map(|(m, _)| m).collect();
        if types.iter().all(|m| v.contains(m)) {
            v.retain(|m| !types.contains(m));
        } else {
            v.extend(types.into_iter().filter(|m| !picked.contains(m)));
        }
    }
    // Table order, no duplicates (what the setting stores).
    mime::parse_extra_types(&mime::format_extra_types(&v))
}

impl App {
    /// Once per event-loop turn while Settings is open: rebuilds the panel when the choice,
    /// what is open or the language changed, and keeps the environment's extra types.
    pub(super) fn drain_extra_types(&mut self) {
        if int::options().is_empty() {
            return;
        }
        // A change made elsewhere (settings.toml, an import) is taken over once seen; a tick
        // here is ahead of the stored settings until they come back.
        let stored = &self.settings.integration.extra_default_types;
        let d = &mut self.integration.desktop;
        if d.extras_seen != *stored {
            d.extras_seen.clone_from(stored);
            if d.extras_setting != *stored {
                d.extras_setting.clone_from(stored);
                d.env = d.env.clone().with_extra_default_types(stored);
                // The status count includes the extra types.
                d.refreshed = None;
            }
        }
        let setting = d.extras_setting.clone();
        if self.chrome.modal() != Some(Modal::Settings) {
            return;
        }
        let built: Built = (
            setting,
            d.extras_open,
            d.groups_open.clone(),
            I18n::global().generation(),
        );
        if d.extras_built.as_ref() == Some(&built) {
            return;
        }
        let picked = mime::parse_extra_types(&d.extras_setting);
        let p = panel(&picked, built.1, &built.2);
        d.extras_built = Some(built);
        if self.chrome.set_setting_panel(UNDER, Some(p)) {
            self.redraw = true;
        }
    }

    /// The panel's tick boxes and arrows.
    pub(in crate::app) fn extra_types_event(&mut self, e: &PanelEvent) {
        let d = &mut self.integration.desktop;
        match e {
            PanelEvent::Expand(id) if id == "more" => d.extras_open = !d.extras_open,
            PanelEvent::Expand(id) => {
                let Some(g) = id
                    .strip_prefix("g:")
                    .and_then(|k| ExtraGroup::ALL.into_iter().find(|g| g.key() == k))
                else {
                    return;
                };
                if let Some(i) = d.groups_open.iter().position(|o| *o == g) {
                    d.groups_open.remove(i);
                } else {
                    d.groups_open.push(g);
                }
            }
            PanelEvent::Check(id) => {
                let picked = mime::parse_extra_types(&d.extras_setting);
                let next = toggle(&picked, id);
                if next == picked {
                    return;
                }
                let value = mime::format_extra_types(&next);
                info!(target: SETTINGS, "integration: extra default types {value:?}");
                self.store.update(Origin::SettingsWindow, |s| {
                    s.integration.extra_default_types.clone_from(&value)
                });
                self.reapply_default_viewer(&value);
            }
            PanelEvent::Row(_) | PanelEvent::Action => return,
        }
        self.drain_extra_types();
        self.redraw = true;
    }

    /// Applies the default viewer again with the new extra types, when it is on (when it is
    /// off, the choice waits for it to be turned on).
    fn reapply_default_viewer(&mut self, setting: &str) {
        let d = &mut self.integration.desktop;
        d.extras_setting = setting.to_owned();
        d.env = d.env.clone().with_extra_default_types(setting);
        let on = d
            .statuses
            .iter()
            .any(|s| s.id == OptionId::DefaultViewer && s.is_applied());
        if !on {
            d.refreshed = None;
            return;
        }
        if d.busy.is_some() {
            // Applied after the running change (the next refresh shows "partly set up"
            // until then); rare enough not to queue.
            d.refreshed = None;
            return;
        }
        let id = OptionId::DefaultViewer;
        d.busy = Some(id);
        d.dirty = true;
        let env = d.env.clone();
        let tx = d.tx.clone();
        let labels = crate::integration_cli::labels();
        let wake = self.loader.waker();
        self.loader.background("int-apply", move |_| {
            let r = int::apply(&env, id, &labels);
            let _ = tx.send(Msg::Done {
                id,
                on: true,
                result: r.map_err(|e| crate::error_text::reason(&e)),
            });
            wake();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticking_groups_and_types() {
        let v = toggle(&[], "g:medical");
        assert!(v.contains(&"application/dicom") && v.contains(&"application/x-nifti"));
        assert!(
            v.iter()
                .all(|m| mime::extra_group(m) == Some(ExtraGroup::Medical))
        );
        // One type off: the group is mixed; ticking the group fills it again.
        let w = toggle(&v, "t:application/dicom");
        assert_eq!(w.len(), v.len() - 1);
        assert_eq!(toggle(&w, "g:medical"), v);
        // All on: the group's box turns them off.
        assert!(toggle(&v, "g:medical").is_empty());
        // Unknown and default types are ignored.
        assert!(toggle(&[], "t:image/jpeg").is_empty());
        assert!(toggle(&[], "g:nonsense").is_empty());
        assert_eq!(toggle(&[], "g:videos").len(), 33);
    }

    #[test]
    fn panel_rows_follow_the_choice() {
        let closed = panel(&[], false, &[]);
        assert_eq!(closed.rows.len(), 1);
        assert_eq!(closed.rows[0].expand, Some(false));
        let picked = ["application/x-krita"];
        let p = panel(&picked, true, &[ExtraGroup::Editors]);
        let row = |id: &str| p.rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!(row("g:editors").check, Some(CheckState::Mixed));
        assert_eq!(row("g:editors").expand, Some(true));
        assert_eq!(row("t:application/x-krita").check, Some(CheckState::On));
        assert_eq!(row("t:image/x-xcf").check, Some(CheckState::Off));
        assert_eq!(row("g:medical").check, Some(CheckState::Off));
        assert_eq!(row("g:videos").check, Some(CheckState::Off));
        // Collapsed groups list no types.
        assert!(!p.rows.iter().any(|r| r.id == "t:application/dicom"));
        // Every group is listed, in order; camera RAW is not one (all claimed already).
        let groups: Vec<_> = p.rows.iter().filter(|r| r.id.starts_with("g:")).collect();
        assert_eq!(groups.len(), ExtraGroup::ALL.len());
    }
}
