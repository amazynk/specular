//! Archives in the viewer: an open archive is browsed like a folder of its pages, in reading
//! order (right to left for manga) and optionally as two-page spreads.

#[cfg(test)]
mod shots;

use std::collections::HashSet;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use specular_archive::{ComicLookup, Password};
use specular_core::Orientation;
use specular_gfx::wgpu;
use specular_i18n::t;
use specular_input::Action;
use specular_log::{FOLDER, debug};
use specular_render::{GpuImageCache, ImageRenderer, PxRect, RenderOptions, View};
use specular_settings::{InitialZoom, PageLayout, ReadingOrder};
use specular_thumbs::DiskCache;

use super::App;
use crate::archive_thumbs;
use crate::book::{self, Book, BookError, Spreads};
use crate::loader::{Event, ImageId};

/// Reading-mode state. The toggles (`rtl`, `spreads`) hold for the session, over the
/// settings and ComicInfo.
#[derive(Default)]
pub(super) struct Reading {
    book: Option<Arc<Book>>,
    rtl: Option<bool>,
    spreads: Option<bool>,
    /// Pages found to be landscape (a spread already): shown alone.
    wide: HashSet<usize>,
    layout: Option<Spreads>,
    /// The spread the view is set up for: each page's image, orientation, stored size and
    /// display size, in page order.
    shown: Option<Vec<Page>>,
    /// The page last noted as read in the open book (its entry name), and whether that is
    /// still to be written to [`book::positions`].
    noted: Option<String>,
    unsaved: bool,
    /// Pages open fitted to the window's width (`true`) or height, for the session.
    fit: Option<bool>,
    /// When the page table was last written.
    saved_at: Option<std::time::Instant>,
}

/// The page table is written at most this often while pages turn (and when the book closes).
const SAVE_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Page {
    id: ImageId,
    orient: Orientation,
    stored: (u32, u32),
    display: (u32, u32),
}

/// One page of a spread on screen: its image and the view that draws it.
pub(super) type SpreadView = (ImageId, View);

impl App {
    /// Opens `p` as a book when it is an archive and returns `true`; anything else closes the
    /// open book and returns `false`.
    pub(super) fn open_book(&mut self, p: &Path) -> bool {
        self.open_book_with(p, None)
    }

    /// Opens the encrypted archive `archive` again with `password` (from the prompt):
    /// remembered for the session when it opens it, else asked for again.
    pub(super) fn unlock_book(&mut self, archive: &Path, password: Password) {
        self.open_book_with(archive, Some(password));
    }

    /// [`App::open_book`] with `password` (else the one remembered for the archive). An
    /// encrypted archive without one shows as encrypted and asks for it.
    fn open_book_with(&mut self, p: &Path, password: Option<Password>) -> bool {
        if p.is_dir() || !book::is_archive_path(p) {
            self.close_book();
            return false;
        }
        let loader = &self.loader;
        let threads = book::reading_threads(loader.threads());
        // What its ComicInfo says, when known for the archive as it is: the book opens with
        // its reading order and without its deleted pages.
        let cache = self.thumb_cache();
        let known = |stamp| archive_thumbs::known_comic(cache.as_deref()?, p, stamp);
        let tried = password.clone();
        let is_image = |e: &Path| loader.supports_extension(e);
        let opened = Book::unlock(p, is_image, threads, known, password);
        self.close_book();
        self.folder = None;
        self.current = None;
        let mut start = 0;
        match opened {
            Ok(b) => {
                let b = Arc::new(b);
                debug!(target: FOLDER, "archive {}: {} pages", p.display(), b.len());
                if let (Some(pw), Some(stamp)) = (tried, b.stamp()) {
                    book::passwords::remember(p, stamp, pw);
                    // Its tile in the gallery shows its pages now, not a lock.
                    self.book_tile_known(&b);
                }
                self.loader.books().insert(b.clone());
                self.set_entries(b.paths().to_vec());
                start = self.remembered_page(&b).unwrap_or(0);
                self.reading.noted = b.entry_name(start).map(str::to_owned);
                self.reading.book = Some(b.clone());
                if b.comic_known().is_none() {
                    self.read_comic_info(b, cache);
                }
            }
            Err(e) => {
                // The archive itself shows as an image that cannot be opened, with the reason.
                debug!(target: FOLDER, "archive {}: {e:?}", p.display());
                self.set_entries(vec![p.to_path_buf()]);
                let id = self.id_for(0);
                // Encrypted: shown as such (also after Cancel) while the password is asked.
                let locked = matches!(e, BookError::Locked | BookError::WrongPassword);
                let e = if locked { BookError::Locked } else { e };
                if let Some(img) = self.images.get_mut(&id) {
                    img.error = Some(e.message());
                }
                if locked {
                    self.password_prompt(p, tried.is_some());
                }
            }
        }
        self.go_to(start);
        true
    }

