//! HEIF image sequences (`.heics`, brand `msf1`): the `moov` box's first HEVC picture track.
//!
//! Only what showing the frames needs is read: the track's `hvc1`/`hev1` sample entry (size,
//! `hvcC`, `colr`, `clap`), its timescale and the sample table (offset, size, duration,
//! composition offset, sync). A sync sample decodes on its own by wrapping it as the one coded
//! item of a minimal HEIF file ([`Sequence::sample_heif`]) that goes through
//! [`crate::HeicFile`]. [`SeqPlayer`] plays any track, inter-coded ones included (P and B
//! pictures, reordering), through the HEVC video decoder: pictures in display order, each
//! with its display time from the track's composition times.

/// Most samples accepted in one track (bounds the sample table on hostile files).
const MAX_SAMPLES: usize = 1 << 20;
/// Box nesting accepted while looking for the track's boxes.
const MAX_DEPTH: usize = 8;

/// One sample of the picture track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeqSample {
    /// File offset of the sample.
    pub offset: u64,
    /// Bytes.
    pub size: u32,
    /// Duration in [`Sequence::timescale`] units.
    pub duration: u32,
    /// A sync sample (an IRAP picture, decodable on its own).
    pub sync: bool,
    /// Composition time offset (`ctts`), in [`Sequence::timescale`] units.
    pub cts_offset: i64,
    /// Decoded but never shown (a `ctts` version 1 offset of -2^31).
    pub hidden: bool,
}

/// The first HEVC picture track of a `moov` box.
#[derive(Clone, Debug)]
pub struct Sequence {
    /// Track width and height (the sample entry's).
    pub width: u32,
    /// See [`Sequence::width`].
    pub height: u32,
    /// Media timescale (units per second).
    pub timescale: u32,
    /// The samples in decode order.
    pub samples: Vec<SeqSample>,
    /// Sample entry type, `hvc1` or `hev1`.
    pub entry: [u8; 4],
    /// The sample entry's `hvcC` box and its `colr` / `clap` boxes (whole boxes).
    properties: Vec<Vec<u8>>,
}

fn be16(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes(
        b.get(p..p + 2)?.try_into().ok()?,
    )))
}

fn be32(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(p..p + 4)?.try_into().ok()?))
}

fn be64(b: &[u8], p: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(p..p + 8)?.try_into().ok()?))
}

/// The boxes of `data`: (type, whole box, payload).
fn boxes(data: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8], &[u8])> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let rest = data.get(at..)?;
        let size32 = be32(rest, 0)? as u64;
        let typ: [u8; 4] = rest.get(4..8)?.try_into().ok()?;
        let (size, head) = match size32 {
            0 => (rest.len() as u64, 8),
            1 => (be64(rest, 8)?, 16),
            s => (s, 8),
        };
        if size < head as u64 || size > rest.len() as u64 {
            return None;
        }
        let whole = &rest[..size as usize];
        at += size as usize;
        Some((typ, whole, &whole[head..]))
    })
}

fn child<'a>(data: &'a [u8], typ: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data).find(|b| &b.0 == typ).map(|b| b.2)
}

/// The payload of the first top-level `moov` box.
pub fn find_moov(file: &[u8]) -> Option<&[u8]> {
    child(file, b"moov")
}

impl Sequence {
    /// Parses the first track of `moov` whose handler is `pict` or `vide` and whose sample
    /// entry is HEVC. `None` when there is none or its tables are inconsistent.
    pub fn parse(moov: &[u8]) -> Option<Sequence> {
        boxes(moov)
            .filter(|b| &b.0 == b"trak")
            .find_map(|t| Self::track(t.2))
    }

