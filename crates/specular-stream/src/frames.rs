//! Cutting an endless sound stream (internet radio) into files that play on their own: MP3
//! and ADTS AAC at frame boundaries, Ogg at page boundaries with the stream's header pages
//! put in front of every piece. The player splices the pieces without a gap.

/// How the stream is framed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framing {
    Mp3,
    Adts,
    Ogg,
}

impl Framing {
    /// From a `Content-Type`.
    pub fn of_content_type(ct: &str) -> Option<Framing> {
        let ct = ct.split(';').next()?.trim().to_ascii_lowercase();
        Some(match ct.as_str() {
            "audio/mpeg" | "audio/mp3" | "audio/mpeg3" | "audio/x-mpeg" => Framing::Mp3,
            "audio/aac" | "audio/aacp" | "audio/x-aac" | "audio/mp4a-latm" => Framing::Adts,
            "application/ogg" | "audio/ogg" | "audio/opus" | "audio/vorbis" => Framing::Ogg,
            _ => return None,
        })
    }

    /// From the first bytes.
    pub fn sniff(d: &[u8]) -> Option<Framing> {
        if d.starts_with(b"OggS") {
            return Some(Framing::Ogg);
        }
        let mut i = skip_id3(d);
        while i + 8 < d.len().min(64 * 1024) {
            if let Some((n, _)) = adts(&d[i..])
                && adts(d.get(i + n..).unwrap_or(&[])).is_some()
            {
                return Some(Framing::Adts);
            }
            if let Some((n, _)) = mp3(&d[i..])
                && mp3(d.get(i + n..).unwrap_or(&[])).is_some()
            {
                return Some(Framing::Mp3);
            }
            i += 1;
        }
        None
    }

    /// The file extension of a piece.
    pub fn extension(self) -> &'static str {
        match self {
            Framing::Mp3 => "mp3",
            Framing::Adts => "aac",
            Framing::Ogg => "ogg",
        }
    }
}

fn skip_id3(d: &[u8]) -> usize {
    if d.len() >= 10 && &d[..3] == b"ID3" {
        let n = d[6..10]
            .iter()
            .fold(0usize, |a, &b| (a << 7) | usize::from(b & 0x7F));
        (10 + n + if d[5] & 0x10 != 0 { 10 } else { 0 }).min(d.len())
    } else {
        0
    }
}

/// An MP3 (MPEG audio) frame header at the start of `d`: `(frame length, seconds)`.
pub fn mp3(d: &[u8]) -> Option<(usize, f64)> {
    if d.len() < 4 || d[0] != 0xFF || d[1] & 0xE0 != 0xE0 {
        return None;
    }
    let version = (d[1] >> 3) & 3; // 3: MPEG-1, 2: MPEG-2, 0: MPEG-2.5
    let layer = (d[1] >> 1) & 3; // 1: III, 2: II, 3: I
    let br_i = usize::from(d[2] >> 4);
    let sr_i = usize::from((d[2] >> 2) & 3);
    if version == 1 || layer == 0 || br_i == 0 || br_i == 15 || sr_i == 3 {
        return None;
    }
    const V1: [[u32; 14]; 3] = [
        [
            32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
        ],
        [
            32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
        ],
        [
            32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ],
    ];
    const V2: [[u32; 14]; 2] = [
        [
            32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
        ],
        [8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
    ];
    let l = usize::from(3 - layer); // 0: I, 1: II, 2: III
    let kbps = if version == 3 {
        V1[l][br_i - 1]
    } else {
        V2[l.min(1)][br_i - 1]
    };
    let rate = [44_100u32, 48_000, 32_000][sr_i]
        >> match version {
            3 => 0,
            2 => 1,
            _ => 2,
        };
    let pad = u32::from((d[2] >> 1) & 1);
    let br = kbps * 1000;
    let (len, samples) = match l {
        0 => ((12 * br / rate + pad) * 4, 384),
        1 => (144 * br / rate + pad, 1152),
        _ if version == 3 => (144 * br / rate + pad, 1152),
        _ => (72 * br / rate + pad, 576),
    };
    (len >= 4).then_some((len as usize, f64::from(samples) / f64::from(rate)))
}

/// An ADTS frame header at the start of `d`: `(frame length, seconds)`.
pub fn adts(d: &[u8]) -> Option<(usize, f64)> {
    if d.len() < 7 || d[0] != 0xFF || d[1] & 0xF6 != 0xF0 {
        return None;
    }
    let sr_i = usize::from((d[2] >> 2) & 0xF);
    const RATES: [u32; 13] = [
        96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025,
        8_000, 7_350,
    ];
    let rate = *RATES.get(sr_i)?;
    let len = (usize::from(d[3] & 3) << 11) | (usize::from(d[4]) << 3) | usize::from(d[5] >> 5);
    let blocks = u32::from(d[6] & 3) + 1;
    (len >= 7).then_some((len, f64::from(blocks * 1024) / f64::from(rate)))
}

/// An Ogg page at the start of `d`: `(page length, granule position)`.
pub fn ogg_page(d: &[u8]) -> Option<(usize, u64)> {
    if d.len() < 27 || &d[..4] != b"OggS" {
        return None;
    }
    let n = usize::from(d[26]);
    let segs = d.get(27..27 + n)?;
    let len = 27 + n + segs.iter().map(|&s| usize::from(s)).sum::<usize>();
    let granule = u64::from_le_bytes(d[6..14].try_into().ok()?);
    (len <= d.len()).then_some((len, granule))
}

/// Cuts a sound stream into pieces of about `target` seconds.
#[derive(Clone, Debug)]
pub struct Cutter {
    framing: Framing,
    target: f64,
    buf: Vec<u8>,
    /// Bytes of `buf` already walked (whole frames), and their seconds.
    walked: usize,
    seconds: f64,
    /// Ogg: the header pages (put in front of every piece) and the samples per second.
    headers: Vec<u8>,
    in_headers: bool,
    ogg_rate: f64,
    granule0: Option<u64>,
    /// The last frame was followed by another: the next header is believed on its own.
    synced: bool,
}

/// A piece: whole frames, and its seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub data: Vec<u8>,
    pub seconds: f64,
}

