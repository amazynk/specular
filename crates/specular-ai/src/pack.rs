//! The pack as the app sees it: what is offered, installed, downloading; installing and
//! removing features on background threads; the People data controls.
//!
//! Everything that writes the catalogue (opening it, migrating it, dropping tables, any of
//! which can wait out another writer's transaction for seconds) runs on the pack's own threads
//! (`ai-get-*`, `ai-rm-*`, `ai-forget`), never on the caller's: the Settings page's
//! buttons return at once and the threads report back through [`Pack::take_finished`],
//! [`Pack::changes`] and the wake callback. [`Pack::counts`] and [`Pack::people`] read the
//! catalogue on the calling thread, which must not be the UI's (debug builds panic there).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

use parking_lot::Mutex;

use crate::install::{Install, InstallError, Installed, State, Store, Transfer};
use crate::manifest::{Feature, Manifest, Model, Norm, Offer, Tier};
use crate::transport::Transports;

/// The failure of an install or removal that panicked (in [`FeatureState::Failed`] and
/// [`Pack::take_finished`]): the app shows its own words for it, the log has the details.
pub const INTERNAL_ERROR: &str = "internal error";

/// One feature's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeatureState {
    /// The manifest offers no model for it.
    Unavailable,
    /// Offered, not installed.
    Available,
    /// Downloading: bytes done and total.
    Downloading {
        done: u64,
        total: u64,
    },
    /// The last install failed (offered again).
    Failed(String),
    Installed,
    /// Being removed (its tables dropped, its files deleted) on a background thread.
    Removing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureStatus {
    pub feature: Feature,
    /// [`FeatureState::Downloading`] and [`FeatureState::Failed`] also while an older size
    /// is installed (and in use): `tier` says which.
    pub state: FeatureState,
    /// What the manifest offers (also for an installed feature, if still offered).
    pub offer: Option<Offer>,
    /// The installed size (None: not installed, or installed before sizes and no longer
    /// offered).
    pub tier: Option<Tier>,
    /// The size being downloaded or that failed to.
    pub target: Option<Tier>,
    /// The size installed before a switch, still in use until the new one has re-indexed.
    pub previous: Option<Tier>,
    /// What the installed size was chosen on ([`Pack::install_for`]).
    pub basis: Option<String>,
}

/// The models a feature indexes with: one installed size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexModel {
    pub tier: Option<Tier>,
    pub models: Vec<Installed>,
    /// What the catalogue stores its results under (`<size>/<model keys>`): a new size or
    /// version is a new key, so its results never mix with the old ones'.
    pub key: String,
}

impl IndexModel {
    fn new(tier: Option<Tier>, mut models: Vec<Installed>) -> Option<IndexModel> {
        if models.is_empty() {
            return None;
        }
        models.sort_by_key(Installed::key);
        let keys: Vec<String> = models.iter().map(Installed::key).collect();
        let key = format!("{}/{}", tier.map_or("unsized", Tier::id), keys.join("+"));
        Some(IndexModel { tier, models, key })
    }

    /// The model with this manifest id.
    pub fn model(&self, id: &str) -> Option<&Installed> {
        self.models.iter().find(|m| m.id == id)
    }
}

enum Running {
    Install(Arc<Transfer>, Tier, Option<JoinHandle<()>>),
    Remove(Option<JoinHandle<()>>),
    Failed(String, Tier),
}

/// What a background removal or People change reported: what it was, and how it ended.
pub type Finished = (String, Result<(), String>);

/// What [`Pack::sideload`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sideload {
    /// Known files, copied in: ready to install their feature without the network.
    pub ready: Vec<(PathBuf, Feature)>,
    /// Unknown hashes with a model's file name: used only if the user confirms.
    pub unverified: Vec<(PathBuf, Model)>,
    /// Not a model Specular knows (refused).
    pub unknown: Vec<PathBuf>,
    /// Could not be read or copied (why).
    pub failed: Vec<(PathBuf, String)>,
}

/// An add-on's state (an OCR script recogniser beside the Text feature's size).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddonStatus {
    pub feature: Feature,
    /// The manifest's add-on id ("arabic").
    pub id: String,
    /// [`FeatureState::Available`], `Downloading`, `Failed`, `Installed` or `Removing`.
    pub state: FeatureState,
    /// Bytes of its files.
    pub size: u64,
}

enum AddonRun {
    Install(Arc<Transfer>, Option<JoinHandle<()>>),
    Remove(Option<JoinHandle<()>>),
    Failed(String),
}

