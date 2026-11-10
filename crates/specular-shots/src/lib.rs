//! Screenshot tests of the chrome: draw lists rendered offscreen
//! through the real pipeline (instanced UI shader, glass blur, final pass) at a given scale
//! factor, read back and compared with committed PNG goldens.
//!
//! **Determinism.** Every shot is rendered on the software Vulkan rasteriser (Mesa's lavapipe,
//! `mesa-vulkan-drivers` on Debian/Ubuntu), never on the machine's GPU, so a golden made on one
//! machine matches on another. Text uses the bundled fonts only ([`Text::bundled_only`]). The
//! final pass is told the output has 16 bits, which turns its blue-noise dither into less than
//! a hundredth of an 8-bit step: the dither is tested elsewhere, and its noise would make the
//! goldens several times larger.
//!
//! Without lavapipe the tests print why and pass, unless `SPECULAR_SHOTS_REQUIRE=1` (set in CI)
//! makes a missing adapter a failure.
//!
//! **Tolerance.** A pixel differs when one of its channels is more than [`PIXEL_TOLERANCE`]
//! away from the golden's; a shot fails when more than one pixel in [`MAX_DIFFERING`] differs.
//! That absorbs rounding differences between lavapipe builds (LLVM versions, CPU features)
//! while a moved control, a changed colour or a different string still fails.
//!
//! **Failures** write `<name>.actual.png` and `<name>.diff.png` (differing pixels red over a
//! dimmed copy of the golden) to `<target dir>/shots/<suite>/`.
//!
//! **Languages.** A shot drawn in another language than English must not show strings made
//! in English that the language translates: they were made before
//! [`I18n::with_thread_language`] took effect ([`Goldens::check`] fails, naming them). A golden named
//! for a right-to-left language (a `he`, `ar` or `rtl` part in its name) must be drawn in one
//! ([`Goldens::check`]); a reading order of right to left (manga spreads) says so in
//! [`READING_ORDER`].
//!
//! **Blessing.** `SPECULAR_BLESS=1 cargo test -p <crate> --test shots` (or `--bin specular` for
//! the gallery's shots) rewrites every golden that is missing or has a pixel past the
//! tolerance (passing allows up to [`MAX_DIFFERING`] such pixels; blessing rewrites from one,
//! so a changed glyph is written); review the new PNGs before committing them.
//! `SPECULAR_SHOTS_DRIFT=1` lists shots that differ from their goldens within tolerance.
//!
//! **Signs.** A shot that draws a negative number with a hyphen-minus fails ([`hyphen_sign`]):
//! signed values take a true minus (U+2212).

use std::path::{Path, PathBuf};

use specular_gfx::wgpu;
use specular_gfx::{Compositor, Crossfade, DrawList, Gpu, Text, UiRenderer};
use specular_i18n::I18n;

/// Largest channel difference (of 255) that still counts as the same pixel.
pub const PIXEL_TOLERANCE: u8 = 16;
/// A shot passes with up to `pixels / MAX_DIFFERING` differing pixels (0.05 %).
pub const MAX_DIFFERING: usize = 2000;

/// The output format of the final pass.
const OUT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A rendered frame: tightly packed RGB8 rows, top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    /// The UI language the shot was drawn in (its BCP 47 tag).
    pub language: String,
    /// Strings it drew that were made in English although its language translates them
    /// ([`specular_i18n::scope::strays`]): [`Goldens::check`] fails on them.
    pub strays: Vec<String>,
    /// Strings it drew with a hyphen-minus as a number's sign ([`hyphen_sign`]): signed
    /// values take a true minus (U+2212, `specular_i18n::signed_number`), and
    /// [`Goldens::check`] fails on them.
    pub signs: Vec<String>,
}

