//! Internet radio's in-band titles (the ICY protocol of Shoutcast and Icecast): asked for with
//! `Icy-MetaData: 1`, the server puts a metadata block after every `icy-metaint` bytes of
//! sound: one length byte (×16), then `StreamTitle='…';StreamUrl='…';` padded with zeros.
//! [`Demux`] takes the blocks out so what is left is the plain sound stream.

/// Splits an ICY body into sound and metadata.
#[derive(Clone, Debug)]
pub struct Demux {
    metaint: usize,
    state: State,
}

#[derive(Clone, Debug)]
enum State {
    /// Sound bytes left before the next length byte.
    Sound(usize),
    /// Metadata bytes still to come, and those read.
    Meta(usize, Vec<u8>),
}

impl Demux {
    /// For a server that said `icy-metaint: metaint` (`0`: no metadata, all sound).
    pub fn new(metaint: usize) -> Demux {
        Demux {
            metaint,
            state: State::Sound(metaint),
        }
    }

    /// Splits `data`: sound is appended to `sound`; each complete metadata block (non-empty)
    /// comes back as text.
    pub fn feed(&mut self, mut data: &[u8], sound: &mut Vec<u8>) -> Vec<String> {
        let mut blocks = Vec::new();
        if self.metaint == 0 {
            sound.extend_from_slice(data);
            return blocks;
        }
        while !data.is_empty() {
            match &mut self.state {
                State::Sound(left) => {
                    if *left == 0 {
                        let n = usize::from(data[0]) * 16;
                        data = &data[1..];
                        self.state = if n == 0 {
                            State::Sound(self.metaint)
                        } else {
                            State::Meta(n, Vec::with_capacity(n))
                        };
                        continue;
                    }
                    let take = (*left).min(data.len());
                    sound.extend_from_slice(&data[..take]);
                    *left -= take;
                    data = &data[take..];
                }
                State::Meta(left, buf) => {
                    let take = (*left).min(data.len());
                    buf.extend_from_slice(&data[..take]);
                    *left -= take;
                    data = &data[take..];
                    if *left == 0 {
                        let text = text_of(buf);
                        if !text.is_empty() {
                            blocks.push(text);
                        }
                        self.state = State::Sound(self.metaint);
                    }
                }
            }
        }
        blocks
    }
}

/// A block's text: UTF-8 when it is, else Latin-1 (older servers), without the padding.
fn text_of(b: &[u8]) -> String {
    let end = b.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
    let b = &b[..end];
    match std::str::from_utf8(b) {
        Ok(s) => s.to_owned(),
        Err(_) => b.iter().map(|&c| char::from(c)).collect(),
    }
}

/// The value of `key` in a metadata block (`StreamTitle='Artist - Song';`). A quote inside
/// the value is allowed: the value ends at the `';` that closes it.
pub fn field(block: &str, key: &str) -> Option<String> {
    let start = block.find(&format!("{key}='"))? + key.len() + 2;
    let rest = &block[start..];
    let end = rest
        .find("';")
        .unwrap_or_else(|| rest.trim_end().trim_end_matches('\'').len());
    let v = rest[..end].trim();
    (!v.is_empty()).then(|| v.to_owned())
}

/// The now-playing title of a metadata block, if it has one.
pub fn stream_title(block: &str) -> Option<String> {
    field(block, "StreamTitle")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str) -> Vec<u8> {
        let n = text.len().div_ceil(16);
        let mut b = vec![n as u8];
        b.extend_from_slice(text.as_bytes());
        b.resize(1 + n * 16, 0);
        b
    }

    /// Sound and titles come apart whatever the chunking of the body.
    #[test]
    fn titles_come_out_of_the_sound() {
        let metaint = 100;
        let mut body = Vec::new();
        let mut want = Vec::new();
        let titles = ["StreamTitle='Artist - It''s a Song';StreamUrl='';", ""];
        for i in 0..6u8 {
            let s: Vec<u8> = (0..metaint).map(|k| i.wrapping_mul(7) ^ k as u8).collect();
            body.extend_from_slice(&s);
            want.extend_from_slice(&s);
            let t = titles[usize::from(i % 2)];
            if t.is_empty() {
                body.push(0);
            } else {
                body.extend(block(t));
            }
        }
        body.extend_from_slice(&[9; 40]);
        want.extend_from_slice(&[9; 40]);
        for chunk in [1, 3, 17, 100, 101, 4096] {
            let mut d = Demux::new(metaint);
            let mut sound = Vec::new();
            let mut got = Vec::new();
            for c in body.chunks(chunk) {
                got.extend(d.feed(c, &mut sound));
            }
            assert_eq!(sound, want, "chunk {chunk}");
            assert_eq!(got.len(), 3, "chunk {chunk}");
            assert_eq!(
                stream_title(&got[0]).as_deref(),
                Some("Artist - It''s a Song")
            );
        }
    }

    #[test]
    fn titles_are_read_in_utf8_or_latin1() {
        assert_eq!(
            stream_title("StreamTitle='Björk - Jóga';").as_deref(),
            Some("Björk - Jóga")
        );
        assert_eq!(
            text_of(b"StreamTitle='Bj\xF6rk';\0\0"),
            "StreamTitle='Björk';"
        );
        assert_eq!(stream_title("StreamTitle='';"), None);
        assert_eq!(stream_title("StreamUrl='x';"), None);
        assert_eq!(
            stream_title("StreamTitle='Don't Stop';StreamUrl='http://x';").as_deref(),
            Some("Don't Stop")
        );
        assert_eq!(
            field("StreamTitle='a';StreamUrl='http://x';", "StreamUrl").as_deref(),
            Some("http://x")
        );
        let mut d = Demux::new(0);
        let mut s = Vec::new();
        assert!(d.feed(b"abc", &mut s).is_empty());
        assert_eq!(s, b"abc");
    }
}
