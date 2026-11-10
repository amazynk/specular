//! Merges in the gallery (HDR, panorama and focus stack): a merge
//! (`IMG_0002-HDR.dng`, `-Pano.dng`, `-Stack.dng`, or `.tif` / `.jxl` / `.exr` for JPEG,
//! HEIC and TIFF photos) is stacked with the photos it was made from, shown on top. The
//! sources come from a DNG merge's own XMP (`specular_develop::merge::dng::sources_of`),
//! read in the listing's probe, or from the merge just made. Also the gallery's merge
//! items: which flow a selection goes to.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use specular_thumbs::Entry;

use crate::app::App;

/// The merges among `images` and their sources (paths beside each merge). Reads at most
/// the first 256 KiB of each file whose name looks like a merge's.
pub(super) fn probe(images: &[PathBuf]) -> Vec<(PathBuf, Vec<PathBuf>)> {
    use std::io::Read;
    images
        .iter()
        .filter(|p| specular_develop::merge::dng::looks_merged(p))
        .filter_map(|p| {
            let mut head = Vec::new();
            std::fs::File::open(p)
                .ok()?
                .take(256 * 1024)
                .read_to_end(&mut head)
                .ok()?;
            let (sources, _) = specular_develop::merge::dng::sources_of(&head)?;
            let dir = p.parent()?;
            Some((p.clone(), sources.iter().map(|s| dir.join(s)).collect()))
        })
        .collect()
}

/// Moves each source's group into its merge's group (members after the merge's own). A
/// source whose merge is not listed stays where it is.
pub(super) fn group_merges(
    groups: Vec<(usize, Vec<usize>)>,
    entries: &[Entry],
    merge_of: &HashMap<PathBuf, PathBuf>,
) -> Vec<(usize, Vec<usize>)> {
    if merge_of.is_empty() {
        return groups;
    }
    let mut group_of: HashMap<&Path, usize> = HashMap::new();
    for (g, (_, members)) in groups.iter().enumerate() {
        for &m in members {
            group_of.insert(entries[m].path.as_path(), g);
        }
    }
    let mut target = vec![None; groups.len()];
    for (g, (_, members)) in groups.iter().enumerate() {
        let to = members.iter().find_map(|&m| {
            let merge = merge_of.get(&entries[m].path)?;
            group_of.get(merge.as_path()).copied()
        });
        if let Some(t) = to
            && t != g
        {
            target[g] = Some(t);
        }
    }
    // A merge's own group never moves (sources of two merges go to the first).
    for g in 0..groups.len() {
        if let Some(t) = target[g]
            && target[t].is_some()
        {
            target[g] = None;
        }
    }
    let mut extra: Vec<Vec<usize>> = vec![Vec::new(); groups.len()];
    for (g, (_, members)) in groups.iter().enumerate() {
        if let Some(t) = target[g] {
            extra[t].extend(members.iter().copied());
        }
    }
    groups
        .into_iter()
        .zip(extra)
        .enumerate()
        .filter(|(g, _)| target[*g].is_none())
        .map(|(_, ((shown, mut members), extra))| {
            members.extend(extra);
            (shown, members)
        })
        .collect()
}

impl super::Gallery {
    /// Merges and their sources learnt: source → merge.
    pub(super) fn merges_add(&mut self, found: Vec<(PathBuf, Vec<PathBuf>)>) {
        for (merge, sources) in found {
            for s in sources {
                self.merge_of.insert(s, merge.clone());
            }
        }
        self.dirty = true;
    }
}

/// A gallery or palette merge item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum MergeItem {
    Hdr,
    Pano,
    Stack,
}

impl App {
    /// A merge item on `targets`: raws go to Merge to HDR, other photos to exposure fusion;
    /// one photo looks for its set among its neighbours in the listing.
    pub(in crate::app) fn gallery_merge_item(&mut self, item: MergeItem, targets: Vec<PathBuf>) {
        use crate::app::develop::merges::Kind;
        let any_raw = targets.iter().any(|p| {
            p.extension()
                .is_some_and(|e| super::model::is_raw_ext(&e.to_string_lossy()))
        });
        let k = match item {
            MergeItem::Hdr if any_raw => crate::app::develop::merge::AROUND,
            MergeItem::Hdr => Kind::Fusion.around(),
            MergeItem::Pano => Kind::Pano.around(),
            MergeItem::Stack => Kind::Stack.around(),
        };
        let order = self.gallery.image_order();
        let around = match targets.as_slice() {
            [one] => match order.iter().position(|p| p == one) {
                Some(i) => order[i.saturating_sub(k)..(i + k + 1).min(order.len())].to_vec(),
                None => Vec::new(),
            },
            _ => Vec::new(),
        };
        match item {
            MergeItem::Hdr if any_raw => self.develop_merge_ask(targets, around),
            MergeItem::Hdr => self.develop_fusion_ask(targets, around),
            MergeItem::Pano => self.develop_pano_ask(targets, around),
            MergeItem::Stack => self.develop_stack_ask(targets, around),
        }
    }

    /// A merge was just saved: stacked with its sources once listed.
    pub(in crate::app) fn gallery_merge_known(&mut self, merge: &Path, sources: &[PathBuf]) {
        self.gallery
            .merges_add(vec![(merge.to_path_buf(), sources.to_vec())]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_thumbs::EntryKind;

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
    fn sources_stack_under_their_merge() {
        let entries: Vec<Entry> = [
            "/a/1.CR2",
            "/a/2.CR2",
            "/a/2-HDR.dng",
            "/a/3.CR2",
            "/a/9.CR2",
        ]
        .iter()
        .map(|p| entry(p))
        .collect();
        let groups: Vec<(usize, Vec<usize>)> = (0..entries.len()).map(|i| (i, vec![i])).collect();
        let merge_of: HashMap<PathBuf, PathBuf> = ["/a/1.CR2", "/a/2.CR2", "/a/3.CR2"]
            .iter()
            .map(|s| (PathBuf::from(s), PathBuf::from("/a/2-HDR.dng")))
            .collect();
        let g = group_merges(groups, &entries, &merge_of);
        assert_eq!(g, vec![(2, vec![2, 0, 1, 3]), (4, vec![4])]);
        // The merge not listed: nothing moves.
        let few = vec![(0, vec![0]), (1, vec![1])];
        assert_eq!(group_merges(few.clone(), &entries, &merge_of), few);
    }
}