    /// Where the page each book was last read at is kept; `None` when nothing is saved
    /// (screenshots, benchmarks, no settings file).
    fn positions_file(&self) -> Option<std::path::PathBuf> {
        if self.headless() {
            return None;
        }
        Some(self.store.path()?.parent()?.join(book::positions::FILE))
    }

    /// The page `b` was last read at, when remembered and still in it.
    fn remembered_page(&self, b: &Book) -> Option<usize> {
        if b.encrypted() {
            return None;
        }
        let text = std::fs::read_to_string(self.positions_file()?).ok()?;
        let entry = book::positions::find(&text, b.path())?;
        (0..b.len()).find(|&i| b.entry_name(i) == Some(entry))
    }

    /// Notes the page on screen as the open book's reading position (each frame) and writes
    /// it now and then ([`SAVE_EVERY`]).
    pub(super) fn note_book_page(&mut self) {
        let Some(b) = self.reading.book.as_ref() else {
            return;
        };
        if b.encrypted() {
            return;
        }
        let Some(entry) = self.current.and_then(|i| b.entry_name(i)) else {
            return;
        };
        if self.reading.noted.as_deref() != Some(entry) {
            self.reading.noted = Some(entry.to_owned());
            self.reading.unsaved = true;
        }
        if self.reading.unsaved
            && self
                .reading
                .saved_at
                .is_none_or(|t| t.elapsed() >= SAVE_EVERY)
        {
            self.save_book_page(false);
        }
    }

    /// Writes the open book's noted page to the table, on a background thread unless `now`.
    pub(super) fn save_book_page(&mut self, now: bool) {
        if !self.reading.unsaved {
            return;
        }
        self.reading.unsaved = false;
        self.reading.saved_at = Some(std::time::Instant::now());
        let (Some(b), Some(entry), Some(file)) = (
            self.reading.book.as_ref(),
            self.reading.noted.clone(),
            self.positions_file(),
        ) else {
            return;
        };
        let path = b.path().to_path_buf();
        let write = move || {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            if let Some(new) = book::positions::with_entry(&text, &path, &entry)
                && let Err(e) = specular_settings::write_atomic(&file, &new)
            {
                specular_log::warn!(target: FOLDER, "book pages: {e}");
            }
        };
        if now {
            write();
        } else {
            self.loader.background("book-pages", move |_| write());
        }
    }

    /// Reads the ComicInfo (reading order, spreads, deleted pages) of `book`, opened without
    /// it, on a thread of its own (in a solid archive reaching it may take as long as decoding
    /// every page), keeps what it says in `cache` for the next open of the archive as it is,
    /// and posts the book with it ([`Event::ComicInfo`]). ComicInfo that marks every page
    /// deleted is kept but changes nothing: the book shows them all.
    fn read_comic_info(&self, book: Arc<Book>, cache: Option<Arc<DiskCache>>) {
        self.loader.background("book-comicinfo", move |loader| {
            match book.read_comic_info() {
                ComicLookup::Found(c) => {
                    let Some(b) = book.with_comic(c) else { return };
                    if let Some(cache) = &cache {
                        archive_thumbs::remember_comic(cache, &b, b.comic_known().flatten());
                    }
                    if b.comic_applies() {
                        loader.post(Event::ComicInfo { book: Arc::new(b) });
                    }
                }
                ComicLookup::Absent => {
                    if let Some(cache) = &cache {
                        archive_thumbs::remember_comic(cache, &book, None);
                    }
                }
                // A read that failed may pass: nothing is kept.
                ComicLookup::Unread => {}
            }
        });
    }

