//! Develop in the gallery and the Library:
//!
//! - **Edited thumbnails.** A photo with a Specular edit shows it developed: the listing's
//!   edited photos are found by their sidecars (the Library's by its rows), their edits read
//!   in the background ([`crate::app::develop::edited::facts_of`]), and their thumbnails
//!   made by the edited-thumbnail worker, tiles on screen first. Until it is ready the
//!   ordinary thumbnail stays; unedited photos never wait for anything new.
//! - **Facts** for `edited:`, `module:`, `style:` and `version:` in the folder's filter
//!   (the Library searches its catalogue).
//! - **Badges**: "Edited" and "N versions" (gallery view menu › Edited badges).
//! - **Versions** (`IMG.CR2.v2.specular.xmp`): each its own entry, keyed by its sidecar's
//!   path, stacked under its photo like a RAW+JPEG pair; opening one opens the photo in
//!   Develop at that version. File operations never touch them (the photo's do).
//! - **Re-develop**: a save (an edit, a paste, a style, a sync) reads the photo's edits
//!   again, and the thumbnail of a changed edit is made again; the old one shows meanwhile.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_i18n::t;
use specular_thumbs::pixels::Pixels;
use specular_thumbs::{Entry, EntryKind};

use super::{Gallery, Msg};
use crate::app::App;
use crate::app::develop::edited::{self as dev, Facts, Hash, Job};

/// Seconds after a save before the photo's edits are read again (the catalogue writes on
/// the indexer, after the sidecar).
const RELOAD_AFTER_S: f64 = 0.4;

/// What the gallery knows of its listing's Develop edits.
#[derive(Default)]
pub(super) struct Edited {
    /// The listing's edited photos are known (unedited ones read as `edited:no`).
    pub listed: bool,
    /// Each edited key's facts: the photo's first version under its path, the others under
    /// their sidecar's ([`dev::version_key`]).
    pub facts: HashMap<PathBuf, Facts>,
    /// Version keys → their photo.
    pub photo_of: HashMap<PathBuf, PathBuf>,
    /// Edited thumbnails made: key → (the edit's hash, the classes smallest first).
    pub thumbs: HashMap<PathBuf, (Hash, [Arc<Pixels>; 3])>,
    /// Keys whose thumbnail was asked for, with the hash asked.
    asked: HashMap<PathBuf, Hash>,
    /// Keys whose thumbnail just changed: the atlas drops the old one as the new goes in.
    pub swap: HashSet<PathBuf>,
    /// Photos saved, and when to read them again.
    reload: Vec<(PathBuf, f64)>,
    pub worker: dev::EditedThumbs,
}

impl Edited {
    /// Forgets the listing (another folder or search).
    pub fn clear(&mut self) {
        self.listed = false;
        self.facts.clear();
        self.photo_of.clear();
        self.thumbs.clear();
        self.asked.clear();
        self.swap.clear();
        self.worker.clear();
    }

    /// Asks again for the raws' edited thumbnails (their develop size changed: Settings ›
    /// Gallery › Sharper raw thumbnails); the ones shown stay until the new ones come.
    /// Other images' thumbnails do not depend on it.
    pub fn ask_raws_again(&mut self) {
        let photo_of = &self.photo_of;
        self.asked.retain(|k, _| {
            let image = photo_of.get(k).unwrap_or(k);
            !specular_develop::thumb::may_be_raw(image)
        });
    }

    /// Whether `key` (a photo or a version) has an edit.
    pub fn is_edited(&self, key: &Path) -> bool {
        self.facts.contains_key(key)
    }

    /// The photo a version key is of (None for anything else).
    pub fn photo(&self, key: &Path) -> Option<&Path> {
        self.photo_of.get(key).map(PathBuf::as_path)
    }