/// Whether `text` is a signed value written with a hyphen-minus: a short value ("-0.30 EV",
/// "-12", "-3°", isolated or not) that starts with `-` and a digit. Signed values take a true
/// minus (`specular_i18n::signed_number`). Longer texts with a negative number in them
/// (data, file names, dates) are not checked.
pub fn hyphen_sign(text: &str) -> bool {
    let c: Vec<char> = text
        .chars()
        .filter(|c| !matches!(c, '\u{200e}' | '\u{200f}' | '\u{2066}'..='\u{2069}'))
        .collect();
    let digit_next = match c.get(1) {
        Some(d) => d.is_ascii_digit() || (*d == '.' && c.get(2).is_some_and(char::is_ascii_digit)),
        None => false,
    };
    c.first() == Some(&'-') && digit_next && c.len() <= 16
}

/// Goldens (by the start of their names) whose `rtl` is the reading order of a book, not
/// the UI's direction: drawn in any language.
pub const READING_ORDER: &[&str] = &["spread-rtl"];

/// Whether golden `name` is named for a right-to-left UI (a `he`, `ar` or `rtl` part).
fn rtl_name(name: &str) -> bool {
    !READING_ORDER.iter().any(|p| name.starts_with(p))
        && name
            .split(['-', '_'])
            .any(|p| matches!(p, "he" | "ar" | "rtl"))
}

/// Whether `tag` is a right-to-left language.
fn rtl_language(tag: &str) -> bool {
    matches!(tag.split('-').next(), Some("he" | "ar" | "fa" | "ur"))
}

/// The software GPU, the renderers and the text system, reused across shots.
pub struct Harness {
    pub gpu: Gpu,
    pub comp: Compositor,
    pub ui: UiRenderer,
    pub text: Text,
    /// Texts shots draw as data (a folder's or a font's own name) that read like an English
    /// message: not strings made outside the language ([`Harness::data_text`]).
    data: Vec<String>,
}

impl std::fmt::Debug for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Harness").field("gpu", &self.gpu).finish()
    }
}

/// `SPECULAR_<name>=1`.
fn flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| v == "1")
}

/// The software Vulkan adapter (lavapipe), if installed.
fn software_gpu() -> Result<Gpu, String> {
    let t0 = std::time::Instant::now();
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.flags = wgpu::InstanceFlags::empty().with_env();
    desc.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(desc);
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
    let names: Vec<String> = adapters.iter().map(|a| a.get_info().name).collect();
    let adapter = adapters
        .into_iter()
        .find(|a| a.get_info().device_type == wgpu::DeviceType::Cpu)
        .ok_or_else(|| {
            format!("no software Vulkan adapter (lavapipe; mesa-vulkan-drivers) among {names:?}")
        })?;
    let info = adapter.get_info();
    let limits = wgpu::Limits::default();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("specular-shots"),
        required_features: wgpu::Features::empty(),
        required_limits: limits.clone(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("cannot open {}: {e}", info.name))?;
    Ok(Gpu {
        instance,
        adapter,
        device,
        queue,
        info,
        limits,
        init_ms: t0.elapsed().as_secs_f64() * 1000.0,
        stages: Vec::new(),
        lost: Default::default(),
    })
}

impl Harness {
    /// The harness on lavapipe; `None` (after saying why) when there is none and
    /// `SPECULAR_SHOTS_REQUIRE` is not set.
    pub fn new() -> Option<Harness> {
        let gpu = match software_gpu() {
            Ok(g) => g,
            Err(e) if flag("SPECULAR_SHOTS_REQUIRE") => {
                panic!("SPECULAR_SHOTS_REQUIRE=1 but {e}")
            }
            Err(e) => {
                eprintln!("screenshot tests skipped: {e}");
                return None;
            }
        };
        let comp = Compositor::new(&gpu, OUT, 16, 64, 64);
        let ui = UiRenderer::new(&gpu, &comp);
        let mut text = Text::bundled_only();
        text.record(true);
        specular_i18n::scope::trace(true);
        Some(Harness {
            gpu,
            comp,
            ui,
            text,
            data: Vec::new(),
        })
    }