    /// The open book's ComicInfo has been read ([`Book::read_comic_info`]): the book takes
    /// its place, the current page stays (the first when ComicInfo hides it) and the view is
    /// set up again for its reading order and spreads.
    pub(super) fn comic_info_read(&mut self, book: Arc<Book>) {
        let Some(old) = self.reading.book.as_ref().filter(|b| b.same_open(&book)) else {
            return;
        };
        let old = old.clone();
        debug!(target: FOLDER, "archive {}: ComicInfo, {} pages", book.path().display(), book.len());
        self.book_tile_known(&book);
        self.loader.books().insert(book.clone());
        self.reading.book = Some(book.clone());
        self.reading.layout = None;
        if old.paths() != book.paths() {
            let cur = self.current.and_then(|i| self.entries.get(i)).cloned();
            self.set_entries(book.paths().to_vec());
            // Landscape pages stay known as such under their new numbers.
            let wide = std::mem::take(&mut self.reading.wide);
            self.reading.wide = wide
                .into_iter()
                .filter_map(|i| self.path_index.get(&self.entries, old.paths().get(i)?))
                .collect();
            match cur.and_then(|p| self.index_of(&p)) {
                Some(i) => self.current = Some(i),
                None => {
                    self.reading.shown = None;
                    self.go_to(0);
                    self.redraw = true;
                    return;
                }
            }
        }
        // The page order or count on screen may have changed.
        self.reading.shown = None;
        self.shown = None;
        self.scene = None;
        self.setup_view();
        self.schedule();
        self.redraw = true;
    }

    pub(super) fn close_book(&mut self) {
        self.save_book_page(true);
        self.reading.noted = None;
        if self.reading.book.take().is_some() {
            self.loader.books().retain(None);
            self.reading.wide.clear();
            self.reading.layout = None;
            self.reading.shown = None;
        }
    }

    /// The current image is a page of an archive.
    pub(super) fn in_book(&self) -> bool {
        self.reading.book.is_some() && self.current.is_some()
    }

    /// Pages turn right to left (manga): Left is the next page.
    pub(super) fn reading_rtl(&self) -> bool {
        let Some(b) = &self.reading.book else {
            return false;
        };
        self.reading
            .rtl
            .unwrap_or(match self.settings.browsing.reading_order {
                ReadingOrder::Auto => b.right_to_left(),
                ReadingOrder::LeftToRight => false,
                ReadingOrder::RightToLeft => true,
            })
    }

    fn spreads_on(&self) -> bool {
        let Some(b) = &self.reading.book else {
            return false;
        };
        self.reading
            .spreads
            .unwrap_or(match self.settings.browsing.page_layout {
                PageLayout::Auto => b.suggests_spreads(),
                PageLayout::Single => false,
                PageLayout::Spreads => true,
            })
    }

    fn layout(&mut self) -> Option<&Spreads> {
        if !self.spreads_on() {
            return None;
        }
        let r = &mut self.reading;
        let b = r.book.as_ref()?;
        if r.layout.is_none() {
            let wide = &r.wide;
            r.layout = Some(Spreads::new(b.len(), |i| {
                b.is_double_page(i) || wide.contains(&i)
            }));
        }
        r.layout.as_ref()
    }

    /// The pages on screen with the current one, when that is two or more.
    fn spread_pages(&mut self) -> Option<Range<usize>> {
        let cur = self.current?;
        let r = self.layout()?.screen_of(cur);
        (r.len() > 1).then_some(r)
    }

    /// A spread is on screen.
    pub(super) fn spread_active(&self) -> bool {
        self.reading.shown.is_some()
    }

    /// Index of the last page on screen when a spread is (the toolbar's "4–5 / 20").
    pub(super) fn spread_last(&self) -> Option<usize> {
        let shown = self.reading.shown.as_ref()?;
        let last = shown.last()?;
        let path = &self.images.get(&last.id)?.path;
        self.index_of(path)
    }

