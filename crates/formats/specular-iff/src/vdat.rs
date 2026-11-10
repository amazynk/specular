//! ByteRun2 (`BMHD` compression 2, written by Atari ST programs): the `BODY` holds one `VDAT`
//! chunk per plane (and one for the mask plane). Each codes its plane down the columns, one
//! 16-bit word wide: the first word is the length of the command bytes (counting itself),
//! the commands follow, then the data words they take. A command `c` (signed byte):
//!
//! - `c < 0`: copy the next `-c` words;
//! - `c == 0`: the next word is a count; copy that many words;
//! - `c == 1`: the next word is a count, the word after it is repeated that many times;
//! - `c > 1`: repeat the next word `c` times.
//!
//! Words go down a column, then to the top of the next one.

/// The planes decoded whole and laid out back to back (an ACBM body): `planes` planes of
/// `height` rows of 16-bit aligned plane rows. Planes or words missing are zero.
pub(crate) fn decode(body: &[u8], width: usize, height: usize, planes: usize) -> Vec<u8> {
    let row = width.div_ceil(16) * 2;
    let plane = row * height;
    let mut out = vec![0u8; plane * planes];
    let mut pos = 0usize;
    for p in 0..planes {
        // The next VDAT chunk.
        let Some(head) = body.get(pos..pos + 8) else {
            break;
        };
        let len = u32::from_be_bytes([head[4], head[5], head[6], head[7]]) as usize;
        let start = pos + 8;
        let chunk = &body[start.min(body.len())..start.saturating_add(len).min(body.len())];
        pos = start.saturating_add(len).saturating_add(len & 1);
        if &head[..4] != b"VDAT" {
            continue;
        }
        column_words(chunk, &mut out[p * plane..(p + 1) * plane], row, height);
    }
    out
}

/// Decodes one `VDAT` payload into `dst` (`height` rows of `row` bytes).
fn column_words(chunk: &[u8], dst: &mut [u8], row: usize, height: usize) {
    if chunk.len() < 2 || height == 0 {
        return;
    }
    let ncmd = usize::from(u16::from_be_bytes([chunk[0], chunk[1]])).saturating_sub(2);
    let cmds = &chunk[2..(2 + ncmd).min(chunk.len())];
    let mut data = (2 + ncmd).min(chunk.len());
    let word = |at: &mut usize| -> Option<[u8; 2]> {
        let w = chunk.get(*at..*at + 2)?;
        *at += 2;
        Some([w[0], w[1]])
    };
    let (mut x, mut y) = (0usize, 0usize);
    let mut put = |w: [u8; 2], x: &mut usize, y: &mut usize| -> bool {
        if *x >= row {
            return false;
        }
        let at = *y * row + *x;
        dst[at..at + 2].copy_from_slice(&w);
        *y += 1;
        if *y >= height {
            *y = 0;
            *x += 2;
        }
        true
    };
    for &c in cmds {
        let c = c as i8;
        let ok = match c {
            ..0 => {
                (0..-(c as i32)).all(|_| word(&mut data).is_some_and(|w| put(w, &mut x, &mut y)))
            }
            0 => match word(&mut data) {
                Some(n) => (0..u16::from_be_bytes(n))
                    .all(|_| word(&mut data).is_some_and(|w| put(w, &mut x, &mut y))),
                None => false,
            },
            1 => match (word(&mut data), word(&mut data)) {
                (Some(n), Some(w)) => (0..u16::from_be_bytes(n)).all(|_| put(w, &mut x, &mut y)),
                _ => false,
            },
            _ => match word(&mut data) {
                Some(w) => (0..c).all(|_| put(w, &mut x, &mut y)),
                None => false,
            },
        };
        if !ok {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_fill_columns() {
        // 32×3 plane: two word columns of 3 rows. Commands: repeat 0xAAAA twice (c = 2),
        // copy 1 word (c = -1), then count 3 of 0x0F0F (c = 1).
        let mut v = b"VDAT".to_vec();
        let payload = [
            0, 5, 2, 0xff, 1, // length 5 (2 + 3 commands), commands
            0xaa, 0xaa, 0x12, 0x34, 0, 3, 0x0f, 0x0f,
        ];
        v.extend((payload.len() as u32).to_be_bytes());
        v.extend(payload);
        let out = decode(&v, 32, 3, 1);
        assert_eq!(
            out,
            [
                0xaa, 0xaa, 0x0f, 0x0f, 0xaa, 0xaa, 0x0f, 0x0f, 0x12, 0x34, 0x0f, 0x0f
            ]
        );
    }
}