    fn track(trak: &[u8]) -> Option<Sequence> {
        let mdia = child(trak, b"mdia")?;
        let hdlr = child(mdia, b"hdlr")?;
        let handler = hdlr.get(8..12)?;
        if handler != b"pict" && handler != b"vide" {
            return None;
        }
        let mdhd = child(mdia, b"mdhd")?;
        let timescale = if mdhd.first() == Some(&1) {
            be32(mdhd, 20)?
        } else {
            be32(mdhd, 12)?
        }
        .max(1);
        let stbl = child(child(mdia, b"minf")?, b"stbl")?;
        let stsd = child(stbl, b"stsd")?;
        let (entry, _, se) = boxes(stsd.get(8..)?).next()?;
        if &entry != b"hvc1" && &entry != b"hev1" {
            return None;
        }
        let (width, height) = (be16(se, 24)?, be16(se, 26)?);
        let mut properties = Vec::new();
        for (typ, whole, _) in boxes(se.get(78..)?) {
            match &typ {
                b"hvcC" => properties.insert(0, whole.to_vec()),
                b"colr" | b"clap" => properties.push(whole.to_vec()),
                _ => {}
            }
        }
        if properties.first().is_none_or(|p| &p[4..8] != b"hvcC") {
            return None;
        }
        let samples = Self::samples(stbl)?;
        Some(Sequence {
            width,
            height,
            timescale,
            samples,
            entry,
            properties,
        })
    }

