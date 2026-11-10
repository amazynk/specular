//! Pages of multi-image files: the sizes of an ICNS icon, the images of an ICO/CUR file, the
//! layers, cube faces and depth slices of a KTX/KTX2/DDS/PVR/VTF texture, the pages of a
//! multi-page TIFF, the top-level images of a HEIF/AVIF collection, the merged image and
//! pixel layers of a Photoshop document.
//!
//! Alt+Page Down/Up step pages and Alt+Home/End jump to the first and last (the page
//! actions, [`Action::NextPage`] and friends); the page stepper above the toolbar does the
//! same. Pages clamp at the ends like the slices of a medical image. A page switch opens the
//! page from the session already open ([`Session::open_page`]): the file is not read again
//! and the format is not probed, the page lands in the display cache under its own key, and
//! the info panel shows the old page's report until the page's own (read from the same
//! bytes) replaces it. The page is remembered while the image is kept (like its rotation)
//! and starts at the first again once it has been dropped.
//!
//! The old page stays on screen until the new one has pixels (the stand-in, without its
//! time limit; an error ends it). The next page, and the previous one when the prefetch
//! depth is 2 or more, open ahead from the current page's session at prefetch priority:
//! entries in the image table that the path map does not name, uploaded within the video
//! memory budget like the folder's next images, and let go (their jobs cancelled) when the
//! current image is another file, another page's neighbours or opened afresh. A switch to
//! a page opened ahead shows it at once, and the page left is kept as a neighbour.
//!
//! DICOM's frames are not pages here: a grey multi-frame image steps its frames in medical
//! mode (the page keys step them too), a colour one plays in the animation player.

use std::collections::HashSet;
use std::sync::Arc;

use specular_core::Priority;
use specular_decode::{Format, PageLabel, Session, SlideImage};
use specular_i18n::{I18n, t};
use specular_input::Action;
use specular_log::{DECODE, PREFETCH, debug};
use specular_metadata::{MetadataReport, SummaryGroup, SummaryItem};

use super::{App, Img};
use crate::loader::ImageId;

/// How many pages the viewer steps in `session`: 1 for medical images (DICOM and research
/// volumes: frames and slices step in medical mode) and single-image files.
pub(super) fn page_count(session: &dyn Session) -> u32 {
    if session.info().format == Format::Dicom || session.medical().is_some() {
        1
    } else {
        session.page_count().max(1)
    }
}

/// The page action `a` leads to from page `index` of `count`; None at the ends (and for
/// other actions).
pub(super) fn target_page(a: Action, index: u32, count: u32) -> Option<u32> {
    let last = count.max(1) - 1;
    let to = match a {
        Action::NextPage => (index + 1).min(last),
        Action::PrevPage => index.saturating_sub(1),
        Action::FirstPage => 0,
        Action::LastPage => last,
        _ => return None,
    };
    (to != index).then_some(to)
}

/// The pages opened ahead of page `page` of `count` with prefetch depth `depth`: the next,
/// and the previous when the depth is 2 or more; none without prefetching.
pub(super) fn neighbours(page: u32, count: u32, depth: usize) -> Vec<u32> {
    let mut out = Vec::new();
    if depth == 0 {
        return out;
    }
    if page + 1 < count {
        out.push(page + 1);
    }
    if depth >= 2 && page > 0 && page < count {
        out.push(page - 1);
    }
    out
}

/// Whether `a` is one of the page actions.
pub(super) fn is_page_action(a: Action) -> bool {
    matches!(
        a,
        Action::NextPage | Action::PrevPage | Action::FirstPage | Action::LastPage
    )
}

/// Cube faces in their order in a texture.
const FACES: [&str; 6] = ["+X", "−X", "+Y", "−Y", "+Z", "−Z"];

