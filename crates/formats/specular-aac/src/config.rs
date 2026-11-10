//! AudioSpecificConfig (ISO/IEC 14496-3 §1.6.2.1), GASpecificConfig and the program config
//! element (§4.4.1.1), reduced to what the decoder needs: the core coder, its sampling rate,
//! the channel elements in bitstream order, and how SBR and PS are signalled.

use crate::bits::Bits;
use crate::{Error, Result};

/// Sampling frequencies by `samplingFrequencyIndex`.
pub const SAMPLE_RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

/// The table index used for an arbitrary (explicitly coded) rate: the nearest standard rate,
/// by the thresholds of §4.5.1.1 (Table 4.82).
pub fn rate_index(rate: u32) -> u8 {
    const MIN: [u32; 11] = [
        92017, 75132, 55426, 46009, 37566, 27713, 23004, 18783, 13856, 11502, 9391,
    ];
    MIN.iter().position(|&m| rate >= m).unwrap_or(11) as u8
}

/// How a stream says whether SBR or PS is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Explicitly signalled as present.
    Present,
    /// Explicitly signalled as absent (a sync extension says so).
    Absent,
    /// Not signalled: only the bitstream can tell ("implicit" signalling).
    Implicit,
}

/// One syntactic channel element of the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    /// Single channel element (one channel).
    Sce,
    /// Channel pair element (two channels).
    Cpe,
    /// Low-frequency effects element (one channel).
    Lfe,
}

impl Element {
    pub fn channels(self) -> usize {
        match self {
            Element::Cpe => 2,
            _ => 1,
        }
    }
}

/// Where an output channel sits, for the stereo downmix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    FrontCentre,
    FrontLeft,
    FrontRight,
    SideLeft,
    SideRight,
    BackLeft,
    BackRight,
    BackCentre,
    LowFrequency,
}

/// A decoded AudioSpecificConfig.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Audio object type of the core coder (2 = AAC-LC).
    pub object_type: u8,
    /// Core sampling rate in Hz.
    pub sample_rate: u32,
    /// Index into [`SAMPLE_RATES`] whose tables the core uses.
    pub rate_index: u8,
    /// `channelConfiguration` (0 = given by a program config element).
    pub channel_config: u8,
    /// Channel elements in bitstream order, with the position of each output channel.
    pub elements: Vec<(Element, [Position; 2])>,
    /// The element instance tag of each element when a program config element gave the
    /// layout (`None` for the standard layouts, whose elements come in order).
    pub tags: Vec<Option<u8>>,
    /// SBR signalling.
    pub sbr: Signal,
    /// PS signalling.
    pub ps: Signal,
    /// The SBR output rate, when signalled.
    pub sbr_rate: Option<u32>,
    /// Core samples per frame: 1024, or 960 (`frameLengthFlag`, as DAB+ sends).
    pub frame_len: usize,
}

impl Config {
    /// Output channels of the core decoder (before SBR/PS and any downmix).
    pub fn core_channels(&self) -> usize {
        self.elements.iter().map(|(e, _)| e.channels()).sum()
    }
}

fn object_type(b: &mut Bits) -> u8 {
    let t = b.read(5) as u8;
    if t == 31 { 32 + b.read(6) as u8 } else { t }
}

fn sampling_rate(b: &mut Bits) -> Result<u32> {
    let idx = b.read(4) as usize;
    if idx == 15 {
        let r = b.read(24);
        if r == 0 {
            return Err(Error::Malformed("zero sampling rate".into()));
        }
        Ok(r)
    } else {
        SAMPLE_RATES
            .get(idx)
            .copied()
            .ok_or_else(|| Error::Malformed(format!("reserved sampling frequency index {idx}")))
    }
}

/// A name for an object type this decoder does not handle.
pub(crate) fn object_type_name(t: u8) -> String {
    match t {
        1 => "AAC Main profile".into(),
        3 => "AAC-SSR (scalable sample rate)".into(),
        4 => "AAC-LTP (long-term prediction)".into(),
        6 => "scalable AAC".into(),
        7 => "TwinVQ".into(),
        8 => "CELP speech".into(),
        9 => "HVXC speech".into(),
        17 => "error-resilient AAC".into(),
        19 => "error-resilient AAC-LTP".into(),
        20 => "error-resilient scalable AAC".into(),
        21 => "error-resilient TwinVQ".into(),
        22 => "BSAC".into(),
        23 => "AAC-LD (low delay)".into(),
        24..=27 => "error-resilient speech".into(),
        32..=34 => "MPEG-1 or MPEG-2 audio".into(),
        36 => "ALS lossless".into(),
        39 => "AAC-ELD (enhanced low delay)".into(),
        42 => "USAC (xHE-AAC)".into(),
        _ => format!("MPEG-4 audio object type {t}"),
    }
}

