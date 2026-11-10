//! NumPy `.npz` archives (`np.savez`, `np.savez_compressed`): a ZIP of `.npy` members, stored
//! or DEFLATE-compressed, ZIP64 included. Stored members are read in place; compressed ones
//! are inflated when first read (the caller caches and budgets them).

use crate::npy::{self, Npy};
use crate::{Error, Result, Zip, ZipEntry};

/// One array of the archive.
#[derive(Clone, Debug)]
pub struct Member {
    /// The array's name (the member name without `.npy`).
    pub name: String,
    pub entry: ZipEntry,
    /// The header, or why the member cannot be read.
    pub npy: std::result::Result<Npy, String>,
}

impl Member {
    pub fn compressed(&self) -> bool {
        self.entry.method != 0
    }
}

#[derive(Clone, Debug)]
pub struct Npz {
    pub zip: Zip,
    pub members: Vec<Member>,
}

/// Header bytes inflated to read a compressed member's header.
const HEAD: usize = 64 << 10;

/// Whether the archive's first member is a `.npy`: a ZIP local header naming one.
pub fn sniff(head: &[u8]) -> bool {
    if !head.starts_with(b"PK\x03\x04") || head.len() < 30 {
        return false;
    }
    let n = usize::from(u16::from_le_bytes([head[26], head[27]]));
    head.get(30..30 + n)
        .is_some_and(|name| name.ends_with(b".npy"))
}

pub fn parse(data: &[u8]) -> Result<Npz> {
    let zip = Zip::parse(data)?;
    let mut members = Vec::new();
    for e in &zip.entries {
        let Some(name) = e.name.strip_suffix(".npy") else {
            continue;
        };
        let npy = zip
            .read_prefix(data, e, HEAD)
            .and_then(|h| npy::parse(&h))
            .map_err(|x| match x {
                Error::Unsupported(m) => m,
                x => x.to_string(),
            });
        members.push(Member {
            name: name.to_owned(),
            entry: e.clone(),
            npy,
        });
    }
    if members.is_empty() {
        return Err(Error::NotArray);
    }
    Ok(Npz { zip, members })
}

impl Npz {
    /// A stored member's element bytes, in place.
    pub fn stored_body<'a>(&self, data: &'a [u8], m: &Member) -> Result<Option<&'a [u8]>> {
        let npy = m.npy.as_ref().map_err(|e| Error::Unsupported(e.clone()))?;
        Ok(self
            .zip
            .stored(data, &m.entry)?
            .map(|b| b.get(npy.data_offset..).unwrap_or(&[])))
    }

    /// A compressed member inflated whole (its `.npy` bytes, header included), at most `max`.
    pub fn inflate(&self, data: &[u8], m: &Member, max: u64) -> Result<Vec<u8>> {
        self.zip.read(data, &m.entry, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Part, PlaneSpec};

    #[test]
    fn stored_and_deflated_members() {
        let body: Vec<u8> = (0..6u8).collect();
        let a = npy::tests::file(
            1,
            "{'descr': '|u1', 'fortran_order': False, 'shape': (2, 3), }",
            &body,
        );
        // A stored DEFLATE block (BTYPE 00) holding `a`.
        let mut d = vec![1u8];
        d.extend_from_slice(&(a.len() as u16).to_le_bytes());
        d.extend_from_slice(&(!(a.len() as u16)).to_le_bytes());
        d.extend_from_slice(&a);
        let o = npy::tests::file(
            1,
            "{'descr': '|O', 'fortran_order': False, 'shape': (1,), }",
            &[0],
        );
        let z = crate::zip::tests::build(
            &[
                ("x.npy", &a, None),
                ("y.npy", &[], Some((d, a.len()))),
                ("obj.npy", &o, None),
                ("notes.txt", b"hi", None),
            ],
            true,
        );
        assert!(sniff(&z));
        let n = parse(&z).unwrap();
        assert_eq!(n.members.len(), 3);
        let x = &n.members[0];
        assert_eq!(x.name, "x");
        assert_eq!(n.stored_body(&z, x).unwrap().unwrap(), &body[..]);
        let y = &n.members[1];
        assert!(y.compressed());
        let bytes = n.inflate(&z, y, 1 << 20).unwrap();
        let h = y.npy.as_ref().unwrap();
        let mut out = Vec::new();
        h.read_plane(
            &bytes,
            &PlaneSpec {
                x_dim: 1,
                y_dim: Some(0),
                index: vec![0, 0],
            },
            Part::Real,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(n.members[2].npy.as_ref().unwrap_err().contains("Python"));
    }
}