/// What a page is, in words: "128×128 @2x", "Layer 2 · Face +X", "Slice 3", "Merged image",
/// "Background (hidden)".
pub(super) fn label_text(l: &PageLabel) -> Option<String> {
    match *l {
        PageLabel::Icon {
            width,
            height,
            density,
        } => Some(if density > 1 {
            t!(
                "page-label-icon-density",
                width = plain(width),
                height = plain(height),
                density = i64::from(density)
            )
        } else {
            t!(
                "page-label-icon",
                width = plain(width),
                height = plain(height)
            )
        }),
        PageLabel::Image {
            width,
            height,
            bits,
        } => Some(match bits {
            Some(bits) => t!(
                "page-label-size-bits",
                width = plain(width),
                height = plain(height),
                bits = i64::from(bits)
            ),
            None => t!(
                "page-label-size",
                width = plain(width),
                height = plain(height)
            ),
        }),
        PageLabel::Merged => Some(t!("page-label-merged")),
        PageLabel::FromLayers => Some(t!("page-label-from-layers")),
        PageLabel::Sheet { ref name, model } => Some(if model {
            t!("page-label-model")
        } else {
            name.to_string()
        }),
        PageLabel::DocumentLayer {
            ref name,
            index,
            hidden,
        } => {
            let name = if name.trim().is_empty() {
                t!("page-label-layer-unnamed", index = i64::from(index))
            } else {
                name.to_string()
            };
            Some(if hidden {
                t!("page-label-layer-hidden", name = name)
            } else {
                name
            })
        }
        PageLabel::Texture { layer, face, slice } => {
            let parts: Vec<String> = [
                layer.map(|i| t!("page-label-layer", index = i64::from(i))),
                face.map(|f| match FACES.get(f as usize) {
                    Some(name) => t!("page-label-face", face = *name),
                    None => t!("page-label-sphere"),
                }),
                slice.map(|i| t!("page-label-slice", index = i64::from(i))),
            ]
            .into_iter()
            .flatten()
            .collect();
            parts
                .into_iter()
                .reduce(|a, b| t!("page-label-join", a = a, b = b))
        }
        PageLabel::Slide(ref image) => Some(match image {
            SlideImage::Main => t!("page-label-slide"),
            SlideImage::Label => t!("page-label-slide-label"),
            SlideImage::Macro => t!("page-label-slide-macro"),
            SlideImage::Thumbnail => t!("page-label-slide-thumbnail"),
            SlideImage::Other(name) => name.to_string(),
        }),
        PageLabel::Stereo { right } => Some(if right {
            t!("page-label-right-view")
        } else {
            t!("page-label-left-view")
        }),
        PageLabel::Thermal { visible } => Some(if visible {
            t!("page-label-visible")
        } else {
            t!("page-label-thermal")
        }),
        PageLabel::Object {
            ref name,
            ref class,
        } => Some(t!(
            "page-label-object",
            name = name.to_string(),
            class = class.to_string()
        )),
        PageLabel::Map { ref name } => (!name.is_empty()).then(|| name.to_string()),
        PageLabel::Generated(g) => Some(super::cad::shown_name(&g.english(), Some(g))),
        PageLabel::BoardStep { ref step, view } => Some(t!(
            "page-label-board-step",
            step = step.to_string(),
            view = super::cad::shown_name(&view.english(), Some(view))
        )),
        PageLabel::Plane { z, t } => [
            z.map(|(i, n)| {
                t!(
                    "page-label-z",
                    index = i64::from(i) + 1,
                    count = i64::from(n)
                )
            }),
            t.map(|(i, n)| {
                t!(
                    "page-label-t",
                    index = i64::from(i) + 1,
                    count = i64::from(n)
                )
            }),
        ]
        .into_iter()
        .flatten()
        .reduce(|a, b| t!("page-label-join", a = a, b = b)),
        PageLabel::Chapter { ref title } => Some(title.to_string()).filter(|t| !t.is_empty()),
        PageLabel::Face { ref name } => Some(name.to_string()).filter(|n| !n.is_empty()),
        PageLabel::Product { view, metres } => {
            use specular_decode::ProductView as V;
            let v = match view {
                V::Composite => t!("page-label-product-composite"),
                V::Bands => t!("page-label-product-bands"),
                V::TrueColour => t!("page-label-product-true-colour"),
                V::FalseColour => t!("page-label-product-false-colour"),
                V::Swir => t!("page-label-product-swir"),
                V::Agriculture => t!("page-label-product-agriculture"),
                V::SceneClass => t!("page-label-product-scene-class"),
                V::CloudProbability => t!("page-label-product-cloud"),
            };
            Some(match metres {
                Some(m) => t!("page-label-product-at", view = v, metres = i64::from(m)),
                None => v,
            })
        }
    }
}

