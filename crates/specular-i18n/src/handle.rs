//! [`I18n`]: the shared, live-switchable localisation handle.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, OnceLock, RwLock};

use fluent_bundle::FluentArgs;
use unic_langid::LanguageIdentifier;

use crate::bidi::Direction;
use crate::localizer::Localizer;
use crate::negotiate::LanguagePreference;

thread_local! {
    /// The global handle's language on this thread, while [`I18n::with_thread_language`] runs.
    static THREAD_LOCALIZER: RefCell<Option<Arc<Localizer>>> = const { RefCell::new(None) };
}

/// Threads inside [`I18n::with_thread_language`]: while none are, `localizer` skips the
/// thread-local lookup.
static THREAD_OVERRIDES: AtomicUsize = AtomicUsize::new(0);

/// Restores the thread's previous language when `with_thread_language` returns or unwinds.
struct ThreadOverride(Option<Arc<Localizer>>);

impl Drop for ThreadOverride {
    fn drop(&mut self) {
        let prev = self.0.take();
        THREAD_LOCALIZER.with(|c| *c.borrow_mut() = prev);
        THREAD_OVERRIDES.fetch_sub(1, AtomicOrdering::AcqRel);
    }
}

struct Inner {
    current: RwLock<Arc<Localizer>>,
    preference: RwLock<(LanguagePreference, Option<LanguageIdentifier>)>,
    generation: AtomicU64,
}

/// The localisation handle: `Send + Sync`, and cloning it is an `Arc` clone.
///
/// Changing the language swaps in a new [`Localizer`] snapshot and bumps
/// [`generation`](I18n::generation). UI code caches translated and shaped text together with
/// the generation it was made at, and rebuilds it when the number changes; nothing needs a
/// restart.
#[derive(Clone)]
pub struct I18n {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for I18n {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("I18n")
            .field("language", &self.language().to_string())
            .field("generation", &self.generation())
            .finish()
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::system()
    }
}

static GLOBAL: OnceLock<I18n> = OnceLock::new();

impl I18n {
    /// A handle for `preference`, with dates and numbers formatted for `region_formats` when
    /// given (the "Region formats" setting), otherwise for the UI language.
    pub fn new(preference: LanguagePreference, region_formats: Option<LanguageIdentifier>) -> Self {
        let loc = Localizer::new(&preference.requested(), region_formats.as_ref());
        Self {
            inner: Arc::new(Inner {
                current: RwLock::new(Arc::new(loc)),
                preference: RwLock::new((preference, region_formats)),
                generation: AtomicU64::new(0),
            }),
        }
    }

    /// A handle following the operating system's languages.
    pub fn system() -> Self {
        Self::new(LanguagePreference::System, None)
    }

    /// A handle fixed to one language (tests, tools).
    pub fn for_language(langid: LanguageIdentifier) -> Self {
        Self::new(LanguagePreference::Language(langid), None)
    }

