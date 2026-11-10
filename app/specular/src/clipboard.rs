//! Copy image / copy path, and move-to-trash with undo. All blocking work runs on
//! background threads; results come back as loader events.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster};
use specular_decode::{DecodeRequest, Session};

use crate::loader::{Copied, Event, Loader};

/// Images larger than this are copied at a reduced scale (the clipboard holds RGBA8).
const MAX_COPY_PIXELS: u64 = 120_000_000;

/// The clipboard must stay alive while it owns the data (X11/Wayland serve it on request).
#[derive(Clone, Default)]
pub struct Clipboard(Arc<Mutex<Option<arboard::Clipboard>>>);

impl Clipboard {
    fn with<R>(
        &self,
        f: impl FnOnce(&mut arboard::Clipboard) -> Result<R, arboard::Error>,
    ) -> Result<R, String> {
        // Tests never touch the desktop session's clipboard (text fields are tested with an
        // in-memory one: `specular_widgets::line_edit::MemoryClipboard`).
        if cfg!(test) {
            return Err("no system clipboard in tests".into());
        }
        let mut g = self.0.lock();
        if g.is_none() {
            *g = Some(arboard::Clipboard::new().map_err(|e| crate::error_text::reason(&e))?);
        }
        f(g.as_mut().expect("clipboard")).map_err(|e| crate::error_text::reason(&e))
    }

    /// What the clipboard holds that can be opened: files (a file list, or text that is a list
    /// of `file://` URIs or existing paths), else an image. Blocking: call it off the UI
    /// thread.
    pub fn read_paste(&self) -> Result<Pasted, String> {
        if let Ok(files) = self.with(|c| c.get().file_list())
            && !files.is_empty()
        {
            // arboard splits `text/uri-list` at LF only: drop the CR of CRLF lines.
            let files = files
                .into_iter()
                .map(|p| match p.to_str() {
                    Some(s) if s.ends_with(['\r', ' ']) => PathBuf::from(s.trim_end()),
                    _ => p,
                })
                .collect();
            return Ok(Pasted::Files(files));
        }
        if let Ok(text) = self.with(|c| c.get_text()) {
            if crate::zarr_remote::is_remote_zarr(&text) || is_stream_address(&text) {
                return Ok(Pasted::Address(text.trim().to_owned()));
            }
            let files = paths_in_text(&text);
            if !files.is_empty() {
                return Ok(Pasted::Files(files));
            }
        }
        match self.with(|c| c.get_image()) {
            Ok(img) => Ok(Pasted::Image {
                width: img.width,
                height: img.height,
                rgba: img.bytes.into_owned(),
            }),
            Err(e) => Err(e),
        }
    }

    /// The clipboard's text (the settings.toml editor's paste). Blocking: call it off the UI
    /// thread.
    pub fn read_text(&self) -> Result<String, String> {
        self.with(|c| c.get_text())
    }

