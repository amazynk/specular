//! A lost GPU device (a driver reset, for example after a long submission under load):
//! the app opens a new device on the same window, makes the image renderer, compositor and
//! UI renderer again, uploads the open image again and says so in a toast. Develop drops its
//! engine and develops the photo again on the new device.
//!
//! Every view drops what it made on the old device and makes it again as it next draws
//! ([`App::views_gpu_reset`]): the gallery's thumbnail pages, quick look, compare and its
//! map (base map tiles asked for again), 3D models, point clouds, splats, molecules and
//! tractograms, MPR and volume rendering (the volumes read so far uploaded again, with
//! their segmentations, fused series, TMTV voxels and dose), medical segmentations, the
//! drawing (vector) renderer, the sound page (entered again; the session keeps its
//! analysis), the movie's seek previews and bitmap subtitles (a paused movie's frame is
//! decoded again), spread pages, the pixel inspector, slideshow crossfades, the
//! presentation window, and the textures of the welcome page, studies, OMERO, print
//! preview, linked viewports, sheet compare, font specimens and sky surveys. Texture ids are
//! unique in the process, so an id kept from before draws nothing rather than another
//! texture.
//!
//! Waits on the GPU go through [`Gpu::poll`], which turns wgpu's panic on a lost device
//! into an error and marks the device lost. Work under way when the device goes is started
//! again once on the new device, with a toast: a slideshow video export and a print or PDF
//! (once their worker has given up on the old device), a map export and a figure (on the
//! window's device, at once). Local AI (speech, OCR, people, search, Develop's AI tools)
//! makes its inference contexts again on the new device ([`super::ai::device`]).

use specular_core::Priority;
use specular_gfx::{Compositor, Gpu, UiRenderer, WindowSurface};
use specular_i18n::t;
use specular_render::{GpuImageCache, ImageRenderer};

use super::{App, performance};

/// How many times the app opened a new device after a loss.
static RESETS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The number of GPU resets so far: work started at one value and ending at another ran
/// into a reset.
pub(super) fn gpu_resets() -> u64 {
    RESETS.load(std::sync::atomic::Ordering::Acquire)
}

impl App {
    /// At the start of a frame: when the device is lost, opens a new one. Returns whether
    /// the window has a working device (false while none could be opened: tried again
    /// next frame).
    pub(super) fn recover_lost_gpu(&mut self) -> bool {
        let Some(g) = self.gfx.as_ref() else {
            return true;
        };
        if !g.gpu.is_lost() {
            return true;
        }
        specular_log::warn!(target: specular_log::GPU, "GPU device lost: opening a new one");
        let gpu = match Gpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                specular_log::error!(target: specular_log::GPU, "no new GPU device: {e}");
                return false;
            }
        };
        let Some(old) = self.gfx.take() else {
            return false;
        };
        let window = old.window.clone();
        let text = old.text;
        drop((old.renderer, old.ui, old.comp, old.surface));
        let budget = match self.settings.performance.vram_cache {
            0 => 384u64 << 20,
            mb => ((mb as u64) << 20) / 2,
        };
        let surface = match WindowSurface::new(&gpu, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                specular_log::error!(target: specular_log::GPU, "no surface on the new device: {e}");
                return false;
            }
        };
        let (sw, sh) = surface.size();
        let comp = Compositor::new(&gpu, surface.format(), surface.bits, sw, sh);
        comp.set_bits(
            &gpu,
            if self.settings.colour.dithering {
                surface.bits
            } else {
                16
            },
        );
        let ui = UiRenderer::new(&gpu, &comp);
        let mut renderer = ImageRenderer::new(&gpu, budget);
        performance::apply(&self.settings, Some(&mut renderer));
        performance::memory_readings(&self.hw, &gpu);
        self.gfx = Some(super::Gfx {
            window,
            gpu,
            surface,
            comp,
            ui,
            renderer,
            text,
        });
        // Everything uploaded went with the old device.
        self.gpu_images = GpuImageCache::default();
        for img in self.images.values_mut() {
            img.gpu_rank = None;
        }
        self.scene = None;
        RESETS.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        // Local AI makes its contexts again on the new device as it next runs.
        super::ai::device::publish(self.gfx.as_ref().map(|g| &g.gpu));
        self.develop_gpu_reset();
        self.views_gpu_reset();
        if let Some(id) = self.current_id() {
            self.request_levels(id, Priority::Immediate);
            self.upload(id);
        }
        // Pages shown beside it (spreads).
        let partners: Vec<_> = self
            .images
            .keys()
            .copied()
            .filter(|&id| Some(id) != self.current_id() && self.is_spread_partner(id))
            .collect();
        for id in partners {
            self.upload(id);
        }
        self.schedule();
        let now = self.now();
        self.chrome.toast(t!("toast-gpu-reset"), None, now);
        self.redraw = true;
        true
    }
}

impl App {
    /// Whether work started when [`gpu_resets`] was `since` ran into a GPU reset: the device
    /// was replaced since, or is lost now (replaced at the next frame).
    pub(super) fn gpu_reset_since(&self, since: u64) -> bool {
        gpu_resets() != since || self.gfx.as_ref().is_some_and(|g| g.gpu.is_lost())
    }

    /// Whether the window's device works (not lost, or replaced already).
    pub(super) fn gpu_ready(&self) -> bool {
        self.gfx.as_ref().is_some_and(|g| !g.gpu.is_lost())
    }

    /// Runs `f`, work on the window's device; when it fails because the device was lost,
    /// opens a new one and runs it once more, saying so in a toast.
    pub(super) fn with_gpu_retry<T>(
        &mut self,
        f: impl Fn(&mut App) -> Result<T, String>,
    ) -> Result<T, String> {
        let since = gpu_resets();
        match f(self) {
            Err(_)
                if self.gpu_reset_since(since) && self.recover_lost_gpu() && self.gpu_ready() =>
            {
                let now = self.now();
                self.chrome.toast(t!("toast-gpu-reset-retry"), None, now);
                f(self)
            }
            r => r,
        }
    }

    /// The views drop what they made on the old device; each makes it again as it next
    /// draws. Called once the new device is in [`App::gfx`].
    pub(super) fn views_gpu_reset(&mut self) {
        self.gallery.gpu_reset();
        self.mesh.gpu_reset();
        self.cad.gpu_reset();
        self.sound_page.gpu_reset();
        self.player.preview.gpu_reset();
        self.player.bitmaps.gpu_reset();
        self.inspect.gpu_reset();
        self.slideshow.gpu_reset();
        self.font_view.gpu_reset();
        self.text_view.gpu_reset();
        self.sheets.gpu_reset();
        self.welcome.gpu_reset();
        self.studies.gpu_reset();
        self.omero.gpu_reset();
        self.print.gpu_reset();
        self.linked.gpu_reset();
        self.astro.hips.gpu_reset();
        if let Some(g) = self.gfx.as_ref() {
            let gpu = g.gpu.clone();
            self.mpr.gpu_reset(&gpu);
            self.medical.an.gpu_reset(&gpu);
        }
        self.present_gpu_reset();
        self.gis_export_gpu_reset();
        // A paused movie shows its frame again (a playing one shows the next).
        let now = std::time::Instant::now();
        if let Some(a) = self.anim.as_mut()
            && !a.is_playing()
            && let Some(t) = a.time(now)
        {
            a.seek_time(t);
        }
    }
}
