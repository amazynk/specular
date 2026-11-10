//! Colour management in the viewer (settings `colour.*`): the process-wide policy for
//! decoders and source transforms, and the display transform of the window's monitor.
//!
//! * Policy (management on/off, rendering intent, black point compensation): set before any
//!   decode; a change reloads every image, since CMYK/L*a*b* pixels and source LUTs depend on it.
//! * Display: the profile of the monitor the window is on (`specular_platform::display_profile`,
//!   looked up off the UI thread except on macOS), or the `colour.display_profile` override.
//!   It becomes the compositor's final-pass transform and the renderer's storage space; a new
//!   storage space recreates the images' GPU textures (the decoded overviews stay).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};
use specular_color::{Display, Intent, Policy};
use specular_log::{debug, info, warn};
use specular_platform::display_profile::{DisplayProfile, DisplayQuery, display_profile};
use specular_settings::{RenderingIntent, Settings};

use super::{App, ImageId};

const COLOUR: &str = specular_log::COLOUR;

/// What the display transform was built from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DisplayKey {
    query: DisplayQuery,
    override_path: Option<PathBuf>,
    policy: Policy,
}

#[derive(Default)]
pub(super) struct ColourState {
    /// The display transform in use was built for this.
    applied: Option<DisplayKey>,
    /// A lookup in flight.
    pending: Option<(DisplayKey, Receiver<DisplayProfile>)>,
    checked: Option<Instant>,
    /// Hash of the display transform in use.
    display_hash: u64,
    /// The display transform in use: a presentation window on another display applies it
    /// too, since the images' storage space on the GPU is shared (`present.rs`).
    pub(super) display: Option<Display>,
}

/// The policy the settings ask for.
pub(super) fn policy_of(s: &Settings) -> Policy {
    Policy {
        enabled: s.colour.management,
        intent: match s.colour.rendering_intent {
            RenderingIntent::Perceptual => Intent::Perceptual,
            RenderingIntent::RelativeColorimetric => Intent::Relative,
            RenderingIntent::Saturation => Intent::Saturation,
            RenderingIntent::AbsoluteColorimetric => Intent::Absolute,
        },
        bpc: s.colour.black_point_compensation,
    }
}

impl App {
    /// Applies the colour policy of the current settings. Called from `apply_settings` after
    /// `self.settings` is updated; reloads all images when the policy changed.
    pub(super) fn apply_colour_policy(&mut self) {
        let p = policy_of(&self.settings);
        if p == specular_color::policy() {
            return;
        }
        specular_color::set_policy(p);
        info!(target: COLOUR, "colour policy: {p:?}");
        self.colour.checked = None;
        self.reload_all_images();
    }

    /// Drops every decoded image and decodes the current one again (the view stays).
    fn reload_all_images(&mut self) {
        let ids: Vec<ImageId> = self.images.keys().copied().collect();
        for id in ids {
            if let Some(img) = self.images.remove(&id) {
                img.token.cancel();
                self.by_path.remove(&img.path);
            }
            self.gpu_images.remove(id);
            if let Some(g) = self.gfx.as_mut() {
                g.renderer.release_image(id);
            }
        }
        self.deferred_uploads.clear();
        self.shown = None;
        if let Some(i) = self.current {
            self.go_to(i);
        }
        self.scene = None;
        self.redraw = true;
    }