impl Cutter {
    pub fn new(framing: Framing, target: f64) -> Cutter {
        Cutter {
            framing,
            target,
            buf: Vec::new(),
            walked: 0,
            seconds: 0.0,
            headers: Vec::new(),
            in_headers: true,
            ogg_rate: 48_000.0,
            granule0: None,
            synced: false,
        }
    }

    pub fn framing(&self) -> Framing {
        self.framing
    }

    /// Seconds walked but not yet handed out.
    pub fn pending(&self) -> f64 {
        self.seconds
    }

    /// Takes stream bytes; returns the pieces now complete.
    pub fn push(&mut self, data: &[u8]) -> Vec<Piece> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            let rest = &self.buf[self.walked..];
            let step = match self.framing {
                Framing::Mp3 | Framing::Adts => {
                    let parse = if self.framing == Framing::Mp3 {
                        mp3
                    } else {
                        adts
                    };
                    match parse(rest) {
                        Some((n, s)) if n <= rest.len() => {
                            // A frame is believed when the next one follows (or the data ends
                            // right there, to be checked when more comes).
                            if !self.synced && rest.len() >= n + 4 && parse(&rest[n..]).is_none() {
                                None
                            } else {
                                Some((n, s))
                            }
                        }
                        Some(_) => break,
                        None => None,
                    }
                }
                Framing::Ogg => match ogg_page(rest) {
                    Some((n, g)) => {
                        if self.in_headers && (g == 0 || self.headers.is_empty()) {
                            if self.headers.is_empty() {
                                self.ogg_rate = ogg_rate(rest).unwrap_or(48_000.0);
                            }
                            self.headers.extend_from_slice(&rest[..n]);
                            self.buf.drain(self.walked..self.walked + n);
                            continue;
                        }
                        self.in_headers = false;
                        let s = if g == u64::MAX {
                            0.0
                        } else {
                            let g0 = *self.granule0.get_or_insert(g);
                            let s = (g.saturating_sub(g0)) as f64 / self.ogg_rate - self.seconds;
                            s.max(0.0)
                        };
                        Some((n, s))
                    }
                    None if rest.len() >= 27 && &rest[..4] == b"OggS" => break,
                    None if rest.len() < 27 => break,
                    None => None,
                },
            };
            match step {
                Some((n, s)) => {
                    self.synced = true;
                    self.walked += n;
                    self.seconds += s;
                    if self.seconds >= self.target {
                        out.push(self.cut());
                    }
                }
                None => {
                    self.synced = false;
                    // Lost sync: drop a byte and look again.
                    if self.buf.len() - self.walked < 8 {
                        break;
                    }
                    self.buf.remove(self.walked);
                }
            }
        }
        // Keep what is left small when nothing frames (not sound at all).
        if self.walked == 0 && self.buf.len() > 1 << 20 {
            self.buf.clear();
        }
        out
    }

    fn cut(&mut self) -> Piece {
        let mut data = Vec::with_capacity(self.headers.len() + self.walked);
        if self.framing == Framing::Ogg {
            data.extend_from_slice(&self.headers);
        }
        data.extend(self.buf.drain(..self.walked));
        let seconds = std::mem::take(&mut self.seconds);
        if self.framing == Framing::Ogg {
            // The next piece's times count from where this one ended.
            self.granule0 = None;
        }
        self.walked = 0;
        Piece { data, seconds }
    }

    /// What is left at the stream's end, if anything.
    pub fn finish(&mut self) -> Option<Piece> {
        (self.walked > 0).then(|| self.cut())
    }
}

