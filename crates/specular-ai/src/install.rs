//! Downloading and installing a feature's models, and removing them again.
//!
//! Layout under the pack's root (`<data dir>/ai`, [`Store::new`]):
//!
//! ```text
//! downloads/<sha256>.part     partial downloads, resumed from their length
//! models/<id>-<version>/      installed models (renamed into place whole)
//! installed.toml              what is installed (replaced atomically)
//! ```
//!
//! `installed.toml` (schema 2) lists each feature's installed size (`[[feature]]`: `tier`,
//! the `previous` size still installed while a switch is finishing, and the `basis` the size
//! was recommended on) and each model per size it belongs to (`[[model]]`; a file shared by
//! two sizes is listed twice and stored once). Schema 1 (no sizes) is still read: its models
//! belong to an unknown size until [`Store::adopt_tiers`] names it from the manifest.
//!
//! Switching sizes ([`Install::keep_previous`]) installs the new size beside the old one; the
//! old size stays listed as `previous` (and usable) until [`Store::finish_switch`], so a
//! feature that must re-index keeps working until the new index is complete. A failed or
//! cancelled install changes nothing that is installed.
//!
//! A download resumes where it stopped (the prefix already on disk is hashed again), is
//! refused when it grows past the manifest's size, and is installed only when its SHA-256
//! matches. Install is all-or-nothing per feature: every model of the feature is downloaded
//! and verified first, then each is renamed into place, then the catalogue gets the
//! feature's tables, and only then does `installed.toml` list it. Removal goes the other
//! way: `installed.toml` first (a crash leaves at most unreferenced files, which
//! [`Store::tidy`] deletes), then the tables, then the files.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use parking_lot::Mutex;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::manifest::{self, Feature, Manifest, Model, Offer, Tier};
use crate::transport::Transport;

/// Progress of an install, shared with the UI; also how it is cancelled.
#[derive(Debug, Default)]
pub struct Transfer {
    /// Bytes on disk of the feature's models (resumed bytes count as done).
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

impl Transfer {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// (done, total) bytes.
    pub fn get(&self) -> (u64, u64) {
        (
            self.done.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }
}

#[derive(Debug)]
pub enum InstallError {
    /// Stopped on request; what was downloaded is kept for the next try.
    Cancelled,
    /// The download is not the file the manifest names (deleted).
    Hash {
        model: String,
    },
    /// The download is bigger than the manifest says, or the source says another size.
    Size {
        model: String,
    },
    /// No URL could be opened or read to the end (the last error).
    Download {
        model: String,
        error: io::Error,
    },
    Io(io::Error),
    Sql(rusqlite::Error),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Cancelled => f.write_str("cancelled"),
            InstallError::Hash { model } => write!(f, "{model}: SHA-256 does not match"),
            InstallError::Size { model } => write!(f, "{model}: size does not match"),
            InstallError::Download { model, error } => write!(f, "{model}: {error}"),
            InstallError::Io(e) => write!(f, "{e}"),
            InstallError::Sql(e) => write!(f, "catalogue: {e}"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<io::Error> for InstallError {
    fn from(e: io::Error) -> InstallError {
        InstallError::Io(e)
    }
}

impl From<rusqlite::Error> for InstallError {
    fn from(e: rusqlite::Error) -> InstallError {
        InstallError::Sql(e)
    }
}

/// An installed model, as `installed.toml` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    pub feature: Feature,
    /// The size it was installed for (None: installed before sizes, not yet adopted).
    pub tier: Option<Tier>,
    pub id: String,
    pub version: String,
    pub sha256: [u8; 32],
    pub size: u64,
    /// The weights, relative to the pack's root.
    pub file: String,
    /// Sideloaded without a hash check (the user confirmed): `sha256` and `size` are the
    /// file's own, not the manifest's.
    pub unverified: bool,
    /// The add-on it belongs to (an OCR script's recogniser), installed beside the
    /// feature's size and kept across size switches.
    pub addon: Option<String>,
}

impl Installed {
    pub fn key(&self) -> String {
        format!("{}-{}", self.id, self.version)
    }
}

/// A feature's installed size, as `installed.toml` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureRecord {
    pub feature: Feature,
    pub tier: Tier,
    /// The size installed before a switch, kept until [`Store::finish_switch`].
    pub previous: Option<Tier>,
    /// What the size was chosen on (the app's words: which GPU, calibrated or not), to
    /// notice when the machine changed.
    pub basis: Option<String>,
}

/// How [`Store::install_with`] installs.
#[derive(Clone, Debug, Default)]
pub struct Install {
    /// Keep the feature's installed size, as `previous`, until [`Store::finish_switch`]
    /// (instead of replacing it).
    pub keep_previous: bool,
    /// Recorded as the feature's [`FeatureRecord::basis`].
    pub basis: Option<String>,
}

/// Everything `installed.toml` says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub features: Vec<FeatureRecord>,
    /// Every model of every size listed (the current and the previous ones).
    pub models: Vec<Installed>,
}

impl State {
    pub fn record(&self, feature: Feature) -> Option<&FeatureRecord> {
        self.features.iter().find(|f| f.feature == feature)
    }