/// The Local AI pack.
pub struct Pack {
    /// Features installed before sizes existed (see [`Pack::upgraded`]).
    upgraded: Vec<Feature>,
    manifest: Manifest,
    store: Arc<Store>,
    transports: Arc<Transports>,
    /// The library's catalogue, for the features' tables.
    catalogue: Option<PathBuf>,
    placeholders: bool,
    running: Mutex<HashMap<Feature, Running>>,
    /// Install failures reported by the threads.
    failed: Arc<Mutex<HashMap<Feature, (String, Tier)>>>,
    /// "Remove pack" running: the features it is removing, and its thread.
    removing_all: Mutex<RemovingAll>,
    /// Features installed when [`Pack::status`] last looked (what "Remove pack" shows as
    /// removing, without reading the store on the caller's thread).
    last_installed: Mutex<Vec<Feature>>,
    /// People changes' threads (for [`Pack::join`]).
    others: Mutex<Vec<JoinHandle<()>>>,
    /// Removals and People changes that ended, not yet taken.
    finished: Arc<Mutex<Vec<Finished>>>,
    /// Bumped whenever a background removal or People change ends.
    changes: Arc<AtomicU64>,
    wake: Arc<dyn Fn() + Send + Sync>,
    /// Add-on installs and removals, by (feature, add-on id).
    addons: Mutex<HashMap<(Feature, String), AddonRun>>,
    /// Add-on install failures reported by their threads.
    addon_failed: Arc<Mutex<HashMap<(Feature, String), String>>>,
}

/// "Remove pack" running: the features it is removing, and its thread.
type RemovingAll = Option<(Vec<Feature>, Option<JoinHandle<()>>)>;

/// Whether "Remove pack" is still running.
fn running_all(all: &RemovingAll) -> bool {
    all.as_ref()
        .is_some_and(|(_, h)| h.as_ref().is_some_and(|h| !h.is_finished()))
}

/// Whether `r` is a removal that has ended.
fn removed(r: &Running) -> bool {
    matches!(r, Running::Remove(h) if h.as_ref().is_none_or(JoinHandle::is_finished))
}

/// The pack's own connection to the catalogue (None without one).
fn open_conn(
    catalogue: Option<&std::path::Path>,
) -> Result<Option<rusqlite::Connection>, InstallError> {
    match catalogue {
        Some(p) => specular_library::ai::open(p)
            .map(Some)
            .map_err(|e| InstallError::Io(std::io::Error::other(e.to_string()))),
        None => Ok(None),
    }
}

