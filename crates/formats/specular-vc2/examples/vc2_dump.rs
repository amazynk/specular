//! Decodes raw VC-2 streams to planar YUV the way FFmpeg's rawvideo writes it (a dev tool for
//! comparing fixtures): `vc2_dump IN.drc OUT.yuv [IN OUT]...`.

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let [list] = &args[..] {
        // One argument: a file listing the pairs.
        let text = std::fs::read_to_string(list).expect("list");
        args = text.split_whitespace().map(str::to_owned).collect();
    }
    for pair in args.chunks(2) {
        let d = std::fs::read(&pair[0]).expect("input");
        let mut dec = specular_vc2::Decoder::new();
        let mut out = Vec::new();
        let mut at = 0;
        while at + 13 <= d.len() {
            let next = u32::from_be_bytes([d[at + 5], d[at + 6], d[at + 7], d[at + 8]]) as usize;
            let end = if next == 0 { d.len() } else { at + next };
            if let Some(p) = dec.decode(&d[at..end]).expect("decode") {
                for pl in &p.planes {
                    for &v in pl {
                        if p.bit_depth > 8 {
                            out.extend_from_slice(&v.to_le_bytes());
                        } else {
                            out.push(v as u8);
                        }
                    }
                }
            }
            at = end;
        }
        std::fs::write(&pair[1], out).expect("output");
    }
}