    /// The primary selection's text (X11 and Wayland: what was last selected, which a middle
    /// click pastes). Blocking: call it off the UI thread. Other platforms have none.
    pub fn read_primary(&self) -> Result<String, String> {
        #[cfg(all(
            unix,
            not(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "android",
                target_os = "emscripten"
            ))
        ))]
        {
            use arboard::{GetExtLinux, LinuxClipboardKind};
            self.with(|c| c.get().clipboard(LinuxClipboardKind::Primary).text())
        }
        #[cfg(not(all(
            unix,
            not(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "android",
                target_os = "emscripten"
            ))
        )))]
        {
            Err("no primary selection on this platform".into())
        }
    }

    /// Makes `text` the primary selection (a text field's copy-on-select), in the background
    /// and silently: a failure is only logged. Nothing happens on other platforms.
    pub fn set_primary(&self, loader: &Loader, text: String) {
        #[cfg(all(
            unix,
            not(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "android",
                target_os = "emscripten"
            ))
        ))]
        {
            let me = self.clone();
            loader.background("clip-primary", move |_| {
                use arboard::{LinuxClipboardKind, SetExtLinux};
                if let Err(e) =
                    me.with(|c| c.set().clipboard(LinuxClipboardKind::Primary).text(text))
                {
                    specular_log::debug!(target: specular_log::UI, "primary selection: {e}");
                }
            });
        }
        #[cfg(not(all(
            unix,
            not(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "android",
                target_os = "emscripten"
            ))
        )))]
        {
            let _ = (loader, text);
        }
    }

    pub fn copy_path(&self, loader: &Loader, path: &Path) {
        self.copy_text(loader, path.display().to_string(), Copied::Path);
    }

    pub fn copy_text(&self, loader: &Loader, text: String, what: Copied) {
        let me = self.clone();
        loader.background("clip-copy", move |l| {
            let r = me.with(|c| c.set_text(text));
            l.post(Event::Copied { what, result: r });
        });
    }

    /// Puts straight RGBA8 (rows top to bottom) on the clipboard as an image (the desktop
    /// offers it as PNG) and posts the result as `what`. Blocking: call it off the UI thread.
    pub fn put_rgba(
        &self,
        loader: &Loader,
        width: usize,
        height: usize,
        rgba: Vec<u8>,
        what: Copied,
    ) {
        let r = self.with(|c| {
            c.set_image(arboard::ImageData {
                width,
                height,
                bytes: rgba.into(),
            })
        });
        loader.post(Event::Copied { what, result: r });
    }

    pub fn copy_image(&self, loader: &Loader, session: Arc<dyn Session>, orient: Orientation) {
        let me = self.clone();
        loader.background("clip-copy", move |l| {
            let info = session.info();
            let px = info.width as u64 * info.height as u64;
            let mut scale = 1;
            while px / (scale * scale) as u64 > MAX_COPY_PIXELS && scale < 8 {
                scale *= 2;
            }
            let r = session
                .decode(&DecodeRequest::full(scale), &CancelToken::new())
                .map_err(|e| crate::error_text::reason(&e))
                .and_then(|r| {
                    let (w, h, rgba) = oriented_rgba(&r, orient);
                    me.with(|c| {
                        c.set_image(arboard::ImageData {
                            width: w,
                            height: h,
                            bytes: rgba.into(),
                        })
                    })
                });
            l.post(Event::Copied {
                what: Copied::Image,
                result: r,
            });
        });
    }
}

/// A network stream's address (a media file, playlist, HLS or DASH by its extension).
pub fn is_stream_address(text: &str) -> bool {
    use specular_stream::addr::{self, Guess};
    let t = text.trim();
    addr::is_stream_url(t)
        && matches!(
            addr::guess(t),
            Some(Guess::Hls | Guess::Dash | Guess::Media | Guess::Playlist)
        )
}

/// What [`Clipboard::read_paste`] found.
#[derive(Debug, PartialEq)]
pub enum Pasted {
    Files(Vec<PathBuf>),
    /// A remote Zarr store's address (`https://…/x.zarr`, `s3://…`), or a network stream's.
    Address(String),
    /// Straight RGBA8, rows top to bottom.
    Image {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
}

/// Files named by clipboard text: `text/uri-list` lines (`#` comments skipped) or plain paths,
/// one per line. Only text where every line names an existing file or folder counts, so a
/// copied sentence is never taken for a path.
pub fn paths_in_text(text: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let p = if line.starts_with("file:") {
            match specular_platform::launch::path_from_uri(line) {
                Some(p) => p,
                None => return Vec::new(),
            }
        } else {
            PathBuf::from(line)
        };
        if !p.is_absolute() || !p.exists() {
            return Vec::new();
        }
        out.push(p);
    }
    out
}

