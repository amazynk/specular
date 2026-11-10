//! `stack.xml` (OpenRaster) parsing; `maindoc.xml` (Krita) is in `kra`.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{Error, MAX_DEPTH, MAX_LAYERS};

/// The OpenRaster spelling of normal blending.
pub const SRC_OVER: &str = "svg:src-over";

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub src: String,
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Layer(Layer),
    Stack(Vec<Item>),
}

/// A child of a stack with the attributes layers and stacks share.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub node: Node,
    /// The `name` attribute (empty when absent).
    pub name: String,
    pub opacity: f32,
    pub visible: bool,
    pub composite_op: String,
    /// A stack's `isolation="isolate"`: drawn on its own even at full opacity with
    /// `svg:src-over` (`auto`, the default, isolates only when needed).
    pub isolate: bool,
    /// Offset added to everything inside (layers carry their own too).
    pub x: i64,
    pub y: i64,
}

/// `stack.xml`: the canvas size and the root stack, first child on top.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    pub width: u32,
    pub height: u32,
    pub root: Vec<Item>,
}

pub(crate) type Attrs = Vec<(Vec<u8>, String)>;

pub(crate) fn attrs(e: &BytesStart<'_>) -> Attrs {
    e.attributes()
        .with_checks(false)
        .flatten()
        .map(|a| {
            let v = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(|v| v.into_owned())
                .unwrap_or_default();
            (a.key.local_name().as_ref().to_vec(), v)
        })
        .collect()
}

pub(crate) fn get<'a>(a: &'a [(Vec<u8>, String)], key: &[u8]) -> Option<&'a str> {
    a.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim())
}

fn item(a: &[(Vec<u8>, String)], node: Node) -> Item {
    let num = |k: &[u8]| {
        get(a, k)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    Item {
        node,
        name: get(a, b"name").unwrap_or_default().to_owned(),
        opacity: num(b"opacity").map_or(1.0, |v| v.clamp(0.0, 1.0) as f32),
        visible: get(a, b"visibility") != Some("hidden"),
        composite_op: get(a, b"composite-op")
            .filter(|v| !v.is_empty())
            .unwrap_or(SRC_OVER)
            .to_owned(),
        isolate: get(a, b"isolation") == Some("isolate"),
        x: num(b"x").map_or(0, |v| v.clamp(-1e9, 1e9) as i64),
        y: num(b"y").map_or(0, |v| v.clamp(-1e9, 1e9) as i64),
    }
}

/// Parses `stack.xml`. Unknown elements are kept out of the tree but
/// refused later if visible (see [`crate::Document::composite`]).
pub fn parse(xml: &[u8]) -> Result<Stack, Error> {
    let mut r = Reader::from_reader(xml);
    let mut size = None;
    // Open stacks: their attributes and children so far.
    let mut open: Vec<(Attrs, Vec<Item>)> = Vec::new();
    let mut root: Option<Vec<Item>> = None;
    let mut layers = 0usize;
    let mut buf = Vec::new();
    loop {
        let ev = r
            .read_event_into(&mut buf)
            .map_err(|_| Error::Corrupt("stack.xml"))?;
        let (e, empty) = match &ev {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                if e.local_name().as_ref() == b"stack"
                    && let Some((a, children)) = open.pop()
                {
                    let it = item(&a, Node::Stack(children));
                    match open.last_mut() {
                        Some((_, parent)) => parent.push(it),
                        None if root.is_none() => root = Some(vec![it]),
                        None => {}
                    }
                }
                buf.clear();
                continue;
            }
            Event::Eof => break,
            _ => {
                buf.clear();
                continue;
            }
        };
        let a = attrs(e);
        match e.local_name().as_ref() {
            b"image" if size.is_none() => {
                let dim = |k: &[u8]| {
                    get(&a, k)
                        .and_then(|v| v.parse::<u32>().ok())
                        .filter(|&v| v > 0)
                };
                size = Some((
                    dim(b"w").ok_or(Error::Corrupt("stack.xml: image width"))?,
                    dim(b"h").ok_or(Error::Corrupt("stack.xml: image height"))?,
                ));
            }
            b"stack" => {
                if open.len() >= MAX_DEPTH {
                    return Err(Error::Unsupported(format!(
                        "layer groups nested deeper than {MAX_DEPTH}"
                    )));
                }
                if empty {
                    let it = item(&a, Node::Stack(Vec::new()));
                    if let Some((_, parent)) = open.last_mut() {
                        parent.push(it);
                    }
                } else {
                    open.push((a, Vec::new()));
                }
            }
            name @ (b"layer" | b"text" | b"filter") => {
                layers += 1;
                if layers > MAX_LAYERS {
                    return Err(Error::Unsupported(format!("more than {MAX_LAYERS} layers")));
                }
                let node = if name == b"layer" {
                    Node::Layer(Layer {
                        src: get(&a, b"src").unwrap_or_default().to_owned(),
                        x: 0,
                        y: 0,
                    })
                } else {
                    // Kept as a layer without a source: refused when visible.
                    Node::Layer(Layer {
                        src: String::new(),
                        x: 0,
                        y: 0,
                    })
                };
                let mut it = item(&a, node);
                // A layer's x/y place its pixels; keep them on the layer.
                if let Node::Layer(l) = &mut it.node {
                    l.x = it.x;
                    l.y = it.y;
                }
                it.x = 0;
                it.y = 0;
                if let Some((_, parent)) = open.last_mut() {
                    parent.push(it);
                }
            }
            _ => {}
        }
        buf.clear();
    }
    let (width, height) = size.ok_or(Error::Corrupt("stack.xml: no image element"))?;
    // An unclosed root still counts (lenient towards cut-off files).
    while let Some((a, children)) = open.pop() {
        let it = item(&a, Node::Stack(children));
        match open.last_mut() {
            Some((_, parent)) => parent.push(it),
            None if root.is_none() => root = Some(vec![it]),
            None => {}
        }
    }
    Ok(Stack {
        width,
        height,
        root: root.unwrap_or_default(),
    })
}

/// A layer with pixels of its own, as a page shows it ([`Stack::pixel_layers`]).
#[derive(Clone, Debug, PartialEq)]
pub struct PixelLayer {
    pub name: String,
    /// The PNG member.
    pub src: String,
    /// Canvas position of its top-left pixel (its own offset plus its stacks').
    pub x: i64,
    pub y: i64,
    /// Its own opacity, 0..=1.
    pub opacity: f32,
    /// Its own `visibility` (a hidden stack above it is not counted).
    pub visible: bool,
}

impl Stack {
    /// The layers that name a pixel source, top-most first as a layers panel lists them,
    /// depth first through the stacks (text and filter layers are left out).
    pub fn pixel_layers(&self) -> Vec<PixelLayer> {
        fn walk(items: &[Item], ox: i64, oy: i64, out: &mut Vec<PixelLayer>) {
            for it in items {
                match &it.node {
                    Node::Layer(l) if !l.src.is_empty() => out.push(PixelLayer {
                        name: it.name.clone(),
                        src: l.src.clone(),
                        x: ox.saturating_add(l.x),
                        y: oy.saturating_add(l.y),
                        opacity: it.opacity,
                        visible: it.visible,
                    }),
                    Node::Layer(_) => {}
                    Node::Stack(children) => walk(
                        children,
                        ox.saturating_add(it.x),
                        oy.saturating_add(it.y),
                        out,
                    ),
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, 0, 0, &mut out);
        out
    }
}
