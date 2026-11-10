//! Inputs found by fuzzing (`fuzz/`), each built here from a small synthetic file.

use specular_legacy2::aseprite;

/// A 2×1 RGBA sprite of one frame holding `chunks` (kind, body).
fn sprite(chunks: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (kind, c) in chunks {
        body.extend_from_slice(&((c.len() + 6) as u32).to_le_bytes());
        body.extend_from_slice(&kind.to_le_bytes());
        body.extend_from_slice(c);
    }
    let mut d = vec![0u8; 128];
    d[4..6].copy_from_slice(&0xA5E0u16.to_le_bytes());
    d[6] = 1;
    d[8] = 2;
    d[10] = 1;
    d[12] = 32;
    d.extend_from_slice(&((body.len() + 16) as u32).to_le_bytes());
    d.extend_from_slice(&0xF1FAu16.to_le_bytes());
    d.extend_from_slice(&(chunks.len() as u16).to_le_bytes());
    d.extend_from_slice(&100u16.to_le_bytes());
    d.extend_from_slice(&[0, 0]);
    d.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    d.extend(body);
    let n = d.len() as u32;
    d[..4].copy_from_slice(&n.to_le_bytes());
    d
}

/// A zlib stream of stored blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = data.chunks(65535).collect();
    for (k, b) in blocks.iter().enumerate() {
        z.push(u8::from(k + 1 == blocks.len()));
        z.extend((b.len() as u16).to_le_bytes());
        z.extend((!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    z
}

/// A 300 × 300 tilemap of 256 × 256 tiles on a 2 × 1 canvas: every tile's 65 536 pixels
/// were visited although all but one fall off the canvas (5.9 billion, a hang from a
/// 350 KB file, and from far less with compressed data). Only the canvas part is drawn now.
#[test]
fn off_canvas_tiles_are_skipped() {
    let mut layer = Vec::new();
    for v in [1u16, 2, 0, 0, 0, 0] {
        layer.extend_from_slice(&v.to_le_bytes());
    }
    layer.extend_from_slice(&[255, 0, 0, 0, 2, 0, b't', b'm']);
    layer.extend_from_slice(&0u32.to_le_bytes()); // tileset 0
    let mut ts = Vec::new();
    ts.extend_from_slice(&0u32.to_le_bytes()); // id
    ts.extend_from_slice(&2u32.to_le_bytes()); // tiles in the file
    ts.extend_from_slice(&1u32.to_le_bytes()); // count
    ts.extend_from_slice(&256u16.to_le_bytes());
    ts.extend_from_slice(&256u16.to_le_bytes());
    ts.resize(32, 0);
    ts.extend_from_slice(&0u16.to_le_bytes());
    let z = zlib_stored(&vec![255u8; 256 * 256 * 4]);
    ts.extend_from_slice(&(z.len() as u32).to_le_bytes());
    ts.extend(z);
    let mut cel = vec![0, 0, 0, 0, 0, 0, 255, 3, 0];
    cel.resize(16, 0);
    cel.extend_from_slice(&300u16.to_le_bytes());
    cel.extend_from_slice(&300u16.to_le_bytes());
    cel.extend_from_slice(&8u16.to_le_bytes()); // bits per tile
    cel.extend_from_slice(&0xFFu32.to_le_bytes()); // id mask
    cel.resize(48, 0);
    cel.extend(zlib_stored(&vec![0u8; 300 * 300]));
    let d = sprite(&[(0x2004, layer), (0x2023, ts), (0x2005, cel)]);
    let t = std::time::Instant::now();
    let img = aseprite::decode(&d, 0).unwrap();
    assert!(t.elapsed().as_secs_f64() < 2.0, "{:?}", t.elapsed());
    let specular_legacy2::Pixels::Rgba8(px) = img.pixels else {
        panic!("not RGBA")
    };
    assert_eq!(px, vec![255; 8]);
}

/// A frame claiming 2^32 − 1 chunks, followed by bytes that read as a chunk header: once
/// past the frame's end the chunk loop stayed there and spun for every claimed chunk
/// (seconds to minutes for a 200-byte file).
#[test]
fn chunk_count_stops_at_the_frame_end() {
    let mut layer = Vec::new();
    for v in [1u16, 0, 0, 0, 0, 0] {
        layer.extend_from_slice(&v.to_le_bytes());
    }
    layer.extend_from_slice(&[255, 0, 0, 0, 2, 0, b'b', b'g']);
    let mut d = sprite(&[(0x2004, layer)]);
    d[128 + 12..128 + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    d.extend_from_slice(&[0x10, 0, 0, 0, 0x04, 0x20, 0, 0]);
    let t = std::time::Instant::now();
    let s = aseprite::parse(&d).unwrap();
    assert!(t.elapsed().as_secs_f64() < 2.0, "{:?}", t.elapsed());
    assert_eq!((s.frames.len(), s.layers.len()), (1, 1));
}

/// A tileset of 65535 × 65535 tiles, 2^32 − 1 of them, embedded: its pixel size
/// (tile w × h × count × 4) overflowed `usize` (a panic in debug builds).
#[test]
fn huge_tileset_size_does_not_overflow() {
    let mut c = Vec::new();
    c.extend_from_slice(&0u32.to_le_bytes()); // id
    c.extend_from_slice(&2u32.to_le_bytes()); // tiles in the file
    c.extend_from_slice(&u32::MAX.to_le_bytes()); // count
    c.extend_from_slice(&u16::MAX.to_le_bytes()); // tile width
    c.extend_from_slice(&u16::MAX.to_le_bytes()); // tile height
    c.resize(32, 0);
    c.extend_from_slice(&0u16.to_le_bytes()); // name
    c.extend_from_slice(&0u32.to_le_bytes()); // compressed length
    let s = aseprite::parse(&sprite(&[(0x2023, c)])).unwrap();
    assert_eq!(s.frames.len(), 1);
    let _ = aseprite::decode(&sprite(&[]), 0);
}