    /// The edited thumbnail of `key` in the class nearest `class` (not smaller when there is
    /// a larger one).
    pub fn pixels(&self, key: &Path, class: u32) -> Option<(u32, Arc<Pixels>)> {
        let (_, px) = self.thumbs.get(key)?;
        let k = specular_thumbs::CLASSES
            .iter()
            .position(|&c| c >= class)
            .unwrap_or(2);
        Some((specular_thumbs::CLASSES[k], px[k].clone()))
    }
}

/// The version number in a version key (`IMG.CR2.v2.specular.xmp` → 2).
fn version_of_key(p: &Path) -> u32 {
    p.to_str()
        .and_then(|s| s.strip_suffix(specular_develop::sidecar::SUFFIX))
        .and_then(|s| s.rsplit_once(".v"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0)
}

/// Moves each version entry's group under its photo's (a stack, like RAW+JPEG): the photo's
/// group gets the versions as members after its own. A version whose photo is filtered out
/// stays on its own.
pub(super) fn group_versions(
    groups: Vec<(usize, Vec<usize>)>,
    entries: &[Entry],
    photo_of: &HashMap<PathBuf, PathBuf>,
) -> Vec<(usize, Vec<usize>)> {
    if photo_of.is_empty() {
        return groups;
    }
    // The group of each photo shown.
    let mut group_of: HashMap<&Path, usize> = HashMap::new();
    for (g, (_, members)) in groups.iter().enumerate() {
        for &m in members {
            group_of.insert(entries[m].path.as_path(), g);
        }
    }
    let mut extra: Vec<Vec<usize>> = vec![Vec::new(); groups.len()];
    let mut moved = vec![false; groups.len()];
    for (g, (shown, members)) in groups.iter().enumerate() {
        let Some(photo) = photo_of.get(&entries[*shown].path) else {
            continue;
        };
        if members.len() != 1 {
            continue;
        }
        if let Some(&pg) = group_of.get(photo.as_path())
            && pg != g
        {
            extra[pg].push(*shown);
            moved[g] = true;
        }
    }
    groups
        .into_iter()
        .zip(extra)
        .zip(moved)
        .filter(|(_, moved)| !moved)
        .map(|(((shown, mut members), mut extra), _)| {
            if !extra.is_empty() {
                // Versions in their number's order.
                extra.sort_by_key(|&i| version_of_key(&entries[i].path));
                members.extend(extra);
            }
            (shown, members)
        })
        .collect()
}

/// The query's facts of entry `e` with its Develop edits (`edited:`, `module:`, `style:`,
/// `version:`); `versions` is the gallery's count of each photo's versions.
pub(super) fn develop_facts<'a>(
    ed: &'a Edited,
    versions: &HashMap<PathBuf, u32>,
    e: &'a Entry,
    mut f: specular_query::Facts<'a>,
) -> specular_query::Facts<'a> {
    f.is_version = ed.photo_of.contains_key(&e.path);
    let photo = ed.photo(&e.path).unwrap_or(&e.path);
    f.versions = versions.get(photo).copied().unwrap_or(0);
    match ed.facts.get(&e.path) {
        Some(x) => {
            f.edited = Some(true);
            f.modules = Some(&x.modules);
            f.styles = Some(&x.styles);
        }
        None if ed.listed && e.kind == EntryKind::Image => {
            f.edited = Some(false);
            f.modules = Some("");
            f.styles = Some("");
        }
        None => {}
    }
    f
}

impl Gallery {
    /// The badges of `e`'s tile: "Edited" (when the badges are on).
    pub(super) fn develop_badge(&self, e: &Entry) -> Option<String> {
        (self.edited_badges && self.edited.is_edited(&e.path)).then(|| t!("gallery-badge-edited"))
    }