    /// `img` is drawn next to the current page.
    pub(super) fn is_spread_partner(&self, id: ImageId) -> bool {
        self.reading
            .shown
            .as_ref()
            .is_some_and(|s| s.iter().any(|p| p.id == id))
    }

    /// Whether `spread_navigate(delta)` would turn to another screen; `None` without spreads.
    pub(super) fn spread_can_navigate(&mut self, delta: i64) -> Option<bool> {
        let cur = self.current?;
        let wrap = self.settings.browsing.loop_folder;
        let layout = self.layout()?;
        let (next, clamped) = layout.step(cur, delta);
        Some(
            !clamped || (wrap && layout.screens() > 1) || layout.screen(next) != layout.screen(cur),
        )
    }

    /// Page turns in spread mode go by screens. Returns `false` when spreads are off.
    pub(super) fn spread_navigate(&mut self, delta: i64) -> bool {
        let Some(cur) = self.current else {
            return false;
        };
        let wrap = self.settings.browsing.loop_folder;
        let Some(layout) = self.layout() else {
            return false;
        };
        let n = layout.screens() as i64;
        let at = layout.screen(cur) as i64;
        let (mut next, clamped) = layout.step(cur, delta);
        let mut toast = None;
        if clamped {
            if wrap && n > 1 {
                next = layout.step(0, (at + delta).rem_euclid(n)).0;
                toast = Some(if delta > 0 {
                    t!("toast-loop-start")
                } else {
                    t!("toast-loop-end")
                });
            } else if layout.screen(next) == layout.screen(cur) {
                return true;
            }
        }
        if let Some(msg) = toast {
            let now = self.now();
            self.chrome.toast(msg, None, now);
        }
        self.nav_dir = if delta >= 0 { 1 } else { -1 };
        self.go_to(next);
        true
    }

    /// Whether book pages open fitted to the window's width (`Some(true)`) or height.
    pub(super) fn reading_fit(&self) -> Option<bool> {
        self.reading.book.as_ref().and(self.reading.fit)
    }

    /// Fit width or height (`None`: neither) for the open book's pages from now on.
    pub(super) fn set_reading_fit(&mut self, fit: Option<bool>) {
        if self.reading.book.is_some() {
            self.reading.fit = fit;
        }
    }

    /// Fits the width or the height (`width` false); a wide page or spread fitted to the
    /// height starts on the side reading starts from.
    pub(super) fn zoom_fit_reading_side(&mut self, width: bool, snap: bool) {
        self.view.start_right = self.reading_rtl();
        self.view.zoom_fit_side(width, snap);
    }

    /// The reading-mode toggles. Returns `false` for other actions.
    pub(super) fn reading_action(&mut self, action: Action) -> bool {
        let msg = match action {
            Action::ToggleRightToLeft => {
                let on = !self.reading_rtl();
                self.reading.rtl = Some(on);
                if on {
                    t!("toast-right-to-left-on")
                } else {
                    t!("toast-right-to-left-off")
                }
            }
            Action::ToggleSpreads => {
                let on = !self.spreads_on();
                self.reading.spreads = Some(on);
                if on {
                    t!("toast-spreads-on")
                } else {
                    t!("toast-spreads-off")
                }
            }
            _ => return false,
        };
        let now = self.now();
        self.chrome.toast(msg, None, now);
        // Set the view up again (page order or count changed).
        self.reading.shown = None;
        self.shown = None;
        self.scene = None;
        self.setup_view();
        self.schedule();
        self.redraw = true;
        true
    }

    /// A page's size is known: landscape pages are spreads already and are shown alone.
    pub(super) fn note_page_size(&mut self, id: ImageId) {
        if self.reading.book.is_none() {
            return;
        }
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let Some(info) = &img.info else { return };
        let (w, h) = info.display_size();
        if w <= h {
            return;
        }
        if let Some(i) = self.path_index.get(&self.entries, &img.path)
            && self.reading.wide.insert(i)
        {
            self.reading.layout = None;
        }
    }