/// A pixel size without grouping ("1024").
fn plain(n: u32) -> String {
    I18n::global().localizer().integer_plain(i64::from(n))
}

/// The info panel's Page group: "Page 3 of 12" and what it is.
pub(super) fn page_group(index: u32, count: u32, label: Option<String>) -> SummaryGroup {
    let mut items = vec![SummaryItem {
        key: "page",
        label: t!("info-page-position"),
        value: t!(
            "page-indicator",
            index = i64::from(index) + 1,
            count = i64::from(count)
        ),
        detail: None,
    }];
    if let Some(l) = label {
        items.push(SummaryItem {
            key: "page-contents",
            label: t!("info-page-contents"),
            value: l,
            detail: None,
        });
    }
    SummaryGroup {
        key: "page",
        title: t!("info-page-group"),
        items,
    }
}

/// `report` with its Page group replaced by `group` (after the Image group, else first).
pub(super) fn with_page(report: &MetadataReport, group: Option<SummaryGroup>) -> MetadataReport {
    let mut r = report.clone();
    r.summary.retain(|g| g.key != "page");
    if let Some(g) = group {
        let at = r
            .summary
            .iter()
            .position(|g| g.key == "image")
            .map_or(0, |i| i + 1);
        r.summary.insert(at, g);
    }
    r
}

impl Img {
    /// The image whose file this is (itself, unless it is another page of one).
    pub(super) fn file_of(&self, id: ImageId) -> ImageId {
        self.file_id.unwrap_or(id)
    }

    /// Takes the page facts of a session that has just opened.
    pub(super) fn note_pages(&mut self, session: &dyn Session) {
        self.pages = page_count(session);
        self.page = session.page_index().min(self.pages - 1);
        self.page_label = session.page_label();
    }
}

impl App {
    /// The page actions; true when `a` was one. In medical mode they step the frames.
    pub(super) fn page_action(&mut self, a: Action) -> bool {
        if !is_page_action(a) {
            return false;
        }
        if self.medical_state().is_some_and(|n| n > 1) {
            self.medical_page(a);
            return true;
        }
        let Some(id) = self.current_id() else {
            return true;
        };
        let Some(img) = self.images.get(&id) else {
            return true;
        };
        if let Some(page) = target_page(a, img.page, img.pages) {
            self.page_switch(id, page);
        }
        true
    }