    /// The screen reader's words for `e`'s edits: "Edited", "Version 2", "3 versions".
    pub(super) fn develop_a11y(&self, e: &Entry) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(f) = self.edited.facts.get(&e.path) {
            if self.edited.photo_of.contains_key(&e.path) {
                v.push(specular_i18n::isolate(&dev::version_label(
                    f.version, &f.name,
                )));
            }
            v.push(t!("gallery-a11y-edited"));
        }
        if let Some(&n) = self.versions.get(&e.path) {
            v.push(t!("gallery-badge-versions", count = i64::from(n)));
        }
        v
    }

    /// Adds an entry for each version above the first of the edited photos listed (keyed by
    /// its sidecar's path, with the photo's facts), and drops those of versions gone.
    pub(super) fn develop_version_entries(&mut self) {
        let want: Vec<(PathBuf, PathBuf, u32, String)> = self
            .edited
            .facts
            .iter()
            .filter(|(k, _)| self.edited.photo_of.contains_key(*k))
            .filter_map(|(k, f)| {
                let photo = self.edited.photo_of.get(k)?;
                self.by_path
                    .contains_key(photo.as_path())
                    .then(|| (k.clone(), photo.clone(), f.version, f.name.clone()))
            })
            .collect();
        let stale: HashSet<PathBuf> = self
            .entries
            .iter()
            .filter(|e| {
                e.path
                    .to_str()
                    .is_some_and(|s| s.ends_with(specular_develop::sidecar::SUFFIX))
                    && !self.edited.facts.contains_key(&e.path)
            })
            .map(|e| e.path.clone())
            .collect();
        let mut changed = !stale.is_empty();
        if changed {
            let kept: Vec<bool> = self
                .entries
                .iter()
                .map(|e| !stale.contains(&e.path))
                .collect();
            self.drop_tiles_of(&kept);
            self.entries.retain(|e| !stale.contains(&e.path));
            self.reindex();
        }
        for (key, photo, version, name) in want {
            if self.by_path.contains_key(key.as_path()) {
                continue;
            }
            let Some(&pi) = self.by_path.get(photo.as_path()) else {
                continue;
            };
            let p = self.entries[pi].clone();
            let label = dev::version_label(version, &name);
            self.entries.push(Entry {
                path: key.clone(),
                // "IMG_0001.CR2 · Black & white": each part keeps its own direction.
                name: specular_i18n::dot_join(&[p.name.clone(), label]),
                kind: EntryKind::Image,
                len: p.len,
                modified: p.modified,
                hidden: p.hidden,
                taken: p.taken,
                dims: p.dims,
            });
            // The photo's colour stands in until the version's thumbnail is made.
            if let Some(a) = self.averages.get(photo.as_path()).copied() {
                let k = super::shared_in(&self.by_path, &self.metas, &self.averages, &key);
                self.averages.insert(k, a);
            }
            if let Some(m) = self.metas.get(photo.as_path()).cloned() {
                let k = super::shared_in(&self.by_path, &self.metas, &self.averages, &key);
                self.metas.insert(k, m);
            }
            changed = true;
        }
        if changed {
            self.reindex();
            self.list_appended = None;
            self.resort();
        }
        self.dirty = true;
    }
}

impl App {
    /// Whether the gallery knows `p` to have a Develop edit (None: its listing's edits are
    /// not known).
    pub(in crate::app) fn gallery_edited(&self, p: &Path) -> Option<bool> {
        let ed = &self.gallery.edited;
        (ed.listed || ed.facts.contains_key(p)).then(|| ed.facts.contains_key(p))
    }

    /// How many Develop versions the gallery knows `p` to have (more than one).
    pub(in crate::app) fn gallery_versions(&self, p: &Path) -> Option<u32> {
        self.gallery.versions.get(p).copied()
    }

