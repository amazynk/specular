//! [`SettingsStore`]: the live, shared settings with change notification, persistence,
//! file watching, undo and import/export.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::Duration;

use notify::{RecommendedWatcher, Watcher as _};
use toml_edit::{DocumentMut, Item};

use crate::commands::{CommandProblem, UserCommand};
use crate::error::{SettingError, SettingsError, Warning};
use crate::file::{load_str, new_document, to_full_document, update_document, write_atomic};
use crate::meta::{self, SettingMeta, suggest};
use crate::paths::{self, Locations};
use crate::schema::{Section, Settings};
use crate::undo::{Edit, UndoStack};
use crate::value::Value;

/// Where a change came from. Decides whether it is saved and whether it can be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// The Settings window: saved, and undoable with Ctrl+Z in the window.
    SettingsWindow,
    /// The command palette ("Set: … → …"): saved.
    Palette,
    /// `specular --set key=value`: applies to this session only, not saved, and kept when
    /// the file is reloaded.
    CommandLine,
    /// `settings.toml` was edited outside the app and reloaded.
    File,
    /// "Import settings…": saved, and undoable as one step.
    Import,
    /// Undo or redo in the Settings window: saved.
    Undo,
    /// The program itself (e.g. remembering a choice): saved.
    Program,
    /// The built-in `settings.toml` editor saved the file ([`SettingsStore::write_text`]):
    /// already written, exactly as typed.
    Editor,
}

impl Origin {
    fn persists(self) -> bool {
        !matches!(self, Origin::CommandLine | Origin::File | Origin::Editor)
    }

    fn undoable(self) -> bool {
        matches!(self, Origin::SettingsWindow | Origin::Import)
    }
}

/// A batch of changes, published with a new generation number.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub generation: u64,
    pub keys: Vec<&'static str>,
    pub origin: Origin,
}

const CHANGE_LOG: usize = 64;

struct FileState {
    path: Option<PathBuf>,
    /// The parsed file. `None` when there is no file yet, or it has a syntax error.
    doc: Option<DocumentMut>,
    syntax_error: bool,
    /// Keys changed in this session and not yet written.
    dirty: BTreeSet<&'static str>,
    /// Migrated in memory; write back on the next save.
    migrated: bool,
    /// The text last read or written, to ignore our own writes in the watcher.
    last_text: Option<String>,
    autosave: bool,
}

struct Inner {
    current: RwLock<Arc<Settings>>,
    generation: AtomicU64,
    changes: Mutex<VecDeque<Change>>,
    file: Mutex<FileState>,
    warnings: Mutex<Vec<Warning>>,
    undo: Mutex<UndoStack>,
    /// The file watcher; dropping it ends the debounce thread.
    watcher: Mutex<Option<RecommendedWatcher>>,
    /// Executable folder for the portable-mode marker; `None` disables the side effect.
    portable_dir: Option<PathBuf>,
    /// Bumped whenever the file is read again (for parts outside the schema, like
    /// `[[commands]]`).
    file_generation: AtomicU64,
    /// Bumped whenever the text of the file as last read or written changes.
    text_generation: AtomicU64,
    /// Called from the watcher thread after a reload changed something (to wake the UI).
    on_reload: Mutex<Option<ReloadHook>>,
    /// `--set` values: they win over the file on every reload until the app exits, or
    /// until the setting is changed in the app.
    session: Mutex<BTreeMap<&'static str, Value>>,
}

/// See [`SettingsStore::set_on_reload`].
pub type ReloadHook = Arc<dyn Fn() + Send + Sync>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The application's settings. `Send + Sync`; cloning shares the same store.
///
/// Read with [`snapshot`](Self::snapshot) (an `Arc<Settings>` that never changes). Poll
/// [`generation`](Self::generation) each frame and call
/// [`changes_since`](Self::changes_since) to learn which keys changed.
#[derive(Clone)]
pub struct SettingsStore {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for SettingsStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsStore")
            .field("path", &self.path())
            .field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}