    /// Shows page `page` of image `id` (the current one) in its place: the page opened
    /// ahead, else a new image entry for the same file, opened from the old one's session.
    /// The old page is kept as a neighbour while it is one.
    pub(super) fn page_switch(&mut self, id: ImageId, page: u32) {
        let Some(cur) = self.current else { return };
        let Some(mut old) = self.images.remove(&id) else {
            return;
        };
        let file = old.file_of(id);
        debug!(target: DECODE, "img={id} {}: page {} → {} of {}", old.name(), old.page + 1, page + 1, old.pages);
        old.token.cancel();
        old.opening = false;
        old.levels.clear();
        old.page_hold = false;
        self.by_path.remove(&old.path);
        self.shown = None;
        let ahead = self.page_ahead.iter().position(|a| {
            self.images
                .get(a)
                .is_some_and(|i| i.page == page && i.file_of(*a) == file)
        });
        let new = match ahead {
            Some(i) => {
                let new = self.page_ahead.remove(i);
                debug!(target: PREFETCH, "img={new} page {} was opened ahead", page + 1);
                new
            }
            None => {
                let new = self.next_id;
                self.next_id += 1;
                let mut img = Img::new(old.path.clone());
                img.page = page;
                // Keeps the stepper up (without a label) while the page opens.
                img.pages = old.pages;
                img.file_id = Some(file);
                img.page_base = old.session.clone().map(|s| (id, s));
                self.images.insert(new, img);
                new
            }
        };
        if let Some(img) = self.images.get_mut(&new) {
            // The info panel keeps the old page's report until the page's own arrives (its
            // size and encoding, a TIFF page's tags), read from the bytes the session holds;
            // the motion clip is the file's.
            if img.metadata.is_none() {
                img.metadata = old.metadata.clone();
            }
            if img.motion.is_none() {
                img.motion = old.motion.clone();
            }
            img.page_hold = true;
        }
        self.by_path.insert(old.path.clone(), new);
        // The page left stays as a neighbour (`page_neighbours` lets it go when it is not
        // one); one that never opened is dropped.
        if old.session.is_some() && old.error.is_none() {
            old.file_id = Some(file);
            old.shown_at = None;
            self.images.insert(id, old);
            self.page_ahead.push(id);
        } else {
            self.gpu_images.remove(id);
            if let Some(g) = self.gfx.as_mut() {
                g.renderer.release_image(id);
            }
        }
        self.page_patch(new);
        // As `go_to` does for another image.
        self.present_now = true;
        for j in self.tile_jobs.drain(..) {
            j.handle.cancel();
        }
        if let Some(img) = self.images.get_mut(&new) {
            img.shown_at = Some(std::time::Instant::now());
            img.open_ms = None;
            img.whole_ms = None;
            img.track.shown();
        }
        self.chrome.set_open_report(None);
        self.go_to(cur);
    }

    /// Opens the pages next to the current one ahead (with `want`; [`neighbours`]) and adds
    /// them to `window`; lets go of those of another file or no longer next. Without `want`
    /// (prefetching held) the current file's are kept but nothing new opens.
    pub(super) fn page_neighbours(&mut self, want: bool, window: &mut HashSet<ImageId>) {
        let depth = specular_platform::prefetch_depth(
            self.settings.performance.prefetch_depth.clamp(0, 16) as usize,
        );
        // Medical mode steps frames, not pages.
        let cur = self.current_id().filter(|_| self.medical_state().is_none());
        let base = cur.and_then(|id| {
            let img = self.images.get(&id)?;
            let session = img.session.clone().filter(|_| img.error.is_none())?;
            Some((id, img.file_of(id), img.page, img.pages, session))
        });
        let file = base.as_ref().map(|b| b.1);
        let mut pages = match &base {
            Some((_, _, page, count, _)) if want => neighbours(*page, *count, depth),
            _ => Vec::new(),
        };
        let mut keep = Vec::new();
        for id in std::mem::take(&mut self.page_ahead) {
            let Some(img) = self.images.get(&id) else {
                continue;
            };
            let same = file.is_some_and(|f| img.file_of(id) == f) && Some(id) != cur;
            if same && (!want || pages.contains(&img.page)) {
                pages.retain(|&p| p != img.page);
                keep.push(id);
                continue;
            }
            debug!(target: PREFETCH, "img={id} {} page {}: not next to the current page; dropped", img.name(), img.page + 1);
            if let Some(img) = self.images.remove(&id) {
                img.token.cancel();
            }
            self.gpu_images.remove(id);
            if let Some(g) = self.gfx.as_mut() {
                g.renderer.release_image(id);
            }
        }
        self.page_ahead = keep;
        if !want {
            return;
        }
        if let Some((cur_id, file, _, count, session)) = base
            && let Some(path) = self.images.get(&cur_id).map(|i| i.path.clone())
        {
            for page in pages {
                let id = self.next_id;
                self.next_id += 1;
                let mut img = Img::new(path.clone());
                img.page = page;
                img.pages = count;
                img.file_id = Some(file);
                img.page_base = Some((cur_id, session.clone()));
                debug!(target: PREFETCH, "img={id} {} page {} of {count}: opening ahead", img.name(), page + 1);
                self.images.insert(id, img);
                self.page_ahead.push(id);
            }
        }
        for id in self.page_ahead.clone() {
            window.insert(id);
            self.ensure_open(id, Priority::Prefetch);
            self.request_levels(id, Priority::Prefetch);
            self.upload_or_defer(id);
        }
    }

