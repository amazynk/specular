//! Hand-built DWAA / DWAB files (the `exr` oracle cannot write DWA): single-part scanline files
//! whose chunks are assembled section by section.

/// Pixel type codes of the file format.
pub const UINT: u32 = 0;
pub const HALF: u32 = 1;
pub const FLOAT: u32 = 2;
pub const DWAA: u8 = 8;
pub const DWAB: u8 = 9;

/// zlib-compressed `data`. (A chunk must come out smaller than its raw samples, or it is read
/// as stored raw.)
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; zlib_rs::compress_bound(data.len())];
    let (z, rc) = zlib_rs::compress_slice(&mut out, data, zlib_rs::DeflateConfig::default());
    assert_eq!(rc, zlib_rs::ReturnCode::Ok);
    z.to_vec()
}

/// OpenEXR RLE, literal runs only.
pub fn rle(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for run in data.chunks(127) {
        out.push((-(run.len() as i8)) as u8);
        out.extend_from_slice(run);
    }
    out
}

/// ZIP's interleave and byte predictor, the inverse of what the decoder undoes.
pub fn predict_interleave(data: &[u8]) -> Vec<u8> {
    let mut t: Vec<u8> = data.iter().step_by(2).copied().collect();
    t.extend(data.iter().skip(1).step_by(2));
    let mut out = t.clone();
    for i in 1..t.len() {
        out[i] = t[i].wrapping_sub(t[i - 1]).wrapping_add(128);
    }
    out
}

/// One version-2 rule: suffix, colour component (-1 none), scheme (0 unknown, 1 lossy, 2 RLE),
/// pixel type.
pub fn rule(suffix: &str, csc: i8, scheme: u8, ty: u32) -> Vec<u8> {
    let mut r = suffix.as_bytes().to_vec();
    r.push(0);
    r.push((((csc + 1) as u8) << 4) | (scheme << 2));
    r.push(ty as u8);
    r
}

/// The sections of one DWA chunk, before compression.
#[derive(Default)]
pub struct Sections {
    /// Concatenated rules (version 2), or `None` for a version-1 chunk.
    pub rules: Option<Vec<u8>>,
    /// Raw samples of the unknown channels, channel after channel.
    pub unknown: Vec<u8>,
    /// Byte planes of the RLE channels.
    pub rle: Vec<u8>,
    /// AC codes (stored with deflate).
    pub ac: Vec<u16>,
    /// DC values (half bits), component after component.
    pub dc: Vec<u16>,
}

pub fn chunk(s: &Sections) -> Vec<u8> {
    let unknown = if s.unknown.is_empty() {
        Vec::new()
    } else {
        zlib(&s.unknown)
    };
    let ac_bytes: Vec<u8> = s.ac.iter().flat_map(|v| v.to_le_bytes()).collect();
    let ac = if s.ac.is_empty() {
        Vec::new()
    } else {
        zlib(&ac_bytes)
    };
    let dc_bytes: Vec<u8> = s.dc.iter().flat_map(|v| v.to_le_bytes()).collect();
    let dc = if s.dc.is_empty() {
        Vec::new()
    } else {
        zlib(&predict_interleave(&dc_bytes))
    };
    let rle_packed = rle(&s.rle);
    let rle_z = if s.rle.is_empty() {
        Vec::new()
    } else {
        zlib(&rle_packed)
    };
    let counters: [u64; 11] = [
        if s.rules.is_some() { 2 } else { 1 },
        s.unknown.len() as u64,
        unknown.len() as u64,
        ac.len() as u64,
        dc.len() as u64,
        rle_z.len() as u64,
        if s.rle.is_empty() {
            0
        } else {
            rle_packed.len() as u64
        },
        s.rle.len() as u64,
        s.ac.len() as u64,
        s.dc.len() as u64,
        1,
    ];
    let mut out: Vec<u8> = counters.iter().flat_map(|v| v.to_le_bytes()).collect();
    if let Some(r) = &s.rules {
        out.extend_from_slice(&((r.len() + 2) as u16).to_le_bytes());
        out.extend_from_slice(r);
    }
    for sec in [unknown, ac, dc, rle_z] {
        out.extend_from_slice(&sec);
    }
    out
}

fn attr(out: &mut Vec<u8>, name: &str, ty: &str, value: &[u8]) {
    for s in [name, ty] {
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    }
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value);
}