impl Pack {
    /// `data_dir` is the platform data directory; `catalogue` the library's file (None: no
    /// library, the tables come when it is opened with the pack installed). Leftovers of
    /// interrupted work are tidied.
    pub fn new(
        manifest: Manifest,
        data_dir: &std::path::Path,
        catalogue: Option<PathBuf>,
        transports: Transports,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Pack {
        let store = Arc::new(Store::new(data_dir));
        let upgraded = store.adopt_tiers(&manifest);
        store.tidy(&manifest);
        Pack {
            upgraded,
            manifest,
            store,
            transports: Arc::new(transports),
            catalogue,
            placeholders: false,
            running: Mutex::new(HashMap::new()),
            failed: Arc::default(),
            removing_all: Mutex::new(None),
            last_installed: Mutex::new(Vec::new()),
            others: Mutex::new(Vec::new()),
            finished: Arc::default(),
            changes: Arc::default(),
            wake,
            addons: Mutex::new(HashMap::new()),
            addon_failed: Arc::default(),
        }
    }

    /// Looks for model files in `folders` (placed by an administrator) before downloading.
    /// Call right after [`Pack::new`].
    pub fn with_model_folders(mut self, folders: Vec<PathBuf>) -> Pack {
        let root = self.store.root().to_owned();
        self.store = Arc::new(Store::at(root).with_folders(folders));
        self
    }

    /// Looks at model files copied to this computer (files, or folders of them): each known
    /// one is copied in, ready for its feature's install without the network; the others
    /// come back for the app to refuse or (a known name) to ask about.
    pub fn sideload(&self, paths: &[PathBuf]) -> Sideload {
        let mut out = Sideload::default();
        for p in crate::sideload::files_in(paths) {
            match crate::sideload::identify(&self.manifest, &p) {
                Ok(c) => match c.found {
                    crate::sideload::Match::Verified(m) => match self.store.sideload(&m, &p) {
                        Ok(()) => out.ready.push((p, m.feature)),
                        Err(e) => out.failed.push((p, e.to_string())),
                    },
                    crate::sideload::Match::Unverified(m) => out.unverified.push((p, m)),
                    crate::sideload::Match::Unknown => out.unknown.push(p),
                },
                Err(e) => out.failed.push((p, e.to_string())),
            }
        }
        if !out.ready.is_empty() {
            (self.wake)();
        }
        out
    }

    /// Copies `path` in as model `model` without a hash check: the user confirmed using an
    /// unverified model (named to them first).
    pub fn sideload_unverified(&self, model: &Model, path: &std::path::Path) -> Result<(), String> {
        let r = self
            .store
            .sideload_unverified(model, path)
            .map_err(|e| e.to_string());
        (self.wake)();
        r
    }

    /// Whether installing `feature` at `tier` needs the network: some file of it is
    /// neither installed nor here to install (sideloaded, an admin folder's).
    pub fn needs_network(&self, feature: Feature, tier: Tier) -> bool {
        let Some(offer) = self
            .manifest
            .offer(feature, self.placeholders)
            .and_then(|o| o.with_tier(tier))
        else {
            return false;
        };
        let st = self.store.state();
        offer
            .models
            .iter()
            .any(|m| !st.models.iter().any(|i| i.key() == m.key()) && !self.store.local(m))
    }

    /// Offer placeholder entries too (tests, development).
    pub fn with_placeholders(mut self) -> Pack {
        self.placeholders = true;
        self
    }

    /// Features found installed from before sizes existed when the pack was made (their
    /// switches were on by default then; the app turns them on once).
    pub fn upgraded(&self) -> &[Feature] {
        &self.upgraded
    }

    /// The library's catalogue the pack was made with.
    pub fn catalogue(&self) -> Option<&std::path::Path> {
        self.catalogue.as_deref()
    }

    /// Search's files of an installed size: the image and text encoder's weights, the
    /// tokenizer's merges, and the input normalisation the manifest gives (None: the size
    /// lacks one of them, or it is no longer in the manifest).
    pub fn clip_parts(&self, im: &IndexModel) -> Option<(Installed, Installed, Norm)> {
        let manifest_model = |m: &Installed| {
            self.manifest
                .models
                .iter()
                .find(|x| x.feature == m.feature && x.id == m.id && x.version == m.version)
        };
        let mut weights = None;
        let mut merges = None;
        for m in &im.models {
            let Some(mm) = manifest_model(m) else {
                continue;
            };
            match mm.role.as_str() {
                "tokenizer" => merges = Some(m.clone()),
                "image-text-encoder" => weights = Some((m.clone(), mm.runtime.norm)),
                _ => {}
            }
        }
        let (w, norm) = weights?;
        Some((w, merges?, norm))
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Whether this build offers any model at all.
    pub fn has_offers(&self) -> bool {
        !self.manifest.offers(self.placeholders).is_empty()
    }

    /// Features installed.
    pub fn installed(&self) -> Vec<Feature> {
        self.store.features()
    }

    pub fn installed_models(&self) -> Vec<Installed> {
        self.store.installed()
    }

    /// Whether any feature is installed.
    pub fn any_installed(&self) -> bool {
        !self.installed().is_empty()
    }

    /// Every feature, in order.
    pub fn status(&self) -> Vec<FeatureStatus> {
        self.poll();
        let st = self.store.state();
        let mut installed: Vec<Feature> = st.models.iter().map(|m| m.feature).collect();
        installed.sort();
        installed.dedup();
        *self.last_installed.lock() = installed.clone();
        let removing_all = {
            let mut all = self.removing_all.lock();
            if !running_all(&all) {
                *all = None;
            }
            all.as_ref().map(|(f, _)| f.clone()).unwrap_or_default()
        };
        let mut running = self.running.lock();
        // Reap finished installs and removals.
        running.retain(|_, r| match r {
            Running::Install(_, _, h) => !h.as_ref().is_some_and(|h| h.is_finished()),
            r @ Running::Remove(_) => !removed(r),
            Running::Failed(..) => true,
        });
        Feature::ALL
            .into_iter()
            .map(|feature| {
                let offer = self.manifest.offer(feature, self.placeholders);
                let mut target = None;
                let state = match running.get(&feature) {
                    _ if removing_all.contains(&feature) => FeatureState::Removing,
                    Some(Running::Remove(_)) => FeatureState::Removing,
                    Some(Running::Install(t, tier, _)) => {
                        target = Some(*tier);
                        let (done, total) = t.get();
                        FeatureState::Downloading { done, total }
                    }
                    Some(Running::Failed(e, tier)) if offer.is_some() => {
                        target = Some(*tier);
                        FeatureState::Failed(e.clone())
                    }
                    _ if installed.contains(&feature) => FeatureState::Installed,
                    _ if offer.is_some() => FeatureState::Available,
                    _ => FeatureState::Unavailable,
                };
                let rec = st.record(feature);
                FeatureStatus {
                    feature,
                    state,
                    offer,
                    tier: installed
                        .contains(&feature)
                        .then(|| st.tier(feature))
                        .flatten(),
                    target,
                    previous: rec.and_then(|r| r.previous),
                    basis: rec.and_then(|r| r.basis.clone()),
                }
            })
            .collect()
    }

    /// The installed state (sizes, previous sizes, models). Reads a small file.
    pub fn state(&self) -> State {
        self.store.state()
    }

    /// The models `feature` indexes with now, and those of the size before a switch (in use
    /// until the new size's index is complete, then [`Pack::finish_index`]).
    pub fn index_models(&self, feature: Feature) -> (Option<IndexModel>, Option<IndexModel>) {
        let st = self.store.state();
        let cur: Vec<Installed> = st
            .current()
            .into_iter()
            .filter(|m| m.feature == feature)
            .collect();
        let cur = IndexModel::new(st.tier(feature), cur);
        let prev = st
            .record(feature)
            .and_then(|r| r.previous)
            .and_then(|p| IndexModel::new(Some(p), st.previous(feature)));
        (cur, prev)
    }

    /// Bytes installing `feature` at `tier` would download (files already installed for
    /// another size are not downloaded again). None: no such size.
    pub fn download_size(&self, feature: Feature, tier: Tier) -> Option<u64> {
        let offer = self
            .manifest
            .offer(feature, self.placeholders)?
            .with_tier(tier)?;
        Some(self.store.to_download(&offer))
    }

    /// Bytes `feature` at `tier` takes on disk, its converted files as converted. None: no
    /// such size, or not all of it installed.
    pub fn disk_size(&self, feature: Feature, tier: Tier) -> Option<u64> {
        let offer = self
            .manifest
            .offer(feature, self.placeholders)?
            .with_tier(tier)?;
        self.store.on_disk(&offer)
    }

    /// Forgets a failed install's reason (the row shows its installed size again).
    pub fn dismiss(&self, feature: Feature) {
        self.poll();
        let mut running = self.running.lock();
        if matches!(running.get(&feature), Some(Running::Failed(..))) {
            running.remove(&feature);
        }
    }

    /// Starts downloading and installing `feature` at size `tier` (the user opted in, or
    /// chose another size). False when that size is not offered, the feature is already
    /// downloading, or being removed.
    ///
    /// Search, which must re-index with a new size, switches sizes keeping the old one
    /// installed and in use until [`Pack::finish_index`]; the other features replace it once
    /// the new size is installed. On failure the installed size stays as it was.
    pub fn install(&self, feature: Feature, tier: Tier) -> bool {
        self.install_for(feature, tier, None)
    }

    /// [`Pack::install`], recording what the size was chosen on (`basis`, the app's words:
    /// which GPU, calibrated or not) to notice later when the machine changed.
    pub fn install_for(&self, feature: Feature, tier: Tier, basis: Option<String>) -> bool {
        let Some(offer) = self
            .manifest
            .offer(feature, self.placeholders)
            .and_then(|o| o.with_tier(tier))
        else {
            return false;
        };
        if running_all(&self.removing_all.lock()) {
            return false;
        }
        let mut running = self.running.lock();
        if matches!(
            running.get(&feature),
            Some(Running::Install(..) | Running::Remove(_))
        ) {
            return false;
        }
        let how = Install {
            keep_previous: feature == Feature::Search,
            basis,
        };
        let transfer = Arc::new(Transfer::default());
        transfer
            .total
            .store(offer.size(), std::sync::atomic::Ordering::Relaxed);
        let (store, transports, catalogue, wake) = (
            self.store.clone(),
            self.transports.clone(),
            self.catalogue.clone(),
            self.wake.clone(),
        );
        let t = transfer.clone();
        let failed = self.failed.clone();
        let handle = specular_thread::builder(format!("ai-get-{feature}"))
            .spawn(move || {
                // In the background-work panel while it downloads, in bytes.
                let work = specular_core::work::Work::start(
                    specular_core::work::Kind::Download,
                    specular_core::work::Step::Downloading,
                );
                work.set_unit(specular_core::work::Unit::Bytes);
                let tw = t.clone();
                work.follow(move || {
                    let (done, total) = tw.get();
                    (done, (total > 0).then_some(total))
                });
                // A panic ends the install as failed (shown on the feature's row) rather
                // than with the row back at "available" and no word why.
                let result = specular_core::isolate::run(format!("ai: install {feature}"), || {
                    if let Some(c) = &catalogue {
                        specular_core::isolate::trip(c);
                    }
                    let mut conn =
                        match &catalogue {
                            Some(p) => Some(specular_library::ai::open(p).map_err(|e| {
                                InstallError::Io(std::io::Error::other(e.to_string()))
                            })?),
                            None => None,
                        };
                    store.install_with(&offer, transports.as_ref(), &t, conn.as_mut(), &how)
                });
                drop(work);
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: install {feature}: {e}");
                        failed.lock().insert(feature, (e.to_string(), tier));
                    }
                    Err(_) => {
                        failed.lock().insert(feature, (INTERNAL_ERROR.to_owned(), tier));
                    }
                }
                wake();
            })
            .ok();
        running.insert(feature, Running::Install(transfer, tier, handle));
        drop(running);
        (self.wake)();
        true
    }