    /// The process-wide handle (created on first use, following the system language).
    /// The app configures it once at start-up from settings with [`I18n::set_preference`].
    pub fn global() -> &'static I18n {
        GLOBAL.get_or_init(I18n::system)
    }

    /// The [global](I18n::global) handle set to `preference` and `region_formats` (the
    /// app's start-up), building the localizer once: first use builds it with these, and a
    /// handle made before is rebuilt once (not once per setting: each build makes the Fluent
    /// bundles and ICU formatters).
    pub fn configure_global(
        preference: LanguagePreference,
        region_formats: Option<LanguageIdentifier>,
    ) -> &'static I18n {
        let mut fresh = false;
        let g = GLOBAL.get_or_init(|| {
            fresh = true;
            Self::new(preference.clone(), region_formats.clone())
        });
        if !fresh {
            g.rebuild(preference, region_formats);
        }
        g
    }

    /// Runs `f` with the [global](I18n::global) handle in `langid` on the calling thread only;
    /// other threads, and this one afterwards (also when `f` panics), see the language as set.
    ///
    /// For tests that draw a right-to-left or pseudo-locale UI in a process whose other tests
    /// expect English: [`set_language`](I18n::set_language) would change it under them. The
    /// [generation](I18n::generation) does not change, so text cached by generation is not
    /// rebuilt; draw with fresh state inside `f`. Calls nest.
    pub fn with_thread_language<R>(langid: LanguageIdentifier, f: impl FnOnce() -> R) -> R {
        let region = Self::global().lock_pref().1.clone();
        let loc = Arc::new(Localizer::new(&[langid], region.as_ref()));
        THREAD_OVERRIDES.fetch_add(1, AtomicOrdering::AcqRel);
        let prev = THREAD_LOCALIZER.with(|c| c.borrow_mut().replace(loc));
        let _restore = ThreadOverride(prev);
        f()
    }

    /// Switches to `langid` (which may be a pseudo-locale) and bumps the generation.
    pub fn set_language(&self, langid: LanguageIdentifier) {
        self.set_preference(LanguagePreference::Language(langid));
    }

    /// Switches language preference (including back to "system") and bumps the generation.
    pub fn set_preference(&self, preference: LanguagePreference) {
        let region = self.lock_pref().1.clone();
        self.rebuild(preference, region);
    }

    /// Sets the locale for dates, numbers and sorting (`None`: follow the UI language) and
    /// bumps the generation.
    pub fn set_region_formats(&self, region_formats: Option<LanguageIdentifier>) {
        let pref = self.lock_pref().0.clone();
        self.rebuild(pref, region_formats);
    }

    /// Re-reads the OS languages when following the system (e.g. after a settings-change
    /// notification from the OS), bumping the generation only if the result changed.
    pub fn refresh_system(&self) {
        let (pref, region) = self.lock_pref().clone();
        if pref == LanguagePreference::System {
            let loc = Localizer::new(&pref.requested(), region.as_ref());
            if loc.language() != self.localizer().language() {
                self.install(loc, pref, region);
            }
        }
    }

    fn lock_pref(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, (LanguagePreference, Option<LanguageIdentifier>)> {
        self.inner
            .preference
            .read()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn rebuild(&self, preference: LanguagePreference, region: Option<LanguageIdentifier>) {
        let loc = Localizer::new(&preference.requested(), region.as_ref());
        self.install(loc, preference, region);
    }

    fn install(
        &self,
        loc: Localizer,
        preference: LanguagePreference,
        region: Option<LanguageIdentifier>,
    ) {
        specular_log::info!(target: specular_log::I18N, "UI language {} ({:?})", loc.language(), loc.chain());
        *self
            .inner
            .current
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Arc::new(loc);
        *self
            .inner
            .preference
            .write()
            .unwrap_or_else(|e| e.into_inner()) = (preference, region);
        self.inner.generation.fetch_add(1, AtomicOrdering::AcqRel);
    }

    /// Increases every time the language or region formats change. Compare with the value
    /// a cache was built at to know when to re-translate and re-shape.
    pub fn generation(&self) -> u64 {
        self.inner.generation.load(AtomicOrdering::Acquire)
    }

    /// The current snapshot. Hold it for a frame to translate many strings consistently.
    pub fn localizer(&self) -> Arc<Localizer> {
        if THREAD_OVERRIDES.load(AtomicOrdering::Acquire) > 0
            && Arc::ptr_eq(&self.inner, &Self::global().inner)
            && let Some(loc) = THREAD_LOCALIZER.with(|c| c.borrow().clone())
        {
            return loc;
        }
        self.inner
            .current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The language preference as set (for the Settings window).
    pub fn preference(&self) -> LanguagePreference {
        self.lock_pref().0.clone()
    }

    /// Translates `id` (falls back per message to English; never empty).
    pub fn tr(&self, id: &str) -> String {
        self.localizer().tr(id)
    }

    /// Translates `id` with arguments.
    pub fn tr_args(&self, id: &str, args: &FluentArgs<'_>) -> String {
        self.localizer().tr_args(id, args)
    }

    /// The effective UI language.
    pub fn language(&self) -> LanguageIdentifier {
        self.localizer().language().clone()
    }

    /// Layout direction of the UI language.
    pub fn direction(&self) -> Direction {
        self.localizer().direction()
    }

    /// The parts of one line joined with `sep` in the UI's direction (see
    /// [`crate::join_parts`]): each part keeps its own order, and right to left the line reads
    /// from its first part on the right.
    pub fn join<S: AsRef<str>>(&self, parts: &[S], sep: &str) -> String {
        let parts: Vec<&str> = parts.iter().map(AsRef::as_ref).collect();
        crate::join_parts(&parts, sep, self.direction())
    }

    /// The parts of one line joined with " · " (Info rows, status lines, cards), each kept
    /// in its own order (see [`I18n::join`]).
    pub fn dot_join<S: AsRef<str>>(&self, parts: &[S]) -> String {
        self.join(parts, " · ")
    }

    /// A name or label for a button or menu row, isolated for the UI's direction (see
    /// [`crate::isolate_label`]).
    pub fn label(&self, s: &str) -> String {
        crate::isolate_label(s, self.direction())
    }

    /// A label and a text someone wrote ("author: comment") joined with `sep` in the UI's
    /// direction, the line taking the text's own direction (see [`crate::join_labelled`]).
    pub fn join_labelled(&self, label: &str, text: &str, sep: &str) -> String {
        crate::join_labelled(label, text, sep, self.direction())
    }

    /// Natural, locale-aware name ordering (see [`Localizer::natural_cmp`]).
    pub fn natural_cmp(&self, a: &str, b: &str) -> Ordering {
        self.localizer().natural_cmp(a, b)
    }
}