    fn samples(stbl: &[u8]) -> Option<Vec<SeqSample>> {
        // Sizes.
        let stsz = child(stbl, b"stsz")?;
        let fixed = be32(stsz, 4)?;
        let count = be32(stsz, 8)? as usize;
        if count == 0 || count > MAX_SAMPLES {
            return None;
        }
        let sizes: Vec<u32> = if fixed != 0 {
            vec![fixed; count]
        } else {
            (0..count)
                .map(|i| be32(stsz, 12 + 4 * i))
                .collect::<Option<_>>()?
        };
        // Chunk offsets.
        let chunks: Vec<u64> = if let Some(stco) = child(stbl, b"stco") {
            let n = (be32(stco, 4)? as usize).min(MAX_SAMPLES);
            (0..n)
                .map(|i| be32(stco, 8 + 4 * i).map(u64::from))
                .collect::<Option<_>>()?
        } else {
            let co64 = child(stbl, b"co64")?;
            let n = (be32(co64, 4)? as usize).min(MAX_SAMPLES);
            (0..n)
                .map(|i| be64(co64, 8 + 8 * i))
                .collect::<Option<_>>()?
        };
        // Sample-to-chunk runs: (first chunk, samples per chunk).
        let stsc = child(stbl, b"stsc")?;
        let runs: Vec<(usize, usize)> = (0..(be32(stsc, 4)? as usize).min(MAX_SAMPLES))
            .map(|i| {
                Some((
                    be32(stsc, 8 + 12 * i)? as usize,
                    be32(stsc, 12 + 12 * i)? as usize,
                ))
            })
            .collect::<Option<_>>()?;
        let mut samples = Vec::with_capacity(count);
        'chunks: for (c, &base) in chunks.iter().enumerate() {
            let chunk = c + 1;
            let per = runs
                .iter()
                .rev()
                .find(|r| r.0 <= chunk)
                .map_or(0, |r| r.1)
                .min(MAX_SAMPLES);
            let mut off = base;
            for _ in 0..per {
                let Some(&size) = sizes.get(samples.len()) else {
                    break 'chunks;
                };
                samples.push(SeqSample {
                    offset: off,
                    size,
                    duration: 0,
                    sync: true,
                    cts_offset: 0,
                    hidden: false,
                });
                off = off.saturating_add(u64::from(size));
            }
        }
        if samples.is_empty() {
            return None;
        }
        // Durations.
        if let Some(stts) = child(stbl, b"stts") {
            let mut i = 0;
            for e in 0..(be32(stts, 4)? as usize).min(MAX_SAMPLES) {
                let (n, d) = (be32(stts, 8 + 8 * e)?, be32(stts, 12 + 8 * e)?);
                for _ in 0..n {
                    let Some(s) = samples.get_mut(i) else { break };
                    s.duration = d;
                    i += 1;
                }
            }
        }
        // Composition offsets (version 1: signed).
        if let Some(ctts) = child(stbl, b"ctts") {
            let signed = ctts.first() == Some(&1);
            let mut i = 0;
            for e in 0..(be32(ctts, 4)? as usize).min(MAX_SAMPLES) {
                let (n, o) = (be32(ctts, 8 + 8 * e)?, be32(ctts, 12 + 8 * e)?);
                let o = if signed {
                    i64::from(o as i32)
                } else {
                    i64::from(o)
                };
                for _ in 0..n {
                    let Some(s) = samples.get_mut(i) else { break };
                    s.cts_offset = o;
                    s.hidden = o == i64::from(i32::MIN);
                    i += 1;
                }
            }
        }
        // Sync samples: all when there is no `stss`.
        if let Some(stss) = child(stbl, b"stss") {
            samples.iter_mut().for_each(|s| s.sync = false);
            for e in 0..(be32(stss, 4)? as usize).min(MAX_SAMPLES) {
                let n = be32(stss, 8 + 4 * e)? as usize;
                if let Some(s) = n.checked_sub(1).and_then(|i| samples.get_mut(i)) {
                    s.sync = true;
                }
            }
        }
        Some(samples)
    }

    /// Whether every sample is a sync sample (each decodes on its own).
    pub fn all_sync(&self) -> bool {
        self.samples.iter().all(|s| s.sync)
    }

    /// The samples whose pictures are shown (not [`SeqSample::hidden`]).
    pub fn shown_count(&self) -> usize {
        self.samples.iter().filter(|s| !s.hidden).count()
    }

    /// How long each sample's picture shows, by sample (decode order): the gap to the next
    /// shown picture in composition order; the last shows for its own duration, a hidden
    /// sample for none.
    pub fn display_durations(&self) -> Vec<std::time::Duration> {
        let mut dts = 0i64;
        let mut cts: Vec<(i64, usize)> = Vec::with_capacity(self.samples.len());
        for (i, s) in self.samples.iter().enumerate() {
            if !s.hidden {
                cts.push((dts.saturating_add(s.cts_offset), i));
            }
            dts = dts.saturating_add(i64::from(s.duration));
        }
        cts.sort_unstable();
        let scale = f64::from(self.timescale.max(1));
        let mut out = vec![std::time::Duration::ZERO; self.samples.len()];
        for (k, &(t, i)) in cts.iter().enumerate() {
            let ticks = match cts.get(k + 1) {
                Some(&(next, _)) => next.saturating_sub(t),
                None => i64::from(self.samples[i].duration),
            };
            out[i] = std::time::Duration::from_secs_f64(ticks.max(0) as f64 / scale);
        }
        out
    }

    /// The `hvcC` record (box body) of the sample entry.
    pub fn hvcc(&self) -> &[u8] {
        self.properties
            .first()
            .map_or(&[], |p| p.get(8..).unwrap_or(&[]))
    }

    /// Sample `i`'s bytes in `file`.
    pub fn sample_data<'a>(&self, file: &'a [u8], i: usize) -> Option<&'a [u8]> {
        let s = self.samples.get(i)?;
        let start = usize::try_from(s.offset).ok()?;
        file.get(start..start.checked_add(s.size as usize)?)
    }

    /// Sample `i` of `file` wrapped as the primary (and only) coded item of a minimal HEIF
    /// file, with the sample entry's `hvcC`, `ispe`, `colr` and `clap` as its properties.
    pub fn sample_heif(&self, file: &[u8], i: usize) -> Option<Vec<u8>> {
        let s = self.samples.get(i)?;
        let data = self.sample_data(file, i)?;
        let ftyp = boxed(b"ftyp", &[&b"heic"[..], &[0; 4], b"mif1", b"heic"].concat());
        let meta = |offset: u32| {
            let hdlr = full(b"hdlr", 0, &[&[0; 4][..], b"pict", &[0; 13]].concat());
            let pitm = full(b"pitm", 0, &1u16.to_be_bytes());
            let infe = full(b"infe", 2, &[&[0, 1, 0, 0][..], &self.entry, &[0]].concat());
            let iinf = full(b"iinf", 0, &[&1u16.to_be_bytes()[..], &infe].concat());
            let mut loc = vec![0x44, 0x00, 0, 1, 0, 1, 0, 0, 0, 1];
            loc.extend_from_slice(&offset.to_be_bytes());
            loc.extend_from_slice(&s.size.to_be_bytes());
            let iloc = full(b"iloc", 0, &loc);
            let mut ipco = self.properties.concat();
            ipco.extend_from_slice(&full(
                b"ispe",
                0,
                &[self.width.to_be_bytes(), self.height.to_be_bytes()].concat(),
            ));
            // hvcC (1) essential, then colr / clap, then ispe.
            let n = self.properties.len() + 1;
            let mut assoc = vec![0, 0, 0, 1, 0, 1, n as u8];
            assoc.extend((1..=n as u8).map(|k| if k == 1 { 0x81 } else { k }));
            let ipma = full(b"ipma", 0, &assoc);
            let iprp = boxed(b"iprp", &[boxed(b"ipco", &ipco), ipma].concat());
            full(b"meta", 0, &[hdlr, pitm, iinf, iloc, iprp].concat())
        };
        let head = ftyp.len() + meta(0).len() + 8;
        let mut out = ftyp;
        out.extend_from_slice(&meta(u32::try_from(head).ok()?));
        out.extend_from_slice(&boxed(b"mdat", data));
        Some(out)
    }
}