    /// Picks up install failures reported by the threads (call before [`Pack::status`]).
    pub fn poll(&self) {
        let failed: Vec<(Feature, (String, Tier))> = self.failed.lock().drain().collect();
        if failed.is_empty() {
            return;
        }
        let mut running = self.running.lock();
        for (f, (e, tier)) in failed {
            // An install cancelled by a removal: the removal's entry stays.
            if matches!(running.get(&f), Some(Running::Remove(_))) {
                continue;
            }
            if e != "cancelled" {
                running.insert(f, Running::Failed(e, tier));
            } else {
                running.remove(&f);
            }
        }
    }

    /// Stops a download (what came is kept to resume).
    pub fn cancel(&self, feature: Feature) {
        if let Some(Running::Install(t, ..)) = self.running.lock().get(&feature) {
            t.cancel();
        }
    }

    /// Waits for every install, removal and People thread (tests, quit).
    pub fn join(&self) {
        let mut handles: Vec<_> = self
            .running
            .lock()
            .values_mut()
            .filter_map(|r| match r {
                Running::Install(_, _, h) | Running::Remove(h) => h.take(),
                Running::Failed(..) => None,
            })
            .collect();
        if let Some((_, h)) = self.removing_all.lock().as_mut() {
            handles.extend(h.take());
        }
        handles.append(&mut self.others.lock());
        for h in handles {
            let _ = h.join();
        }
        self.poll();
        self.running
            .lock()
            .retain(|_, r| !matches!(r, Running::Install(_, _, None) | Running::Remove(None)));
        self.removing_all.lock().take_if(|(_, h)| h.is_none());
    }