fn clamp8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Straight RGBA8 of a raster (nearest chroma upsampling is plenty for the clipboard).
pub fn to_rgba(r: &Raster) -> Vec<u8> {
    // A video frame's YCbCr: through R'G'B' (its matrix, range and transfer).
    if let PixelLayout::Yuv(_) = r.layout
        && let Some(rgb) = specular_core::yuv::to_rgb(r)
    {
        return to_rgba(&rgb);
    }
    let (w, h) = (r.width as usize, r.height as usize);
    let mut out = vec![255u8; w * h * 4];
    if w == 0 || h == 0 {
        return out;
    }
    // Row at a time with the layout match and the chroma column mapping hoisted out of the
    // pixel loop: per pixel, the old form re-matched the layout and divided per sample, and
    // a PGO build laid that out worse still (JPEG thumbnails +3.5 %).
    let row = |p: usize, y: usize| -> &[u8] {
        let pl = &r.planes[p];
        let py = (y / pl.v_div.max(1) as usize).min(pl.height.saturating_sub(1) as usize);
        &pl.data[py * pl.stride..]
    };
    // Column of plane `p` for output column x (nearest, clamped to the plane width).
    let cols = |p: usize| -> Vec<usize> {
        let pl = &r.planes[p];
        let (d, last) = (
            pl.h_div.max(1) as usize,
            pl.width.saturating_sub(1) as usize,
        );
        (0..w).map(|x| (x / d).min(last)).collect()
    };
    let rows = out.chunks_exact_mut(w * 4).enumerate();
    match r.layout {
        PixelLayout::Gray => {
            let c0 = cols(0);
            for (y, o) in rows {
                let s = row(0, y);
                for (px, &x) in o.chunks_exact_mut(4).zip(&c0) {
                    px[..3].fill(s[x]);
                }
            }
        }
        PixelLayout::YCbCr => {
            let (c0, c1, c2) = (cols(0), cols(1), cols(2));
            for (y, o) in rows {
                let (sy, sb, sr) = (row(0, y), row(1, y), row(2, y));
                for (px, ((&x0, &x1), &x2)) in
                    o.chunks_exact_mut(4).zip(c0.iter().zip(&c1).zip(&c2))
                {
                    let yy = sy[x0] as f32;
                    let cb = sb[x1] as f32 - 128.0;
                    let cr = sr[x2] as f32 - 128.0;
                    px[0] = clamp8(yy + 1.402 * cr);
                    px[1] = clamp8(yy - 0.344_136 * cb - 0.714_136 * cr);
                    px[2] = clamp8(yy + 1.772 * cb);
                }
            }
        }
        PixelLayout::Rgb => {
            let (c0, c1, c2) = (cols(0), cols(1), cols(2));
            for (y, o) in rows {
                let (s0, s1, s2) = (row(0, y), row(1, y), row(2, y));
                for (px, ((&x0, &x1), &x2)) in
                    o.chunks_exact_mut(4).zip(c0.iter().zip(&c1).zip(&c2))
                {
                    px[0] = s0[x0];
                    px[1] = s1[x1];
                    px[2] = s2[x2];
                }
            }
        }
        PixelLayout::Rgba => {
            let pl = &r.planes[0];
            for (y, o) in rows {
                let s = &pl.data[y * pl.stride..y * pl.stride + w * 4];
                o.copy_from_slice(s);
            }
        }
        // Converted above; planes that would not convert stay opaque white.
        PixelLayout::Yuv(_) => {}
        PixelLayout::Rgba16(_)
        | PixelLayout::Rgba16F(_)
        | PixelLayout::Gray16
        | PixelLayout::GrayAlpha16 => {
            let pl = &r.planes[0];
            let n = r.layout.pixel_bytes();
            let peak = specular_core::hdr::peak_from_nits(None);
            for (y, o) in rows {
                let s = &pl.data[y * pl.stride..y * pl.stride + w * n];
                for (px, sp) in o.chunks_exact_mut(4).zip(s.chunks_exact(n)) {
                    px.copy_from_slice(&specular_core::hdr::wide_to_srgb8(r.layout, sp, peak));
                }
            }
        }
    }
    out
}