    /// The size a feature's models are (also for a schema-1 install, from the models).
    pub fn tier(&self, feature: Feature) -> Option<Tier> {
        self.record(feature).map(|r| r.tier).or_else(|| {
            self.models
                .iter()
                .filter(|m| m.feature == feature)
                .find_map(|m| m.tier)
        })
    }

    /// The models of a feature's current size (all of them when the size is unknown).
    pub fn current(&self) -> Vec<Installed> {
        self.models
            .iter()
            .filter(|m| match self.record(m.feature) {
                Some(r) => m.addon.is_some() || m.tier == Some(r.tier),
                None => true,
            })
            .cloned()
            .collect()
    }

    /// The models of a feature's previous size (empty unless a switch is finishing).
    pub fn previous(&self, feature: Feature) -> Vec<Installed> {
        let Some(p) = self.record(feature).and_then(|r| r.previous) else {
            return Vec::new();
        };
        self.models
            .iter()
            .filter(|m| m.feature == feature && m.tier == Some(p))
            .cloned()
            .collect()
    }

    fn referenced(&self, key: &str) -> bool {
        self.models.iter().any(|m| m.key() == key)
    }
}

const STATE: &str = "installed.toml";
const READ_CHUNK: usize = 1 << 16;

/// The pack's files.
pub struct Store {
    root: PathBuf,
    /// Folders of model files placed by an administrator, looked in before downloading
    /// ([`crate::sideload`]).
    folders: Vec<PathBuf>,
    /// Hashes of the admin folders' files, by path, length and modification time.
    pub(crate) hashes: Mutex<crate::sideload::HashCache>,
    /// Serialises changes to `installed.toml` (features install on their own threads).
    lock: Mutex<()>,
}

impl Store {
    /// The pack under the platform data directory (`$XDG_DATA_HOME/specular` on Linux).
    pub fn new(data_dir: &Path) -> Store {
        Store::at(data_dir.join("ai"))
    }

    pub fn at(root: PathBuf) -> Store {
        Store {
            root,
            folders: Vec::new(),
            hashes: Mutex::default(),
            lock: Mutex::new(()),
        }
    }

    /// Looks for model files in `folders` before downloading them (admin-placed models).
    pub fn with_folders(mut self, folders: Vec<PathBuf>) -> Store {
        self.folders = folders;
        self
    }