    /// Runs `op` on a thread of its own; how it ended goes to [`Pack::take_finished`], then
    /// [`Pack::changes`] moves and the wake callback runs.
    fn spawn(
        &self,
        thread: String,
        what: String,
        op: impl FnOnce() -> Result<(), InstallError> + Send + 'static,
    ) -> Option<JoinHandle<()>> {
        let (finished, changes, wake) = (
            self.finished.clone(),
            self.changes.clone(),
            self.wake.clone(),
        );
        specular_thread::builder(thread)
            .spawn(move || {
                // A panic is reported like any failure (and the waiting state ends).
                let r = match specular_core::isolate::run(format!("ai: {what}"), op) {
                    Ok(r) => r.map_err(|e| e.to_string()),
                    Err(_) => Err(INTERNAL_ERROR.to_owned()),
                };
                finished.lock().push((what, r));
                changes.fetch_add(1, Ordering::AcqRel);
                wake();
            })
            .ok()
    }

    /// Removals and People changes that ended since the last call (what, and how).
    pub fn take_finished(&self) -> Vec<Finished> {
        std::mem::take(&mut *self.finished.lock())
    }

    /// Moves whenever a background removal or People change ends (rebuild what shows then).
    pub fn changes(&self) -> u64 {
        self.changes.load(Ordering::Acquire)
    }

