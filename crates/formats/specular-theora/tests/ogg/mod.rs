//! A minimal Ogg reader for the tests: the packets of the first logical stream.

/// All packets of the first logical stream in `data`, in order.
pub fn packets(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut serial = None;
    let mut pos = 0;
    while pos + 27 <= data.len() && &data[pos..pos + 4] == b"OggS" {
        let h = &data[pos..];
        let sn = u32::from_le_bytes([h[14], h[15], h[16], h[17]]);
        let nseg = h[26] as usize;
        if pos + 27 + nseg > data.len() {
            break;
        }
        let lacing = &h[27..27 + nseg];
        let mut body = pos + 27 + nseg;
        let mine = *serial.get_or_insert(sn) == sn;
        for &l in lacing {
            let end = (body + l as usize).min(data.len());
            if mine {
                cur.extend_from_slice(&data[body..end]);
                if l < 255 {
                    out.push(std::mem::take(&mut cur));
                }
            }
            body = end;
        }
        pos = body;
    }
    out
}
