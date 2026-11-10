//! OpenColorIO display/view for scene-linear images (EXR, Radiance HDR, PFM): the config
//! from Settings › Colour (else the file `$OCIO` names) is read with `specular_ocio`, its
//! displays, views and colour spaces become the choices of the three OCIO settings, and the
//! chosen input → display / view processor is baked on a worker into a log2 shaper and a
//! 65³ table that the image shader applies in place of tone mapping
//! (`specular_render::OcioLut`). Nothing is bundled.

use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use specular_i18n::t;
use specular_ocio::{Config, Processor, bake};

use super::App;

/// Grid points per axis of the baked table.
const SIZE: usize = 65;
/// The shaper's range, log2 of linear values: whole stops land on grid points (0.25 a step).
const LOG2_LO: f32 = -12.0;
const LOG2_HI: f32 = 4.0;

/// What a config, its choices and a bake were made for.
#[derive(Clone, Debug, PartialEq)]
struct Want {
    path: PathBuf,
    display: String,
    view: String,
    input: String,
}

type Baked = Result<(u64, Arc<Vec<[f32; 4]>>), String>;

/// A table to upload: its description and texels.
pub(super) type Upload = (specular_render::OcioLut, Arc<Vec<[f32; 4]>>);

#[derive(Default)]
pub(super) struct Ocio {
    wanted: Option<Want>,
    config: Option<(PathBuf, Arc<Config>)>,
    /// The table in use, and its texels until they are uploaded.
    lut: Option<specular_render::OcioLut>,
    pending: Option<Arc<Vec<[f32; 4]>>>,
    rx: Option<mpsc::Receiver<Baked>>,
    next_id: u64,
    /// Why OCIO is not in use, when a config was asked for.
    pub(super) error: Option<String>,
    /// The display, view and input colour space in use.
    pub(super) shown: Option<(String, String, String)>,
}

/// The config file asked for: the setting, else `$OCIO`.
fn config_path(setting: Option<&PathBuf>) -> Result<Option<PathBuf>, String> {
    if let Some(p) = setting.filter(|p| !p.as_os_str().is_empty()) {
        return Ok(Some(p.clone()));
    }
    match std::env::var_os("OCIO") {
        Some(v) if !v.is_empty() => {
            let s = v.to_string_lossy();
            if s.starts_with("ocio://") {
                return Err(t!("ocio-builtin-config"));
            }
            Ok(Some(PathBuf::from(v)))
        }
        _ => Ok(None),
    }
}