    /// Adds (or replaces) the Page group in image `id`'s metadata, when it has pages.
    pub(super) fn page_patch(&mut self, id: ImageId) {
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let Some(r) = img.metadata.as_ref() else {
            return;
        };
        let group = (img.pages > 1).then(|| {
            page_group(
                img.page,
                img.pages,
                img.page_label.as_ref().and_then(label_text),
            )
        });
        let has = r.summary.iter().any(|g| g.key == "page");
        if group.is_none() && !has {
            return;
        }
        let patched = with_page(r, group);
        if let Some(img) = self.images.get_mut(&id) {
            img.metadata = Some(Arc::new(patched));
        }
        self.redraw = true;
    }

    /// Pages of the current image for the page actions: 0 in medical mode (the frames step
    /// there) and for single-image files.
    pub(super) fn page_count_current(&self) -> u32 {
        if self.medical_state().is_some() {
            return 0;
        }
        self.current_id()
            .and_then(|id| self.images.get(&id))
            .map_or(0, |i| if i.pages > 1 { i.pages } else { 0 })
    }

    /// The page stepper's facts for the current image: page, count and label.
    pub(super) fn page_badge(&self) -> Option<(u32, u32, Option<String>)> {
        if self.page_count_current() < 2 || self.gallery.active {
            return None;
        }
        let img = self.current_id().and_then(|id| self.images.get(&id))?;
        // A page still opening has no label yet: it is not shown for the old page.
        let label = img
            .session
            .is_some()
            .then_some(img.page_label.as_ref())
            .flatten()
            .and_then(label_text);
        Some((img.page, img.pages, label))
    }

    /// The window title of image `img`: "icon.icns — Page 3 of 12" for a page.
    pub(super) fn page_title(img: &Img) -> String {
        let name = img.name();
        if img.pages < 2 || name.is_empty() {
            return name;
        }
        let page = t!(
            "page-indicator",
            index = i64::from(img.page) + 1,
            count = i64::from(img.pages)
        );
        t!("title-file-page", file = name, page = page)
    }
}

#[cfg(test)]
mod tests {
    use super::super::stands_in;
    use super::*;
    use specular_i18n::strip_isolates;

    fn label(l: PageLabel) -> Option<String> {
        label_text(&l).map(|s| strip_isolates(&s))
    }

    #[test]
    fn page_targets_clamp_at_the_ends() {
        assert_eq!(target_page(Action::NextPage, 0, 3), Some(1));
        assert_eq!(target_page(Action::NextPage, 2, 3), None);
        assert_eq!(target_page(Action::PrevPage, 0, 3), None);
        assert_eq!(target_page(Action::PrevPage, 2, 3), Some(1));
        assert_eq!(target_page(Action::FirstPage, 2, 3), Some(0));
        assert_eq!(target_page(Action::FirstPage, 0, 3), None);
        assert_eq!(target_page(Action::LastPage, 0, 3), Some(2));
        assert_eq!(target_page(Action::LastPage, 2, 3), None);
        // One page (or none known): nowhere to go.
        assert_eq!(target_page(Action::NextPage, 0, 1), None);
        assert_eq!(target_page(Action::LastPage, 0, 0), None);
        assert_eq!(target_page(Action::NextImage, 0, 3), None);
        assert!(is_page_action(Action::LastPage) && !is_page_action(Action::NextSlice));
    }

    #[test]
    fn neighbours_within_the_prefetch_depth() {
        // The next page; the previous too from depth 2; none without prefetching.
        assert_eq!(neighbours(0, 3, 1), [1]);
        assert_eq!(neighbours(1, 3, 1), [2]);
        assert_eq!(neighbours(1, 3, 2), [2, 0]);
        assert_eq!(neighbours(2, 3, 4), [1]);
        assert_eq!(neighbours(0, 3, 4), [1]);
        assert!(neighbours(1, 3, 0).is_empty());
        assert!(neighbours(0, 1, 4).is_empty());
    }