    /// The probe found the listing's edited photos: their edits are read in the background.
    pub(super) fn gallery_edited_listed(&mut self, generation: u64, photos: Vec<PathBuf>) {
        let ga = &mut self.gallery;
        if generation != ga.generation {
            return;
        }
        ga.edited.listed = true;
        ga.dirty = true;
        // Photos whose sidecars went (a refresh) are no longer edited.
        let keep: HashSet<&Path> = photos.iter().map(PathBuf::as_path).collect();
        let gone: Vec<PathBuf> = ga
            .edited
            .facts
            .keys()
            .filter(|k| !keep.contains(ga.edited.photo(k).unwrap_or(k)))
            .cloned()
            .collect();
        if !gone.is_empty() {
            let gone_photos: Vec<(PathBuf, Vec<Facts>)> = gone
                .into_iter()
                .filter(|k| !ga.edited.photo_of.contains_key(k))
                .map(|k| (k, Vec::new()))
                .collect();
            self.gallery_edited_facts(generation, gone_photos);
        }
        crate::app::develop::edited::publish_module_names();
        self.gallery_edited_read(photos);
    }

    /// Reads the edits of `photos` (every version) in the background.
    pub(super) fn gallery_edited_read(&mut self, photos: Vec<PathBuf>) {
        if photos.is_empty() {
            return;
        }
        self.develop_publish_style_names();
        let lib = self.gallery.library_handle();
        let tx = self.gallery.tx.clone();
        let wake = self.loader.waker();
        let generation = self.gallery.generation;
        self.loader.background("gal-develop", move |_| {
            let mut batch = Vec::new();
            for p in photos {
                let f = dev::facts_of(&p, lib.as_deref());
                batch.push((p, f));
                if batch.len() >= 64 {
                    let _ = tx.send(Msg::DevelopFacts {
                        generation,
                        facts: std::mem::take(&mut batch),
                    });
                    wake();
                }
            }
            let _ = tx.send(Msg::DevelopFacts {
                generation,
                facts: batch,
            });
            wake();
        });
    }

    /// Edits read: the facts, versions and their entries follow.
    pub(super) fn gallery_edited_facts(
        &mut self,
        generation: u64,
        facts: Vec<(PathBuf, Vec<Facts>)>,
    ) {
        let ga = &mut self.gallery;
        if generation != ga.generation {
            return;
        }
        for (photo, versions) in facts {
            // What this photo had goes (a version deleted, an edit reset).
            let old: Vec<PathBuf> = ga
                .edited
                .photo_of
                .iter()
                .filter(|(_, p)| **p == photo)
                .map(|(k, _)| k.clone())
                .chain(std::iter::once(photo.clone()))
                .collect();
            for k in &old {
                ga.edited.facts.remove(k);
                ga.edited.photo_of.remove(k);
            }
            let n = versions.len() as u32;
            if n > 1 {
                ga.versions.insert(photo.clone(), n);
            } else {
                ga.versions.remove(&photo);
            }
            for f in versions {
                let key = dev::version_key(&photo, f.version);
                if f.version > 0 {
                    ga.edited.photo_of.insert(key.clone(), photo.clone());
                }
                ga.edited.facts.insert(key, f);
            }
            // Keys no longer edited show their ordinary thumbnail again.
            for k in old {
                if !ga.edited.facts.contains_key(&k) && ga.edited.thumbs.remove(&k).is_some() {
                    ga.edited.asked.remove(&k);
                    if let Some(a) = ga.atlas.as_mut() {
                        a.remove(&k);
                    }
                }
            }
        }
        ga.develop_version_entries();
        ga.wish_key = 0;
    }