impl App {
    /// Follows the OCIO settings: reads the config when it changes, offers its names, bakes
    /// the chosen view on a worker, and takes a finished bake.
    pub(super) fn ocio_tick(&mut self) {
        if let Some(rx) = &self.ocio.rx {
            match rx.try_recv() {
                Ok(Ok((id, texels))) => {
                    self.ocio.lut = Some(specular_render::OcioLut {
                        id,
                        size: SIZE as u32,
                        log2_lo: LOG2_LO,
                        log2_hi: LOG2_HI,
                    });
                    self.ocio.pending = Some(texels);
                    self.ocio.rx = None;
                    self.redraw = true;
                }
                Ok(Err(e)) => {
                    self.ocio_failed(e);
                    self.ocio.rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.ocio.rx = None,
            }
        }
        let c = &self.settings.colour;
        let path = match config_path(c.ocio_config.as_ref()) {
            Ok(p) => p,
            Err(e) => {
                if self.ocio.error.as_deref() != Some(e.as_str()) {
                    self.ocio_off();
                    self.ocio_failed(e);
                }
                return;
            }
        };
        let Some(path) = path else {
            if self.ocio.wanted.is_some() || self.ocio.lut.is_some() || self.ocio.error.is_some() {
                self.ocio_off();
                self.ocio.error = None;
            }
            return;
        };
        let want = Want {
            path,
            display: c.ocio_display.clone(),
            view: c.ocio_view.clone(),
            input: c.ocio_input.clone(),
        };
        if self.ocio.wanted.as_ref() == Some(&want) {
            return;
        }
        self.ocio.wanted = Some(want.clone());
        let cfg = match &self.ocio.config {
            Some((p, c)) if *p == want.path => c.clone(),
            _ => match Config::load(&want.path) {
                Ok(c) => {
                    let c = Arc::new(c);
                    self.ocio.config = Some((want.path.clone(), c.clone()));
                    c
                }
                Err(e) => {
                    self.ocio.config = None;
                    self.ocio_off_keep_want();
                    self.ocio_failed(e.to_string());
                    return;
                }
            },
        };
        // The names on offer, and the ones in use (the settings', else the defaults).
        let displays = cfg.display_names();
        let display = Some(want.display.clone())
            .filter(|d| displays.contains(d))
            .or_else(|| displays.first().cloned())
            .unwrap_or_default();
        let views: Vec<String> = cfg.views(&display).iter().map(|v| v.name.clone()).collect();
        let view = Some(want.view.clone())
            .filter(|v| views.contains(v))
            .or_else(|| views.first().cloned())
            .unwrap_or_default();
        let inputs = cfg.input_names();
        let input = Some(want.input.clone())
            .filter(|i| cfg.colorspace(i).is_some())
            .or_else(|| cfg.role("scene_linear").map(str::to_owned))
            .or_else(|| inputs.first().cloned())
            .unwrap_or_default();
        let pairs = |v: &[String]| Some(v.iter().map(|n| (n.clone(), n.clone())).collect());
        self.chrome
            .set_setting_choices("colour.ocio_display", pairs(&displays));
        self.chrome
            .set_setting_choices("colour.ocio_view", pairs(&views));
        self.chrome
            .set_setting_choices("colour.ocio_input", pairs(&inputs));
        self.ocio.error = None;
        self.ocio.shown = Some((display.clone(), view.clone(), input.clone()));
        self.ocio.next_id += 1;
        let id = self.ocio.next_id;
        let (tx, rx) = mpsc::channel();
        self.ocio.rx = Some(rx);
        self.loader.background("ocio-bake", move |l| {
            let t0 = std::time::Instant::now();
            let what = format!("{input} → {display} / {view}");
            let r = Processor::display_view(&cfg, &input, &display, &view)
                .map(|p| {
                    let b = bake(&p, SIZE, LOG2_LO, LOG2_HI);
                    (id, Arc::new(b.texels))
                })
                .map_err(|e| e.to_string());
            specular_log::debug!(
                target: specular_log::IO,
                "ocio: {} baked in {:.0} ms",
                what,
                t0.elapsed().as_secs_f64() * 1e3
            );
            let _ = tx.send(r);
            (l.waker())();
        });
    }

    fn ocio_off_keep_want(&mut self) {
        self.ocio.lut = None;
        self.ocio.pending = None;
        self.ocio.rx = None;
        self.ocio.shown = None;
        self.redraw = true;
    }

    fn ocio_off(&mut self) {
        self.ocio_off_keep_want();
        self.ocio.wanted = None;
        for k in [
            "colour.ocio_display",
            "colour.ocio_view",
            "colour.ocio_input",
        ] {
            self.chrome.set_setting_choices(k, None);
        }
    }

    /// Says once, in plain words, why OCIO is not in use.
    fn ocio_failed(&mut self, reason: String) {
        if self.ocio.error.as_deref() == Some(reason.as_str()) {
            return;
        }
        self.ocio.lut = None;
        self.ocio.shown = None;
        let now = self.now();
        self.chrome
            .warning(t!("ocio-failed", reason = reason.clone()), now);
        self.ocio.error = Some(reason);
        self.redraw = true;
    }

    /// The table for the current image (when it is scene-linear), and a new table to
    /// upload first.
    pub(super) fn ocio_for_current(
        &mut self,
    ) -> (Option<specular_render::OcioLut>, Option<Upload>) {
        let lut = self.ocio.lut;
        let pending = lut.zip(self.ocio.pending.take());
        let scene_linear = self
            .current_id()
            .and_then(|id| self.images.get(&id))
            .and_then(|i| i.info.as_ref())
            .is_some_and(|i| {
                use specular_decode::Format as F;
                matches!(i.format, F::Exr | F::Hdr | F::Pfm)
            });
        (lut.filter(|_| scene_linear), pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_setting_or_env() {
        let p = PathBuf::from("/studio/config.ocio");
        assert_eq!(config_path(Some(&p)).unwrap(), Some(p));
        assert_eq!(
            config_path(Some(&PathBuf::new())).ok().flatten().is_some(),
            std::env::var_os("OCIO")
                .is_some_and(|v| !v.is_empty() && !v.to_string_lossy().starts_with("ocio://"))
        );
    }
}
