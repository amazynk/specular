//! A minimal HDF5 writer for building test fixtures (behind the `writer` feature, never
//! compiled into the viewer): superblock v2 with 8-byte offsets and lengths, version 2
//! object headers holding link and attribute messages, contiguous datasets, and chunked
//! datasets with a single (optionally filtered) chunk or several chunks behind a fixed array
//! index. Checksums are left zero: the reader does not verify them.

/// A stored element type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    I64,
    F32,
    F64,
    /// Big-endian variants of the above (fixed-point or float).
    BeI16,
    BeF32,
}

impl Ty {
    pub fn size(self) -> usize {
        match self {
            Ty::U8 | Ty::I8 => 1,
            Ty::U16 | Ty::I16 | Ty::BeI16 => 2,
            Ty::U32 | Ty::I32 | Ty::F32 | Ty::BeF32 => 4,
            Ty::I64 | Ty::F64 => 8,
        }
    }

    fn encode(self) -> Vec<u8> {
        let size = self.size() as u32;
        let be = matches!(self, Ty::BeI16 | Ty::BeF32);
        let mut b = Vec::new();
        match self {
            Ty::F32 | Ty::F64 | Ty::BeF32 => {
                b.push(0x11);
                b.push(0x20 | u8::from(be));
                b.push(if size == 4 { 31 } else { 63 });
                b.push(0);
                b.extend_from_slice(&size.to_le_bytes());
                b.extend_from_slice(&0u16.to_le_bytes());
                b.extend_from_slice(&((size * 8) as u16).to_le_bytes());
                if size == 4 {
                    b.extend_from_slice(&[23, 8, 0, 23]);
                    b.extend_from_slice(&127u32.to_le_bytes());
                } else {
                    b.extend_from_slice(&[52, 11, 0, 52]);
                    b.extend_from_slice(&1023u32.to_le_bytes());
                }
            }
            _ => {
                let signed = matches!(self, Ty::I8 | Ty::I16 | Ty::I32 | Ty::I64 | Ty::BeI16);
                b.push(0x10);
                b.push(u8::from(be) | if signed { 0x08 } else { 0 });
                b.extend_from_slice(&[0, 0]);
                b.extend_from_slice(&size.to_le_bytes());
                b.extend_from_slice(&0u16.to_le_bytes());
                b.extend_from_slice(&((size * 8) as u16).to_le_bytes());
            }
        }
        b
    }
}

/// An attribute value.
#[derive(Clone, Debug)]
pub enum AttrValue {
    Str(String),
    F64(Vec<f64>),
    I32(Vec<i32>),
    I64(Vec<i64>),
}

/// How a dataset's bytes are stored.
#[derive(Clone, Debug)]
pub enum Storage {
    Contiguous,
    /// One chunk covering the whole dataset; `stored` is the filtered chunk (or the raw
    /// bytes again when the pipeline is empty).
    Single {
        filters: Vec<(u16, Vec<u32>)>,
        stored: Vec<u8>,
    },
    /// Chunks of `chunk` elements per dimension, each stored as given (already filtered),
    /// in row-major chunk order, behind a fixed array index.
    Chunks {
        chunk: Vec<u64>,
        filters: Vec<(u16, Vec<u32>)>,
        stored: Vec<Vec<u8>>,
    },
}

enum Node {
    Group {
        name: String,
        attrs: Vec<(String, AttrValue)>,
        children: Vec<usize>,
    },
    Dataset {
        name: String,
        attrs: Vec<(String, AttrValue)>,
        ty: Ty,
        shape: Vec<u64>,
        raw: Vec<u8>,
        storage: Storage,
    },
    External {
        name: String,
        file: String,
        path: String,
    },
    Soft {
        name: String,
        target: String,
    },
}

/// Builds an HDF5 file in memory.
pub struct Writer {
    nodes: Vec<Node>,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

/// The root group's handle.
pub const ROOT: usize = 0;

impl Writer {
    pub fn new() -> Writer {
        Writer {
            nodes: vec![Node::Group {
                name: String::new(),
                attrs: Vec::new(),
                children: Vec::new(),
            }],
        }
    }