    /// The window title for an archive page: "003.jpg — Book.cbz".
    pub(super) fn book_title(&self) -> Option<String> {
        let b = self.reading.book.as_ref()?;
        let page = b.entry_name(self.current?)?;
        let archive = b.path().file_name()?.to_string_lossy();
        Some(t!(
            "title-archive-page",
            page = page.to_owned(),
            archive = archive.into_owned()
        ))
    }

    /// Sets the view up for a spread: one image as wide as the pages side by side, each
    /// scaled to the tallest. Returns `false` when no spread is on screen (the plain setup
    /// takes over).
    pub(super) fn setup_spread_view(&mut self) -> bool {
        let Some(range) = self.spread_pages() else {
            if self.reading.shown.take().is_some() {
                self.shown = None;
                self.scene = None;
            }
            return false;
        };
        let Some(cur) = self.current_id() else {
            return false;
        };
        let mut pages = Vec::with_capacity(range.len());
        for i in range {
            let id = self.id_for(i);
            if id != cur {
                // The facing page is needed now, not as a prefetch.
                self.ensure_open(id, specular_core::Priority::Immediate);
                self.request_levels(id, specular_core::Priority::Immediate);
            }
            let Some(img) = self.images.get_mut(&id) else {
                continue;
            };
            let Some(info) = &img.info else { continue };
            let orient = *img.orient.get_or_insert(info.orientation);
            pages.push(Page {
                id,
                orient,
                stored: (info.width, info.height),
                display: orient.display_size(info.width, info.height),
            });
        }
        if pages.len() < 2 {
            // Not all sizes are known yet: one page for now.
            if self.reading.shown.take().is_some() {
                self.shown = None;
                self.scene = None;
            }
            return false;
        }
        if self.reading.shown.as_ref() == Some(&pages) && self.shown == Some(cur) {
            return true;
        }
        let (w, h) = composite_size(&pages);
        let v = &self.settings.viewing;
        self.view.upscale_small = v.initial_zoom == InitialZoom::Fit;
        self.view.set_image(w, h, Orientation::NORMAL);
        self.pending_zoom = false;
        if let Some(width) = self.reading_fit() {
            self.zoom_fit_reading_side(width, true);
        }
        self.reading.shown = Some(pages);
        self.shown = Some(cur);
        self.scene = None;
        true
    }

    /// The views that draw the spread's pages, left to right on screen.
    pub(super) fn spread_views(&self) -> Option<Vec<SpreadView>> {
        let pages = self.reading.shown.as_ref()?;
        let rtl = self.reading_rtl();
        Some(page_views(pages, rtl, &self.view))
    }
}

/// Size of the spread image: the pages side by side, each scaled to the tallest.
fn composite_size(pages: &[Page]) -> (u32, u32) {
    let h = pages.iter().map(|p| p.display.1).max().unwrap_or(1).max(1);
    let w: f64 = pages
        .iter()
        .map(|p| p.display.0 as f64 * h as f64 / p.display.1.max(1) as f64)
        .sum();
    (w.round().max(1.0) as u32, h)
}

/// One view per page on screen (left to right; right to left puts the later page left),
/// each limited to the page's rectangle inside the image area.
fn page_views(pages: &[Page], rtl: bool, view: &View) -> Vec<SpreadView> {
    let (_, h) = composite_size(pages);
    let h = h as f32;
    let s = view.scale();
    let vp = view.viewport;
    let mut x = 0.0f32;
    let mut out = Vec::with_capacity(pages.len());
    let order: Vec<&Page> = if rtl {
        pages.iter().rev().collect()
    } else {
        pages.iter().collect()
    };
    for p in order {
        // Composite units per page display pixel.
        let k = h / p.display.1.max(1) as f32;
        let w = p.display.0 as f32 * k;
        let a = view.display_to_screen((x, 0.0));
        let b = view.display_to_screen((x + w, h));
        x += w;
        let Some(r) = intersect(
            PxRect {
                x: a.0,
                y: a.1,
                w: b.0 - a.0,
                h: b.1 - a.1,
            },
            vp,
        ) else {
            continue;
        };
        let c = view.screen_to_display(r.center());
        let left = x - w;
        let center = ((c.0 - left) / k, c.1 / k);
        out.push((p.id, View::fixed(p.stored, p.orient, r, s * k, center)));
    }
    out
}