/// RGBA8 with the EXIF orientation applied (the clipboard has no orientation tag).
pub fn oriented_rgba(r: &Raster, orient: Orientation) -> (usize, usize, Vec<u8>) {
    let src = to_rgba(r);
    let (w, h) = (r.width as usize, r.height as usize);
    if orient == Orientation::NORMAL {
        return (w, h, src);
    }
    let (q, flip) = orient.to_rotation_flip();
    let (dw, dh) = if q % 2 == 1 { (h, w) } else { (w, h) };
    let mut out = vec![0u8; src.len()];
    for y in 0..h {
        for x in 0..w {
            // Same mapping as the view: flip first, then quarter turns clockwise.
            let (mut px, mut py, mut cw, mut ch) = (x, y, w, h);
            if flip {
                px = cw - 1 - px;
            }
            for _ in 0..q {
                let nx = ch - 1 - py;
                let ny = px;
                px = nx;
                py = ny;
                std::mem::swap(&mut cw, &mut ch);
            }
            let s = (y * w + x) * 4;
            let d = (py * dw + px) * 4;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    (dw, dh, out)
}

/// Moves a file to the trash.
pub fn trash(loader: &Loader, path: PathBuf) {
    loader.background("io-trash", move |l| {
        let result = trash_with_sidecars(&path).map_err(|e| crate::error_text::trash_reason(&e));
        l.post(Event::Trashed { path, result });
    });
}

/// Moves `path` to the trash, then its sidecars (`.xmp`, `.aae`, Develop's
/// `.specular.xmp` and `.vN.specular.xmp`; not the other half of a RAW+JPEG pair). A sidecar
/// that cannot go stays where it is.
pub(crate) fn trash_with_sidecars(path: &Path) -> Result<(), trash::Error> {
    let sidecars = specular_fileops::names::sidecars(path, false);
    trash::delete(path)?;
    for s in sidecars {
        if let Err(e) = trash::delete(&s) {
            specular_log::warn!(target: specular_log::FOLDER, "trash: sidecar {}: {e}", s.display());
        }
    }
    Ok(())
}

/// Whether `name` is the name of a sidecar of the image named `image` (what
/// [`trash_with_sidecars`] takes along).
// Only `restore_now` uses it, and that is not built where the OS restores from its own trash.
#[cfg_attr(
    any(target_os = "macos", target_os = "ios", target_os = "android"),
    allow(dead_code)
)]
pub(crate) fn is_sidecar_of(name: &str, image: &str) -> bool {
    if specular_fileops::names::is_develop_sidecar_of(name, image) {
        return true;
    }
    let stem = Path::new(image)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(image);
    let Some((base, ext)) = name.rsplit_once('.') else {
        return false;
    };
    (ext.eq_ignore_ascii_case("xmp") && (base == image || base == stem))
        || (ext.eq_ignore_ascii_case("aae") && base == stem)
}

/// Restores the most recently trashed item that came from `path`.
pub fn restore(loader: &Loader, path: PathBuf) {
    loader.background("io-trash", move |l| {
        let result = restore_now(&path);
        l.post(Event::Restored { path, result });
    });
}

#[cfg(any(
    target_os = "windows",
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
pub(crate) fn restore_now(path: &Path) -> Result<(), String> {
    let items = trash::os_limited::list().map_err(|e| crate::error_text::trash_reason(&e))?;
    let name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let item = items
        .into_iter()
        .filter(|i| i.name == name && i.original_parent == parent)
        .max_by_key(|i| i.time_deleted)
        .ok_or_else(|| specular_i18n::t!("error-reason-not-in-trash"))?;
    let when = item.time_deleted;
    let image = name.to_string_lossy().into_owned();
    trash::os_limited::restore_all([item]).map_err(|e| crate::error_text::trash_reason(&e))?;
    // Its sidecars, trashed with it (within a minute), unless one is back already.
    let Ok(items) = trash::os_limited::list() else {
        return Ok(());
    };
    let mut newest: std::collections::HashMap<std::ffi::OsString, trash::TrashItem> =
        Default::default();
    for i in items {
        let n = i.name.to_string_lossy().into_owned();
        if i.original_parent != parent
            || (i.time_deleted - when).abs() > 60
            || !is_sidecar_of(&n, &image)
            || parent.join(&i.name).exists()
        {
            continue;
        }
        match newest.get(&i.name) {
            Some(o) if o.time_deleted >= i.time_deleted => {}
            _ => {
                newest.insert(i.name.clone(), i);
            }
        }
    }
    if !newest.is_empty()
        && let Err(e) = trash::os_limited::restore_all(newest.into_values())
    {
        specular_log::warn!(target: specular_log::FOLDER, "restore: sidecars of {image}: {e}");
    }
    Ok(())
}