    /// The admin folders ([`Store::with_folders`]).
    pub fn folders(&self) -> &[PathBuf] {
        &self.folders
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The installed models of each feature's current size (none when nothing was
    /// installed). A model whose files are gone is not listed.
    pub fn installed(&self) -> Vec<Installed> {
        self.state().current()
    }

    /// What `installed.toml` says, without models whose files are gone.
    pub fn state(&self) -> State {
        let Ok(text) = fs::read_to_string(self.root.join(STATE)) else {
            return State::default();
        };
        let mut st = parse_state(&text);
        st.models.retain(|m| self.root.join(&m.file).is_file());
        st.features
            .retain(|r| st.models.iter().any(|m| m.feature == r.feature));
        st
    }

    /// A feature's installed size.
    pub fn tier(&self, feature: Feature) -> Option<Tier> {
        self.state().tier(feature)
    }

    /// Names the size of models installed before sizes existed (schema 1): the manifest's
    /// size whose models they are (the standard size first). Written back once. Returns the
    /// features installed before sizes (adopted or not), so the app can carry over what such
    /// an install meant (its switch was on).
    pub fn adopt_tiers(&self, manifest: &Manifest) -> Vec<Feature> {
        let _g = self.lock.lock();
        let mut st = self.state();
        let mut changed = false;
        let mut old = Vec::new();
        for f in Feature::ALL {
            if st.record(f).is_some()
                || !st
                    .models
                    .iter()
                    .any(|m| m.feature == f && m.addon.is_none())
            {
                continue;
            }
            old.push(f);
            let keys: Vec<String> = st
                .models
                .iter()
                .filter(|m| m.feature == f && m.tier.is_none() && m.addon.is_none())
                .map(Installed::key)
                .collect();
            if keys.is_empty() {
                continue;
            }
            let Some(offer) = manifest.offer(f, true) else {
                continue;
            };
            let mut order = offer.tier_ids();
            order.sort_by_key(|t| *t != Tier::Standard);
            let Some(tier) = order.into_iter().find(|t| {
                offer
                    .tier_offer(*t)
                    .is_some_and(|o| keys.iter().all(|k| o.models.iter().any(|m| &m.key() == k)))
            }) else {
                continue;
            };
            for m in st
                .models
                .iter_mut()
                .filter(|m| m.feature == f && m.addon.is_none())
            {
                m.tier = Some(tier);
            }
            st.features.push(FeatureRecord {
                feature: f,
                tier,
                previous: None,
                basis: None,
            });
            changed = true;
        }
        if changed && let Err(e) = self.write_state(&st) {
            specular_log::warn!(target: specular_log::JOBS, "ai: {STATE}: {e}");
        }
        old
    }

    /// Features with every model installed (in feature order).
    pub fn features(&self) -> Vec<Feature> {
        let mut f: Vec<Feature> = self
            .installed()
            .iter()
            .filter(|m| m.addon.is_none())
            .map(|m| m.feature)
            .collect();
        f.sort();
        f.dedup();
        f
    }

    pub fn has(&self, feature: Feature) -> bool {
        self.installed()
            .iter()
            .any(|m| m.feature == feature && m.addon.is_none())
    }

    /// The add-ons of `feature` installed (ids, sorted).
    pub fn addons(&self, feature: Feature) -> Vec<String> {
        let mut v: Vec<String> = self
            .installed()
            .into_iter()
            .filter(|m| m.feature == feature)
            .filter_map(|m| m.addon)
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Bytes [`Store::install_addon`] would download.
    pub fn addon_to_download(&self, models: &[Model]) -> u64 {
        let st = self.state();
        models
            .iter()
            .filter(|m| !have(&st, m))
            .map(|m| m.size)
            .sum()
    }

    /// Downloads, verifies and installs the files of one add-on (`models`, all of one
    /// feature and add-on, from [`Manifest::addon_models`]) beside the feature's size.
    /// All or nothing, as [`Store::install`].
    pub fn install_addon(
        &self,
        models: &[Model],
        transport: &dyn Transport,
        progress: &Transfer,
    ) -> Result<(), InstallError> {
        let Some(first) = models.first() else {
            return Ok(());
        };
        let (feature, addon) = (first.feature, first.addon.clone());
        let Some(addon) = addon.filter(|a| {
            models
                .iter()
                .all(|m| m.feature == feature && m.addon.as_deref() == Some(a))
        }) else {
            return Err(InstallError::Io(io::Error::other("not one add-on's files")));
        };
        let before = self.state();
        let entries =
            self.fetch_and_place(models, &before, transport, progress, None, Some(&addon))?;
        let _g = self.lock.lock();
        let mut st = self.state();
        let old_keys: Vec<String> = st
            .models
            .iter()
            .filter(|m| m.feature == feature && m.addon.as_deref() == Some(&addon))
            .map(Installed::key)
            .collect();
        st.models
            .retain(|m| !(m.feature == feature && m.addon.as_deref() == Some(&addon)));
        st.models.extend(entries);
        self.write_state(&st)?;
        for k in old_keys {
            if !st.referenced(&k) {
                let _ = fs::remove_dir_all(self.root.join("models").join(k));
            }
        }
        specular_log::info!(target: specular_log::JOBS, "ai: installed {feature} add-on {addon}");
        Ok(())
    }

    /// Removes one add-on of a feature (its files; the feature stays).
    pub fn remove_addon(&self, feature: Feature, addon: &str) -> Result<(), InstallError> {
        let (gone, st) = {
            let _g = self.lock.lock();
            let mut st = self.state();
            let (gone, keep): (Vec<_>, Vec<_>) = st
                .models
                .into_iter()
                .partition(|m| m.feature == feature && m.addon.as_deref() == Some(addon));
            st.models = keep;
            self.write_state(&st)?;
            (gone, st)
        };
        for m in gone {
            if !st.referenced(&m.key()) {
                let _ = fs::remove_dir_all(self.root.join("models").join(m.key()));
            }
        }
        specular_log::info!(target: specular_log::JOBS, "ai: removed {feature} add-on {addon}");
        Ok(())
    }

    /// Downloads, verifies and installs every model of `offer`, then gives the catalogue
    /// (if any) the feature's tables. Replaces an earlier install of the feature.
    pub fn install(
        &self,
        offer: &Offer,
        transport: &dyn Transport,
        progress: &Transfer,
        conn: Option<&mut Connection>,
    ) -> Result<(), InstallError> {
        self.install_with(offer, transport, progress, conn, &Install::default())
    }

    /// Bytes [`Store::install`] would download for `offer`: models already installed (for
    /// another size, or by an earlier install) are not downloaded again.
    pub fn to_download(&self, offer: &Offer) -> u64 {
        let st = self.state();
        offer
            .models
            .iter()
            .filter(|m| !have(&st, m))
            .map(|m| m.size)
            .sum()
    }

    /// Bytes the files of `offer` take on disk once all are installed (a converted model
    /// counts its converted file, not the download). None while any is missing.
    pub fn on_disk(&self, offer: &Offer) -> Option<u64> {
        let st = self.state();
        offer
            .models
            .iter()
            .map(|m| {
                let i = st.models.iter().find(|i| same(i, m))?;
                fs::metadata(self.root.join(&i.file))
                    .ok()
                    .map(|md| md.len())
            })
            .sum()
    }

    /// [`Store::install`], optionally keeping the installed size until
    /// [`Store::finish_switch`]. On any error (or a cancel) nothing installed changes: the
    /// old size stays, and what was downloaded is kept to resume.
    pub fn install_with(
        &self,
        offer: &Offer,
        transport: &dyn Transport,
        progress: &Transfer,
        conn: Option<&mut Connection>,
        how: &Install,
    ) -> Result<(), InstallError> {
        let before = self.state();
        let entries = self.fetch_and_place(
            &offer.models,
            &before,
            transport,
            progress,
            Some(offer.tier),
            None,
        )?;
        let models = self.root.join("models");
        if let Some(conn) = conn {
            specular_library::ai::install(conn, offer.feature.id())?;
        }
        let _g = self.lock.lock();
        let mut st = self.state();
        let f = offer.feature;
        let old_tier = st.tier(f);
        // The size to keep: the installed one, or (a reinstall of the same size) the one a
        // pending switch still keeps.
        let keep = if !how.keep_previous {
            None
        } else if old_tier == Some(offer.tier) {
            st.record(f).and_then(|r| r.previous)
        } else {
            old_tier
        }
        .filter(|t| *t != offer.tier);
        let old_keys: Vec<String> = st
            .models
            .iter()
            .filter(|m| m.feature == f && m.addon.is_none())
            .map(Installed::key)
            .collect();
        // The kept size's models stay (named, should they predate sizes), and the add-ons;
        // the rest of the feature's go.
        st.models.retain_mut(|m| {
            if m.feature != f || m.addon.is_some() {
                return true;
            }
            match keep {
                Some(k) if m.tier.is_none_or(|t| t == k) => {
                    m.tier = Some(k);
                    true
                }
                _ => false,
            }
        });
        st.models.extend(entries);
        st.features.retain(|r| r.feature != f);
        st.features.push(FeatureRecord {
            feature: f,
            tier: offer.tier,
            previous: keep,
            basis: how.basis.clone(),
        });
        st.features.sort_by_key(|r| r.feature);
        self.write_state(&st)?;
        // Files of an earlier version or size of the feature's models.
        for k in old_keys {
            if !st.referenced(&k) {
                let _ = fs::remove_dir_all(models.join(k));
            }
        }
        specular_log::info!(
            target: specular_log::JOBS,
            "ai: installed {} ({}{})",
            offer.feature,
            offer.tier,
            if keep.is_some() { ", switching" } else { "" }
        );
        Ok(())
    }

    /// Ends a switch of sizes: the previous size's models are deleted (those the new size
    /// does not share). Nothing to do when no switch is pending.
    pub fn finish_switch(&self, feature: Feature) -> Result<(), InstallError> {
        let _g = self.lock.lock();
        let mut st = self.state();
        let Some(r) = st.features.iter_mut().find(|r| r.feature == feature) else {
            return Ok(());
        };
        let (tier, Some(prev)) = (r.tier, r.previous.take()) else {
            return Ok(());
        };
        let gone: Vec<String> = st
            .models
            .iter()
            .filter(|m| m.feature == feature && m.tier == Some(prev) && m.addon.is_none())
            .map(Installed::key)
            .collect();
        st.models
            .retain(|m| m.feature != feature || m.tier == Some(tier) || m.addon.is_some());
        self.write_state(&st)?;
        for k in gone {
            if !st.referenced(&k) {
                let _ = fs::remove_dir_all(self.root.join("models").join(k));
            }
        }
        specular_log::info!(target: specular_log::JOBS, "ai: {feature}: {prev} removed, {tier} kept");
        Ok(())
    }

    /// Records what a feature's size was chosen on.
    pub fn set_basis(&self, feature: Feature, basis: Option<String>) -> io::Result<()> {
        let _g = self.lock.lock();
        let mut st = self.state();
        let Some(r) = st.features.iter_mut().find(|r| r.feature == feature) else {
            return Ok(());
        };
        if r.basis == basis {
            return Ok(());
        }
        r.basis = basis;
        self.write_state(&st)
    }

    /// Removes a feature: no longer listed, its tables dropped (with everything in them), its
    /// files deleted.
    pub fn remove(
        &self,
        feature: Feature,
        conn: Option<&mut Connection>,
    ) -> Result<(), InstallError> {
        let (gone, st): (Vec<Installed>, State) = {
            let _g = self.lock.lock();
            let mut st = self.state();
            let (gone, keep): (Vec<_>, Vec<_>) =
                st.models.into_iter().partition(|m| m.feature == feature);
            st.models = keep;
            st.features.retain(|r| r.feature != feature);
            self.write_state(&st)?;
            (gone, st)
        };
        // Speech, Masks and Denoise keep nothing in the library (subtitles, masks and
        // denoised pictures are in the cache).
        if let Some(conn) = conn
            && !matches!(
                feature,
                Feature::Speech
                    | Feature::Masks
                    | Feature::Denoise
                    | Feature::Remove
                    | Feature::Depth
            )
        {
            specular_library::ai::drop(conn, feature.id())?;
        }
        for m in gone {
            if !st.referenced(&m.key()) {
                let _ = fs::remove_dir_all(self.root.join("models").join(m.key()));
            }
        }
        specular_log::info!(target: specular_log::JOBS, "ai: removed {feature}");
        Ok(())
    }

    /// Removes the whole pack: every feature's tables, every file under the root.
    pub fn remove_all(&self, conn: Option<&mut Connection>) -> Result<(), InstallError> {
        {
            let _g = self.lock.lock();
            self.write_state(&State::default())?;
        }
        if let Some(conn) = conn {
            specular_library::ai::drop_all(conn)?;
        }
        match fs::remove_dir_all(&self.root) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        specular_log::info!(target: specular_log::JOBS, "ai: pack removed");
        Ok(())
    }

    /// Deletes what nothing refers to: model directories not installed, interrupted
    /// installs, and partial downloads of files the manifest no longer names.
    pub fn tidy(&self, manifest: &Manifest) {
        let _g = self.lock.lock();
        let st = self.state();
        if let Ok(rd) = fs::read_dir(self.root.join("models")) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !st.referenced(&name) {
                    let _ = fs::remove_dir_all(e.path());
                }
            }
        }
        if let Ok(rd) = fs::read_dir(self.root.join("downloads")) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let wanted = manifest.models.iter().any(|m| {
                    name == format!("{}.part", m.sha256_hex())
                        || name == format!("{}.unverified", m.key())
                });
                if !wanted {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
    }

    /// Downloads (or finds) and verifies every file of `want` not installed yet, then moves
    /// each into `models/<id>-<version>/` (converted when the manifest says). Returns the
    /// entries to list, `tier` and `addon` set as given. Nothing is listed here.
    fn fetch_and_place(
        &self,
        want: &[Model],
        before: &State,
        transport: &dyn Transport,
        progress: &Transfer,
        tier: Option<Tier>,
        addon: Option<&str>,
    ) -> Result<Vec<Installed>, InstallError> {
        let fetch: Vec<&Model> = want.iter().filter(|m| !have(before, m)).collect();
        progress
            .total
            .store(fetch.iter().map(|m| m.size).sum(), Ordering::Relaxed);
        progress.done.store(0, Ordering::Relaxed);
        let mut parts = Vec::new();
        for m in &fetch {
            let base = progress.done.load(Ordering::Relaxed);
            parts.push(self.download(m, transport, progress, base)?);
            progress.done.store(base + m.size, Ordering::Relaxed);
        }
        if progress.cancelled() {
            return Err(InstallError::Cancelled);
        }
        let models = self.root.join("models");
        fs::create_dir_all(&models)?;
        let mut entries = Vec::new();
        let mut parts = parts.into_iter();
        for m in want {
            let name = stored_name(m);
            if let Some(old) = before.models.iter().find(|i| same(i, m)) {
                entries.push(Installed {
                    tier,
                    addon: addon.map(str::to_owned),
                    ..old.clone()
                });
                continue;
            }
            let Some((part, unverified)) = parts.next() else {
                return Err(InstallError::Io(io::Error::other("download missing")));
            };
            let tmp = models.join(format!(".tmp-{}-{}", m.key(), std::process::id()));
            let _ = fs::remove_dir_all(&tmp);
            fs::create_dir_all(&tmp)?;
            match m.runtime.convert {
                // The verified download becomes the file the runtime keeps; the original
                // goes (it is not needed again: a reinstall downloads it anew).
                Some(manifest::Convert::F16) => {
                    if let Err(e) = convert(m, &part, &tmp.join(&name), progress) {
                        let _ = fs::remove_dir_all(&tmp);
                        return Err(e);
                    }
                    fs::remove_file(&part)?;
                }
                None => fs::rename(&part, tmp.join(&name))?,
            }
            // Opened for writing: on Windows, flushing a read-only handle is "Access is denied".
            fs::OpenOptions::new()
                .write(true)
                .open(tmp.join(&name))?
                .sync_all()?;
            let dest = models.join(m.key());
            if dest.exists() {
                fs::remove_dir_all(&dest)?;
            }
            fs::rename(&tmp, &dest)?;
            sync_dir(&models);
            let (sha256, size) = unverified.unwrap_or((m.sha256, m.size));
            entries.push(Installed {
                feature: m.feature,
                tier,
                id: m.id.clone(),
                version: m.version.clone(),
                sha256,
                size,
                file: format!("models/{}/{name}", m.key()),
                unverified: unverified.is_some(),
                addon: addon.map(str::to_owned),
            });
        }
        Ok(entries)
    }

    /// The partial download of `m`.
    pub fn part_path(&self, m: &Model) -> PathBuf {
        self.root
            .join("downloads")
            .join(format!("{}.part", m.sha256_hex()))
    }

    /// Gets `m` without the network if it can (sideloaded unverified: that file, with its
    /// own hash and size; else a complete part file or one copied from an admin folder),
    /// else downloads it ([`Store::download_from`]).
    fn download(
        &self,
        m: &Model,
        transport: &dyn Transport,
        progress: &Transfer,
        base: u64,
    ) -> Result<(PathBuf, Option<Unverified>), InstallError> {
        let unverified = self.unverified_path(m);
        if unverified.is_file() {
            let (h, n) = crate::sideload::hash_file(&unverified)?;
            return Ok((unverified, Some((h, n))));
        }
        let part = self.part_path(m);
        let complete = fs::metadata(&part).is_ok_and(|md| md.len() == m.size);
        if !complete && let Some(src) = self.find_in_folders(m) {
            self.sideload(m, &src)?;
            return Ok((part, None));
        }
        self.download_from(m, transport, progress, base)
            .map(|p| (p, None))
    }

    /// Downloads `m` to its part file (resuming), checked against size and hash. `base` is
    /// what the transfer had done before this model.
    fn download_from(
        &self,
        m: &Model,
        transport: &dyn Transport,
        progress: &Transfer,
        base: u64,
    ) -> Result<PathBuf, InstallError> {
        let part = self.part_path(m);
        fs::create_dir_all(part.parent().unwrap_or(&self.root))?;
        let mut last = io::Error::new(io::ErrorKind::NotFound, "no URL");
        for url in &m.urls {
            if progress.cancelled() {
                return Err(InstallError::Cancelled);
            }
            let mut have = match fs::metadata(&part) {
                Ok(md) if md.len() <= m.size => md.len(),
                Ok(_) => {
                    fs::remove_file(&part)?;
                    0
                }
                Err(_) => 0,
            };
            if have == m.size {
                // Complete from an earlier run: only the hash is left to check.
                return verify(&part, m).map(|()| part);
            }
            progress.done.store(base + have, Ordering::Relaxed);
            let mut moved = |n: u64| progress.done.store(base + n.min(m.size), Ordering::Relaxed);
            if let Some(r) = transport.download_to(url, &part, have, &progress.cancel, &mut moved) {
                let len = fs::metadata(&part).map(|md| md.len()).unwrap_or(0);
                if progress.cancelled() {
                    return Err(InstallError::Cancelled);
                }
                if len > m.size {
                    let _ = fs::remove_file(&part);
                    return Err(InstallError::Size {
                        model: m.id.clone(),
                    });
                }
                match r {
                    Ok(()) if len == m.size => return verify(&part, m).map(|()| part),
                    Ok(()) => {
                        last = io::Error::new(io::ErrorKind::UnexpectedEof, "download ended early");
                    }
                    Err(e) if crate::transport::is_stopped(&e) => {
                        return Err(InstallError::Cancelled);
                    }
                    Err(e) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: {}: {e}", m.id);
                        last = e;
                    }
                }
                continue;
            }
            let body = match transport.open_named(url, have, &m.id) {
                Ok(b) => b,
                Err(e) if crate::transport::is_stopped(&e) => return Err(InstallError::Cancelled),
                Err(e) => {
                    specular_log::warn!(target: specular_log::JOBS, "ai: {}: {e}", m.id);
                    last = e;
                    continue;
                }
            };
            if body.total.is_some_and(|t| t != m.size) {
                return Err(InstallError::Size {
                    model: m.id.clone(),
                });
            }
            if have > 0 && !body.resumed {
                have = 0;
            }
            let mut hasher = Sha256::new();
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .read(true)
                .open(&part)?;
            file.set_len(have)?;
            if have > 0 {
                hash_prefix(&mut file, have, &mut hasher)?;
            }
            let mut out = OpenOptions::new().append(true).open(&part)?;
            progress.done.store(base + have, Ordering::Relaxed);
            let mut reader = body.reader;
            let mut buf = vec![0u8; READ_CHUNK];
            let read_err = loop {
                if progress.cancelled() {
                    out.sync_all()?;
                    return Err(InstallError::Cancelled);
                }
                let n = match reader.read(&mut buf) {
                    Ok(0) => break None,
                    Ok(n) => n,
                    // Stopped from the "Downloads" panel: what came stays, to resume.
                    Err(e) if crate::transport::is_stopped(&e) => {
                        out.sync_all()?;
                        return Err(InstallError::Cancelled);
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => break Some(e),
                };
                if have + n as u64 > m.size {
                    drop(out);
                    let _ = fs::remove_file(&part);
                    return Err(InstallError::Size {
                        model: m.id.clone(),
                    });
                }
                out.write_all(&buf[..n])?;
                hasher.update(&buf[..n]);
                have += n as u64;
                progress.done.store(base + have, Ordering::Relaxed);
            };
            out.sync_all()?;
            if let Some(e) = read_err {
                // Keep what came: the next URL (or the next try) resumes.
                last = e;
                continue;
            }
            if have < m.size {
                last = io::Error::new(io::ErrorKind::UnexpectedEof, "download ended early");
                continue;
            }
            if hasher.finalize().as_slice() != m.sha256 {
                let _ = fs::remove_file(&part);
                return Err(InstallError::Hash {
                    model: m.id.clone(),
                });
            }
            return Ok(part);
        }
        Err(InstallError::Download {
            model: m.id.clone(),
            error: last,
        })
    }

    fn write_state(&self, st: &State) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let mut s = String::from(
            "# Written by Specular: the Local AI pack's installed models.\nschema = 2\n",
        );
        for r in &st.features {
            s.push_str(&format!(
                "\n[[feature]]\nid = {:?}\ntier = {:?}\n",
                r.feature.id(),
                r.tier.id()
            ));
            if let Some(p) = r.previous {
                s.push_str(&format!("previous = {:?}\n", p.id()));
            }
            if let Some(b) = &r.basis {
                s.push_str(&format!("basis = {b:?}\n"));
            }
        }
        for m in &st.models {
            s.push_str(&format!(
                "\n[[model]]\nfeature = {:?}\n{}id = {:?}\nversion = {:?}\nsha256 = {:?}\nsize = {}\nfile = {:?}\n",
                m.feature.id(),
                m.tier.map(|t| format!("tier = {:?}\n", t.id())).unwrap_or_default(),
                m.id,
                m.version,
                manifest::hex(&m.sha256),
                m.size,
                m.file
            ));
            if m.unverified {
                s.push_str("unverified = true\n");
            }
            if let Some(a) = &m.addon {
                s.push_str(&format!("addon = {a:?}\n"));
            }
        }
        let tmp = self
            .root
            .join(format!("{STATE}.tmp-{}", std::process::id()));
        {
            let mut f = File::create(&tmp)?;
            f.write_all(s.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, self.root.join(STATE))?;
        sync_dir(&self.root);
        Ok(())
    }
}

/// The installed file's name: `model.<format>`, or `model.safetensors` for a converted
/// download (a `.pt` becomes safetensors).
fn stored_name(m: &Model) -> String {
    match m.runtime.convert {
        Some(manifest::Convert::F16) => "model.safetensors".into(),
        None => format!("model.{}", m.runtime.format),
    }
}

/// A sideloaded file used without a hash check: its own SHA-256 and size.
type Unverified = ([u8; 32], u64);

/// Converts the download of `m` at `src` to the file the pack keeps at `dst`, as an install
/// does (without checking the pin): the bytes written.
pub fn convert_download(
    m: &Model,
    src: &Path,
    dst: &Path,
    cancel: &dyn Fn() -> bool,
) -> Result<u64, specular_infer::Error> {
    use specular_infer::{depth, lama, pecore, pt::PtFile, safetensors, sam, siglip};
    let keep: &dyn Fn(&safetensors::TensorInfo) -> bool = match m.runtime.arch.as_deref() {
        Some("siglip") => &siglip::keep_f32,
        Some("pecore") => &pecore::keep_f32,
        Some("efficientsam") => &sam::keep_f32,
        Some("lama") => &lama::keep_f32,
        Some("depth-anything") => &depth::keep_f32,
        _ => &|_| false,
    };
    if m.runtime.format == "pt" {
        safetensors::convert_f16_from(PtFile::open(src)?, dst, keep, cancel)
    } else {
        safetensors::convert_f16(src, dst, keep, cancel)
    }
}

/// Converts a verified download to an f16 safetensors file (SigLIP and PE Core keep their
/// positions and pooling probe in f32, [`specular_infer::siglip::keep_f32`],
/// [`specular_infer::pecore::keep_f32`]; a PyTorch `.pt` is read by `specular_infer::pt`)
/// and checks the result against the manifest's pin, when it has one.
fn convert(m: &Model, src: &Path, dst: &Path, progress: &Transfer) -> Result<(), InstallError> {
    let cancel = || progress.cancelled();
    match convert_download(m, src, dst, &cancel) {
        Ok(_) => {}
        Err(_) if progress.cancelled() => return Err(InstallError::Cancelled),
        Err(e) => return Err(InstallError::Io(io::Error::other(format!("{}: {e}", m.id)))),
    }
    if let Some(want) = m.runtime.converted_sha256 {
        let mut f = File::open(dst)?;
        let len = f.metadata()?.len();
        let mut h = Sha256::new();
        hash_prefix(&mut f, len, &mut h)?;
        if h.finalize().as_slice() != want {
            let _ = fs::remove_file(dst);
            return Err(InstallError::Hash {
                model: format!("{} (converted)", m.id),
            });
        }
    }
    Ok(())
}

fn hash_prefix(file: &mut File, len: u64, hasher: &mut Sha256) -> io::Result<()> {
    let mut buf = vec![0u8; READ_CHUNK];
    let mut left = len;
    let mut r = file.take(len);
    while left > 0 {
        let n = r.read(&mut buf)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        hasher.update(&buf[..n]);
        left -= n as u64;
    }
    Ok(())
}

fn verify(part: &Path, m: &Model) -> Result<(), InstallError> {
    let mut f = File::open(part)?;
    let mut h = Sha256::new();
    hash_prefix(&mut f, m.size, &mut h)?;
    if h.finalize().as_slice() == m.sha256 {
        Ok(())
    } else {
        let _ = fs::remove_file(part);
        Err(InstallError::Hash {
            model: m.id.clone(),
        })
    }
}

/// Makes a rename in `dir` durable (where the platform can).
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Whether `m` is installed already (for any size), the same file.
fn have(st: &State, m: &Model) -> bool {
    st.models.iter().any(|i| same(i, m))
}

fn same(i: &Installed, m: &Model) -> bool {
    i.feature == m.feature
        && i.key() == m.key()
        && (i.unverified || (i.sha256 == m.sha256 && i.size == m.size))
}

/// Reads `installed.toml` (schema 1 or 2; what a newer schema adds is ignored).
pub fn parse_state(text: &str) -> State {
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
        return State::default();
    };
    let features = doc
        .get("feature")
        .and_then(|m| m.as_array_of_tables())
        .map(|tables| {
            tables
                .iter()
                .filter_map(|t| {
                    let s = |k: &str| t.get(k).and_then(|v| v.as_str());
                    Some(FeatureRecord {
                        feature: Feature::from_id(s("id")?)?,
                        tier: Tier::from_id(s("tier")?)?,
                        previous: s("previous").and_then(Tier::from_id),
                        basis: s("basis").map(str::to_owned),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let Some(tables) = doc.get("model").and_then(|m| m.as_array_of_tables()) else {
        return State {
            features,
            models: Vec::new(),
        };
    };
    let models = tables
        .iter()
        .filter_map(|t| {
            let s = |k: &str| t.get(k).and_then(|v| v.as_str());
            let file = s("file")?.to_owned();
            // Only files below the root.
            if file.starts_with('/') || file.split('/').any(|c| c == ".." || c.is_empty()) {
                return None;
            }
            Some(Installed {
                feature: Feature::from_id(s("feature")?)?,
                tier: s("tier").and_then(Tier::from_id),
                id: s("id")?.to_owned(),
                version: s("version")?.to_owned(),
                sha256: manifest::parse_hex(s("sha256")?)?,
                size: t.get("size")?.as_integer()? as u64,
                file,
                unverified: t
                    .get("unverified")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                addon: s("addon")
                    .filter(|a| manifest::is_name(a))
                    .map(str::to_owned),
            })
        })
        .collect();
    State { features, models }
}