    /// Whether a removal (of a feature or of the pack) is running.
    pub fn removing(&self) -> bool {
        running_all(&self.removing_all.lock())
            || self
                .running
                .lock()
                .values()
                .any(|r| matches!(r, Running::Remove(_)) && !removed(r))
    }

    fn conn(&self) -> Result<Option<rusqlite::Connection>, InstallError> {
        open_conn(self.catalogue.as_deref())
    }

    /// Starts removing a feature on a background thread (`ai-rm-<feature>`): its models,
    /// its tables and everything in them; a download of it is cancelled first. Returns at
    /// once; false when it is already being removed. The feature shows
    /// [`FeatureState::Removing`] until the thread ends.
    pub fn remove(&self, feature: Feature) -> bool {
        if running_all(&self.removing_all.lock()) {
            return false;
        }
        let mut running = self.running.lock();
        let install = match running.remove(&feature) {
            Some(r @ Running::Remove(Some(_))) if !removed(&r) => {
                running.insert(feature, r);
                return false;
            }
            Some(Running::Install(t, _, h)) => {
                t.cancel();
                h
            }
            _ => None,
        };
        let (store, catalogue) = (self.store.clone(), self.catalogue.clone());
        let h = self.spawn(
            format!("ai-rm-{feature}"),
            format!("remove {feature}"),
            move || {
                if let Some(h) = install {
                    let _ = h.join();
                }
                if let Some(c) = &catalogue {
                    specular_core::isolate::trip(c);
                }
                let mut conn = open_conn(catalogue.as_deref())?;
                store.remove(feature, conn.as_mut())
            },
        );
        running.insert(feature, Running::Remove(h));
        drop(running);
        (self.wake)();
        true
    }

    /// Starts removing the whole pack on a background thread (`ai-rm-all`), once every
    /// download is cancelled and every running install or removal has ended. Returns at once;
    /// false when it is already being removed.
    pub fn remove_all(&self) -> bool {
        let mut all = self.removing_all.lock();
        if running_all(&all) {
            return false;
        }
        let mut features = self.last_installed.lock().clone();
        let mut waits = Vec::new();
        for (f, r) in self.running.lock().drain() {
            match r {
                Running::Install(t, _, h) => {
                    t.cancel();
                    waits.extend(h);
                    features.push(f);
                }
                Running::Remove(h) => {
                    waits.extend(h);
                    features.push(f);
                }
                Running::Failed(..) => {}
            }
        }
        features.sort();
        features.dedup();
        let (store, catalogue) = (self.store.clone(), self.catalogue.clone());
        let h = self.spawn("ai-rm-all".into(), "remove pack".into(), move || {
            for h in waits {
                let _ = h.join();
            }
            let mut conn = open_conn(catalogue.as_deref())?;
            store.remove_all(conn.as_mut())
        });
        *all = Some((features, h));
        drop(all);
        (self.wake)();
        true
    }

    /// People known (empty without People or a catalogue).
    pub fn people(&self) -> Vec<specular_library::ai::Person> {
        let Ok(Some(conn)) = self.conn() else {
            return Vec::new();
        };
        if !specular_library::ai::has(&conn, "people").unwrap_or(false) {
            return Vec::new();
        }
        specular_library::ai::people(&conn).unwrap_or_default()
    }

    /// Whether faces are grouped into people in this library (None: no catalogue, or People
    /// not installed). Off until the user turns it on for the library.
    pub fn face_grouping(&self) -> Option<bool> {
        let conn = self.conn().ok()??;
        if !specular_library::ai::has(&conn, "people").unwrap_or(false) {
            return None;
        }
        specular_library::ai::face_grouping(&conn).ok()
    }

    /// Whether the text in this library's photos is read for search (None: no catalogue, or
    /// Text not installed).
    pub fn text_indexing(&self) -> Option<bool> {
        let conn = self.conn().ok()??;
        if !specular_library::ai::has(&conn, "text").unwrap_or(false) {
            return None;
        }
        specular_library::ai::text_indexing(&conn).ok()
    }

    /// Starts turning reading this library's text on or off (on an `ai-forget` thread;
    /// returns at once).
    pub fn set_text_indexing(&self, on: bool) {
        let catalogue = self.catalogue.clone();
        let h = self.spawn(
            "ai-forget".into(),
            format!("text indexing {}", if on { "on" } else { "off" }),
            move || {
                if let Some(conn) = open_conn(catalogue.as_deref())? {
                    specular_library::ai::set_text_indexing(&conn, on)?;
                }
                Ok(())
            },
        );
        let mut others = self.others.lock();
        others.retain(|h| !h.is_finished());
        others.extend(h);
    }