    fn add(&mut self, parent: usize, node: Node) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);
        match &mut self.nodes[parent] {
            Node::Group { children, .. } => children.push(id),
            _ => panic!("parent is not a group"),
        }
        id
    }

    /// A child group.
    pub fn group(&mut self, parent: usize, name: &str) -> usize {
        self.add(
            parent,
            Node::Group {
                name: name.into(),
                attrs: Vec::new(),
                children: Vec::new(),
            },
        )
    }

    /// The group at a slash-separated path under the root, created as needed.
    pub fn groups(&mut self, path: &str) -> usize {
        let mut at = ROOT;
        for part in path.split('/').filter(|p| !p.is_empty()) {
            let found = match &self.nodes[at] {
                Node::Group { children, .. } => children
                    .iter()
                    .copied()
                    .find(|&c| matches!(&self.nodes[c], Node::Group { name, .. } if name == part)),
                _ => None,
            };
            at = match found {
                Some(c) => c,
                None => self.group(at, part),
            };
        }
        at
    }

    /// A dataset of raw little-endian (or, for the `Be*` types, big-endian) elements.
    pub fn dataset(
        &mut self,
        parent: usize,
        name: &str,
        ty: Ty,
        shape: &[u64],
        raw: Vec<u8>,
        storage: Storage,
    ) -> usize {
        let n: u64 = shape.iter().product();
        assert_eq!(n as usize * ty.size(), raw.len(), "dataset {name} size");
        self.add(
            parent,
            Node::Dataset {
                name: name.into(),
                attrs: Vec::new(),
                ty,
                shape: shape.to_vec(),
                raw,
                storage,
            },
        )
    }

    /// A contiguous `f32` dataset.
    pub fn f32s(&mut self, parent: usize, name: &str, shape: &[u64], v: &[f32]) -> usize {
        let raw = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.dataset(parent, name, Ty::F32, shape, raw, Storage::Contiguous)
    }

    /// A contiguous `f64` dataset.
    pub fn f64s(&mut self, parent: usize, name: &str, shape: &[u64], v: &[f64]) -> usize {
        let raw = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.dataset(parent, name, Ty::F64, shape, raw, Storage::Contiguous)
    }

    /// An external link to `path` in `file`.
    pub fn external(&mut self, parent: usize, name: &str, file: &str, path: &str) -> usize {
        self.add(
            parent,
            Node::External {
                name: name.into(),
                file: file.into(),
                path: path.into(),
            },
        )
    }

    /// A soft link to an absolute path.
    pub fn soft(&mut self, parent: usize, name: &str, target: &str) -> usize {
        self.add(
            parent,
            Node::Soft {
                name: name.into(),
                target: target.into(),
            },
        )
    }

    /// Adds an attribute to a group or dataset.
    pub fn attr(&mut self, node: usize, name: &str, value: AttrValue) {
        match &mut self.nodes[node] {
            Node::Group { attrs, .. } | Node::Dataset { attrs, .. } => {
                attrs.push((name.into(), value));
            }
            _ => panic!("links have no attributes"),
        }
    }

    pub fn str_attr(&mut self, node: usize, name: &str, value: &str) {
        self.attr(node, name, AttrValue::Str(value.into()));
    }

    /// The file's bytes.
    pub fn finish(self) -> Vec<u8> {
        let mut out = vec![0u8; 48];
        out[..8].copy_from_slice(b"\x89HDF\r\n\x1a\n");
        out[8] = 2;
        out[9] = 8;
        out[10] = 8;
        out[11] = 0;
        out[12..20].copy_from_slice(&0u64.to_le_bytes());
        out[20..28].copy_from_slice(&u64::MAX.to_le_bytes());
        let root = self.write(ROOT, &mut out);
        let eof = out.len() as u64;
        out[28..36].copy_from_slice(&eof.to_le_bytes());
        out[36..44].copy_from_slice(&root.to_le_bytes());
        out
    }

    /// Writes a node's data and children, then its header; returns the header address.
    fn write(&self, id: usize, out: &mut Vec<u8>) -> u64 {
        let mut msgs: Vec<(u8, Vec<u8>)> = Vec::new();
        match &self.nodes[id] {
            Node::Group {
                attrs, children, ..
            } => {
                for &c in children {
                    let (name, kind, body) = match &self.nodes[c] {
                        Node::Group { name, .. } | Node::Dataset { name, .. } => {
                            (name, 0u8, self.write(c, out).to_le_bytes().to_vec())
                        }
                        Node::External { name, file, path } => {
                            let mut v = vec![0u8];
                            v.extend_from_slice(file.as_bytes());
                            v.push(0);
                            v.extend_from_slice(path.as_bytes());
                            v.push(0);
                            let mut b = (v.len() as u16).to_le_bytes().to_vec();
                            b.extend(v);
                            (name, 64u8, b)
                        }
                        Node::Soft { name, target } => {
                            let mut b = (target.len() as u16).to_le_bytes().to_vec();
                            b.extend_from_slice(target.as_bytes());
                            (name, 1u8, b)
                        }
                    };
                    let mut m = vec![1u8, 0x08, kind, name.len() as u8];
                    m.extend_from_slice(name.as_bytes());
                    m.extend(body);
                    msgs.push((6, m));
                }
                for (n, v) in attrs {
                    msgs.push((0x0C, attribute(n, v)));
                }
            }
            Node::Dataset {
                attrs,
                ty,
                shape,
                raw,
                storage,
                ..
            } => {
                msgs.push((1, dataspace(shape)));
                msgs.push((3, ty.encode()));
                let elem = ty.size() as u64;
                let mut pipeline = None;
                let layout = match storage {
                    Storage::Contiguous => {
                        let addr = out.len() as u64;
                        out.extend_from_slice(raw);
                        let mut l = vec![4u8, 1];
                        l.extend_from_slice(&addr.to_le_bytes());
                        l.extend_from_slice(&(raw.len() as u64).to_le_bytes());
                        l
                    }
                    Storage::Single { filters, stored } => {
                        let addr = out.len() as u64;
                        out.extend_from_slice(stored);
                        let filtered = !filters.is_empty();
                        if filtered {
                            pipeline = Some(filters.clone());
                        }
                        let mut l = vec![4u8, 2, if filtered { 2 } else { 0 }];
                        chunk_dims(&mut l, shape, elem);
                        l.push(1);
                        if filtered {
                            l.extend_from_slice(&(stored.len() as u64).to_le_bytes());
                            l.extend_from_slice(&0u32.to_le_bytes());
                        }
                        l.extend_from_slice(&addr.to_le_bytes());
                        l
                    }
                    Storage::Chunks {
                        chunk,
                        filters,
                        stored,
                    } => {
                        let filtered = !filters.is_empty();
                        if filtered {
                            pipeline = Some(filters.clone());
                        }
                        let addrs: Vec<(u64, u64)> = stored
                            .iter()
                            .map(|s| {
                                let a = out.len() as u64;
                                out.extend_from_slice(s);
                                (a, s.len() as u64)
                            })
                            .collect();
                        let n = addrs.len() as u64;
                        let page_bits = 64 - n.max(2).leading_zeros() as u8;
                        let size_bytes = 8u8;
                        let esize = if filtered { 8 + size_bytes + 4 } else { 8 };
                        let hdr = out.len() as u64;
                        let dblk = hdr + 4 + 4 + 8 + 8 + 4;
                        out.extend_from_slice(b"FAHD");
                        out.extend_from_slice(&[0, u8::from(filtered), esize, page_bits]);
                        out.extend_from_slice(&n.to_le_bytes());
                        out.extend_from_slice(&dblk.to_le_bytes());
                        out.extend_from_slice(&[0; 4]);
                        out.extend_from_slice(b"FADB");
                        out.extend_from_slice(&[0, u8::from(filtered)]);
                        out.extend_from_slice(&hdr.to_le_bytes());
                        for (a, s) in addrs {
                            out.extend_from_slice(&a.to_le_bytes());
                            if filtered {
                                out.extend_from_slice(&s.to_le_bytes());
                                out.extend_from_slice(&0u32.to_le_bytes());
                            }
                        }
                        out.extend_from_slice(&[0; 4]);
                        let mut l = vec![4u8, 2, 0];
                        chunk_dims(&mut l, chunk, elem);
                        l.push(3);
                        l.push(page_bits);
                        l.extend_from_slice(&hdr.to_le_bytes());
                        l
                    }
                };
                msgs.push((8, layout));
                if let Some(filters) = pipeline {
                    let mut p = vec![2u8, filters.len() as u8];
                    for (fid, params) in filters {
                        p.extend_from_slice(&fid.to_le_bytes());
                        if fid >= 256 {
                            p.extend_from_slice(&0u16.to_le_bytes());
                        }
                        p.extend_from_slice(&0u16.to_le_bytes());
                        p.extend_from_slice(&(params.len() as u16).to_le_bytes());
                        for x in params {
                            p.extend_from_slice(&x.to_le_bytes());
                        }
                    }
                    msgs.push((0x0B, p));
                }
                for (n, v) in attrs {
                    msgs.push((0x0C, attribute(n, v)));
                }
            }
            _ => unreachable!("links are written by their group"),
        }
        let addr = out.len() as u64;
        let size: usize = msgs.iter().map(|(_, b)| 4 + b.len()).sum();
        out.extend_from_slice(b"OHDR");
        out.push(2);
        out.push(0x02);
        out.extend_from_slice(&(size as u32).to_le_bytes());
        for (kind, body) in msgs {
            out.push(kind);
            out.extend_from_slice(&(body.len() as u16).to_le_bytes());
            out.push(0);
            out.extend(body);
        }
        out.extend_from_slice(&[0; 4]);
        addr
    }
}