    /// Keeps the display transform in step with the monitor the window is on (checked once a
    /// second) and with the settings.
    pub(super) fn update_display_colour(&mut self) {
        self.poll_display_profile();
        if self
            .colour
            .checked
            .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        self.colour.checked = Some(Instant::now());
        let Some(g) = self.gfx.as_ref() else { return };
        let monitor = g.window.current_monitor();
        let query = DisplayQuery {
            index: monitor
                .as_ref()
                .and_then(|m| g.window.available_monitors().position(|x| &x == m)),
            monitor: monitor.and_then(|m| m.name()),
        };
        let key = DisplayKey {
            query,
            override_path: self.settings.colour.display_profile.clone(),
            policy: specular_color::policy(),
        };
        if self.colour.applied.as_ref() == Some(&key)
            || self.colour.pending.as_ref().is_some_and(|(k, _)| k == &key)
        {
            return;
        }
        // Screenshots and benchmarks are reproducible: always sRGB.
        if !key.policy.enabled || self.args.screenshot.is_some() || self.args.bench_open.is_some() {
            self.colour.pending = None;
            self.colour.applied = Some(key);
            self.set_display(&Display::srgb());
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let lookup = {
            let key = key.clone();
            let wake = self.loader.waker();
            move || {
                let t0 = Instant::now();
                let p = match &key.override_path {
                    Some(path) => match std::fs::read(path) {
                        Ok(d) => DisplayProfile::Icc(d),
                        Err(e) => {
                            warn!(target: COLOUR, "display profile {}: {e}", path.display());
                            display_profile(&key.query)
                        }
                    },
                    None => display_profile(&key.query),
                };
                debug!(target: COLOUR, "display profile of {:?}: {} in {:.1} ms", key.query.monitor, match &p {
                    DisplayProfile::Managed => "managed by the compositor".to_owned(),
                    DisplayProfile::Icc(d) => format!("ICC, {} bytes", d.len()),
                    DisplayProfile::Unknown => "unknown (sRGB)".to_owned(),
                }, t0.elapsed().as_secs_f64() * 1e3);
                let _ = tx.send(p);
                // `about_to_wait` installs it; nothing else may wake the loop for a while.
                wake();
            }
        };
        // AppKit wants the main thread; elsewhere the lookup does D-Bus/X round trips.
        if cfg!(target_os = "macos") {
            lookup();
        } else if specular_thread::builder("disp-profile")
            .spawn(lookup)
            .is_err()
        {
            return;
        }
        self.colour.pending = Some((key, rx));
        self.poll_display_profile();
    }

    /// Installs the display profile looked up in the background, once it is there.
    pub(super) fn poll_display_profile(&mut self) {
        let Some((_, rx)) = &self.colour.pending else {
            return;
        };
        let profile = match rx.try_recv() {
            Ok(p) => p,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => DisplayProfile::Unknown,
        };
        let Some((key, _)) = self.colour.pending.take() else {
            return;
        };
        let display = match &profile {
            DisplayProfile::Icc(icc) => {
                let t0 = Instant::now();
                let d = Display::new(icc, key.policy.intent, key.policy.bpc);
                debug!(target: COLOUR, "display transform {} in {:.1} ms", kind_name(&d), t0.elapsed().as_secs_f64() * 1e3);
                d
            }
            DisplayProfile::Managed | DisplayProfile::Unknown => Display::srgb(),
        };
        self.colour.applied = Some(key);
        self.set_display(&display);
    }

    /// Installs a display transform: the final pass, and the storage space of new GPU images
    /// (recreating the existing ones when it changed).
    fn set_display(&mut self, display: &Display) {
        if display.hash == self.colour.display_hash && self.gfx.is_some() {
            return;
        }
        let Some(g) = self.gfx.as_mut() else { return };
        self.colour.display_hash = display.hash;
        self.colour.display = Some(display.clone());
        let name = kind_name(display);
        info!(target: COLOUR, "display transform: {name}");
        g.comp.set_display(&g.gpu, display);
        let (to, from) = (display.to_storage, display.from_storage);
        let st = g.renderer.storage();
        if st.to != to || st.from != from {
            g.renderer.set_storage(to, from);
            self.recreate_gpu_images();
        }
        self.scene = None;
        self.redraw = true;
    }

    /// Drops the GPU textures of all images (their overviews stay) and uploads the current
    /// ones again, in the renderer's current storage space.
    fn recreate_gpu_images(&mut self) {
        let Some(g) = self.gfx.as_mut() else { return };
        for id in self.gpu_images.ids() {
            self.gpu_images.remove(id);
            g.renderer.release_image(id);
        }
        let mut again = Vec::new();
        for (id, img) in self.images.iter_mut() {
            if img.gpu_rank.take().is_some() {
                again.push(*id);
            }
        }
        for id in again {
            self.upload_or_defer(id);
        }
        self.schedule();
    }
}

fn kind_name(d: &Display) -> &'static str {
    match d.kind {
        specular_color::DisplayKind::Identity => "sRGB (none)",
        specular_color::DisplayKind::Curves { .. } => "matrix + curves",
        specular_color::DisplayKind::Cube { .. } => "3D LUT",
    }
}
