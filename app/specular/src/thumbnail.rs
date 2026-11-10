//! `specular --thumbnail IN OUT SIZE`: the desktop thumbnailer (freedesktop `.thumbnailer`
//! files call it as `specular --thumbnail %i %o %s`).
//!
//! It makes the thumbnail the way the gallery does ([`specular_thumbs::generate`]: the
//! embedded preview when it is large enough, else a strided read of a large science array
//! or the coarsest decoder scale that covers SIZE, then an area-average and the orientation), and takes it from the gallery's own cache
//! when that already holds one at least SIZE large, then writes a PNG with the `Thumb::`
//! keys of the freedesktop thumbnail specification.
//!
//! It is made to run inside a thumbnailer sandbox (GNOME's bubblewrap one: no home folder,
//! no network, no D-Bus): no window, no GPU, no log file, nothing written but OUT (the cache
//! is only read, and only when it can be seen), settings read but never written, and a
//! watchdog that ends the process when one file takes too long, so a broken image cannot
//! hold up the file manager's queue.

use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use specular_core::CancelToken;
use specular_decode::Registry;
use specular_thumbs::generate::{self, GenOptions};
use specular_thumbs::pixels::{self, Pixels};
use specular_thumbs::{CLASSES, DiskCache, ThumbKey};

/// Largest SIZE accepted (freedesktop's largest size is 1024).
const MAX_SIZE: u32 = specular_platform::thumbnail::ThumbnailSize::MAX;

/// A thumbnail taking longer than this is given up (exit code 1). GNOME's own limit for a
/// thumbnailer is 30 s; this leaves the file manager time to try another one.
const WATCHDOG: Duration = Duration::from_secs(20);

/// If the arguments ask for a thumbnail, makes it and returns the exit code.
pub fn from_args() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if args.first().is_none_or(|a| a != "--thumbnail") {
        return None;
    }
    let [_, input, output, size] = args.as_slice() else {
        eprintln!("usage: specular --thumbnail IN OUT SIZE");
        return Some(2);
    };
    let Some(size) = size
        .to_str()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|s| (1..=MAX_SIZE).contains(s))
    else {
        eprintln!("specular: --thumbnail: SIZE must be 1 to {MAX_SIZE}");
        return Some(2);
    };
    specular_log::init(None);
    let input = input_path(input);
    let watched = input.clone();
    // Only a timer: the work itself cannot be interrupted at every step (a decoder deep in
    // one huge strip), so the process ends instead.
    let _ = specular_thread::builder("wd-thumb").spawn(move || {
        std::thread::sleep(WATCHDOG);
        eprintln!(
            "specular: thumbnail of {}: gave up after {} s",
            watched.display(),
            WATCHDOG.as_secs()
        );
        std::process::exit(1);
    });
    match make(&input, Path::new(output), size, &Options::detect()) {
        Ok(_) => Some(0),
        Err(e) => {
            eprintln!("specular: thumbnail of {}: {e}", input.display());
            Some(1)
        }
    }
}

/// `%i` may be a path or (from some file managers) a `file://` URI.
fn input_path(arg: &std::ffi::OsStr) -> PathBuf {
    arg.to_str()
        .filter(|s| s.starts_with("file:"))
        .and_then(specular_platform::launch::path_from_uri)
        .unwrap_or_else(|| PathBuf::from(arg))
}

/// The thumbnail cache folder when `gallery.cache_location` is not set: inside the executable's
/// folder in portable mode (like every other cache, [`specular_settings::Locations`]), else
/// the platform cache folder. `$SPECULAR_THUMB_CACHE` comes first either way.
pub(crate) fn default_cache_dir() -> Option<PathBuf> {
    let loc = specular_settings::Locations::detect();
    DiskCache::default_dir_for(loc.portable.then_some(loc.cache_dir.as_path()))
}

/// What the thumbnailer takes from the user's settings.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// The gallery's thumbnail cache, read when it exists.
    pub cache: Option<PathBuf>,
    pub generate: GenOptions,
}

impl Options {
    /// From the environment and the settings file, read without being written (inside a
    /// sandbox neither is usually there: defaults).
    pub fn detect() -> Options {
        let path = specular_settings::Locations::detect().settings_path();
        let settings = std::fs::read_to_string(path)
            .map(|t| specular_settings::load_str(&t).settings)
            .unwrap_or_default();
        let g = &settings.gallery;
        let cache = std::env::var_os("SPECULAR_THUMB_CACHE")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| g.cache_location.clone())
            .or_else(default_cache_dir)
            .filter(|d| d.is_dir());
        Options {
            cache,
            generate: GenOptions {
                prefer_embedded: g.prefer_embedded,
                // One file per process: its decode may use every core.
                parallel: true,
                ..GenOptions::default()
            },
        }
    }
}