    /// Once per turn: saves to read again, thumbnails made, thumbnails to ask for.
    pub(super) fn gallery_edited_tick(&mut self, now: f64) -> bool {
        let mut changed = false;
        // Saves (Develop, pastes, styles, syncs): read again shortly.
        for p in dev::take_changed() {
            let ga = &mut self.gallery;
            ga.edited.reload.retain(|(q, _)| *q != p);
            ga.edited.reload.push((p, now + RELOAD_AFTER_S));
        }
        let due: Vec<PathBuf> = {
            let r = &mut self.gallery.edited.reload;
            let (due, wait): (Vec<_>, Vec<_>) = r.drain(..).partition(|(_, t)| *t <= now);
            *r = wait;
            due.into_iter().map(|(p, _)| p).collect()
        };
        let listed: Vec<PathBuf> = due
            .into_iter()
            .filter(|p| self.gallery.by_path.contains_key(p.as_path()))
            .collect();
        if !listed.is_empty() {
            self.gallery_edited_read(listed);
        }
        // Thumbnails made.
        let ga = &mut self.gallery;
        for d in ga.edited.worker.take() {
            let current = ga.edited.facts.get(&d.key).map(|f| f.hash);
            match d.pixels {
                Ok(px) if current == Some(d.hash) => {
                    ga.edited.thumbs.insert(d.key.clone(), (d.hash, px));
                    ga.edited.swap.insert(d.key);
                    changed = true;
                }
                Ok(_) => {}
                // Not developed (a decoder or the GPU said no): the ordinary thumbnail
                // stays; not asked again for this edit.
                Err(_) => {}
            }
        }
        changed
    }

    /// Asks for the edited thumbnails of the tiles in `visible` (and their stacks) that are
    /// not made for their current edit; the worker makes those on screen first.
    pub(super) fn gallery_edited_want(&mut self, visible: std::ops::Range<usize>) {
        let ga = &mut self.gallery;
        if ga.edited.facts.is_empty() {
            return;
        }
        if !ga.edited.worker.started() {
            let Some(engine) = ga.engine.as_ref() else {
                return;
            };
            let Some(gpu) = self.gfx.as_ref().map(|g| &g.gpu) else {
                return;
            };
            let integrated = gpu.info.device_type == specular_gfx::wgpu::DeviceType::IntegratedGpu
                || gpu.is_software();
            let cache = engine.cache().cloned();
            ga.edited.worker.start(
                &gpu.device,
                &gpu.queue,
                integrated,
                self.loader.registry(),
                cache,
                self.loader.waker(),
            );
        }
        let mut keys = Vec::new();
        for i in visible {
            let Some(t) = ga.tiles.get(i) else { break };
            for m in std::iter::once(t.entry).chain(t.members.iter().copied()) {
                if let Some(e) = ga.entries.get(m)
                    && ga.edited.facts.contains_key(&e.path)
                {
                    keys.push(e.path.clone());
                }
            }
        }
        keys.dedup();
        for k in &keys {
            let Some(f) = ga.edited.facts.get(k) else {
                continue;
            };
            if ga.edited.asked.get(k) == Some(&f.hash) {
                continue;
            }
            let image = ga.edited.photo(k).unwrap_or(k).to_path_buf();
            ga.edited.asked.insert(k.clone(), f.hash);
            ga.edited.worker.ask(Job {
                key: k.clone(),
                image,
                hash: f.hash,
                edit: f.edit.clone(),
            });
        }
        ga.edited.worker.set_visible(keys);
    }