fn intersect(a: PxRect, b: PxRect) -> Option<PxRect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y1 = (a.y + a.h).min(b.y + b.h);
    (x1 > x0 && y1 > y0).then_some(PxRect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// What the spread's scene depends on (the pages' texture revisions), and whether any page
/// has pixels yet.
pub(super) fn spread_rev(cache: &mut GpuImageCache, views: Option<&[SpreadView]>) -> (u64, bool) {
    let mut rev = 0u64;
    let mut pixels = false;
    for (id, _) in views.unwrap_or_default() {
        let r = cache.get(*id).map(|g| {
            pixels |= g.overview.is_some();
            g.revision
        });
        rev = rev
            .rotate_left(17)
            .wrapping_add(id.wrapping_mul(0x9e37_79b9_7f4a_7c15))
            .wrapping_add(r.unwrap_or(u64::MAX));
    }
    (rev, pixels)
}

/// Draws the spread's pages over the cleared scene.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_spread(
    renderer: &mut ImageRenderer,
    gpu: &specular_gfx::Gpu,
    enc: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    size: (u32, u32),
    cache: &mut GpuImageCache,
    views: &[SpreadView],
    opts: &RenderOptions,
) {
    for (id, v) in views {
        if let Some(g) = cache.get(*id) {
            renderer.render_inset(gpu, enc, target, size, g, v, opts, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: ImageId, w: u32, h: u32) -> Page {
        Page {
            id,
            orient: Orientation::NORMAL,
            stored: (w, h),
            display: (w, h),
        }
    }

    fn fitted(w: u32, h: u32) -> View {
        let mut v = View::new();
        v.set_viewport(PxRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        });
        v.upscale_small = true;
        v.set_image(w, h, Orientation::NORMAL);
        v
    }

    #[test]
    fn pages_sit_side_by_side_scaled_to_the_tallest() {
        let pages = [page(1, 400, 600), page(2, 200, 300)];
        assert_eq!(composite_size(&pages), (800, 600));
        let view = fitted(800, 600);
        let views = page_views(&pages, false, &view);
        assert_eq!(views.len(), 2);
        let (l, r) = (&views[0], &views[1]);
        assert_eq!((l.0, r.0), (1, 2));
        // Fit: 500 px tall, 666.7 px wide, centred.
        let (lv, rv) = (l.1.viewport, r.1.viewport);
        assert!((lv.x - (1000.0 - 666.67) / 2.0).abs() < 0.1, "{lv:?}");
        assert!((lv.w - 333.33).abs() < 0.1 && (rv.w - 333.33).abs() < 0.1);
        assert!((rv.x - (lv.x + lv.w)).abs() < 0.01, "pages touch");
        // The smaller page is drawn at twice the scale.
        assert!((r.1.scale() / l.1.scale() - 2.0).abs() < 1e-3);
        // Each page is centred in its rectangle.
        let c = l.1.center();
        assert!(
            (c.0 - 200.0).abs() < 0.1 && (c.1 - 300.0).abs() < 0.1,
            "{c:?}"
        );
        let c = r.1.center();
        assert!(
            (c.0 - 100.0).abs() < 0.1 && (c.1 - 150.0).abs() < 0.1,
            "{c:?}"
        );
    }

    #[test]
    fn right_to_left_puts_the_later_page_left() {
        let pages = [page(1, 400, 600), page(2, 400, 600)];
        let view = fitted(800, 600);
        let ids: Vec<_> = page_views(&pages, true, &view)
            .iter()
            .map(|v| v.0)
            .collect();
        assert_eq!(ids, [2, 1]);
    }

    #[test]
    fn pages_outside_the_image_area_are_skipped() {
        let pages = [page(1, 400, 600), page(2, 400, 600)];
        let mut view = fitted(800, 600);
        // Zoomed into the left page's middle: the right page is off screen.
        view.zoom_to(4.0, None);
        view.settle();
        view.center_on((200.0, 300.0), false);
        let views = page_views(&pages, false, &view);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].0, 1);
        assert_eq!(views[0].1.viewport, view.viewport);
    }
}