    /// Starts turning grouping faces into people on or off for this library (on an
    /// `ai-forget` thread; returns at once).
    pub fn set_face_grouping(&self, on: bool) {
        let catalogue = self.catalogue.clone();
        let h = self.spawn(
            "ai-forget".into(),
            format!("face grouping {}", if on { "on" } else { "off" }),
            move || {
                if let Some(conn) = open_conn(catalogue.as_deref())? {
                    specular_library::ai::set_face_grouping(&conn, on)?;
                }
                Ok(())
            },
        );
        let mut others = self.others.lock();
        others.retain(|h| !h.is_finished());
        others.extend(h);
    }

    /// Starts forgetting one person and their faces (on an `ai-forget` thread; returns at
    /// once).
    pub fn forget_person(&self, id: i64) {
        let catalogue = self.catalogue.clone();
        let h = self.spawn(
            "ai-forget".into(),
            format!("forget person {id}"),
            move || {
                if let Some(mut conn) = open_conn(catalogue.as_deref())?
                    && specular_library::ai::has(&conn, "people")?
                {
                    specular_library::ai::forget_person(&mut conn, id)?;
                }
                Ok(())
            },
        );
        let mut others = self.others.lock();
        others.retain(|h| !h.is_finished());
        others.extend(h);
    }

    /// Starts forgetting every face and person (on an `ai-forget` thread; returns at once;
    /// the caller turns People off).
    pub fn forget_all_faces(&self) {
        let catalogue = self.catalogue.clone();
        let h = self.spawn("ai-forget".into(), "delete face data".into(), move || {
            if let Some(mut conn) = open_conn(catalogue.as_deref())?
                && specular_library::ai::has(&conn, "people")?
            {
                specular_library::ai::forget_all_faces(&mut conn)?;
            }
            Ok(())
        });
        let mut others = self.others.lock();
        others.retain(|h| !h.is_finished());
        others.extend(h);
    }

    /// Readable files and how many `feature`'s installed model has done.
    pub fn counts(&self, feature: Feature) -> Option<(u64, u64)> {
        let model = self.index_models(feature).0?.key;
        let conn = self.conn().ok()??;
        specular_library::ai::counts(&conn, feature.id(), &model).ok()
    }

    /// The add-ons `feature` offers (in manifest order) and their states.
    pub fn addon_status(&self, feature: Feature) -> Vec<AddonStatus> {
        let installed = self.store.addons(feature);
        let mut failed = self.addon_failed.lock();
        let mut runs = self.addons.lock();
        for (k, e) in failed.drain() {
            if !matches!(runs.get(&k), Some(AddonRun::Remove(_))) {
                if e == "cancelled" {
                    runs.remove(&k);
                } else {
                    runs.insert(k, AddonRun::Failed(e));
                }
            }
        }
        drop(failed);
        let mut out = Vec::new();
        for c in self.manifest.choices(feature) {
            let crate::manifest::Slot::Addon(id) = &c.slot else {
                continue;
            };
            if c.coming {
                continue;
            }
            let models = self.manifest.addon_models(feature, id, self.placeholders);
            if models.is_empty() {
                continue;
            }
            let size = models.iter().map(|m| m.size).sum();
            let key = (feature, id.clone());
            let state = match runs.get(&key) {
                Some(AddonRun::Install(t, h)) if h.as_ref().is_some_and(|h| !h.is_finished()) => {
                    let (done, total) = t.get();
                    FeatureState::Downloading { done, total }
                }
                Some(AddonRun::Remove(h)) if h.as_ref().is_some_and(|h| !h.is_finished()) => {
                    FeatureState::Removing
                }
                Some(AddonRun::Failed(e)) => FeatureState::Failed(e.clone()),
                _ if installed.contains(id) => FeatureState::Installed,
                _ => FeatureState::Available,
            };
            out.push(AddonStatus {
                feature,
                id: id.clone(),
                state,
                size,
            });
        }
        out
    }