/// Where a thumbnail came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The gallery's cache.
    Cache,
    /// Made now (embedded preview or a reduced decode).
    Made,
}

/// The cached thumbnail of `input` that is at least `size` large (or the whole image), with
/// the image's display size. Never writes to the cache.
fn from_cache(dir: &Path, input: &Path, size: u32) -> Option<(Pixels, u32, u32)> {
    let cache = DiskCache::new(dir, 0);
    let key = ThumbKey::of_file(input).ok()?;
    CLASSES.iter().filter(|&&c| c >= size).find_map(|&class| {
        let e = cache.get(&key, class, 0)?;
        if e.meta_stale {
            return None;
        }
        let (w, h) = (e.meta.width, e.meta.height);
        let long = e.pixels.width.max(e.pixels.height);
        (long >= size.min(w.max(h)) && w > 0 && h > 0).then_some((e.pixels, w, h))
    })
}

/// Straight RGBA8 pixels: width, height and the bytes.
pub(crate) type Rgba = (u32, u32, Vec<u8>);

/// The thumbnail of `input`, at most `size` on its long side, as straight RGBA8: from the
/// gallery's cache when it holds one, else made with `registry`. Also returns where it came
/// from and the image's display size.
pub(crate) fn rgba(
    registry: &Registry,
    input: &Path,
    size: u32,
    opts: &Options,
) -> Result<(Source, Rgba, (u32, u32)), String> {
    let cached = opts
        .cache
        .as_deref()
        .and_then(|dir| from_cache(dir, input, size));
    let (source, px, display) = match cached {
        Some((px, w, h)) => (Source::Cache, px, (w, h)),
        None => {
            let session = specular_core::Source::open(input)
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    registry
                        .open_for_thumbnail(std::sync::Arc::new(s))
                        .map_err(|e| e.to_string())
                })?;
            let display = session.info().display_size();
            let px = generate::generate(
                &*session,
                size,
                opts.generate,
                &CancelToken::new(),
                &mut |_| {},
            )
            .map_err(|e| e.to_string())?;
            (Source::Made, px, display)
        }
    };
    let (tw, th) = pixels::fit(px.width, px.height, size);
    if (tw, th) == (px.width, px.height) {
        return Ok((source, (tw, th, px.rgba), display));
    }
    let rgba = downscale(
        &px.rgba,
        px.width as usize,
        px.height as usize,
        tw as usize,
        th as usize,
    );
    Ok((source, (tw, th, rgba), display))
}

pub fn make(input: &Path, output: &Path, size: u32, opts: &Options) -> Result<Source, String> {
    let (source, (tw, th, rgba), (dw, dh)) = rgba(&Registry::new(), input, size, opts)?;
    let uri = specular_platform::launch::file_uri(input);
    let mtime = std::fs::metadata(input)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    let (dw, dh) = (dw.to_string(), dh.to_string());
    let mut text = vec![
        ("Software", "Specular"),
        ("Thumb::URI", uri.as_str()),
        ("Thumb::Image::Width", dw.as_str()),
        ("Thumb::Image::Height", dh.as_str()),
    ];
    if !mtime.is_empty() {
        text.push(("Thumb::MTime", mtime.as_str()));
    }
    let png = specular_png::encode_with_text(tw, th, specular_png::ColorType::Rgba, &rgba, &text)
        .map_err(|e| e.to_string())?;
    std::fs::write(output, png).map_err(|e| format!("{}: {e}", output.display()))?;
    Ok(source)
}

