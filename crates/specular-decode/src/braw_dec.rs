//! Blackmagic RAW (`.braw`) through the Blackmagic RAW SDK the user installed, in the
//! sandboxed decode host ([`specular_sandbox::Host::vendor`]). Never bundled or downloaded:
//! [`crate::optional`] finds the SDK and hands it to the host.
//!
//! A clip shows as its first frame, decoded by the SDK's CPU pipeline at the level asked for
//! (the SDK's full, half, quarter and eighth resolutions are native levels, so a thumbnail
//! costs an eighth-resolution decode). The clip's facts (frames, frame rate, camera) are in
//! the info panel. Without the SDK the file is refused with
//! [`DecodeError::NeedsDecoder`], which the app turns into its "needs a free decoder" card.
//!
//! Content probe: a QuickTime file (`ftyp qt  `) whose sample description names a Blackmagic
//! RAW codec (`brxq`, `brst`, `brvn`, `brs2`, `brxh`, `brhq`, `brlt`…), looked for in the
//! first and last megabytes (the movie header is at either end).

use crate::optional::{self, OptionalId};
use crate::whole::{self, Levels};
use crate::{
    CameraInfo, DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, Result,
    Session,
};
use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_sandbox::{ClipInfo, ClipSource, Error as HostError, ErrorKind, Host, ImageLimits};
use std::sync::Arc;

pub struct BrawDecoder;

/// How much of each end of the file the probe reads.
const PROBE_SPAN: usize = 1 << 20;

/// Whether `data` is a QuickTime movie of Blackmagic RAW frames.
pub(crate) fn is_braw(data: &[u8]) -> bool {
    if data.get(4..12) != Some(b"ftypqt  ") {
        return false;
    }
    let head = &data[..data.len().min(PROBE_SPAN)];
    let tail = &data[data.len().saturating_sub(PROBE_SPAN)..];
    has_braw_stsd(head) || has_braw_stsd(tail)
}

/// A sample description (`stsd`) whose first entry's format is a Blackmagic RAW codec.
fn has_braw_stsd(b: &[u8]) -> bool {
    let mut at = 0;
    while let Some(i) = b[at..].windows(4).position(|w| w == b"stsd") {
        let p = at + i;
        // stsd: version/flags (4), entry count (4), then the entry: size (4), format (4).
        if let Some(fmt) = b.get(p + 16..p + 20)
            && fmt.starts_with(b"br")
            && fmt[2..].iter().all(u8::is_ascii_lowercase)
        {
            return true;
        }
        at = p + 4;
    }
    false
}