/// A single-part scanline file of `w` x `h` pixels with `chans` (name, pixel type; sorted by
/// name), one chunk per `lines` lines.
pub fn file(
    chans: &[(&str, u32)],
    w: u32,
    h: u32,
    compression: u8,
    chunks: &[Vec<u8>],
    lines: u32,
) -> Vec<u8> {
    let mut out = vec![0x76, 0x2f, 0x31, 0x01, 2, 0, 0, 0];
    let mut list = Vec::new();
    for (name, ty) in chans {
        list.extend_from_slice(name.as_bytes());
        list.push(0);
        list.extend_from_slice(&ty.to_le_bytes());
        list.extend_from_slice(&[0, 0, 0, 0]);
        list.extend_from_slice(&1i32.to_le_bytes());
        list.extend_from_slice(&1i32.to_le_bytes());
    }
    list.push(0);
    attr(&mut out, "channels", "chlist", &list);
    attr(&mut out, "compression", "compression", &[compression]);
    let bx: Vec<u8> = [0i32, 0, w as i32 - 1, h as i32 - 1]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    attr(&mut out, "dataWindow", "box2i", &bx);
    attr(&mut out, "displayWindow", "box2i", &bx);
    attr(&mut out, "lineOrder", "lineOrder", &[0]);
    attr(&mut out, "pixelAspectRatio", "float", &1f32.to_le_bytes());
    attr(&mut out, "screenWindowCenter", "v2f", &[0; 8]);
    attr(&mut out, "screenWindowWidth", "float", &1f32.to_le_bytes());
    out.push(0);
    let mut pos = out.len() + chunks.len() * 8;
    let mut table = Vec::new();
    for c in chunks {
        table.extend_from_slice(&(pos as u64).to_le_bytes());
        pos += 8 + c.len();
    }
    out.extend_from_slice(&table);
    for (i, c) in chunks.iter().enumerate() {
        out.extend_from_slice(&((i as u32 * lines) as i32).to_le_bytes());
        out.extend_from_slice(&(c.len() as i32).to_le_bytes());
        out.extend_from_slice(c);
    }
    out
}

/// Half bits of `x`, which must be zero or a normal half.
pub fn half(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    if x == 0.0 {
        return sign;
    }
    let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
    assert!(
        (1..31).contains(&e) && b & 0x1fff == 0,
        "{x} is not a normal half"
    );
    sign | ((e as u16) << 10) | ((b >> 13) & 0x3ff) as u16
}

/// Samples as little-endian bytes of type `ty`.
pub fn bytes(ty: u32, v: &[f32]) -> Vec<u8> {
    v.iter()
        .flat_map(|&x| match ty {
            HALF => half(x).to_le_bytes().to_vec(),
            FLOAT => x.to_le_bytes().to_vec(),
            _ => (x as u32).to_le_bytes().to_vec(),
        })
        .collect()
}

/// A 24 x 40 file with a lossy `Y` (DC, AC values and AC runs), an RLE `A` and an unknown `Z`,
/// two chunks for DWAA, one for DWAB: input for the corruption loops.
pub fn sample_file(compression: u8) -> Vec<u8> {
    let (w, h) = (24u32, 40u32);
    let lines = if compression == DWAA { 32 } else { 256 };
    let mut chunks = Vec::new();
    for y0 in (0..h).step_by(lines as usize) {
        let rows = (y0 + lines).min(h) - y0;
        let n = (w * rows) as usize;
        let blocks = (w as usize / 8) * (rows as usize).div_ceil(8);
        let mut ac = Vec::new();
        let mut dc = Vec::new();
        for b in 0..blocks {
            dc.push(half(1.0 + b as f32 * 0.25));
            match b % 3 {
                0 => ac.push(0xff00),
                1 => ac.extend([half(0.5), 0xff05, half(-0.25), 0xff00]),
                _ => ac.extend([0xff3e, half(0.125)]),
            }
        }
        let a: Vec<f32> = (0..n).map(|i| (i % 5) as f32 * 0.25).collect();
        let z: Vec<f32> = (0..n).map(|i| (i % 9) as f32 * 1.5).collect();
        chunks.push(chunk(&Sections {
            rules: Some([rule("Y", -1, 1, HALF), rule("A", -1, 2, HALF)].concat()),
            unknown: bytes(FLOAT, &z),
            rle: planes(HALF, &a),
            ac,
            dc,
        }));
    }
    file(
        &[("A", HALF), ("Y", HALF), ("Z", FLOAT)],
        w,
        h,
        compression,
        &chunks,
        lines,
    )
}

/// Byte planes (low byte first) of samples.
pub fn planes(ty: u32, v: &[f32]) -> Vec<u8> {
    let b = bytes(ty, v);
    let n = if ty == HALF { 2 } else { 4 };
    (0..n)
        .flat_map(|k| b.iter().skip(k).step_by(n).copied().collect::<Vec<_>>())
        .collect()
}