/// Area-average downscale of straight RGBA8, alpha-weighted so transparent pixels do not
/// darken the edges.
pub(crate) fn downscale(src: &[u8], w: usize, h: usize, tw: usize, th: usize) -> Vec<u8> {
    let mut out = vec![0u8; tw * th * 4];
    let span = |i: usize, n: usize, t: usize| {
        (
            i * n / t,
            ((i + 1) * n).div_ceil(t).max(i * n / t + 1).min(n),
        )
    };
    for ty in 0..th {
        let (y0, y1) = span(ty, h, th);
        for tx in 0..tw {
            let (x0, x1) = span(tx, w, tw);
            let mut acc = [0u64; 4];
            for y in y0..y1 {
                let row = &src[(y * w + x0) * 4..(y * w + x1) * 4];
                for p in row.chunks_exact(4) {
                    let a = p[3] as u64;
                    acc[0] += p[0] as u64 * a;
                    acc[1] += p[1] as u64 * a;
                    acc[2] += p[2] as u64 * a;
                    acc[3] += a;
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            let d = &mut out[(ty * tw + tx) * 4..][..4];
            for c in 0..3 {
                d[c] = (acc[c] + acc[3] / 2).checked_div(acc[3]).unwrap_or(0) as u8;
            }
            d[3] = ((acc[3] + n / 2) / n) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downscale_averages() {
        // 2×2 → 1×1: grey average; a transparent pixel adds no colour.
        let src = [255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 0];
        assert_eq!(downscale(&src, 2, 2, 1, 1), vec![255, 0, 0, 191]);
        let src: Vec<u8> = (0..5 * 3).flat_map(|i| [i as u8 * 10, 0, 0, 255]).collect();
        let d = downscale(&src, 5, 3, 2, 1);
        assert_eq!(d.len(), 8);
        assert!(d[0] < d[4]);
    }

    fn read_png(path: &Path) -> (u32, u32, Vec<(String, String)>) {
        let png = std::fs::read(path).unwrap();
        let r = png::Decoder::new(std::io::Cursor::new(&png))
            .read_info()
            .unwrap();
        let i = r.info();
        let text = i
            .uncompressed_latin1_text
            .iter()
            .map(|t| (t.keyword.clone(), t.text.clone()))
            .collect();
        (i.width, i.height, text)
    }

    #[test]
    fn thumbnail_of_a_png_and_from_the_cache() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/agent-app/test-thumb")
            .join(std::process::id().to_string());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.png");
        let output = dir.join("out.png");
        let (w, h) = (300u32, 120u32);
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| [(i % w) as u8, (i / w) as u8, 128, 255])
            .collect();
        std::fs::write(
            &input,
            specular_png::encode(w, h, specular_png::ColorType::Rgba, &rgba).unwrap(),
        )
        .unwrap();
        let mut opts = Options::default();
        assert_eq!(make(&input, &output, 64, &opts), Ok(Source::Made));
        let (tw, th, text) = read_png(&output);
        assert_eq!((tw, th), (64, 26));
        let get = |k: &str| text.iter().find(|t| t.0 == k).map(|t| t.1.clone());
        assert!(get("Thumb::URI").is_some_and(|u| u.starts_with("file://")));
        assert_eq!(get("Thumb::Image::Width").as_deref(), Some("300"));
        assert!(get("Thumb::MTime").is_some());

        // A gallery cache with a 128 px entry: used for 64 and 128, not for 256 (the
        // image is larger than that), and never written to.
        let cache_dir = dir.join("cache");
        let cache = DiskCache::new(&cache_dir, 64 << 20);
        let key = ThumbKey::of_file(&input).unwrap();
        // A solid colour, so a cache hit is told apart from a fresh thumbnail.
        let tile = Pixels::new(128, 51, [9u8, 200, 30, 255].repeat(128 * 51));
        let meta = specular_thumbs::Meta {
            width: w,
            height: h,
            format: "PNG".into(),
            ..Default::default()
        };
        cache.put(&key, 128, 0, &tile, &meta).unwrap();
        let entries = || walk(&cache_dir);
        let before = entries();
        opts.cache = Some(cache_dir.clone());
        assert_eq!(make(&input, &output, 64, &opts), Ok(Source::Cache));
        let png = std::fs::read(&output).unwrap();
        let mut r = png::Decoder::new(std::io::Cursor::new(&png))
            .read_info()
            .unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        r.next_frame(&mut buf).unwrap();
        // (Opaque cache entries are stored as JPEG: near, not exact.)
        let near = |a: u8, b: u8| a.abs_diff(b) <= 6;
        assert!(
            near(buf[0], 9) && near(buf[1], 200) && near(buf[2], 30),
            "{:?}",
            &buf[..4]
        );
        assert_eq!(read_png(&output).0, 64);
        assert_eq!(make(&input, &output, 128, &opts), Ok(Source::Cache));
        assert_eq!(make(&input, &output, 256, &opts), Ok(Source::Made));
        assert_eq!(read_png(&output).0, 256);
        // Larger than the image: the image's own size, never upscaled.
        assert_eq!(make(&input, &output, 512, &opts), Ok(Source::Made));
        assert_eq!(read_png(&output).0, 300);
        assert_eq!(entries(), before, "the cache is only read");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn walk(d: &Path) -> Vec<(PathBuf, u64)> {
        let mut v = Vec::new();
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                v.extend(walk(&p));
            } else {
                let len = e.metadata().map_or(0, |m| m.len());
                v.push((p, len));
            }
        }
        v.sort();
        v
    }

    #[test]
    fn broken_and_missing_files_fail() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/agent-app/test-thumb")
            .join(format!("bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("bad.jpg");
        std::fs::write(&bad, b"\xff\xd8\xff\xe0 not really").unwrap();
        let out = dir.join("out.png");
        let opts = Options::default();
        assert!(make(&bad, &out, 128, &opts).is_err());
        assert!(make(&dir.join("missing.png"), &out, 128, &opts).is_err());
        assert!(!out.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