/// One picture of a sequence, in display order.
#[derive(Clone, Debug)]
pub struct SeqFrame {
    /// Position in display order.
    pub index: usize,
    /// The picture, with the sample entry's `clap` applied and its colour signalling.
    pub image: crate::Image,
    /// How long it shows.
    pub delay: std::time::Duration,
}

/// Plays a sequence's track from its first sample: every picture, inter-coded ones included,
/// in display order (see the module docs).
pub struct SeqPlayer {
    seq: Sequence,
    length_size: usize,
    dec: hevc::VideoDecoder,
    /// The decoder as built, for [`SeqPlayer::rewind`].
    fresh: hevc::Snapshot,
    /// The first sample as a HEIF file: the colour and transforms every picture shares.
    template: crate::HeicFile,
    delays: Vec<std::time::Duration>,
    next_sample: usize,
    shown: usize,
    flushed: bool,
}

impl std::fmt::Debug for SeqPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeqPlayer")
            .field("samples", &self.seq.samples.len())
            .field("next_sample", &self.next_sample)
            .field("shown", &self.shown)
            .finish()
    }
}

fn hevc_error(e: hevc::Error) -> crate::Error {
    match e {
        hevc::Error::Cancelled => crate::Error::Cancelled,
        hevc::Error::Unsupported(m) => crate::Error::Unsupported(m.into()),
        e => heif::HeifError::invalid(format!("sequence: {e}")).into(),
    }
}

impl SeqPlayer {
    /// A player for `seq`, a track of `file`.
    pub fn new(seq: &Sequence, file: &[u8]) -> crate::Result<SeqPlayer> {
        let template = seq
            .sample_heif(file, 0)
            .ok_or_else(|| heif::HeifError::invalid("sequence: first sample out of the file"))?;
        let template = crate::HeicFile::open(template)?;
        let cfg = hevc::hvcc::parse(seq.hvcc()).map_err(hevc_error)?;
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
        let dec = hevc::VideoDecoder::new(&cfg.nal_units, threads).map_err(hevc_error)?;
        Ok(SeqPlayer {
            length_size: cfg.length_size,
            fresh: dec.snapshot(),
            dec,
            template,
            delays: seq.display_durations(),
            seq: seq.clone(),
            next_sample: 0,
            shown: 0,
            flushed: false,
        })
    }