/// The sample rate an Ogg stream's first page announces (Opus: always 48 kHz; Vorbis: its
/// identification header).
fn ogg_rate(page: &[u8]) -> Option<f64> {
    let n = usize::from(*page.get(26)?);
    let body = page.get(27 + n..)?;
    if body.starts_with(b"OpusHead") {
        return Some(48_000.0);
    }
    if body.len() >= 16 && body[0] == 1 && &body[1..7] == b"vorbis" {
        return Some(f64::from(u32::from_le_bytes(body[12..16].try_into().ok()?)));
    }
    if body.starts_with(b"\x7fFLAC") && body.len() >= 30 {
        let sr =
            (u32::from(body[27]) << 12) | (u32::from(body[28]) << 4) | (u32::from(body[29]) >> 4);
        return Some(f64::from(sr));
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A silent MPEG-1 Layer III frame (128 kbit/s, 44.1 kHz, stereo): side information of
    /// zeros decodes to silence.
    pub(crate) fn mp3_frame(pad: bool) -> Vec<u8> {
        let n = 417 + usize::from(pad);
        let mut f = vec![0u8; n];
        f[..4].copy_from_slice(&[0xFF, 0xFB, 0x90 | if pad { 2 } else { 0 }, 0x64]);
        f
    }

    #[test]
    fn frame_headers_read() {
        let (n, s) = mp3(&mp3_frame(false)).unwrap();
        assert_eq!(n, 417);
        assert!((s - 1152.0 / 44_100.0).abs() < 1e-12);
        assert_eq!(mp3(&mp3_frame(true)).unwrap().0, 418);
        // ADTS, 44.1 kHz, frame of 300 bytes.
        let h = [
            0xFF,
            0xF1,
            0x50,
            0x80,
            (300 >> 3) as u8,
            ((300 & 7) << 5) as u8 | 0x1F,
            0xFC,
        ];
        let (n, s) = adts(&h).unwrap();
        assert_eq!(n, 300);
        assert!((s - 1024.0 / 44_100.0).abs() < 1e-12);
        assert_eq!(Framing::of_content_type("audio/mpeg"), Some(Framing::Mp3));
        assert_eq!(Framing::of_content_type("audio/aacp"), Some(Framing::Adts));
        assert_eq!(Framing::of_content_type("text/html"), None);
    }

    /// Pieces are whole frames of about the target length, whatever the network chunks;
    /// nothing is lost or added; garbage between frames is skipped.
    #[test]
    fn a_radio_stream_is_cut_at_frames() {
        let mut stream = Vec::new();
        let mut frames = 0;
        for i in 0..400 {
            if i == 100 {
                stream.extend_from_slice(&[0x12, 0xFF, 0x00, 0x34]);
            }
            let mut f = mp3_frame(i % 3 == 0);
            f[10] = i as u8;
            stream.extend(f);
            frames += 1;
        }
        assert_eq!(Framing::sniff(&stream), Some(Framing::Mp3));
        for chunk in [1, 7, 500, 4096, 100_000] {
            let mut c = Cutter::new(Framing::Mp3, 2.0);
            let mut pieces = Vec::new();
            for part in stream.chunks(chunk) {
                pieces.extend(c.push(part));
            }
            pieces.extend(c.finish());
            let mut n = 0;
            for p in &pieces {
                let mut at = 0;
                while at < p.data.len() {
                    let (len, _) = mp3(&p.data[at..]).expect("whole frames");
                    at += len;
                    n += 1;
                }
                assert_eq!(at, p.data.len());
            }
            assert_eq!(n, frames, "chunk {chunk}");
            let total: f64 = pieces.iter().map(|p| p.seconds).sum();
            assert!((total - frames as f64 * 1152.0 / 44_100.0).abs() < 1e-6);
            assert!(
                pieces[..pieces.len() - 1]
                    .iter()
                    .all(|p| p.seconds >= 2.0 && p.seconds < 2.1)
            );
        }
    }
}