    /// `text` is data in the shots to come (a folder named "Export", a font's "Light"), not
    /// a string made in English outside the shot's language.
    pub fn data_text(&mut self, text: &str) {
        self.data.push(text.to_owned());
    }

    /// Renders one frame of `logical` size (logical px) at `scale` physical px per logical px.
    /// `scene` draws what stands for the image (under the chrome, and what glass blurs);
    /// `chrome` draws the chrome. Both draw in logical px.
    pub fn render(
        &mut self,
        logical: (f32, f32),
        scale: f32,
        scene: impl FnOnce(&mut DrawList),
        chrome: impl FnOnce(&mut DrawList, &mut Text),
    ) -> Shot {
        let size = self.prepare(logical, scale);
        self.draw_scene(size, scale, scene);
        self.compose(size, scale, None, chrome)
    }

    /// Like [`Harness::render`], with a scene drawn on the GPU by `scene` (the image renderer)
    /// into the cleared scene target of the given physical size.
    pub fn render_gpu(
        &mut self,
        logical: (f32, f32),
        scale: f32,
        scene: impl FnOnce(&Gpu, &mut wgpu::CommandEncoder, &wgpu::TextureView, (u32, u32)),
        chrome: impl FnOnce(&mut DrawList, &mut Text),
    ) -> Shot {
        let size = self.prepare(logical, scale);
        self.draw_scene(size, scale, |_| {});
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        scene(&self.gpu, &mut enc, &self.comp.scene.view, size);
        self.comp.blur(&mut enc);
        self.gpu.queue.submit([enc.finish()]);
        self.compose(size, scale, None, chrome)
    }

    /// A frame of the slideshow's cross-fade: `from` is the outgoing scene, `to` the incoming
    /// one, `k` the outgoing scene's weight (0.5: the midpoint); then the chrome, as in
    /// [`Harness::render`].
    pub fn render_crossfade(
        &mut self,
        logical: (f32, f32),
        scale: f32,
        from: impl FnOnce(&mut DrawList),
        to: impl FnOnce(&mut DrawList),
        k: f32,
        chrome: impl FnOnce(&mut DrawList, &mut Text),
    ) -> Shot {
        let size = self.prepare(logical, scale);
        self.draw_scene(size, scale, from);
        let mut fade = Crossfade::new(&self.gpu);
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        fade.snapshot(&self.gpu, &mut enc, &self.comp);
        self.gpu.queue.submit([enc.finish()]);
        self.draw_scene(size, scale, to);
        self.compose(size, scale, Some((&fade, k)), chrome)
    }

    /// The physical size of a `logical` frame at `scale`; the targets follow it.
    fn prepare(&mut self, logical: (f32, f32), scale: f32) -> (u32, u32) {
        let size = (
            (logical.0 * scale).round() as u32,
            (logical.1 * scale).round() as u32,
        );
        if self.comp.size() != size {
            self.comp.resize(&self.gpu, size.0, size.1);
            self.ui.compositor_resized(&self.gpu, &self.comp);
        }
        size
    }

    /// The scene: cleared, then drawn with the UI shader, and blurred for the glass. Its own
    /// submission, so the UI renderer's staging belt is free again for the chrome.
    fn draw_scene(&mut self, size: (u32, u32), scale: f32, scene: impl FnOnce(&mut DrawList)) {
        let d = self.gpu.device.clone();
        let q = self.gpu.queue.clone();
        let mut dl = DrawList::new(scale);
        scene(&mut dl);
        let mut enc = d.create_command_encoder(&Default::default());
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shot clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.comp.scene.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.text.begin_frame(scale);
        self.ui.render(
            &self.gpu,
            &mut enc,
            &self.comp.scene.view,
            size,
            &dl,
            &mut self.text,
        );
        self.comp.blur(&mut enc);
        q.submit([enc.finish()]);
        self.text.end_frame();
    }