#[cfg(not(any(
    target_os = "windows",
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
)))]
pub(crate) fn restore_now(_path: &Path) -> Result<(), String> {
    Err("restoring from the trash is not supported on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_core::RasterPlane;

    #[test]
    fn text_names_files_only_when_every_line_does() {
        let here = std::path::absolute(env!("CARGO_MANIFEST_DIR")).unwrap();
        let toml = here.join("Cargo.toml");
        let uri = specular_platform::launch::file_uri(&toml);
        assert_eq!(paths_in_text(&format!("{uri}\r\n")), vec![toml.clone()]);
        assert_eq!(
            paths_in_text(&format!(
                "# copied\n{}\n{}\n",
                toml.display(),
                here.display()
            )),
            vec![toml.clone(), here.clone()]
        );
        assert!(paths_in_text("just some words").is_empty());
        assert!(paths_in_text(&format!("{}\nnot a file", toml.display())).is_empty());
        assert!(
            paths_in_text("Cargo.toml").is_empty(),
            "relative paths are not taken"
        );
        assert!(paths_in_text("").is_empty());
    }

    /// The per-pixel form `to_rgba` replaced, as the reference for its row loops.
    fn to_rgba_reference(r: &Raster) -> Vec<u8> {
        let (w, h) = (r.width as usize, r.height as usize);
        let mut out = vec![255u8; w * h * 4];
        let sample = |p: usize, x: usize, y: usize| -> u8 {
            let pl = &r.planes[p];
            let px = (x / pl.h_div.max(1) as usize).min(pl.width.saturating_sub(1) as usize);
            let py = (y / pl.v_div.max(1) as usize).min(pl.height.saturating_sub(1) as usize);
            pl.data[py * pl.stride + px]
        };
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 4;
                match r.layout {
                    PixelLayout::Gray => out[o..o + 3].fill(sample(0, x, y)),
                    PixelLayout::YCbCr => {
                        let yy = sample(0, x, y) as f32;
                        let cb = sample(1, x, y) as f32 - 128.0;
                        let cr = sample(2, x, y) as f32 - 128.0;
                        out[o] = clamp8(yy + 1.402 * cr);
                        out[o + 1] = clamp8(yy - 0.344_136 * cb - 0.714_136 * cr);
                        out[o + 2] = clamp8(yy + 1.772 * cb);
                    }
                    PixelLayout::Rgb => (0..3).for_each(|c| out[o + c] = sample(c, x, y)),
                    _ => unreachable!(),
                }
            }
        }
        out
    }

    #[test]
    fn to_rgba_matches_the_per_pixel_form() {
        // Odd sizes, 4:2:0 / 4:2:2 / 4:4:4 chroma, padded strides and chroma planes one short
        // (the clamp to the plane edge).
        let plane = |w: u32, h: u32, hd: u8, vd: u8, seed: u32| {
            let stride = w as usize + 3;
            let data = (0..stride * h as usize)
                .map(|i| ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(seed) >> 13) as u8)
                .collect();
            RasterPlane {
                width: w,
                height: h,
                stride,
                data,
                h_div: hd,
                v_div: vd,
            }
        };
        let (w, h) = (37u32, 23u32);
        for (layout, hd, vd, short) in [
            (PixelLayout::YCbCr, 2, 2, 0),
            (PixelLayout::YCbCr, 2, 1, 1),
            (PixelLayout::YCbCr, 1, 1, 0),
            (PixelLayout::Rgb, 1, 1, 0),
            (PixelLayout::Gray, 1, 1, 0),
        ] {
            let (cw, ch) = (w.div_ceil(hd as u32) - short, h.div_ceil(vd as u32) - short);
            let mut planes = vec![plane(w, h, 1, 1, 1)];
            if layout != PixelLayout::Gray {
                planes.push(plane(cw, ch, hd, vd, 2));
                planes.push(plane(cw, ch, hd, vd, 3));
            }
            let r = Raster {
                width: w,
                height: h,
                origin: (0, 0),
                scale: 1,
                layout,
                planes,
                truncated: false,
            };
            assert_eq!(to_rgba(&r), to_rgba_reference(&r), "{layout:?} {hd}x{vd}");
        }
    }
}