/// Parses an AudioSpecificConfig.
pub fn parse_asc(data: &[u8]) -> Result<Config> {
    let mut b = Bits::new(data);
    let bits = data.len() * 8;
    read_asc(&mut b, Some(bits))
}

/// Reads an AudioSpecificConfig at the reader's position. `limit` is the bits it takes when
/// known (a sync extension for explicit SBR/PS signalling is only looked for then).
pub(crate) fn read_asc(b: &mut Bits, limit: Option<usize>) -> Result<Config> {
    let start = b.pos();
    let b = &mut *b;
    let mut aot = object_type(b);
    let mut rate = sampling_rate(b)?;
    let channel_config = b.read(4) as u8;
    let mut sbr = Signal::Implicit;
    let mut ps = Signal::Implicit;
    let mut sbr_rate = None;
    if aot == 5 || aot == 29 {
        sbr = Signal::Present;
        if aot == 29 {
            ps = Signal::Present;
        }
        sbr_rate = Some(sampling_rate(b)?);
        aot = object_type(b);
        if aot == 22 {
            b.read(4);
        }
    }
    // AAC Main, LC and LTP
    if !matches!(aot, 1 | 2 | 4) {
        return Err(Error::Unsupported(object_type_name(aot)));
    }
    // GASpecificConfig
    let frame_len = if b.bit() { 960 } else { 1024 };
    if b.bit() {
        b.read(14); // coreCoderDelay
    }
    let extension_flag = b.bit();
    let (elements, tags) = if channel_config == 0 {
        let p = parse_pce(b, start)?;
        (p.elements, p.tags)
    } else {
        let e = layout(channel_config)?;
        let n = e.len();
        (e, vec![None; n])
    };
    if extension_flag {
        b.read(1); // extensionFlag3 (no resilience flags for object type 2)
    }
    if b.overrun() {
        return Err(Error::Malformed("AudioSpecificConfig is truncated".into()));
    }
    // Backward-compatible explicit signalling: a sync extension after the core config.
    let left = |b: &Bits| limit.map_or(0, |l| (start + l) as isize - b.pos() as isize);
    if sbr == Signal::Implicit && left(b) >= 16 && b.peek(11) == 0x2b7 {
        b.read(11);
        let ext = object_type(b);
        if ext == 5 {
            if b.bit() {
                sbr = Signal::Present;
                sbr_rate = Some(sampling_rate(b)?);
                if left(b) >= 12 && b.peek(11) == 0x548 {
                    b.read(11);
                    ps = if b.bit() {
                        Signal::Present
                    } else {
                        Signal::Absent
                    };
                }
            } else {
                sbr = Signal::Absent;
            }
        }
        if b.overrun() {
            return Err(Error::Malformed(
                "AudioSpecificConfig extension is truncated".into(),
            ));
        }
    }
    if rate > 96000 {
        return Err(Error::Unsupported(format!("{rate} Hz AAC")));
    }
    if rate == 0 {
        rate = 44100;
    }
    if frame_len == 960 && sbr == Signal::Present {
        return Err(Error::Unsupported(SBR_960.into()));
    }
    Ok(Config {
        object_type: aot,
        sample_rate: rate,
        rate_index: rate_index(rate),
        channel_config,
        elements,
        tags,
        sbr,
        ps,
        sbr_rate,
        frame_len,
    })
}

/// What a 960-sample stream with SBR is refused as.
pub(crate) const SBR_960: &str = "HE-AAC with 960-sample frames (DAB+)";

impl Config {
    /// The coding's everyday name: AAC, AAC Main, AAC-LTP, HE-AAC or HE-AAC v2 (as
    /// signalled).
    pub fn name(&self) -> &'static str {
        match (self.sbr, self.ps, self.object_type) {
            (Signal::Present, Signal::Present, _) => "HE-AAC v2",
            (Signal::Present, _, _) => "HE-AAC",
            (_, _, 1) => "AAC Main",
            (_, _, 4) => "AAC-LTP",
            _ => "AAC",
        }
    }

    /// The decoded sampling rate, as signalled (implicit SBR is not counted).
    pub fn output_rate(&self) -> u32 {
        match (self.sbr, self.sbr_rate) {
            (Signal::Present, Some(r)) => r.max(self.sample_rate),
            (Signal::Present, None) => 2 * self.sample_rate,
            _ => self.sample_rate,
        }
    }

    /// The decoded channels, as signalled (0 when an in-band program config element will
    /// say); parametric stereo makes mono two.
    pub fn output_channels(&self) -> usize {
        let n = self.core_channels();
        if n == 1 && self.ps == Signal::Present {
            2
        } else {
            n
        }
    }
}