impl SettingsStore {
    fn build(
        settings: Settings,
        file: FileState,
        warnings: Vec<Warning>,
        portable_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                current: RwLock::new(Arc::new(settings)),
                generation: AtomicU64::new(0),
                changes: Mutex::new(VecDeque::new()),
                file: Mutex::new(file),
                warnings: Mutex::new(warnings),
                undo: Mutex::new(UndoStack::default()),
                watcher: Mutex::new(None),
                portable_dir,
                file_generation: AtomicU64::new(0),
                text_generation: AtomicU64::new(0),
                on_reload: Mutex::new(None),
                session: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    /// Defaults only, never saved (tests, `--no-config`).
    pub fn in_memory() -> Self {
        Self::build(
            Settings::default(),
            FileState {
                path: None,
                doc: None,
                syntax_error: false,
                dirty: BTreeSet::new(),
                migrated: false,
                last_text: None,
                autosave: false,
            },
            Vec::new(),
            None,
        )
    }

    /// Loads `path` (a missing file means defaults). Problems in the file become
    /// [`warnings`](Self::warnings); changes are saved back to `path` automatically.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SettingsError> {
        let path = path.into();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => return Err(SettingsError::Read { path, source }),
        };
        let loaded = text.as_deref().map(load_str);
        let (settings, doc, warnings, migrated, syntax_error) = match loaded {
            Some(l) => {
                let syntax_error = l.doc.is_none();
                (l.settings, l.doc, l.warnings, l.migrated, syntax_error)
            }
            None => (Settings::default(), None, Vec::new(), false, false),
        };
        for w in &warnings {
            log::warn!("{}: {w}", path.display());
        }
        let store = Self::build(
            settings,
            FileState {
                path: Some(path),
                doc,
                syntax_error,
                dirty: BTreeSet::new(),
                migrated,
                last_text: text,
                autosave: true,
            },
            warnings,
            None,
        );
        if migrated && let Err(e) = store.save() {
            log::warn!("migrated settings not written: {e}");
        }
        Ok(store)
    }

    /// Loads the settings for this process (portable folder or platform config folder),
    /// with portable mode reflected in `advanced.portable_mode`.
    pub fn open_default() -> Result<Self, SettingsError> {
        let loc = Locations::detect();
        let mut store = Self::open(loc.settings_path())?;
        if let Some(inner) = Arc::get_mut(&mut store.inner) {
            inner.portable_dir = paths::exe_dir();
            let mut s = (**inner.current.get_mut().unwrap_or_else(|e| e.into_inner())).clone();
            s.advanced.portable_mode = loc.portable;
            *inner.current.get_mut().unwrap_or_else(|e| e.into_inner()) = Arc::new(s);
        }
        Ok(store)
    }

    /// The `[[commands]]` entries of the settings file, with problems in bad entries.
    pub fn user_commands(&self) -> (Vec<UserCommand>, Vec<CommandProblem>) {
        match &lock(&self.inner.file).doc {
            Some(doc) => crate::commands::parse_commands(doc),
            None => (Vec::new(), Vec::new()),
        }
    }

    /// The `[[hooks]]` entries of the settings file, with problems in bad entries.
    pub fn hooks(
        &self,
    ) -> (
        Vec<crate::commands::Hook>,
        Vec<crate::commands::HookProblem>,
    ) {
        match &lock(&self.inner.file).doc {
            Some(doc) => crate::commands::parse_hooks(doc),
            None => (Vec::new(), Vec::new()),
        }
    }

    /// Changes whenever the settings file has been read again (live reload): re-read
    /// [`user_commands`](Self::user_commands) when it does.
    pub fn file_generation(&self) -> u64 {
        self.inner.file_generation.load(Ordering::Acquire)
    }

    /// The settings file, if file-backed.
    pub fn path(&self) -> Option<PathBuf> {
        lock(&self.inner.file).path.clone()
    }

    /// Turns automatic saving after each change on or off (on for file-backed stores).
    pub fn set_autosave(&self, on: bool) {
        lock(&self.inner.file).autosave = on;
    }

    // ----- Reading ------------------------------------------------------

    /// The current settings. The snapshot never changes; take a new one after a change.
    pub fn snapshot(&self) -> Arc<Settings> {
        self.inner
            .current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The value of `key`.
    pub fn get(&self, key: &str) -> Result<Value, SettingError> {
        self.snapshot().get(key).ok_or_else(|| unknown(key))
    }

    /// True when `key` differs from its default (the "Modified" marker).
    pub fn is_modified(&self, key: &str) -> bool {
        match (meta::find(key), self.snapshot().get(key)) {
            (Some(m), Some(v)) => m.stored && v != m.default,
            _ => false,
        }
    }

    /// Problems found the last time the file was read.
    pub fn warnings(&self) -> Vec<Warning> {
        lock(&self.inner.warnings).clone()
    }

    // ----- Change notification ---------------------------------------------

    /// Increases with every applied batch of changes.
    pub fn generation(&self) -> u64 {
        self.inner.generation.load(Ordering::Acquire)
    }

    /// Keys changed after generation `since`. `None` if that is too long ago to know
    /// (treat everything as changed).
    pub fn changes_since(&self, since: u64) -> Option<Vec<&'static str>> {
        let log = lock(&self.inner.changes);
        if since >= self.generation() {
            return Some(Vec::new());
        }
        let oldest = log.front().map_or(u64::MAX, |c| c.generation);
        if oldest > since + 1 {
            return None;
        }
        let mut keys: Vec<&'static str> = Vec::new();
        for c in log.iter().filter(|c| c.generation > since) {
            for k in &c.keys {
                if !keys.contains(k) {
                    keys.push(k);
                }
            }
        }
        Some(keys)
    }

    /// The most recent change.
    pub fn last_change(&self) -> Option<Change> {
        lock(&self.inner.changes).back().cloned()
    }

    // ----- Writing -----------------------------------------------------

    /// Validates and sets one setting. Returns whether the value changed.
    pub fn set(&self, key: &str, value: Value, origin: Origin) -> Result<bool, SettingError> {
        let m = meta::find(key).ok_or_else(|| unknown(key))?;
        let v = m.validate(value)?;
        m.check(&v)?;
        Ok(!self.apply(vec![(m, v)], origin).is_empty())
    }

    /// Parses `text` for `key` (as `--set` and the palette do) and sets it.
    pub fn set_text(&self, key: &str, text: &str, origin: Origin) -> Result<bool, SettingError> {
        let m = meta::find(key).ok_or_else(|| unknown(key))?;
        let v = m.parse(text)?;
        Ok(!self.apply(vec![(m, v)], origin).is_empty())
    }

    /// Applies `--set key=value` arguments (session only). They stay in effect when the
    /// file is reloaded, until the app exits or the setting is changed in the app. Returns
    /// the errors, by argument.
    pub fn apply_set_args<S: AsRef<str>>(&self, args: &[S]) -> Vec<(String, SettingError)> {
        let mut edits = Vec::new();
        let mut errors = Vec::new();
        for a in args {
            match parse_set_arg(a.as_ref()) {
                Ok(e) => edits.push(e),
                Err(e) => errors.push((a.as_ref().to_owned(), e)),
            }
        }
        self.apply(edits, Origin::CommandLine);
        errors
    }

    /// Changes settings through the typed struct; the changed keys are found by comparison.
    pub fn update(&self, origin: Origin, f: impl FnOnce(&mut Settings)) -> Vec<&'static str> {
        let before = self.snapshot();
        let mut after = (*before).clone();
        f(&mut after);
        let edits = meta::all()
            .iter()
            .filter(|m| !m.is_action())
            .filter_map(|m| {
                let v = after.get(m.key)?;
                (Some(&v) != before.get(m.key).as_ref()).then_some((m, v))
            })
            .filter_map(|(m, v)| match m.validate(v) {
                Ok(v) => Some((m, v)),
                Err(e) => {
                    log::error!("update(): invalid value for {}: {e}", m.key);
                    None
                }
            })
            .collect();
        self.apply(edits, origin)
    }

    /// Resets one setting to its default.
    pub fn reset(&self, key: &str, origin: Origin) -> Result<bool, SettingError> {
        let m = meta::find(key).ok_or_else(|| unknown(key))?;
        if m.is_action() {
            return Err(SettingError::NotSettable);
        }
        Ok(!self.apply(vec![(m, m.default.clone())], origin).is_empty())
    }

    /// Resets every stored setting of `section` (one undo step).
    pub fn reset_section(&self, section: Section, origin: Origin) -> Vec<&'static str> {
        let edits = meta::in_section(section)
            .filter(|m| m.stored)
            .map(|m| (m, m.default.clone()))
            .collect();
        self.apply(edits, origin)
    }

    /// Resets every stored setting (one undo step). Portable mode is left alone.
    pub fn reset_all(&self, origin: Origin) -> Vec<&'static str> {
        let edits = meta::all()
            .iter()
            .filter(|m| m.stored)
            .map(|m| (m, m.default.clone()))
            .collect();
        self.apply(edits, origin)
    }

    /// Applies validated values as one batch: one generation, one undo step.
    pub(crate) fn apply(
        &self,
        edits: Vec<(&'static SettingMeta, Value)>,
        origin: Origin,
    ) -> Vec<&'static str> {
        if edits.is_empty() {
            return Vec::new();
        }
        if origin == Origin::CommandLine {
            // `--set` and `--lang`: kept over reloads of the file (see `keep_session`).
            lock(&self.inner.session).extend(edits.iter().map(|(m, v)| (m.key, v.clone())));
        }
        let mut done: Vec<Edit> = Vec::new();
        {
            let mut cur = self
                .inner
                .current
                .write()
                .unwrap_or_else(|e| e.into_inner());
            let mut next = (**cur).clone();
            for (m, v) in &edits {
                let before = next.get(m.key).unwrap_or(Value::None);
                if next.put(m.key, v) {
                    done.push(Edit {
                        key: m.key,
                        before,
                        after: v.clone(),
                    });
                }
            }
            if done.is_empty() {
                return Vec::new();
            }
            *cur = Arc::new(next);
            // Publish while holding the write lock so generation order matches value order.
            self.publish(done.iter().map(|e| e.key).collect(), origin);
        }
        let keys: Vec<&'static str> = done.iter().map(|e| e.key).collect();
        if origin.persists() {
            // A change made in the app replaces the `--set` value for the rest of the session.
            let mut session = lock(&self.inner.session);
            if !session.is_empty() {
                keys.iter().for_each(|k| {
                    session.remove(k);
                });
            }
        }

        if origin.undoable() {
            let mut u = lock(&self.inner.undo);
            if done.len() > 1 {
                u.begin_group();
                done.iter().cloned().for_each(|e| u.record(e));
                u.end_group();
            } else {
                u.record(done[0].clone());
            }
        }
        self.side_effects(&done, origin);
        if origin.persists() {
            let autosave = {
                let mut f = lock(&self.inner.file);
                for e in &done {
                    if meta::find(e.key).is_some_and(|m| m.stored) {
                        f.dirty.insert(e.key);
                    }
                }
                f.autosave && f.path.is_some() && !f.dirty.is_empty()
            };
            if autosave && let Err(e) = self.save() {
                log::warn!("settings not saved: {e}");
            }
        }
        keys
    }

    fn publish(&self, keys: Vec<&'static str>, origin: Origin) {
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let mut log = lock(&self.inner.changes);
        log.push_back(Change {
            generation,
            keys,
            origin,
        });
        while log.len() > CHANGE_LOG {
            log.pop_front();
        }
    }

    fn side_effects(&self, done: &[Edit], origin: Origin) {
        if !origin.persists() {
            return;
        }
        for e in done {
            if e.key == "advanced.portable_mode"
                && let (Some(dir), Value::Bool(on)) = (&self.inner.portable_dir, &e.after)
                && let Err(err) = paths::set_portable_in(dir, *on)
            {
                log::error!("portable mode: {err}");
            }
        }
    }

    // ----- Undo ------------------------------------------------------------

    /// Groups the following Settings-window changes into one undo step until
    /// [`end_undo_group`](Self::end_undo_group).
    pub fn begin_undo_group(&self) {
        lock(&self.inner.undo).begin_group();
    }

    pub fn end_undo_group(&self) {
        lock(&self.inner.undo).end_group();
    }

    /// Forgets the undo history (e.g. when the Settings window closes).
    pub fn clear_undo(&self) {
        lock(&self.inner.undo).clear();
    }

    pub fn can_undo(&self) -> bool {
        lock(&self.inner.undo).can_undo()
    }

    pub fn can_redo(&self) -> bool {
        lock(&self.inner.undo).can_redo()
    }

    /// The keys the next [`undo`](Self::undo) would restore (empty: nothing to undo), so an
    /// editor of one setting can undo only its own changes.
    pub fn next_undo_keys(&self) -> Vec<&'static str> {
        lock(&self.inner.undo).next_keys(false)
    }

    /// The keys the next [`redo`](Self::redo) would change (empty: nothing to redo).
    pub fn next_redo_keys(&self) -> Vec<&'static str> {
        lock(&self.inner.undo).next_keys(true)
    }

    /// Undoes the last Settings-window change. Returns the keys restored.
    pub fn undo(&self) -> Vec<&'static str> {
        let Some(step) = lock(&self.inner.undo).pop_undo() else {
            return Vec::new();
        };
        self.restore(step.edits.iter().rev().map(|e| (e.key, e.before.clone())))
    }

    /// Redoes the last undone change.
    pub fn redo(&self) -> Vec<&'static str> {
        let Some(step) = lock(&self.inner.undo).pop_redo() else {
            return Vec::new();
        };
        self.restore(step.edits.iter().map(|e| (e.key, e.after.clone())))
    }

    fn restore(&self, values: impl Iterator<Item = (&'static str, Value)>) -> Vec<&'static str> {
        let edits = values
            .filter_map(|(k, v)| meta::find(k).map(|m| (m, v)))
            .collect();
        self.apply(edits, Origin::Undo)
    }

    // ----- Files -------------------------------------------------------

    /// Writes changed settings to the file, keeping its comments and order. Does nothing
    /// for in-memory stores or when nothing changed. Refuses to overwrite a file with a
    /// syntax error.
    pub fn save(&self) -> Result<(), SettingsError> {
        let snapshot = self.snapshot();
        let mut f = lock(&self.inner.file);
        let Some(path) = f.path.clone() else {
            return Ok(());
        };
        if f.dirty.is_empty() && !f.migrated {
            return Ok(());
        }
        if f.syntax_error {
            return Err(SettingsError::FileHasErrors { path });
        }
        let keys: Vec<&'static str> = f.dirty.iter().copied().collect();
        let doc = f.doc.get_or_insert_with(new_document);
        update_document(doc, &snapshot, &keys);
        let text = doc.to_string();
        write_atomic(&path, &text).map_err(|source| SettingsError::Write {
            path: path.clone(),
            source,
        })?;
        f.last_text = Some(text);
        f.dirty.clear();
        f.migrated = false;
        self.inner.text_generation.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    /// Changes whenever the text of the settings file changes (a save, a reload, a
    /// [`write_text`](Self::write_text)). An editor of the file polls it to notice edits
    /// made elsewhere.
    pub fn text_generation(&self) -> u64 {
        self.inner.text_generation.load(Ordering::Acquire)
    }

    /// The text of the settings file as last read or written. For a store without a file
    /// yet (or in memory), the document a save would write: the header and every setting
    /// that differs from its default.
    pub fn file_text(&self) -> String {
        let snapshot = self.snapshot();
        let f = lock(&self.inner.file);
        if let Some(t) = &f.last_text
            && f.dirty.is_empty()
        {
            return t.clone();
        }
        let mut doc = f.doc.clone().unwrap_or_else(new_document);
        let keys: Vec<&'static str> = if f.path.is_some() {
            f.dirty.iter().copied().collect()
        } else {
            meta::all()
                .iter()
                .filter(|m| m.stored)
                .map(|m| m.key)
                .collect()
        };
        update_document(&mut doc, &snapshot, &keys);
        doc.to_string()
    }

    /// Replaces the settings file with `text`, exactly as given (written atomically), and
    /// applies it: settings it leaves out return to their defaults. Refused, with nothing
    /// written, when the text is not valid TOML. The file watcher does not see this as an
    /// edit made elsewhere. Returns the keys that changed.
    pub fn write_text(&self, text: &str) -> Result<Vec<&'static str>, SettingsError> {
        let loaded = load_str(text);
        let path = self.path();
        let Some(doc) = loaded.doc else {
            return Err(SettingsError::FileHasErrors {
                path: path.unwrap_or_default(),
            });
        };
        {
            let mut f = lock(&self.inner.file);
            if let Some(path) = &f.path {
                write_atomic(path, text).map_err(|source| SettingsError::Write {
                    path: path.clone(),
                    source,
                })?;
            }
            f.last_text = Some(text.to_owned());
            f.doc = Some(doc);
            f.syntax_error = false;
            f.dirty.clear();
            f.migrated = loaded.migrated;
            self.inner.file_generation.fetch_add(1, Ordering::AcqRel);
            self.inner.text_generation.fetch_add(1, Ordering::AcqRel);
        }
        *lock(&self.inner.warnings) = loaded.warnings.clone();
        let edits: Vec<_> = meta::all()
            .iter()
            .filter(|m| m.stored)
            .filter_map(|m| loaded.settings.get(m.key).map(|v| (m, v)))
            .collect();
        Ok(self.apply(self.keep_session(edits), Origin::Editor))
    }

    /// Re-reads the file. Returns whether any setting changed. External edits win over
    /// unsaved in-session changes. With a syntax error the current values stay in effect.
    pub fn reload(&self) -> Result<bool, SettingsError> {
        let Some(path) = self.path() else {
            return Ok(false);
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(source) => return Err(SettingsError::Read { path, source }),
        };
        let loaded = {
            let mut f = lock(&self.inner.file);
            if f.last_text.as_deref() == Some(text.as_str())
                || (f.last_text.is_none() && text.is_empty())
            {
                return Ok(false);
            }
            let loaded = load_str(&text);
            f.last_text = Some(text);
            self.inner.text_generation.fetch_add(1, Ordering::AcqRel);
            f.syntax_error = loaded.doc.is_none();
            if loaded.doc.is_some() {
                f.doc = loaded.doc.clone();
                f.dirty.clear();
                f.migrated = loaded.migrated;
                self.inner.file_generation.fetch_add(1, Ordering::AcqRel);
            }
            loaded
        };
        for w in &loaded.warnings {
            log::warn!("{}: {w}", path.display());
        }
        *lock(&self.inner.warnings) = loaded.warnings.clone();
        if loaded.doc.is_none() {
            return Ok(false);
        }
        let edits: Vec<_> = meta::all()
            .iter()
            .filter(|m| m.stored)
            .filter_map(|m| loaded.settings.get(m.key).map(|v| (m, v)))
            .collect();
        Ok(!self
            .apply(self.keep_session(edits), Origin::File)
            .is_empty())
    }

    /// Replaces values read from the file with the `--set` values of this session.
    fn keep_session(
        &self,
        mut edits: Vec<(&'static SettingMeta, Value)>,
    ) -> Vec<(&'static SettingMeta, Value)> {
        let session = lock(&self.inner.session);
        if session.is_empty() {
            return edits;
        }
        for (m, v) in &mut edits {
            if let Some(s) = session.get(m.key) {
                *v = s.clone();
            }
        }
        edits
    }

    /// Sets what the watcher thread calls after an external edit was reloaded and changed a
    /// setting or the file's other parts ([`file_generation`](Self::file_generation)): an
    /// event loop that sleeps until woken learns of the change at once, not at its next
    /// unrelated wake-up.
    pub fn set_on_reload(&self, hook: ReloadHook) {
        *lock(&self.inner.on_reload) = Some(hook);
    }

    /// Watches the settings file and reloads it when it changes on disk (debounced; the
    /// store's own writes are ignored). Changes arrive as [`Origin::File`], followed by a
    /// call to the [reload hook](Self::set_on_reload).
    pub fn watch(&self) -> Result<(), SettingsError> {
        let Some(path) = self.path() else {
            return Ok(());
        };
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let werr = |e: notify::Error| SettingsError::Watch {
            path: path.clone(),
            reason: e.to_string(),
        };
        std::fs::create_dir_all(&dir).map_err(|source| SettingsError::Write {
            path: dir.clone(),
            source,
        })?;
        let name = path.file_name().map(|n| n.to_os_string());
        let weak = Arc::downgrade(&self.inner);
        // Debounced by hand rather than with a debouncer crate: those poll on a timer (20
        // wakeups a second for as long as the app runs), this thread sleeps until an event.
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(e) = res else { return };
            let ours = e
                .paths
                .iter()
                .any(|p| p.file_name().map(|n| n.to_os_string()) == name);
            if ours {
                let _ = tx.send(true);
            }
        })
        .map_err(werr)?;
        watcher
            .watch(&dir, notify::RecursiveMode::NonRecursive)
            .map_err(werr)?;
        specular_thread::builder("set-watch")
            .spawn(move || {
                // A panic in a reload (or in the app's reload hook) costs that reload: the
                // thread restarts after a back-off and the next edit is read again.
                let quit = || weak.strong_count() == 0;
                specular_log::service::supervise(
                    "set-watch",
                    specular_log::service::Policy::RESTART,
                    quit,
                    || {
                        // Ends when the watcher (and with it the sender) is dropped.
                        while rx.recv().is_ok() {
                            // Wait for 200 ms without events: editors write in several steps.
                            loop {
                                match rx.recv_timeout(Duration::from_millis(200)) {
                                    Ok(_) => {}
                                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                                }
                            }
                            let Some(inner) = weak.upgrade() else { return };
                            let store = SettingsStore { inner };
                            let file_gen = store.file_generation();
                            let changed = match store.reload() {
                                Ok(true) => {
                                    log::info!("settings.toml reloaded");
                                    true
                                }
                                Ok(false) => store.file_generation() != file_gen,
                                Err(e) => {
                                    log::warn!("{e}");
                                    false
                                }
                            };
                            let hook = lock(&store.inner.on_reload).clone();
                            if changed && let Some(hook) = hook {
                                hook();
                            }
                        }
                    },
                    |_| {},
                );
            })
            .map_err(|e| werr(notify::Error::io(e)))?;
        *lock(&self.inner.watcher) = Some(watcher);
        Ok(())
    }

    /// Stops watching the file.
    pub fn unwatch(&self) {
        lock(&self.inner.watcher).take();
    }

    /// All stored settings as one TOML document (for "Export settings…").
    pub fn export_string(&self) -> String {
        to_full_document(&self.snapshot()).to_string()
    }

    /// Writes all stored settings to `path` (atomically).
    pub fn export_to(&self, path: &Path) -> Result<(), SettingsError> {
        write_atomic(path, &self.export_string()).map_err(|source| SettingsError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Applies the settings present in TOML `text` (an exported file, or any partial
    /// settings file) as one undoable step. Returns the warnings for that text.
    pub fn import_str(&self, text: &str) -> Result<Vec<Warning>, String> {
        let loaded = load_str(text);
        let Some(doc) = loaded.doc else {
            return Err(loaded
                .warnings
                .first()
                .map(|w| w.to_string())
                .unwrap_or_default());
        };
        let edits = meta::all()
            .iter()
            .filter(|m| m.stored)
            .filter(|m| {
                doc.get(m.section.key())
                    .and_then(Item::as_table_like)
                    .is_some_and(|t| t.contains_key(m.name))
            })
            .filter_map(|m| loaded.settings.get(m.key).map(|v| (m, v)))
            .collect();
        self.apply(edits, Origin::Import);
        Ok(loaded.warnings)
    }

    /// Imports a settings file (see [`import_str`](Self::import_str)).
    pub fn import_from(&self, path: &Path) -> Result<Vec<Warning>, SettingsError> {
        let text = std::fs::read_to_string(path).map_err(|source| SettingsError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        self.import_str(&text)
            .map_err(|reason| SettingsError::Import {
                path: path.to_path_buf(),
                reason,
            })
    }
}

fn unknown(key: &str) -> SettingError {
    SettingError::UnknownKey {
        key: key.to_owned(),
        suggestion: suggest(key),
    }
}

/// Parses one `--set` argument, `key=value` (also `key = value`), into a validated value.
pub fn parse_set_arg(arg: &str) -> Result<(&'static SettingMeta, Value), SettingError> {
    let (key, value) = arg.split_once('=').ok_or(SettingError::Syntax)?;
    let key = key.trim();
    if key.is_empty() {
        return Err(SettingError::Syntax);
    }
    let m = meta::find(key).ok_or_else(|| unknown(key))?;
    let value = value.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    Ok((m, m.parse(value)?))
}

#[allow(dead_code)]
fn assert_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<SettingsStore>();
}