    /// Opening a version's tile: the photo in the viewer, in Develop at that version.
    /// False for any other entry.
    pub(super) fn gallery_open_version(&mut self, key: &Path) -> bool {
        let Some(photo) = self.gallery.edited.photo(key).map(Path::to_path_buf) else {
            return false;
        };
        let version = self.gallery.edited.facts.get(key).map_or(0, |f| f.version);
        let Some(i) = self.gallery.by_path.get(photo.as_path()).copied() else {
            return true;
        };
        let Some(t) = self.gallery.entry_tile.get(i).copied() else {
            return true;
        };
        self.gallery_open(t as usize);
        self.develop_open_at(&photo, version);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(p: &str) -> Entry {
        Entry {
            path: PathBuf::from(p),
            name: Path::new(p)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            kind: EntryKind::Image,
            len: 1,
            modified: None,
            hidden: false,
            taken: None,
            dims: None,
        }
    }

    #[test]
    fn versions_stack_under_their_photo() {
        let entries = vec![
            entry("/p/a.cr2"),
            entry("/p/a.cr2.v2.specular.xmp"),
            entry("/p/a.jpg"),
            entry("/p/b.jpg"),
            entry("/p/b.jpg.v3.specular.xmp"),
            entry("/p/b.jpg.v2.specular.xmp"),
            entry("/p/c.jpg.v2.specular.xmp"),
        ];
        let photo_of: HashMap<PathBuf, PathBuf> = [
            ("/p/a.cr2.v2.specular.xmp", "/p/a.cr2"),
            ("/p/b.jpg.v3.specular.xmp", "/p/b.jpg"),
            ("/p/b.jpg.v2.specular.xmp", "/p/b.jpg"),
            ("/p/c.jpg.v2.specular.xmp", "/p/c.jpg"),
        ]
        .into_iter()
        .map(|(a, b)| (PathBuf::from(a), PathBuf::from(b)))
        .collect();
        // a.cr2 and a.jpg are a RAW+JPEG stack; c.jpg is filtered out.
        let groups = vec![
            (0, vec![0, 2]),
            (1, vec![1]),
            (3, vec![3]),
            (4, vec![4]),
            (5, vec![5]),
            (6, vec![6]),
        ];
        let g = group_versions(groups, &entries, &photo_of);
        assert_eq!(
            g,
            vec![(0, vec![0, 2, 1]), (3, vec![3, 5, 4]), (6, vec![6])]
        );
        // Nothing to do without versions.
        let plain = vec![(0, vec![0]), (3, vec![3])];
        assert_eq!(
            group_versions(plain.clone(), &entries, &HashMap::new()),
            plain
        );
    }

    /// Switching Sharper raw thumbnails asks again for raws and their versions only.
    #[test]
    fn a_new_raw_size_asks_again_for_raws_only() {
        let mut g = Edited::default();
        let version = PathBuf::from("/p/a.cr2.v2.specular.xmp");
        g.photo_of
            .insert(version.clone(), PathBuf::from("/p/a.cr2"));
        g.photo_of.insert(
            PathBuf::from("/p/b.jpg.v2.specular.xmp"),
            PathBuf::from("/p/b.jpg"),
        );
        for k in ["/p/a.cr2", "/p/b.jpg", "/p/b.jpg.v2.specular.xmp"] {
            g.asked.insert(PathBuf::from(k), [1; 32]);
        }
        g.asked.insert(version.clone(), [1; 32]);
        g.ask_raws_again();
        let mut left: Vec<&PathBuf> = g.asked.keys().collect();
        left.sort();
        assert_eq!(
            left,
            [
                &PathBuf::from("/p/b.jpg"),
                &PathBuf::from("/p/b.jpg.v2.specular.xmp")
            ]
        );
    }

    #[test]
    fn version_facts_answer_the_query() {
        let mut g = Edited {
            listed: true,
            ..Edited::default()
        };
        let reg = specular_develop::model::Registry::builtin();
        let e = specular_develop::model::Edit::default_for(
            reg,
            specular_develop::model::SourceKey::default(),
            true,
        );
        g.facts.insert(
            PathBuf::from("/p/a.cr2"),
            Facts {
                version: 0,
                name: String::new(),
                hash: [1; 32],
                modules: "exposure".into(),
                styles: String::new(),
                edit: Arc::new(e),
            },
        );
        assert!(g.is_edited(Path::new("/p/a.cr2")));
        assert!(!g.is_edited(Path::new("/p/b.cr2")));
        assert!(g.pixels(Path::new("/p/a.cr2"), 256).is_none());
        let px = |n: u32| Arc::new(Pixels::new(n, n, vec![0; (n * n * 4) as usize]));
        g.thumbs.insert(
            PathBuf::from("/p/a.cr2"),
            ([1; 32], [px(128), px(256), px(512)]),
        );
        assert_eq!(g.pixels(Path::new("/p/a.cr2"), 200).unwrap().0, 256);
        assert_eq!(g.pixels(Path::new("/p/a.cr2"), 900).unwrap().0, 512);
        g.clear();
        assert!(!g.listed && g.facts.is_empty() && g.thumbs.is_empty());
    }
}
