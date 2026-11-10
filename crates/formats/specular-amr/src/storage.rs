//! The AMR / AMR-WB storage format (RFC 4867 §5, TS 26.101 / 26.201 Annex): a magic line,
//! then frames of one TOC byte (frame type, quality bit) and the speech bits sorted by
//! importance. 3GP/MP4 (`samr`, `sawb`) and Matroska samples hold the same frames.

use crate::{Error, Frame, RxType, nb, wb};

/// Which codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Codec {
    /// AMR (narrowband), 8 kHz.
    Nb,
    /// AMR-WB (wideband), 16 kHz.
    Wb,
}

impl Codec {
    pub fn sample_rate(self) -> u32 {
        match self {
            Codec::Nb => 8000,
            Codec::Wb => 16000,
        }
    }

    pub fn frame_len(self) -> usize {
        match self {
            Codec::Nb => 160,
            Codec::Wb => 320,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Codec::Nb => "AMR",
            Codec::Wb => "AMR-WB",
        }
    }

    /// Number of speech modes (frame types 0..n are speech).
    pub fn modes(self) -> u8 {
        match self {
            Codec::Nb => 8,
            Codec::Wb => 9,
        }
    }

    /// The bit rate of a speech mode in bit/s.
    pub fn bitrate(self, mode: u8) -> Option<u32> {
        const NB: [u32; 8] = [4750, 5150, 5900, 6700, 7400, 7950, 10200, 12200];
        const WB: [u32; 9] = [6600, 8850, 12650, 14250, 15850, 18250, 19850, 23050, 23850];
        match self {
            Codec::Nb => NB.get(mode as usize).copied(),
            Codec::Wb => WB.get(mode as usize).copied(),
        }
    }

    /// "12.2 kbit/s"-style name of a speech mode.
    pub fn mode_name(self, mode: u8) -> Option<String> {
        self.bitrate(mode).map(|b| {
            let s = format!("{:.2}", b as f64 / 1000.0);
            let s = s.trim_end_matches('0').trim_end_matches('.');
            format!("{s} kbit/s")
        })
    }

    fn sid_type(self) -> u8 {
        match self {
            Codec::Nb => 8,
            Codec::Wb => 9,
        }
    }

    /// Speech (or SID) bits of a frame type, None for types without bits.
    fn bits(self, ft: u8) -> Option<usize> {
        match self {
            Codec::Nb => nb::BITS.get(ft as usize).copied(),
            Codec::Wb => wb::BITS.get(ft as usize).copied(),
        }
    }

    fn sort(self, ft: u8) -> &'static [i16] {
        match self {
            Codec::Nb => nb::sort(ft),
            Codec::Wb => wb::sort(ft),
        }
    }
}

/// Bytes after the TOC byte for a frame type (0 for NO_DATA and the reserved types).
pub fn packed_size(codec: Codec, ft: u8) -> usize {
    match codec.bits(ft) {
        Some(b) => {
            // SID frames carry STI and the mode indication after their 35 bits
            let b = if ft == codec.sid_type() {
                b + 1 + if codec == Codec::Nb { 3 } else { 4 }
            } else {
                b
            };
            b.div_ceil(8)
        }
        None => 0,
    }
}

/// Reads one storage-format frame from the start of `data`: the frame and the bytes it
/// took. A frame cut short by the end of the data comes back as a bad frame. None if
/// `data` is empty.
pub fn read_frame(codec: Codec, data: &[u8]) -> Option<(Frame, usize)> {
    let (&toc, rest) = data.split_first()?;
    let ft = (toc >> 3) & 0x0f;
    let good = toc & 0x04 != 0;
    let size = packed_size(codec, ft);
    let sid = codec.sid_type();
    if ft > sid {
        let rx = if codec == Codec::Wb && ft == 14 {
            RxType::SpeechLost
        } else {
            RxType::NoData
        };
        return Some((Frame::empty(rx, 0), 1 + size));
    }
    if rest.len() < size {
        let rx = if ft == sid {
            RxType::SidBad
        } else {
            RxType::SpeechBad
        };
        return Some((Frame::empty(rx, if ft == sid { 0 } else { ft }), data.len()));
    }
    let payload = &rest[..size];
    let bit = |k: usize| (payload[k >> 3] >> (7 - (k & 7))) & 1;
    let n = codec.bits(ft).unwrap_or(0);
    let sort = codec.sort(ft);
    let mut frame = Frame::empty(RxType::SpeechGood, ft);
    for (k, &dst) in sort.iter().enumerate().take(n) {
        frame.bits[dst as usize] = bit(k);
    }
    if ft == sid {
        let sti = bit(n);
        frame.rx = match (good, sti) {
            (false, _) => RxType::SidBad,
            (true, 1) => RxType::SidUpdate,
            (true, _) => RxType::SidFirst,
        };
        frame.mode = match codec {
            // AMR's mode indication is sent least significant bit first
            Codec::Nb => bit(n + 1) | (bit(n + 2) << 1) | (bit(n + 3) << 2),
            // AMR-WB's decoder keeps the previous speech mode through comfort noise
            Codec::Wb => 0xff,
        };
    } else if !good {
        frame.rx = RxType::SpeechBad;
    }
    Some((frame, 1 + size))
}

