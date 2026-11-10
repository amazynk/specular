//! JPEG XL: the codestream header's own orientation field (the one decoders honour; an Exif
//! box's Orientation is ignored by them). The field is three bits inside `ImageMetadata`;
//! when the header doesn't carry it yet (all defaults, or no `extra_fields`) the bundle is
//! re-expressed with it, which grows the header by a few bits. Everything after the header,
//! frames included, is copied byte for byte: frames start on a byte boundary.

use specular_jpeg::Transform;
use specular_jxl::header_layout;

fn bit(d: &[u8], k: usize) -> bool {
    d[k / 8] >> (k % 8) & 1 == 1
}

fn push_value(bits: &mut Vec<bool>, v: u32, n: usize) {
    for i in 0..n {
        bits.push(v >> i & 1 == 1);
    }
}

/// The codestream with orientation `o` (1..=8), or `None` when its header doesn't parse.
pub fn set_codestream_orientation(cs: &[u8], o: u8) -> Option<Vec<u8>> {
    let l = header_layout(cs)?;
    let end = l.end_bit;
    if end > cs.len() * 8 {
        return None;
    }
    let mut bits: Vec<bool> = (0..end).map(|k| bit(cs, k)).collect();
    let o3 = u32::from(o - 1);
    let m = l.metadata_bit;
    if l.extra_fields {
        for i in 0..3 {
            bits[m + 2 + i] = o3 >> i & 1 == 1;
        }
    } else if l.all_default {
        // all_default = 0, extra_fields = 1, orientation, no intrinsic size, preview or
        // animation; then every field all_default implied: 8-bit integer samples, 16-bit
        // modular buffers, no extra channels, XYB, default colour encoding, default tone
        // mapping, no extensions. `default_m` and what follows stay as they were.
        let mut seq = vec![false, true];
        push_value(&mut seq, o3, 3);
        seq.extend([false, false, false]); // intrinsic size, preview, animation
        seq.push(false); // integer samples
        push_value(&mut seq, 0, 2); // bits_per_sample selector 0: 8
        seq.push(true); // modular_16bit_buffers
        push_value(&mut seq, 0, 2); // num_extra selector 0: none
        seq.push(true); // xyb_encoded
        seq.push(true); // colour_encoding.all_default
        seq.push(true); // tone_mapping.all_default
        push_value(&mut seq, 0, 2); // extensions: none
        bits.splice(m..m + 1, seq);
    } else {
        // Insert the tone-mapping bundle first (it sits later), then the extra fields.
        let tm = l.tone_mapping_bit?;
        bits.insert(tm, true);
        bits[m + 1] = true;
        let mut seq = Vec::new();
        push_value(&mut seq, o3, 3);
        seq.extend([false, false, false]);
        bits.splice(m + 2..m + 2, seq);
    }
    let mut out = vec![0u8; bits.len().div_ceil(8)];
    for (k, &b) in bits.iter().enumerate() {
        if b {
            out[k / 8] |= 1 << (k % 8);
        }
    }
    out.extend_from_slice(&cs[end.div_ceil(8)..]);
    // The result must read back as asked.
    let back = header_layout(&out)?;
    (back.orientation == u32::from(o)).then_some(out)
}

const SIG: &[u8] = b"\0\0\0\x0cJXL \r\n\x87\n";

/// The codestream's orientation, bare or in the container.
pub fn orientation(d: &[u8]) -> Option<u8> {
    let (start, _) = locate(d)?;
    header_layout(&d[start..]).map(|l| l.orientation as u8)
}

/// Where the codestream starts, and for a container the offset of the box holding it.
fn locate(d: &[u8]) -> Option<(usize, Option<usize>)> {
    if d.starts_with(&[0xFF, 0x0A]) {
        return Some((0, None));
    }
    if !d.starts_with(SIG) {
        return None;
    }
    let mut i = SIG.len();
    while i + 8 <= d.len() {
        let size = u32::from_be_bytes(d[i..i + 4].try_into().ok()?) as u64;
        let ty = &d[i + 4..i + 8];
        let (len, head) = match size {
            0 => ((d.len() - i) as u64, 8),
            1 => (
                u64::from_be_bytes(d.get(i + 8..i + 16)?.try_into().ok()?),
                16,
            ),
            n => (n, 8),
        };
        let len = usize::try_from(len).ok()?;
        if len < head || i + len > d.len() {
            return None;
        }
        match ty {
            b"jxlc" => return Some((i + head, Some(i))),
            // The first partial box carries a 4-byte index before its bytes.
            b"jxlp" => return Some((i + head + 4, Some(i))),
            _ => {}
        }
        i += len;
    }
    None
}

/// The file with orientation `o`. In a container the codestream's box is resized; the header
/// must lie wholly inside that box.
pub fn set_orientation(d: &[u8], o: u8) -> Option<Vec<u8>> {
    let (start, bx) = locate(d)?;
    let Some(at) = bx else {
        return set_codestream_orientation(d, o);
    };
    let size_field = u32::from_be_bytes(d[at..at + 4].try_into().ok()?);
    let box_end = match size_field {
        0 => d.len(),
        1 => at + usize::try_from(u64::from_be_bytes(d[at + 8..at + 16].try_into().ok()?)).ok()?,
        n => at + n as usize,
    };
    let l = header_layout(&d[start..box_end])?;
    if l.end_bit.div_ceil(8) > box_end - start {
        return None;
    }
    let new = set_codestream_orientation(&d[start..box_end], o)?;
    let mut out = d[..start].to_vec();
    out.extend_from_slice(&new);
    out.extend_from_slice(&d[box_end..]);
    let new_len = (box_end - at) + new.len() - (box_end - start);
    match size_field {
        0 => {}
        1 => out[at + 8..at + 16].copy_from_slice(&(new_len as u64).to_be_bytes()),
        _ => out[at..at + 4].copy_from_slice(&u32::try_from(new_len).ok()?.to_be_bytes()),
    }
    Some(out)
}

/// The file shown with `user` applied after its current orientation.
pub fn rotate(d: &[u8], user: Transform) -> Option<(Vec<u8>, u8)> {
    let old = orientation(d)?;
    let o = Transform::from_exif(old).then(user).to_exif();
    Some((set_orientation(d, o)?, o))
}
