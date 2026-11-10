//! The values of a USD layer, as both the text and the binary reader give them: scalars,
//! strings, tokens, asset paths and scene paths, tuples and lists, numeric arrays packed by
//! arity, dictionaries, time samples and composition arcs.

use std::collections::HashMap;

/// A composition arc to another layer's prim (reference or payload).
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Arc {
    /// Empty: the same layer stack.
    pub asset: String,
    /// Empty: the layer's default prim.
    pub prim: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) enum V {
    #[default]
    None,
    Bool(bool),
    Num(f64),
    /// A string or a token.
    Str(String),
    Asset(String),
    Path(String),
    /// An identifier the text gives bare (`def`, `uniform`, `inf`).
    Ident(String),
    /// Numbers in groups of `arity` (arrays of vectors, matrices, a single tuple).
    Nums {
        data: Vec<f64>,
        arity: usize,
    },
    Strs(Vec<String>),
    Paths(Vec<String>),
    /// A list of anything else (text: mixed lists).
    List(Vec<V>),
    Dict(Vec<(String, V)>),
    /// (time, value), in file order.
    Samples(Vec<(f64, V)>),
    Arcs(Vec<Arc>),
}

impl V {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            V::Str(s) | V::Ident(s) | V::Asset(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            V::Num(v) => Some(*v),
            V::Bool(b) => Some(f64::from(u8::from(*b))),
            V::Nums { data, .. } if data.len() == 1 => Some(data[0]),
            V::Ident(s) => match s.as_str() {
                "inf" => Some(f64::INFINITY),
                "-inf" => Some(f64::NEG_INFINITY),
                "nan" => Some(f64::NAN),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            V::Bool(b) => Some(*b),
            V::Num(v) => Some(*v != 0.0),
            V::Ident(s) => match s.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    /// Numbers flattened (a scalar is one; a list of tuples is all their numbers).
    pub fn nums(&self) -> Option<std::borrow::Cow<'_, [f64]>> {
        match self {
            V::Nums { data, .. } => Some(std::borrow::Cow::Borrowed(data)),
            V::Num(v) => Some(std::borrow::Cow::Owned(vec![*v])),
            V::List(l) => {
                let mut out = Vec::new();
                for v in l {
                    out.extend_from_slice(&v.nums()?);
                }
                Some(std::borrow::Cow::Owned(out))
            }
            _ => None,
        }
    }

    pub fn strs(&self) -> Option<Vec<String>> {
        match self {
            V::Strs(s) | V::Paths(s) => Some(s.clone()),
            V::Str(s) | V::Ident(s) | V::Path(s) | V::Asset(s) => Some(vec![s.clone()]),
            V::List(l) => l.iter().map(|v| v.as_str().map(str::to_owned)).collect(),
            _ => None,
        }
    }

    pub fn paths(&self) -> Vec<String> {
        match self {
            V::Paths(p) => p.clone(),
            V::Path(p) => vec![p.clone()],
            V::List(l) => l
                .iter()
                .filter_map(|v| match v {
                    V::Path(p) => Some(p.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn dict_get(&self, key: &str) -> Option<&V> {
        match self {
            V::Dict(d) => d.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The value at the earliest time sample, or itself.
    pub fn first_sample(&self) -> &V {
        match self {
            V::Samples(s) => s
                .iter()
                .filter(|(_, v)| *v != V::None)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map_or(&V::None, |(_, v)| v),
            v => v,
        }
    }
}

/// What a spec is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpecKind {
    PseudoRoot,
    Prim,
    Attribute,
    Relationship,
    Variant,
    VariantSet,
    Other,
}

#[derive(Clone, Debug)]
pub(crate) struct Spec {
    pub kind: SpecKind,
    pub fields: Vec<(String, V)>,
}

impl Spec {
    pub fn new(kind: SpecKind) -> Spec {
        Spec {
            kind,
            fields: Vec::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&V> {
        self.fields.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    pub fn set(&mut self, name: &str, v: V) {
        match self.fields.iter_mut().find(|(k, _)| k == name) {
            Some((_, old)) => *old = v,
            None => self.fields.push((name.to_owned(), v)),
        }
    }
}

/// One layer: its specs by path ("/", "/World", "/World/Mesh.points",
/// "/Asset{shape=Cube}Body").
#[derive(Clone, Debug, Default)]
pub(crate) struct Layer {
    pub specs: HashMap<String, Spec>,
}

impl Layer {
    pub fn root(&self) -> Option<&Spec> {
        self.specs.get("/")
    }

    pub fn meta(&self, name: &str) -> Option<&V> {
        self.root().and_then(|r| r.get(name))
    }

    pub fn spec_mut(&mut self, path: &str, kind: SpecKind) -> &mut Spec {
        self.specs
            .entry(path.to_owned())
            .or_insert_with(|| Spec::new(kind))
    }
}

/// A child path: `/A` + `B` → `/A/B`; under a variant `/A{v=x}` + `B` → `/A{v=x}B`; under the
/// root `/` + `B` → `/B`.
pub(crate) fn child_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else if parent.ends_with('}') {
        format!("{parent}{name}")
    } else {
        format!("{parent}/{name}")
    }
}

/// The parent of a stage path (`/A/B` → `/A`, `/A` → `/`).
pub(crate) fn parent_path(p: &str) -> &str {
    match p.rfind('/') {
        Some(0) | None => "/",
        Some(i) => &p[..i],
    }
}