impl Decoder for BrawDecoder {
    fn format(&self) -> Format {
        Format::Braw
    }
    fn probe(&self, head: &[u8]) -> bool {
        is_braw(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        is_braw(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["braw"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let lib = match optional::state(OptionalId::Braw) {
            optional::State::Ready { path, .. } => path,
            optional::State::Off { .. } => {
                return Err(DecodeError::Unsupported(
                    "Blackmagic RAW clips open with the Blackmagic RAW SDK, which is switched \
                     off in Settings › Optional decoders"
                        .into(),
                ));
            }
            optional::State::Broken { why, .. } => {
                return Err(DecodeError::Unsupported(format!(
                    "the Blackmagic RAW SDK on this computer can't be used: {why}"
                )));
            }
            _ => return Err(DecodeError::NeedsDecoder(OptionalId::Braw)),
        };
        let clip = clip_source(&source)?;
        let t0 = std::time::Instant::now();
        // The eighth-resolution frame: cheap, and it brings the clip's facts.
        let (facts, first) = decode(&lib, &clip, 8)?;
        specular_log::debug!(
            target: specular_log::DECODE,
            "braw: {}×{}, {} frames, 1/8 decoded by the SDK in the host in {:.1} ms",
            facts.width,
            facts.height,
            facts.frame_count,
            t0.elapsed().as_secs_f64() * 1e3
        );
        if facts.width == 0 || facts.height == 0 {
            return Err(DecodeError::Malformed("the clip has no size".into()));
        }
        if crate::heic::too_large(facts.width, facts.height) {
            return Err(DecodeError::TooLarge);
        }
        let version = Host::vendor().info().ok().and_then(|i| i.braw);
        let session = BrawSession::new(source, clip, lib, facts, version);
        session.levels_seed(first);
        Ok(Arc::new(session))
    }
}

/// The clip as an open file for the host: the file itself when the source is on disk, else
/// a private copy of its bytes.
fn clip_source(source: &Source) -> Result<ClipSource> {
    let r = if crate::geo::on_disk(source) {
        ClipSource::open(source.path())
    } else {
        ClipSource::from_bytes(source.bytes())
    };
    r.map_err(map_err)
}

fn map_err(e: HostError) -> DecodeError {
    match e {
        HostError::NotInstalled(m) | HostError::Unavailable(m) => DecodeError::Unsupported(m),
        HostError::Remote { kind, message } => match kind {
            ErrorKind::NotInstalled | ErrorKind::Unsupported => DecodeError::Unsupported(message),
            ErrorKind::TooLarge => DecodeError::TooLarge,
            ErrorKind::Io => DecodeError::Io(std::io::Error::other(message)),
            _ => DecodeError::Malformed(message),
        },
        e @ (HostError::Crashed(_) | HostError::Timeout(_) | HostError::Protocol(_)) => {
            DecodeError::Malformed(format!("the Blackmagic RAW SDK failed: {e}"))
        }
        HostError::Io(m) => DecodeError::Io(std::io::Error::other(m)),
    }
}

/// Frame 0 at 1/`scale`, with the clip's facts. Tells [`optional`] what the host said about
/// the SDK.
fn decode(lib: &std::path::Path, clip: &ClipSource, scale: u32) -> Result<(ClipInfo, Raster)> {
    let host = Host::vendor();
    let r = host.braw_decode(clip, 0, scale.clamp(1, 8) as u8, ImageLimits::default());
    if let Ok(i) = host.info() {
        let said = match i.braw {
            Some(v) => Ok(v),
            None => Err(i.braw_error),
        };
        if said.as_ref().is_ok() || said.as_ref().is_err_and(|e| !e.is_empty()) {
            optional::note_braw(lib, said);
        }
    }
    let (facts, img) = r.map_err(map_err)?;
    let raster = whole::interleaved(img.width, img.height, PixelLayout::Rgba, img.rgba, false);
    Ok((facts, raster))
}

/// `r` (the SDK's level 1/`scale`, whose size is rounded down) at exactly the level's size
/// (rounded up), repeating the last column and row where it is a pixel short.
fn exact_level(r: Raster, full: (u32, u32), scale: u32) -> Raster {
    let (w, h) = (full.0.div_ceil(scale), full.1.div_ceil(scale));
    if (r.width, r.height) == (w, h) || r.width == 0 || r.height == 0 {
        return r;
    }
    let src = &r.planes[0];
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        let sy = y.min(r.height - 1) as usize;
        let row = &src.data[sy * src.stride..][..r.width as usize * 4];
        for x in 0..w {
            let sx = x.min(r.width - 1) as usize;
            out.extend_from_slice(&row[sx * 4..sx * 4 + 4]);
        }
    }
    whole::interleaved(w, h, PixelLayout::Rgba, out, false)
}

struct BrawSession {
    source: Arc<Source>,
    clip: ClipSource,
    lib: std::path::PathBuf,
    info: ImageInfo,
    camera: CameraInfo,
    header: HeaderFacts,
    levels: Levels,
    /// The eighth-resolution frame decoded at open, until the level cache takes it.
    seed: Mutex<Option<Raster>>,
}

impl BrawSession {
    fn new(
        source: Arc<Source>,
        clip: ClipSource,
        lib: std::path::PathBuf,
        f: ClipInfo,
        version: Option<String>,
    ) -> BrawSession {
        let info = ImageInfo {
            format: Format::Braw,
            width: f.width,
            height: f.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 12,
            progressive: false,
            coding: "Blackmagic RAW, first frame (Blackmagic RAW SDK, sandboxed)".into(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let mut fields = vec![("Frames".to_owned(), f.frame_count.to_string())];
        if f.frame_rate > 0.0 {
            fields.push(("Frame rate".into(), format!("{:.3} fps", f.frame_rate)));
            let secs = f.frame_count as f64 / f64::from(f.frame_rate);
            fields.push(("Duration".into(), format!("{secs:.2} s")));
        }
        if !f.camera.is_empty() {
            fields.push(("Camera".into(), f.camera.clone()));
        }
        let sdk = match version {
            Some(v) => format!("Blackmagic RAW SDK {v}"),
            None => "Blackmagic RAW SDK".into(),
        };
        fields.push(("Decoder".into(), sdk));
        let camera = CameraInfo {
            camera: f.camera,
            ..CameraInfo::default()
        };
        BrawSession {
            source,
            clip,
            lib,
            info,
            camera,
            header: HeaderFacts {
                summary: Vec::new(),
                fields,
            },
            levels: Levels::new(),
            seed: Mutex::new(None),
        }
    }

    fn levels_seed(&self, r: Raster) {
        let full = (self.info.width, self.info.height);
        *self.seed.lock() = Some(exact_level(r, full, 8));
    }
}

/// The SDK level a request at 1/`scale` is cut down from.
fn base_of(scale: u32) -> u32 {
    match scale {
        0..=1 => 1,
        2..=3 => 2,
        4..=7 => 4,
        _ => 8,
    }
}

impl Session for BrawSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn camera(&self) -> Option<&CameraInfo> {
        (!self.camera.camera.is_empty()).then_some(&self.camera)
    }
    fn header(&self) -> Option<&HeaderFacts> {
        Some(&self.header)
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let full = (self.info.width, self.info.height);
        self.levels.decode(
            req,
            full,
            base_of,
            |b| {
                if b == 8
                    && let Some(r) = self.seed.lock().take()
                {
                    return Ok(r);
                }
                let (_, r) = decode(&self.lib, &self.clip, b)?;
                Ok(exact_level(r, full, b))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let b = base_of(s);
        let mut m = whole::level_bytes(w, h, s, 4);
        if !self.levels.has(s) && !self.levels.has(b) {
            // The level from the host, and its copy out of shared memory.
            m += whole::level_bytes(w, h, b, 8);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.seed.lock().as_ref().map_or(0, |r| r.memory_bytes())
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.seed.lock().take().map_or(0, |r| r.memory_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A QuickTime file with a Blackmagic RAW sample description at `at` from the end.
    fn movie(codec: &[u8; 4], tail: bool) -> Vec<u8> {
        let mut b = vec![0, 0, 0, 0x14];
        b.extend_from_slice(b"ftypqt  \0\0\0\0qt  ");
        let mut stsd = vec![0, 0, 0, 0x30];
        stsd.extend_from_slice(b"stsd");
        stsd.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x20]);
        stsd.extend_from_slice(codec);
        stsd.resize(0x30, 0);
        if tail {
            b.resize(3 << 20, 0);
        }
        b.extend_from_slice(&stsd);
        b
    }

    #[test]
    fn probes_quicktime_with_a_braw_codec_at_either_end() {
        assert!(is_braw(&movie(b"brxq", false)));
        assert!(is_braw(&movie(b"brst", true)));
        assert!(!is_braw(&movie(b"apch", false)), "ProRes is a movie");
        let mut mp4 = movie(b"brxq", false);
        mp4[8..12].copy_from_slice(b"isom");
        assert!(!is_braw(&mp4));
        assert!(!is_braw(b"\0\0\0\x14ftypqt  "));
        assert!(!is_braw(&[]));
    }

    #[test]
    fn levels_are_padded_to_the_exact_size() {
        let r = whole::interleaved(2, 1, PixelLayout::Rgba, vec![1, 2, 3, 4, 5, 6, 7, 8], false);
        let e = exact_level(r, (5, 3), 2);
        assert_eq!((e.width, e.height), (3, 2));
        assert_eq!(
            &e.planes[0].data[..12],
            &[1, 2, 3, 4, 5, 6, 7, 8, 5, 6, 7, 8]
        );
        assert_eq!(&e.planes[0].data[12..], &e.planes[0].data[..12]);
        assert_eq!([1, 2, 3, 4, 5, 8, 9].map(base_of), [1, 2, 2, 4, 4, 8, 8]);
    }

    #[test]
    fn without_the_sdk_it_asks_for_it() {
        let src = Arc::new(Source::from_vec("A001.braw", movie(b"brxq", false)));
        let e = BrawDecoder.open(src).err().expect("refused");
        assert!(
            matches!(e, DecodeError::NeedsDecoder(OptionalId::Braw)),
            "{e}"
        );
    }
}