    /// A page switched to keeps the old page on screen past the stand-in's time limit, until
    /// it has pixels; an error ends it.
    #[test]
    fn page_hold_outlasts_the_stand_in() {
        use std::time::{Duration, Instant};
        let mut img = Img::new("a.icns".into());
        img.shown_at = Some(Instant::now() - Duration::from_secs(5));
        assert!(!stands_in(&img, false));
        assert!(stands_in(&img, true));
        img.page_hold = true;
        assert!(stands_in(&img, false));
        img.error = Some("bad page".into());
        assert!(!stands_in(&img, false));
        let mut fresh = Img::new("b.png".into());
        fresh.shown_at = Some(Instant::now());
        assert!(stands_in(&fresh, false));
        assert_eq!(fresh.file_of(7), 7);
        fresh.file_id = Some(3);
        assert_eq!(fresh.file_of(7), 3);
    }

    #[test]
    fn labels() {
        let icon = |width, height, density| PageLabel::Icon {
            width,
            height,
            density,
        };
        assert_eq!(label(icon(128, 128, 2)).unwrap(), "128×128 @2x");
        assert_eq!(label(icon(16, 16, 1)).unwrap(), "16×16");
        let tex = |layer, face, slice| PageLabel::Texture { layer, face, slice };
        assert_eq!(label(tex(Some(2), None, None)).unwrap(), "Layer 2");
        assert_eq!(label(tex(None, Some(1), None)).unwrap(), "Face −X");
        assert_eq!(
            label(tex(Some(1), Some(4), None)).unwrap(),
            "Layer 1 · Face +Z"
        );
        assert_eq!(label(tex(None, Some(6), None)).unwrap(), "Sphere map");
        assert_eq!(label(tex(None, None, Some(3))).unwrap(), "Slice 3");
        assert_eq!(label(tex(None, None, None)), None);
        let image = |width, height, bits| PageLabel::Image {
            width,
            height,
            bits,
        };
        assert_eq!(label(image(32, 32, Some(32))).unwrap(), "32×32 · 32-bit");
        assert_eq!(label(image(1440, 960, None)).unwrap(), "1440×960");
        let layer = |name: &str, hidden| PageLabel::DocumentLayer {
            name: name.into(),
            index: 3,
            hidden,
        };
        assert_eq!(label(PageLabel::Merged).unwrap(), "Merged image");
        assert_eq!(label(PageLabel::FromLayers).unwrap(), "Built from layers");
        assert_eq!(label(layer("Sky", false)).unwrap(), "Sky");
        assert_eq!(label(layer("Sky", true)).unwrap(), "Sky (hidden)");
        assert_eq!(label(layer("", false)).unwrap(), "Unnamed layer 3");
        assert_eq!(label(layer(" ", true)).unwrap(), "Unnamed layer 3 (hidden)");
    }

    #[test]
    fn page_group_goes_after_the_image_group() {
        let g = |key| SummaryGroup {
            key,
            title: key.into(),
            items: Vec::new(),
        };
        let r = MetadataReport {
            summary: vec![g("camera"), g("image"), g("file")],
            ..MetadataReport::default()
        };
        let p = with_page(&r, Some(page_group(2, 12, Some("Layer 2".into()))));
        let keys: Vec<_> = p.summary.iter().map(|g| g.key).collect();
        assert_eq!(keys, ["camera", "image", "page", "file"]);
        let page = &p.summary[2];
        assert_eq!(strip_isolates(&page.items[0].value), "Page 3 of 12");
        assert_eq!(page.items[1].value, "Layer 2");
        // Replaced, not added again; removed with None.
        let again = with_page(&p, Some(page_group(3, 12, None)));
        assert_eq!(again.summary.iter().filter(|g| g.key == "page").count(), 1);
        assert_eq!(again.summary[2].items.len(), 1);
        assert!(with_page(&p, None).summary.iter().all(|g| g.key != "page"));
    }
}