    /// The chrome over the scene (and the outgoing scene faded over it), then the final pass
    /// into a texture that is read back.
    fn compose(
        &mut self,
        size: (u32, u32),
        scale: f32,
        fade: Option<(&Crossfade, f32)>,
        chrome: impl FnOnce(&mut DrawList, &mut Text),
    ) -> Shot {
        let d = self.gpu.device.clone();
        let q = self.gpu.queue.clone();
        let mut dl = DrawList::new(scale);
        self.text.begin_frame(scale);
        chrome(&mut dl, &mut self.text);
        // Strings made in English, drawn in a language that translates them.
        let language = I18n::global().language();
        let recorded = self.text.take_recorded();
        let signs: Vec<String> = recorded
            .iter()
            .filter(|t| hyphen_sign(t) && !self.data.iter().any(|d| d == *t))
            .cloned()
            .collect();
        let mut strays = specular_i18n::scope::strays(&recorded, &language);
        strays.retain(|s| !self.data.iter().any(|d| s.starts_with(&format!("{d:?} ("))));
        let mut enc = d.create_command_encoder(&Default::default());
        self.comp.begin(&mut enc);
        if let Some((f, k)) = fade {
            f.draw(&mut enc, &self.comp, k);
        }
        self.ui.render(
            &self.gpu,
            &mut enc,
            &self.comp.comp.view,
            size,
            &dl,
            &mut self.text,
        );
        let out = d.create_texture(&wgpu::TextureDescriptor {
            label: Some("shot"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.comp
            .finish(&mut enc, &out.create_view(&Default::default()));
        let row = (size.0 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buf = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shot readback"),
            size: u64::from(row) * u64::from(size.1),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            out.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(size.1),
                },
            },
            out.size(),
        );
        q.submit([enc.finish()]);
        self.text.end_frame();
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        d.poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU readback");
        let data = buf.slice(..).get_mapped_range().expect("mapped readback");
        let mut rgb = Vec::with_capacity(size.0 as usize * size.1 as usize * 3);
        for r in data.chunks_exact(row as usize) {
            for px in r[..size.0 as usize * 4].chunks_exact(4) {
                rgb.extend_from_slice(&px[..3]);
            }
        }
        Shot {
            width: size.0,
            height: size.1,
            rgb,
            language: language.to_string(),
            strays,
            signs,
        }
    }
}

/// Where failing shots go: `<target dir>/shots/<suite>`, found from the test binary's path
/// (`<target dir>/<profile>/deps/<binary>`), so it follows `CARGO_TARGET_DIR`.
fn out_dir(suite: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    let target = exe
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == "deps"))
        .and_then(|deps| deps.parent()?.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("target"));
    target.join("shots").join(suite)
}

/// How far a shot is from its golden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delta {
    /// Pixels with a channel more than [`PIXEL_TOLERANCE`] off.
    pub differing: usize,
    /// The largest channel difference anywhere.
    pub max: u8,
}

/// Compares two shots of the same size.
pub fn delta(a: &Shot, b: &Shot) -> Delta {
    let mut d = Delta {
        differing: 0,
        max: 0,
    };
    for (p, q) in a.rgb.chunks_exact(3).zip(b.rgb.chunks_exact(3)) {
        let m = (0..3).map(|c| p[c].abs_diff(q[c])).max().unwrap_or(0);
        d.max = d.max.max(m);
        d.differing += usize::from(m > PIXEL_TOLERANCE);
    }
    d
}

