//! Version 2 B-trees: every record of a tree, in order.

use crate::io::{Ctx, enc_size};
use crate::{Error, Result, limits};

/// Signature, version, type and checksum around every node.
const PREFIX: u64 = 10;

/// Calls `f` with every record of the B-tree whose header is at `addr`; returns the tree's
/// type and record size first through `f`'s first argument.
pub(crate) fn records<'a>(
    data: &'a [u8],
    c: &Ctx,
    addr: u64,
    f: &mut dyn FnMut(u8, &'a [u8]) -> Result<()>,
) -> Result<()> {
    let mut r = c.cur(data, addr)?;
    r.sig(b"BTHD", "B-tree v2 header")?;
    let _version = r.u8()?;
    let kind = r.u8()?;
    let node_size = u64::from(r.u32()?);
    let rec_size = u64::from(r.u16()?);
    let depth = r.u16()? as usize;
    r.skip(2)?;
    let root = r.addr(c)?;
    let root_nrec = u64::from(r.u16()?);
    let _total = r.len(c)?;
    let Some(root) = root else {
        return Ok(());
    };
    if rec_size == 0 || node_size <= PREFIX || node_size > 1 << 24 || depth > 16 {
        return Err(Error::Malformed("B-tree v2 header".into()));
    }
    let so = u64::from(c.so);
    // Per-depth node information, as the library derives it from the header.
    let leaf_max = (node_size - PREFIX) / rec_size;
    let max_nrec_size = enc_size(leaf_max) as u64;
    let mut cum_size = vec![0u64; depth + 1];
    let mut cum = vec![leaf_max; depth + 1];
    for u in 1..=depth {
        let ptr = so + max_nrec_size + if u > 1 { cum_size[u - 1] } else { 0 };
        let max = node_size.saturating_sub(PREFIX + ptr) / (rec_size + ptr);
        cum[u] = (max + 1).saturating_mul(cum[u - 1]).saturating_add(max);
        cum_size[u] = enc_size(cum[u]) as u64;
    }
    let rec_size = rec_size as usize;
    let mut stack = vec![(root, root_nrec, depth)];
    let mut nodes = 0usize;
    // A node reached twice (children shared between parents, or a cycle) is read once: shared
    // leaves would otherwise multiply the records.
    let mut seen = std::collections::HashSet::new();
    while let Some((addr, nrec, d)) = stack.pop() {
        nodes += 1;
        if nodes > limits::MAX_NODES {
            return Err(Error::TooLarge);
        }
        if !seen.insert(addr) {
            continue;
        }
        let mut r = c.cur(data, addr)?;
        let nrec = nrec as usize;
        if d == 0 {
            r.sig(b"BTLF", "B-tree v2 leaf")?;
            r.skip(2)?;
            for _ in 0..nrec {
                f(kind, r.bytes(rec_size)?)?;
            }
        } else {
            r.sig(b"BTIN", "B-tree v2 internal node")?;
            r.skip(2)?;
            let recs = r.bytes(rec_size.checked_mul(nrec).ok_or(Error::TooLarge)?)?;
            let mut children = Vec::with_capacity(nrec + 1);
            for _ in 0..=nrec {
                let child = r.addr(c)?;
                let n = r.uint(max_nrec_size as usize)?;
                if d > 1 {
                    r.skip(cum_size[d - 1] as usize)?;
                }
                if let Some(a) = child {
                    children.push((a, n));
                }
            }
            // Children and records interleave; order does not matter to the callers, but keep
            // it anyway: child 0, record 0, child 1, ... (records are emitted first here as
            // the walk is depth-first through a stack).
            for rec in recs.chunks_exact(rec_size) {
                f(kind, rec)?;
            }
            for &(a, n) in children.iter().rev() {
                stack.push((a, n, d - 1));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// One internal node whose 30 children all name the same full leaf: the leaf's records
    /// were emitted 30 times (shared leaves multiply records without bound).
    #[test]
    fn shared_leaf_read_once() {
        let c = Ctx {
            so: 8,
            sl: 8,
            base: 0,
        };
        let (root, leaf) = (64u64, 1024u64);
        let mut d = b"BTHD\0\x05".to_vec();
        d.extend_from_slice(&512u32.to_le_bytes());
        d.extend_from_slice(&8u16.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&[0, 0]);
        d.extend_from_slice(&root.to_le_bytes());
        d.extend_from_slice(&29u16.to_le_bytes());
        d.extend_from_slice(&0u64.to_le_bytes());
        d.resize(root as usize, 0);
        d.extend_from_slice(b"BTIN\0\x05");
        d.extend_from_slice(&[7; 29 * 8]);
        for _ in 0..30 {
            d.extend_from_slice(&leaf.to_le_bytes());
            d.push(62);
        }
        d.resize(leaf as usize, 0);
        d.extend_from_slice(b"BTLF\0\x05");
        d.extend_from_slice(&[1; 62 * 8]);
        d.resize(2048, 0);
        let mut n = 0;
        records(&d, &c, 0, &mut |_, _| {
            n += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(n, 29 + 62);
    }
}