fn chunk_dims(l: &mut Vec<u8>, dims: &[u64], elem: u64) {
    l.push(dims.len() as u8 + 1);
    l.push(8);
    for d in dims.iter().copied().chain([elem]) {
        l.extend_from_slice(&d.to_le_bytes());
    }
}

fn dataspace(shape: &[u64]) -> Vec<u8> {
    let mut b = vec![2u8, shape.len() as u8, 0, u8::from(!shape.is_empty())];
    for d in shape {
        b.extend_from_slice(&d.to_le_bytes());
    }
    b
}

fn attribute(name: &str, v: &AttrValue) -> Vec<u8> {
    let (dt, shape, data): (Vec<u8>, Vec<u64>, Vec<u8>) = match v {
        AttrValue::Str(s) => {
            let mut dt = vec![0x13, 0, 0, 0];
            dt.extend_from_slice(&(s.len().max(1) as u32).to_le_bytes());
            let mut d = s.as_bytes().to_vec();
            if d.is_empty() {
                d.push(0);
            }
            (dt, Vec::new(), d)
        }
        AttrValue::F64(x) => (
            Ty::F64.encode(),
            vec![x.len() as u64],
            x.iter().flat_map(|v| v.to_le_bytes()).collect(),
        ),
        AttrValue::I32(x) => (
            Ty::I32.encode(),
            vec![x.len() as u64],
            x.iter().flat_map(|v| v.to_le_bytes()).collect(),
        ),
        AttrValue::I64(x) => (
            Ty::I64.encode(),
            vec![x.len() as u64],
            x.iter().flat_map(|v| v.to_le_bytes()).collect(),
        ),
    };
    let ds = dataspace(&shape);
    let mut b = vec![3u8, 0];
    b.extend_from_slice(&(name.len() as u16 + 1).to_le_bytes());
    b.extend_from_slice(&(dt.len() as u16).to_le_bytes());
    b.extend_from_slice(&(ds.len() as u16).to_le_bytes());
    b.push(0);
    b.extend_from_slice(name.as_bytes());
    b.push(0);
    b.extend(dt);
    b.extend(ds);
    b.extend(data);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{File, Kind, Value};

    #[test]
    fn round_trip() {
        let mut w = Writer::new();
        let g = w.groups("/entry/data");
        w.str_attr(g, "NX_class", "NXdata");
        let v: Vec<f32> = (0..24).map(|i| i as f32).collect();
        let d = w.f32s(g, "data", &[2, 3, 4], &v);
        w.attr(d, "scale", AttrValue::F64(vec![0.5, 2.0]));
        w.external(ROOT, "ext", "other.h5", "/x/y");
        w.soft(ROOT, "alias", "/entry/data/data");
        let raw: Vec<u8> = (0..24u16).flat_map(|x| x.to_le_bytes()).collect();
        let chunks = vec![raw[..24].to_vec(), raw[24..].to_vec()];
        w.dataset(
            ROOT,
            "chunked",
            Ty::U16,
            &[4, 6],
            raw.clone(),
            Storage::Chunks {
                chunk: vec![2, 6],
                filters: Vec::new(),
                stored: chunks,
            },
        );
        let bytes = w.finish();
        let f = File::open(&bytes).unwrap();
        let o = f.get("/entry/data").unwrap();
        assert_eq!(o.attr("NX_class").and_then(Value::as_str), Some("NXdata"));
        let i = f.index_of("/entry/data/data").unwrap();
        assert_eq!(f.read_all::<f32>(&bytes, i, 100).unwrap(), v);
        assert_eq!(
            f.objects[i].attr("scale"),
            Some(&Value::Float(vec![0.5, 2.0]))
        );
        assert!(matches!(&f.get("/ext").unwrap().kind,
            Kind::External { file, path } if file == "other.h5" && path == "/x/y"));
        assert_eq!(f.get("/alias").unwrap().path, "/entry/data/data");
        let i = f.index_of("/chunked").unwrap();
        let back: Vec<f64> = f.read_all(&bytes, i, 100).unwrap();
        assert_eq!(back, (0..24).map(f64::from).collect::<Vec<_>>());
    }
}
