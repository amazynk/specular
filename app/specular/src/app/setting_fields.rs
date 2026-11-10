//! Settings' text and path fields, the application's side: the checks of the settings whose
//! readers have rules (the rename and folder patterns, the tile server's address), and a
//! path's Choose… (the platform's file or folder dialog, off the UI thread).

use std::path::PathBuf;

use specular_i18n::t;
use specular_log::{SETTINGS, info, warn};
use specular_settings::{Origin, Value};

use crate::app::App;

/// Registers the checks the Settings window (and `--set`) runs on these settings' text.
pub(super) fn register_checks() {
    specular_settings::set_text_check("convert.pattern", |s| {
        specular_convert::Pattern::parse(s)
            .map(drop)
            .map_err(|e| super::convert::pattern_error(&e))
    });
    specular_settings::set_text_check("import.folder_pattern", |s| {
        specular_import::FolderPattern::parse(s)
            .map(drop)
            .map_err(|e| t!("import-pattern-error", error = e.to_string()))
    });
    specular_settings::set_text_check("import.rename_pattern", |s| {
        specular_import::RenamePattern::parse(s)
            .map(drop)
            .map_err(|e| t!("import-pattern-error", error = e.to_string()))
    });
    specular_settings::set_text_check("cull.tether_rename", |s| {
        specular_import::RenamePattern::parse(s)
            .map(drop)
            .map_err(|e| t!("import-pattern-error", error = e.to_string()))
    });
    // Every SHX font folder must be a folder.
    specular_settings::set_text_check("drawings.shx_folders", |s| {
        use specular_decode::drawing::specular_vector::shx;
        match shx::parse_folders(s).into_iter().find(|d| !d.is_dir()) {
            None => Ok(()),
            Some(d) => Err(t!(
                "settings-error-shx-folder",
                folder = d.display().to_string()
            )),
        }
    });
    // Empty: no address of its own (the base map says so where it is used).
    specular_settings::set_text_check("metadata.basemap_url", |s| {
        if s.trim().is_empty() || specular_map::basemap::valid_template(s) {
            Ok(())
        } else {
            Err(t!("settings-error-tile-url"))
        }
    });
}

/// A Choose… waiting for its dialog: the setting and what was picked.
#[derive(Default)]
pub(super) struct PathPick {
    rx: Option<crossbeam_channel::Receiver<(&'static str, PathBuf)>>,
}

impl App {
    /// Choose… of path setting `key`: the file dialog (a folder picker for a folder), at the
    /// current value when there is one.
    pub(super) fn choose_setting_path(&mut self, key: &'static str, directory: bool) {
        if self.setting_pick.rx.is_some() {
            return;
        }
        let Some(m) = specular_settings::find(key) else {
            return;
        };
        let title = m.label(specular_i18n::I18n::global());
        let start = match self.store.get(key) {
            Ok(Value::Path(Some(p))) if directory => Some(p),
            Ok(Value::Path(Some(p))) => p.parent().map(PathBuf::from),
            _ => None,
        }
        .filter(|p| p.is_dir());
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-set-path", move |l| {
            let mut dlg = rfd::FileDialog::new().set_title(title);
            if let Some(d) = start {
                dlg = dlg.set_directory(d);
            }
            let picked = if directory {
                dlg.pick_folder()
            } else {
                dlg.pick_file()
            };
            if let Some(p) = picked {
                let _ = tx.send((key, p));
                l.waker()();
            }
        });
        self.setting_pick.rx = Some(rx);
    }

    /// Once per event-loop turn: a picked path is stored (the field shows it).
    pub(super) fn setting_path_tick(&mut self) {
        let Some(rx) = &self.setting_pick.rx else {
            return;
        };
        match rx.try_recv() {
            Ok((key, p)) => {
                self.setting_pick.rx = None;
                info!(target: SETTINGS, "settings: {key} chosen");
                if let Err(e) = self
                    .store
                    .set(key, Value::Path(Some(p)), Origin::SettingsWindow)
                {
                    warn!(target: SETTINGS, "settings: {key}: {e}");
                }
                self.redraw = true;
            }
            // Cancelled (the dialog's thread is gone).
            Err(crossbeam_channel::TryRecvError::Disconnected) => self.setting_pick.rx = None,
            Err(crossbeam_channel::TryRecvError::Empty) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use specular_settings::{Origin, SettingError, SettingsStore};

    /// The checks are the readers' own parsers: what Convert, Import and the base map refuse,
    /// the Settings window refuses with their reason; what they take, it keeps.
    #[test]
    fn checks_follow_the_readers() {
        super::register_checks();
        let s = SettingsStore::in_memory();
        let o = Origin::SettingsWindow;
        let refused = |k: &str, v: &str| match s.set_text(k, v, o) {
            Err(SettingError::Invalid(why)) => assert!(!why.is_empty(), "{k}={v}"),
            r => panic!("{k}={v}: {r:?}"),
        };
        refused("convert.pattern", "{nme}");
        refused("convert.pattern", "a}b");
        refused("import.folder_pattern", "{year}/{bogus}");
        refused("import.rename_pattern", "{w}");
        refused("metadata.basemap_url", "https://tiles.example/{z}/{x}.png");
        refused("metadata.basemap_url", "file:///tiles/{z}/{x}/{y}.png");
        refused("drawings.shx_folders", "/no/such/specular/folder");
        let here = env!("CARGO_MANIFEST_DIR");
        refused(
            "drawings.shx_folders",
            &format!("{here}; {here}/Cargo.toml"),
        );
        assert_eq!(
            s.set_text("drawings.shx_folders", &format!("{here} ; {here}/src"), o)
                .map(|_| ()),
            Ok(())
        );
        assert_eq!(
            s.set_text("drawings.shx_folders", "", o).map(|_| ()),
            Ok(())
        );
        for (k, v) in [
            ("convert.pattern", "{name}-{n:03}"),
            ("import.folder_pattern", "{year}/{year}-{month}"),
            ("import.rename_pattern", ""),
            ("import.rename_pattern", "{date:%Y%m%d}-{n:03}"),
            ("metadata.basemap_url", ""),
            (
                "metadata.basemap_url",
                "https://tiles.example/{z}/{x}/{-y}.png",
            ),
        ] {
            assert_eq!(s.set_text(k, v, o).map(|_| ()), Ok(()), "{k}={v}");
        }
    }
}
