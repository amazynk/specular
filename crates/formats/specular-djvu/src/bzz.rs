//! BZZ (DjVu v3 specification, appendix 4): a Burrows-Wheeler transform whose move-to-front
//! ranks are coded with the ZP-coder. Used for DIRM, NAVM, ANTz, TXTz and the FGbz colour
//! indices.

use crate::Error;
use crate::zp::Zp;

/// The largest block the format allows (4 MiB).
const MAX_BLOCK: usize = 4 << 20;

/// Decodes a whole BZZ stream, refusing output larger than `limit` bytes.
pub(crate) fn decode(data: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let mut zp = Zp::new(data);
    let mut ctx = [0u8; 300];
    let mut out = Vec::new();
    loop {
        let size = raw(&mut zp, 24) as usize;
        if size == 0 {
            return Ok(out);
        }
        if size > MAX_BLOCK || out.len() + size > limit.saturating_add(1) {
            return Err(Error::Malformed("BZZ block too large"));
        }
        if zp.overrun() > 64 {
            return Err(Error::Malformed("truncated BZZ data"));
        }
        block(&mut zp, &mut ctx, size, &mut out)?;
    }
}

fn raw(zp: &mut Zp<'_>, bits: u32) -> u32 {
    let mut n = 1u32;
    let top = 1u32 << bits;
    while n < top {
        n = (n << 1) | u32::from(zp.decode_raw());
    }
    n - top
}

fn bin(zp: &mut Zp<'_>, ctx: &mut [u8], base: usize, bits: u32) -> usize {
    let mut n = 1usize;
    let top = 1usize << bits;
    while n < top {
        n = (n << 1) | usize::from(zp.decode(&mut ctx[base + n - 1]));
    }
    n - top
}

fn block(
    zp: &mut Zp<'_>,
    ctx: &mut [u8; 300],
    size: usize,
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    // Estimation speed.
    let mut fshift = 0;
    if zp.decode_raw() {
        fshift = if zp.decode_raw() { 2 } else { 1 };
    }
    let mut mtf: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut freq = [0u32; 4];
    let mut fadd: u32 = 4;
    let mut mtfno: usize = 3;
    let mut marker: Option<usize> = None;
    let mut data = vec![0u8; size];
    for (i, slot) in data.iter_mut().enumerate() {
        let c = mtfno.min(2);
        mtfno = if zp.decode(&mut ctx[c]) {
            0
        } else if zp.decode(&mut ctx[c + 3]) {
            1
        } else if zp.decode(&mut ctx[6]) {
            2 + bin(zp, ctx, 7, 1)
        } else if zp.decode(&mut ctx[8]) {
            4 + bin(zp, ctx, 9, 2)
        } else if zp.decode(&mut ctx[12]) {
            8 + bin(zp, ctx, 13, 3)
        } else if zp.decode(&mut ctx[20]) {
            16 + bin(zp, ctx, 21, 4)
        } else if zp.decode(&mut ctx[36]) {
            32 + bin(zp, ctx, 37, 5)
        } else if zp.decode(&mut ctx[68]) {
            64 + bin(zp, ctx, 69, 6)
        } else if zp.decode(&mut ctx[132]) {
            128 + bin(zp, ctx, 133, 7)
        } else {
            256
        };
        if mtfno == 256 {
            if marker.is_some() {
                return Err(Error::Malformed("BZZ block with two end markers"));
            }
            marker = Some(i);
            *slot = 0;
            continue;
        }
        let symbol = mtf[mtfno];
        *slot = symbol;
        // Frequencies.
        fadd = fadd.wrapping_add(fadd >> fshift);
        if fadd > 0x1000_0000 {
            fadd >>= 24;
            for f in &mut freq {
                *f >>= 24;
            }
        }
        let fc = fadd.wrapping_add(if mtfno < 4 { freq[mtfno] } else { 0 });
        // Rotate the move-to-front table.
        let mut k = mtfno;
        while k > 3 {
            mtf[k] = mtf[k - 1];
            k -= 1;
        }
        while k > 0 && fc >= freq[k - 1] {
            mtf[k] = mtf[k - 1];
            freq[k] = freq[k - 1];
            k -= 1;
        }
        mtf[k] = symbol;
        freq[k] = fc;
    }
    let Some(marker) = marker.filter(|&m| m > 0 && m < size) else {
        return Err(Error::Malformed("BZZ block without an end marker"));
    };
    // Inverse Burrows-Wheeler transform.
    let mut count = [0u32; 256];
    let mut posn = vec![0u32; size];
    for (i, &k) in data.iter().enumerate() {
        if i != marker {
            posn[i] = count[usize::from(k)];
            count[usize::from(k)] += 1;
        }
    }
    let mut last = 1u32;
    for c in &mut count {
        let k = *c;
        *c = last;
        last += k;
    }
    if last as usize != size {
        return Err(Error::Malformed("BZZ block counts"));
    }
    let start = out.len();
    out.resize(start + size - 1, 0);
    let dst = &mut out[start..];
    let mut k = 0usize;
    for d in dst.iter_mut().rev() {
        let c = data[k];
        *d = c;
        k = (count[usize::from(c)] + posn[k]) as usize;
        if k >= size {
            return Err(Error::Malformed("BZZ block permutation"));
        }
    }
    if k != marker {
        return Err(Error::Malformed("BZZ block permutation"));
    }
    Ok(())
}