/// Whether `data` is exactly a run of storage-format frames (TOC bytes with F = 0 and the
/// padding bits clear, frame types the codec knows, sizes adding up).
pub fn is_storage_packet(codec: Codec, data: &[u8]) -> bool {
    let mut pos = 0;
    while pos < data.len() {
        let toc = data[pos];
        let ft = (toc >> 3) & 0x0f;
        if toc & 0x83 != 0 || (ft > codec.sid_type() && ft < 14) {
            return false;
        }
        pos += 1 + packed_size(codec, ft);
    }
    pos == data.len() && !data.is_empty()
}

/// Splits an RTP payload in the octet-aligned mode of RFC 4867 §4.4 (a CMR byte, the
/// table of contents with its F bits, then the frames) into frames; None if `data` is not
/// one. The bandwidth-efficient mode is not accepted (no container stores it).
pub fn read_rtp_octet_aligned(codec: Codec, data: &[u8]) -> Option<Vec<Frame>> {
    let (&cmr, rest) = data.split_first()?;
    if cmr & 0x0f != 0 || ((cmr >> 4) > codec.sid_type() && (cmr >> 4) != 15) {
        return None;
    }
    let mut tocs = Vec::new();
    let mut at = 0;
    loop {
        let &toc = rest.get(at)?;
        at += 1;
        if toc & 0x03 != 0 {
            return None;
        }
        tocs.push(toc & 0x7f);
        if toc & 0x80 == 0 {
            break;
        }
        if tocs.len() > 64 {
            return None;
        }
    }
    let total: usize = tocs
        .iter()
        .map(|&t| packed_size(codec, (t >> 3) & 0x0f))
        .sum();
    if at + total != rest.len() {
        return None;
    }
    let mut frames = Vec::with_capacity(tocs.len());
    let mut buf = Vec::new();
    for toc in tocs {
        let size = packed_size(codec, (toc >> 3) & 0x0f);
        buf.clear();
        buf.push(toc);
        buf.extend_from_slice(&rest[at..at + size]);
        at += size;
        frames.push(read_frame(codec, &buf)?.0);
    }
    Some(frames)
}

/// The frames of one container sample: storage-format frames (3GP/MP4 `samr`/`sawb`,
/// Matroska), else an RTP octet-aligned payload, else whatever storage frames can be read.
pub fn packet_frames(codec: Codec, data: &[u8]) -> Vec<Frame> {
    if !is_storage_packet(codec, data)
        && let Some(f) = read_rtp_octet_aligned(codec, data)
    {
        return f;
    }
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some((f, used)) = read_frame(codec, &data[pos.min(data.len())..]) {
        out.push(f);
        pos += used;
    }
    out
}

/// What a `.amr` / `.awb` file holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo {
    pub codec: Codec,
    pub channels: u16,
    /// Bytes before the first frame.
    pub header_len: usize,
}

const MAGIC_NB: &[u8] = b"#!AMR\n";
const MAGIC_WB: &[u8] = b"#!AMR-WB\n";
const MAGIC_NB_MC: &[u8] = b"#!AMR_MC1.0\n";
const MAGIC_WB_MC: &[u8] = b"#!AMR-WB_MC1.0\n";

/// Whether `data` starts like an AMR storage file.
pub fn is_amr_file(data: &[u8]) -> bool {
    data.starts_with(b"#!AMR")
}

/// Reads the magic line (and the multichannel description) of a storage file.
pub fn probe_file(data: &[u8]) -> Result<FileInfo, Error> {
    let mc = |codec, magic: &[u8]| -> Result<FileInfo, Error> {
        let d = data
            .get(magic.len()..magic.len() + 4)
            .ok_or_else(|| Error::Malformed("header cut short".into()))?;
        let channels = (u32::from_be_bytes([d[0], d[1], d[2], d[3]]) & 0x0f) as u16;
        if channels == 0 {
            return Err(Error::Malformed(
                "multichannel file with no channels".into(),
            ));
        }
        Ok(FileInfo {
            codec,
            channels,
            header_len: magic.len() + 4,
        })
    };
    if data.starts_with(MAGIC_NB) {
        Ok(FileInfo {
            codec: Codec::Nb,
            channels: 1,
            header_len: MAGIC_NB.len(),
        })
    } else if data.starts_with(MAGIC_WB) {
        Ok(FileInfo {
            codec: Codec::Wb,
            channels: 1,
            header_len: MAGIC_WB.len(),
        })
    } else if data.starts_with(MAGIC_NB_MC) {
        mc(Codec::Nb, MAGIC_NB_MC)
    } else if data.starts_with(MAGIC_WB_MC) {
        mc(Codec::Wb, MAGIC_WB_MC)
    } else if data.starts_with(b"#!AMR") {
        Err(Error::Unsupported(
            "an AMR storage format version this decoder does not know".into(),
        ))
    } else {
        Err(Error::Malformed("no #!AMR header".into()))
    }
}

