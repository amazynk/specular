//! A minimal Ogg page reader for the tests: the packets of the first logical stream.

/// One packet and the granule position of the page it ends on (-1 if it ends on none).
pub struct Packet {
    pub data: Vec<u8>,
    pub granule: i64,
}

/// The packets of the first logical stream in `file` (no CRC check).
pub fn packets(file: &[u8]) -> Vec<Packet> {
    let mut out = Vec::new();
    let mut pos = 0;
    let mut serial = None;
    let mut partial = Vec::new();
    while pos + 27 <= file.len() && &file[pos..pos + 4] == b"OggS" {
        let h = &file[pos..];
        let granule = i64::from_le_bytes(h[6..14].try_into().unwrap());
        let ser = u32::from_le_bytes(h[14..18].try_into().unwrap());
        let nseg = usize::from(h[26]);
        let lacing = &h[27..27 + nseg];
        let mut body = 27 + nseg;
        let size: usize = lacing.iter().map(|&l| usize::from(l)).sum();
        if *serial.get_or_insert(ser) == ser {
            let mut ended = Vec::new();
            for &l in lacing {
                partial.extend_from_slice(&h[body..body + usize::from(l)]);
                body += usize::from(l);
                if l < 255 {
                    ended.push(std::mem::take(&mut partial));
                }
            }
            let last = ended.len();
            for (i, data) in ended.into_iter().enumerate() {
                let granule = if i + 1 == last { granule } else { -1 };
                out.push(Packet { data, granule });
            }
        }
        pos += 27 + nseg + size;
    }
    out
}