    /// Back to the first picture.
    pub fn rewind(&mut self) {
        // The fresh state holds no pictures: restoring it cannot fail.
        self.dec.restore(&self.fresh);
        self.next_sample = 0;
        self.shown = 0;
        self.flushed = false;
    }

    /// The next picture in display order; `None` after the last. A sample that fails to
    /// decode costs its picture (later ones predicted from it are concealed by the decoder).
    pub fn next_frame(
        &mut self,
        file: &[u8],
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Option<crate::Result<SeqFrame>> {
        loop {
            if let Some(d) = self.dec.next_output() {
                let hidden = usize::try_from(d.tag)
                    .ok()
                    .and_then(|t| self.seq.samples.get(t))
                    .is_some_and(|s| s.hidden);
                if hidden {
                    continue;
                }
                return Some(self.frame(&d));
            }
            while self.dec.next_dropped().is_some() {}
            if self.next_sample < self.seq.samples.len() {
                let i = self.next_sample;
                self.next_sample += 1;
                let Some(data) = self.seq.sample_data(file, i) else {
                    continue;
                };
                let Ok(nals) = hevc::hvcc::split(data, self.length_size) else {
                    continue;
                };
                match self.dec.decode_tagged(&nals, i as i64, cancel) {
                    Ok(()) => {}
                    Err(e @ (hevc::Error::Cancelled | hevc::Error::Unsupported(_))) => {
                        return Some(Err(hevc_error(e)));
                    }
                    Err(_) => {}
                }
            } else if !self.flushed {
                self.flushed = true;
                self.dec.flush();
            } else {
                return None;
            }
        }
    }

    fn frame(&mut self, d: &hevc::Decoded) -> crate::Result<SeqFrame> {
        let size = (self.seq.width, self.seq.height);
        let fits = size.0 > 0 && size.1 > 0 && size.0 <= d.frame.width && size.1 <= d.frame.height;
        let out = crate::core::hevc_output(d, fits.then_some(size))
            .map_err(|e| crate::Error::Hevc { item: 1, source: e })?;
        let root = self.template.graph();
        let frame = heif::compose::apply_transforms_owned(
            out.frame,
            root.properties.transformative(),
            false,
        )?;
        let mut colour = self.template.info().colour.clone();
        colour.vui = out.vui;
        let index = self.shown;
        self.shown += 1;
        let delay = usize::try_from(d.tag)
            .ok()
            .and_then(|t| self.delays.get(t).copied())
            .unwrap_or_default();
        Ok(SeqFrame {
            index,
            image: crate::Image {
                frame,
                colour,
                premultiplied_alpha: false,
                warnings: if d.damaged > 0 {
                    vec![format!("{} CTUs concealed", d.damaged)]
                } else {
                    Vec::new()
                },
            },
            delay,
        })
    }
}

fn boxed(typ: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut b = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(typ);
    b.extend_from_slice(payload);
    b
}

fn full(typ: &[u8; 4], version: u8, payload: &[u8]) -> Vec<u8> {
    boxed(typ, &[&[version, 0, 0, 0][..], payload].concat())
}

/// Nesting is bounded by construction (fixed paths); kept for documentation of the limit.
const _: () = assert!(MAX_DEPTH >= 5);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_tables_do_not_panic() {
        assert!(find_moov(b"").is_none());
        assert!(find_moov(&[0, 0, 0, 1, b'm', b'o', b'o', b'v']).is_none());
        let moov = boxed(b"trak", &boxed(b"mdia", &full(b"hdlr", 0, b"\0\0\0\0pict")));
        assert!(Sequence::parse(&moov).is_none());
        // A sample table claiming a million samples in a few bytes.
        let stsz = full(b"stsz", 0, &[0, 0, 0, 0, 0, 0x0f, 0xff, 0xff]);
        assert!(Sequence::samples(&stsz).is_none());
        for n in 0..64u8 {
            let junk: Vec<u8> = (0..200).map(|i| (i as u8).wrapping_mul(n)).collect();
            let _ = Sequence::parse(&junk);
        }
    }
}