/// Differing pixels red, the rest a dimmed grey copy of `golden`.
fn diff_image(golden: &Shot, actual: &Shot) -> Vec<u8> {
    let mut out = Vec::with_capacity(golden.rgb.len());
    for (p, q) in golden.rgb.chunks_exact(3).zip(actual.rgb.chunks_exact(3)) {
        let m = (0..3).map(|c| p[c].abs_diff(q[c])).max().unwrap_or(0);
        if m > PIXEL_TOLERANCE {
            out.extend_from_slice(&[255, 0, 0]);
        } else {
            let g = ((u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 9) as u8;
            let r = g.saturating_add(m.saturating_mul(8));
            out.extend_from_slice(&[r, g, g]);
        }
    }
    out
}

/// The `png` crate at its strongest setting (goldens about a third smaller than with
/// `specular-png`'s fast encoder); read back with `specular-png`.
fn encode(s: &Shot, rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, s.width, s.height);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    enc.set_filter(png::Filter::Adaptive);
    let mut w = enc.write_header().expect("PNG header");
    w.write_image_data(rgb).expect("encode PNG");
    w.finish().expect("finish PNG");
    out
}

fn decode(png: &[u8]) -> Result<Shot, String> {
    let img = specular_png::decode(png, &Default::default()).map_err(|e| e.to_string())?;
    let rgb = img
        .to_rgba8()
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    Ok(Shot {
        width: img.width,
        height: img.height,
        rgb,
        language: String::new(),
        strays: Vec::new(),
        signs: Vec::new(),
    })
}

/// A set of goldens in one directory. [`Goldens::check`] each shot, then [`Goldens::finish`],
/// which fails the test with every mismatch at once.
#[derive(Debug)]
pub struct Goldens {
    dir: PathBuf,
    out: PathBuf,
    bless: bool,
    failures: Vec<String>,
    blessed: Vec<String>,
    checked: usize,
}

impl Goldens {
    /// Goldens in `dir` (committed); failures go to `<target dir>/shots/<suite>`.
    pub fn new(dir: impl Into<PathBuf>, suite: &str) -> Goldens {
        Goldens {
            dir: dir.into(),
            out: out_dir(suite),
            bless: flag("SPECULAR_BLESS"),
            failures: Vec::new(),
            blessed: Vec::new(),
            checked: 0,
        }
    }