/// An AudioSpecificConfig for an object type, core rate and channel configuration, with SBR
/// signalled explicitly (object type 5) at `sbr_rate` when given: for containers that name
/// the coding but carry no configuration (Matroska's old `A_AAC/MPEG4/LC/SBR` ids).
pub fn make_asc(object_type: u8, rate: u32, channel_config: u8, sbr_rate: Option<u32>) -> Vec<u8> {
    let mut bits: Vec<(u32, u32)> = Vec::new();
    let rate_field =
        |r: u32, bits: &mut Vec<(u32, u32)>| match SAMPLE_RATES.iter().position(|&s| s == r) {
            Some(i) => bits.push((i as u32, 4)),
            None => {
                bits.push((15, 4));
                bits.push((r & 0xff_ffff, 24));
            }
        };
    if let Some(sr) = sbr_rate {
        bits.push((5, 5));
        rate_field(rate, &mut bits);
        bits.push((u32::from(channel_config & 15), 4));
        rate_field(sr, &mut bits);
        bits.push((u32::from(object_type & 31), 5));
    } else {
        bits.push((u32::from(object_type & 31), 5));
        rate_field(rate, &mut bits);
        bits.push((u32::from(channel_config & 15), 4));
    }
    bits.push((0, 3)); // frameLengthFlag, dependsOnCoreCoder, extensionFlag
    let mut out = Vec::new();
    let (mut acc, mut n) = (0u64, 0u32);
    for (v, w) in bits {
        acc = (acc << w) | u64::from(v);
        n += w;
        while n >= 8 {
            out.push((acc >> (n - 8)) as u8);
            n -= 8;
        }
    }
    if n > 0 {
        out.push((acc << (8 - n)) as u8);
    }
    out
}

/// The element layout of `channelConfiguration` 1–7.
pub(crate) fn layout(cfg: u8) -> Result<Vec<(Element, [Position; 2])>> {
    use Element::*;
    use Position::*;
    let c = (Sce, [FrontCentre, FrontCentre]);
    let f = (Cpe, [FrontLeft, FrontRight]);
    Ok(match cfg {
        1 => vec![c],
        2 => vec![f],
        3 => vec![c, f],
        4 => vec![c, f, (Sce, [BackCentre, BackCentre])],
        5 => vec![c, f, (Cpe, [BackLeft, BackRight])],
        6 => vec![
            c,
            f,
            (Cpe, [BackLeft, BackRight]),
            (Lfe, [LowFrequency, LowFrequency]),
        ],
        // 7: ISO 14496-3 names the second pair "outside front", but encoders (ffmpeg's,
        // Apple's) write 7.1 with side and back pairs under it, as 12 says outright
        7 | 12 => vec![
            c,
            f,
            (Cpe, [SideLeft, SideRight]),
            (Cpe, [BackLeft, BackRight]),
            (Lfe, [LowFrequency, LowFrequency]),
        ],
        11 => vec![
            c,
            f,
            (Cpe, [SideLeft, SideRight]),
            (Sce, [BackCentre, BackCentre]),
            (Lfe, [LowFrequency, LowFrequency]),
        ],
        _ => return Err(Error::Unsupported(format!("channel configuration {cfg}"))),
    })
}