/// Facts about a stream, gathered while decoding.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub frames: u64,
    /// Frames per speech mode.
    pub per_mode: [u64; 9],
    pub sid: u64,
    pub no_data: u64,
    pub bad: u64,
}

impl Stats {
    pub fn count(&mut self, f: &Frame) {
        self.frames += 1;
        match f.rx {
            RxType::SpeechGood | RxType::SpeechDegraded | RxType::Onset => {
                if let Some(c) = self.per_mode.get_mut(f.mode as usize) {
                    *c += 1;
                }
            }
            RxType::SidFirst | RxType::SidUpdate => self.sid += 1,
            RxType::NoData => self.no_data += 1,
            RxType::SpeechBad | RxType::SidBad | RxType::SpeechLost => self.bad += 1,
        }
    }

    /// The speech mode most used, if any speech frames.
    pub fn main_mode(&self) -> Option<u8> {
        let (m, &n) = self
            .per_mode
            .iter()
            .enumerate()
            .max_by_key(|&(i, &n)| (n, std::cmp::Reverse(i)))?;
        (n > 0).then_some(m as u8)
    }

    /// Whether more than one speech mode occurs.
    pub fn mode_switching(&self) -> bool {
        self.per_mode.iter().filter(|&&n| n > 0).count() > 1
    }
}

/// A whole `.amr`/`.awb` file decoded frame by frame, all channels interleaved.
pub struct Stream<'a> {
    pub info: FileInfo,
    data: &'a [u8],
    pos: usize,
    decoders: Vec<crate::Decoder>,
    pub stats: Stats,
}

impl<'a> Stream<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Error> {
        let info = probe_file(data)?;
        let decoders = (0..info.channels)
            .map(|_| crate::Decoder::new(info.codec))
            .collect();
        Ok(Stream {
            pos: info.header_len,
            info,
            data,
            decoders,
            stats: Stats::default(),
        })
    }

    /// Restarts at the first frame.
    pub fn rewind(&mut self) {
        self.pos = self.info.header_len;
        for d in &mut self.decoders {
            d.reset();
        }
        self.stats = Stats::default();
    }

    /// Byte offset of the next frame block.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Skips one frame block without decoding it (for seeking); false at the end.
    pub fn skip_block(&mut self) -> bool {
        for _ in 0..self.info.channels {
            let Some((_, used)) =
                read_frame(self.info.codec, &self.data[self.pos.min(self.data.len())..])
            else {
                return false;
            };
            self.pos += used;
        }
        true
    }

    /// Decodes the next frame block (one frame per channel), appending interleaved
    /// samples; false at the end.
    pub fn next_block(&mut self, out: &mut Vec<i16>) -> bool {
        let n = self.info.codec.frame_len();
        let ch = self.info.channels as usize;
        let mut per = Vec::with_capacity(ch);
        for c in 0..ch {
            let Some((frame, used)) =
                read_frame(self.info.codec, &self.data[self.pos.min(self.data.len())..])
            else {
                if c == 0 {
                    return false;
                }
                per.push(vec![0i16; n]);
                continue;
            };
            self.pos += used;
            if c == 0 {
                self.stats.count(&frame);
            }
            let mut s = Vec::with_capacity(n);
            self.decoders[c].decode_frame(&frame, &mut s);
            per.push(s);
        }
        if ch == 1 {
            out.extend_from_slice(&per[0]);
        } else {
            for i in 0..n {
                for p in &per {
                    out.push(p[i]);
                }
            }
        }
        true
    }

    /// Decodes everything left.
    pub fn decode_all(&mut self) -> Vec<i16> {
        let mut out = Vec::new();
        while self.next_block(&mut out) {}
        out
    }
}

/// Counts frames and modes of a storage file without decoding.
pub fn scan(data: &[u8]) -> Result<(FileInfo, Stats), Error> {
    let info = probe_file(data)?;
    let mut stats = Stats::default();
    let mut pos = info.header_len;
    let mut ch = 0;
    while let Some((frame, used)) = read_frame(info.codec, &data[pos.min(data.len())..]) {
        if ch == 0 {
            stats.count(&frame);
        }
        ch = (ch + 1) % info.channels as usize;
        pos += used;
    }
    Ok((info, stats))
}