    /// Starts downloading and installing one add-on (`ai-get-addon`); returns at
    /// once, false when it is not offered or already busy.
    pub fn install_addon(&self, feature: Feature, id: &str) -> bool {
        let models = self.manifest.addon_models(feature, id, self.placeholders);
        if models.is_empty() || running_all(&self.removing_all.lock()) {
            return false;
        }
        let key = (feature, id.to_owned());
        let mut runs = self.addons.lock();
        if matches!(
            runs.get(&key),
            Some(AddonRun::Install(_, Some(h)) | AddonRun::Remove(Some(h))) if !h.is_finished()
        ) {
            return false;
        }
        let transfer = Arc::new(Transfer::default());
        transfer
            .total
            .store(models.iter().map(|m| m.size).sum(), Ordering::Relaxed);
        let (store, transports, wake, failed) = (
            self.store.clone(),
            self.transports.clone(),
            self.wake.clone(),
            self.addon_failed.clone(),
        );
        let t = transfer.clone();
        let k = key.clone();
        let handle = specular_thread::builder("ai-get-addon")
            .spawn(move || {
                let work = specular_core::work::Work::start(
                    specular_core::work::Kind::Download,
                    specular_core::work::Step::Downloading,
                );
                work.set_unit(specular_core::work::Unit::Bytes);
                let tw = t.clone();
                work.follow(move || {
                    let (done, total) = tw.get();
                    (done, (total > 0).then_some(total))
                });
                let result = specular_core::isolate::run(
                    format!("ai: install {feature} add-on {}", k.1),
                    || store.install_addon(&models, transports.as_ref(), &t),
                );
                drop(work);
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: install {feature} add-on {}: {e}", k.1);
                        failed.lock().insert(k, e.to_string());
                    }
                    Err(_) => {
                        failed.lock().insert(k, INTERNAL_ERROR.to_owned());
                    }
                }
                wake();
            })
            .ok();
        runs.insert(key, AddonRun::Install(transfer, handle));
        drop(runs);
        (self.wake)();
        true
    }

    /// Stops an add-on's download (what came is kept to resume).
    pub fn cancel_addon(&self, feature: Feature, id: &str) {
        if let Some(AddonRun::Install(t, _)) = self.addons.lock().get(&(feature, id.to_owned())) {
            t.cancel();
        }
    }

    /// Starts removing one add-on (its files; text already found stays) on an
    /// `ai-rm-addon` thread. Returns at once.
    pub fn remove_addon(&self, feature: Feature, id: &str) -> bool {
        let key = (feature, id.to_owned());
        let mut runs = self.addons.lock();
        let install = match runs.remove(&key) {
            Some(AddonRun::Remove(Some(h))) if !h.is_finished() => {
                runs.insert(key, AddonRun::Remove(Some(h)));
                return false;
            }
            Some(AddonRun::Install(t, h)) => {
                t.cancel();
                h
            }
            _ => None,
        };
        let store = self.store.clone();
        let addon = id.to_owned();
        let h = self.spawn(
            "ai-rm-addon".into(),
            format!("remove {feature} add-on {id}"),
            move || {
                if let Some(h) = install {
                    let _ = h.join();
                }
                store.remove_addon(feature, &addon)
            },
        );
        runs.insert(key, AddonRun::Remove(h));
        drop(runs);
        (self.wake)();
        true
    }

    /// The Text feature's installed files, when it is installed.
    pub fn ocr_files(&self) -> Option<crate::ocr::OcrFiles> {
        crate::ocr::OcrFiles::find(&self.manifest, self.store.root(), &self.store.installed())
    }

    /// Ends a feature's re-index with `keep` (an [`IndexModel::key`]), on an `ai-sw-<feature>`
    /// thread: results of every other model are deleted (Search's embeddings of an earlier
    /// size, version or schema), then the size installed before a switch, with its files.
    /// Call once the index under `keep` is complete. Returns at once.
    pub fn finish_index(&self, feature: Feature, keep: String) {
        let (store, catalogue) = (self.store.clone(), self.catalogue.clone());
        let h = self.spawn(
            format!("ai-sw-{feature}"),
            format!("finish {feature} index"),
            move || {
                if feature == Feature::Search
                    && let Some(conn) = open_conn(catalogue.as_deref())?
                    && specular_library::ai::has(&conn, "search")?
                {
                    let n = specular_library::ai::prune_embeddings(&conn, &[keep.as_str()])?;
                    if n > 0 {
                        specular_log::info!(
                            target: specular_log::JOBS,
                            "ai: {n} embeddings of other models deleted"
                        );
                    }
                }
                store.finish_switch(feature)
            },
        );
        let mut others = self.others.lock();
        others.retain(|h| !h.is_finished());
        others.extend(h);
    }
}