/// Reads `n` PCE element entries of one group (0 front, 1 side, 2 back) with their tags.
/// Front channels run from the centre outwards (a centre only for an odd channel count),
/// back channels from the outside in (a centre last for an odd count); other single channel
/// elements alternate left and right.
fn pce_group(
    b: &mut Bits,
    n: u32,
    kind: u8,
    elements: &mut Vec<(Element, [Position; 2])>,
    tags: &mut Vec<Option<u8>>,
) {
    use Position::*;
    let mut group = Vec::new();
    for _ in 0..n {
        let cpe = b.bit();
        let tag = b.read(4) as u8;
        group.push((cpe, tag));
    }
    let total: usize = group.iter().map(|&(c, _)| if c { 2 } else { 1 }).sum();
    let (left, right) = match kind {
        0 => (FrontLeft, FrontRight),
        1 => (SideLeft, SideRight),
        _ => (BackLeft, BackRight),
    };
    let mut side_next_left = true;
    // with an odd channel count, a single channel element is the centre (front: the first,
    // back: the last)
    let centre = if total.is_multiple_of(2) {
        None
    } else if kind == 0 {
        group.iter().position(|&(cpe, _)| !cpe)
    } else if kind == 2 {
        group.iter().rposition(|&(cpe, _)| !cpe)
    } else {
        None
    };
    for (i, &(cpe, tag)) in group.iter().enumerate() {
        let pos = if cpe {
            [left, right]
        } else if centre == Some(i) && kind == 0 {
            [FrontCentre, FrontCentre]
        } else if centre == Some(i) {
            [BackCentre, BackCentre]
        } else {
            let p = if side_next_left { left } else { right };
            side_next_left = !side_next_left;
            [p, p]
        };
        elements.push((if cpe { Element::Cpe } else { Element::Sce }, pos));
        tags.push(Some(tag));
    }
}

/// A program config element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pce {
    pub rate_index: u8,
    pub elements: Vec<(Element, [Position; 2])>,
    /// The element instance tag of each element.
    pub tags: Vec<Option<u8>>,
}