    /// Compares `shot` with `<dir>/<name>.png`.
    pub fn check(&mut self, name: &str, shot: &Shot) {
        self.checked += 1;
        if rtl_name(name) && !rtl_language(&shot.language) {
            self.failures.push(format!(
                "{name}: named for a right-to-left language but drawn in {} (draw it inside \
                 I18n::with_thread_language)",
                shot.language
            ));
            return;
        }
        if !shot.strays.is_empty() {
            self.failures.push(format!(
                "{name}: drawn in {} with strings made in English (outside \
                 I18n::with_thread_language): {}",
                shot.language,
                shot.strays.join(", ")
            ));
            return;
        }
        if !shot.signs.is_empty() {
            self.failures.push(format!(
                "{name}: a hyphen-minus as a number's sign (use specular_i18n::signed_number, or \
                 Harness::data_text for data shown as it is): {:?}",
                shot.signs
            ));
            return;
        }
        let path = self.dir.join(format!("{name}.png"));
        let golden = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| decode(&b));
        let problem = match &golden {
            Err(e) => Some(format!("{name}: no golden ({}: {e})", path.display())),
            Ok(g) if (g.width, g.height) != (shot.width, shot.height) => Some(format!(
                "{name}: {}x{} but the golden is {}x{}",
                shot.width, shot.height, g.width, g.height
            )),
            Ok(g) => {
                let d = delta(g, shot);
                let allowed = (shot.rgb.len() / 3) / MAX_DIFFERING;
                if d.differing > 0 && flag("SPECULAR_SHOTS_DRIFT") {
                    // Within tolerance but not the same: a list to look at.
                    eprintln!(
                        "shot drift: {name}: {} pixels differ, largest {}",
                        d.differing, d.max
                    );
                }
                // Blessing rewrites any golden that differs at all (a "-" turned "−" stays
                // within tolerance); passing still allows the tolerance.
                if self.bless && d.differing > 0 {
                    return self.rewrite(name, &path, shot);
                }
                (d.differing > allowed).then(|| {
                    format!(
                        "{name}: {} pixels differ by more than {PIXEL_TOLERANCE} (allowed {allowed}), \
                         largest difference {}",
                        d.differing, d.max
                    )
                })
            }
        };
        let Some(problem) = problem else { return };
        if self.bless {
            self.rewrite(name, &path, shot);
            return;
        }
        std::fs::create_dir_all(&self.out).expect("create the shots output directory");
        std::fs::write(
            self.out.join(format!("{name}.actual.png")),
            encode(shot, &shot.rgb),
        )
        .expect("write the actual shot");
        if let Ok(g) = &golden
            && (g.width, g.height) == (shot.width, shot.height)
        {
            std::fs::write(
                self.out.join(format!("{name}.diff.png")),
                encode(shot, &diff_image(g, shot)),
            )
            .expect("write the diff");
        }
        self.failures.push(problem);
    }

    fn rewrite(&mut self, name: &str, path: &Path, shot: &Shot) {
        std::fs::create_dir_all(&self.dir).expect("create the goldens directory");
        std::fs::write(path, encode(shot, &shot.rgb)).expect("write golden");
        self.blessed.push(name.to_owned());
    }

    /// Fails the test if any shot did not match.
    pub fn finish(self) {
        if !self.blessed.is_empty() {
            eprintln!(
                "blessed {} of {} goldens in {}: {}",
                self.blessed.len(),
                self.checked,
                self.dir.display(),
                self.blessed.join(", ")
            );
        }
        assert!(
            self.failures.is_empty(),
            "{} of {} screenshots differ from their goldens (actual and diff images in {}; \
             if the change is intended, rerun with SPECULAR_BLESS=1 and review the PNGs):\n  {}",
            self.failures.len(),
            self.checked,
            self.out.display(),
            self.failures.join("\n  ")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(px: &[[u8; 3]]) -> Shot {
        Shot {
            width: px.len() as u32,
            height: 1,
            rgb: px.concat(),
            language: "en-US".into(),
            strays: Vec::new(),
            signs: Vec::new(),
        }
    }

    #[test]
    fn right_to_left_goldens_are_known_by_name() {
        for n in [
            "he-confirm-remove-light",
            "gallery-similar-he-list-1x",
            "omero-edit-rtl-light-1x",
        ] {
            assert!(rtl_name(n), "{n}");
        }
        for n in [
            "spread-rtl-1x",
            "spread-rtl-fit-width-1x",
            "help-dark",
            "theme-light",
            "shell-1x",
            "earth-2x",
        ] {
            assert!(!rtl_name(n), "{n}");
        }
        assert!(rtl_language("he") && rtl_language("ar-EG") && !rtl_language("en-US"));
    }

    #[test]
    fn a_hyphen_as_a_sign_is_found() {
        for t in [
            "-0.30 EV",
            "-1",
            "-.5",
            "\u{2066}-3\u{2069}",
            "-12 dB",
            "-7°",
        ] {
            assert!(hyphen_sign(t), "{t}");
        }
        for t in [
            "\u{2212}0.30 EV",
            "+0.30 EV",
            "2026-11-01",
            "IMG-2041.jpg",
            "pages 3-5",
            "UTC-5",
            "Ctrl+-",
            "x = -1",
            "-12 to 12 over 24 bins of width",
        ] {
            assert!(!hyphen_sign(t), "{t}");
        }
    }

    #[test]
    fn small_differences_are_noise() {
        let a = shot(&[[10, 20, 30], [200, 200, 200]]);
        let b = shot(&[[26, 4, 30], [200, 183, 200]]);
        assert_eq!(
            delta(&a, &b),
            Delta {
                differing: 1,
                max: 17
            }
        );
    }

    #[test]
    fn goldens_round_trip_through_png() {
        let s = Shot {
            width: 3,
            height: 2,
            rgb: (0..18).map(|i| i * 14).collect(),
            language: String::new(),
            strays: Vec::new(),
            signs: Vec::new(),
        };
        assert_eq!(decode(&encode(&s, &s.rgb)).unwrap(), s);
    }
}