/// Parses a program config element; `align_base` is the bit position its byte alignment is
/// relative to.
pub(crate) fn parse_pce(b: &mut Bits, align_base: usize) -> Result<Pce> {
    use Position::*;
    b.read(4); // element_instance_tag
    // profile: Main, LC, SSR or LTP (the AudioSpecificConfig's object type decides)
    let object_type = b.read(2);
    if object_type == 2 {
        return Err(Error::Unsupported(object_type_name(3)));
    }
    let rate_index = b.read(4) as u8;
    let front = b.read(4);
    let side = b.read(4);
    let back = b.read(4);
    let lfe = b.read(2);
    let assoc = b.read(3);
    let cc = b.read(4);
    for _ in 0..2 {
        if b.bit() {
            b.read(4); // mono / stereo mixdown element
        }
    }
    if b.bit() {
        b.read(3); // matrix_mixdown_idx, pseudo_surround_enable
    }
    let mut elements = Vec::new();
    let mut tags = Vec::new();
    pce_group(b, front, 0, &mut elements, &mut tags);
    pce_group(b, side, 1, &mut elements, &mut tags);
    pce_group(b, back, 2, &mut elements, &mut tags);
    // Front channels only, with more than one pair: the outer pair is left and right and
    // the inner one (left and right of centre) plays as the surround pair, as such streams
    // mean (a 5.1 signalled as five front channels). With surrounds present, the inner pair
    // folds into left and right.
    if side == 0 && back == 0 {
        let pairs: Vec<usize> = (0..elements.len())
            .filter(|&i| elements[i].0 == Element::Cpe)
            .collect();
        if pairs.len() >= 2 {
            elements[pairs[0]].1 = [SideLeft, SideRight];
        }
    }
    for _ in 0..lfe {
        tags.push(Some(b.read(4) as u8));
        elements.push((Element::Lfe, [LowFrequency, LowFrequency]));
    }
    b.skip(4 * assoc as usize);
    b.skip(5 * cc as usize);
    // byte_alignment() relative to the enclosing structure
    let rel = b.pos().saturating_sub(align_base);
    b.skip((8 - rel % 8) % 8);
    let comment = b.read(8);
    b.skip(8 * comment as usize);
    if b.overrun() {
        return Err(Error::Malformed(
            "program config element is truncated".into(),
        ));
    }
    if elements.is_empty() {
        return Err(Error::Malformed(
            "program config element has no channels".into(),
        ));
    }
    if elements.iter().map(|(e, _)| e.channels()).sum::<usize>() > 16 {
        return Err(Error::Unsupported("more than 16 channels".into()));
    }
    if usize::from(rate_index) >= SAMPLE_RATES.len() {
        return Err(Error::Malformed("reserved sampling frequency index".into()));
    }
    Ok(Pce {
        rate_index,
        elements,
        tags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lc_stereo_44k() {
        let c = parse_asc(&[0x12, 0x10]).unwrap();
        assert_eq!(c.object_type, 2);
        assert_eq!(c.sample_rate, 44100);
        assert_eq!(c.core_channels(), 2);
        assert_eq!(c.sbr, Signal::Implicit);
    }

    #[test]
    fn he_aac_v2_explicit() {
        // aot 29, 24 kHz core (index 6), 1 channel, ext rate 48 kHz (index 3), aot 2, GA
        // flags 0: 11101 0110 0001 0011 00010 000 + padding
        let bits = "11101011000010011000100000000000";
        let bytes: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| c.iter().fold(0u8, |a, &d| (a << 1) | (d - b'0')))
            .collect();
        let c = parse_asc(&bytes).unwrap();
        assert_eq!(c.sample_rate, 24000);
        assert_eq!(c.sbr, Signal::Present);
        assert_eq!(c.ps, Signal::Present);
        assert_eq!(c.sbr_rate, Some(48000));
        assert_eq!(c.core_channels(), 1);
    }

    #[test]
    fn sync_extension() {
        // LC 22.05 kHz stereo, then 0x2b7, aot 5, sbr 1, 44.1 kHz, 0x548, ps 1
        let bits = "00010 0111 0010 000 01010110111 00101 1 0100 10101001000 1";
        let s: String = bits.chars().filter(|c| !c.is_whitespace()).collect();
        let mut bytes: Vec<u8> = Vec::new();
        for c in s.as_bytes().chunks(8) {
            let mut v = 0u8;
            for (i, &d) in c.iter().enumerate() {
                v |= (d - b'0') << (7 - i);
            }
            bytes.push(v);
        }
        let c = parse_asc(&bytes).unwrap();
        assert_eq!(c.sample_rate, 22050);
        assert_eq!(c.sbr, Signal::Present);
        assert_eq!(c.sbr_rate, Some(44100));
        assert_eq!(c.ps, Signal::Present);
    }

    #[test]
    fn main_ltp_and_960() {
        // AAC Main, 44.1 kHz, stereo
        let c = parse_asc(&[0x0a, 0x10]).unwrap();
        assert_eq!(
            (c.object_type, c.name(), c.frame_len),
            (1, "AAC Main", 1024)
        );
        // AAC-LTP, 44.1 kHz, stereo
        let c = parse_asc(&[0x22, 0x10]).unwrap();
        assert_eq!((c.object_type, c.name()), (4, "AAC-LTP"));
        // AAC-LC, 48 kHz, stereo, frameLengthFlag set
        let c = parse_asc(&[0x11, 0x94]).unwrap();
        assert_eq!((c.sample_rate, c.frame_len), (48000, 960));
    }

    #[test]
    fn refuses_by_name() {
        // AAC-SSR, 44.1 kHz, stereo
        let e = parse_asc(&[0x1a, 0x10]).unwrap_err();
        assert_eq!(
            e,
            Error::Unsupported("AAC-SSR (scalable sample rate)".into())
        );
        // HE-AAC (explicit, 24 kHz core) with 960-sample frames
        let e = parse_asc(&[0x2b, 0x11, 0x8a, 0x00]).unwrap_err();
        assert_eq!(e, Error::Unsupported(SBR_960.into()));
        // ER AAC-ELD: escape 31 + 7 = 39
        let e = parse_asc(&[0xf8, 0xe8, 0x80, 0x00]).unwrap_err();
        assert!(matches!(e, Error::Unsupported(s) if s.contains("ELD")));
    }

    #[test]
    fn made_configs_parse_back() {
        let c = parse_asc(&make_asc(2, 44100, 2, None)).unwrap();
        assert_eq!(
            (c.sample_rate, c.core_channels(), c.sbr),
            (44100, 2, Signal::Implicit)
        );
        let c = parse_asc(&make_asc(2, 24000, 1, Some(48000))).unwrap();
        assert_eq!(
            (c.sample_rate, c.sbr, c.sbr_rate),
            (24000, Signal::Present, Some(48000))
        );
        assert_eq!(
            (c.name(), c.output_rate(), c.output_channels()),
            ("HE-AAC", 48000, 1)
        );
        let c = parse_asc(&make_asc(2, 37800, 2, None)).unwrap();
        assert_eq!(c.sample_rate, 37800);
    }

    #[test]
    fn rate_indices() {
        assert_eq!(rate_index(48000), 3);
        assert_eq!(rate_index(44100), 4);
        assert_eq!(rate_index(8000), 11);
        assert_eq!(rate_index(7350), 11);
        assert_eq!(rate_index(96000), 0);
    }
}
