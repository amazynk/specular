//! X3D (XML encoding, `.x3d`, `.x3dz`) and VRML 97 / X3D classic encoding (`.wrl`, `.wrz`,
//! `.x3dv`, `.x3dvz`), gzipped or not. Both parse into one scene graph ([`scene`]) that one
//! walker turns into the mesh: grouping nodes (Transform with centre and scale orientation,
//! Group, Collision, Switch by its choice, the first LOD level, Anchor, Billboard,
//! StaticGroup, the CAD nodes), Shapes with their Appearance (Material, PhysicalMaterial,
//! UnlitMaterial, TwoSidedMaterial; ImageTexture and PixelTexture with TextureTransform), DEF/USE, and
//! Inline files from the model's own folder. Geometry is in [`geom`]. Lines, points, text,
//! NURBS and PROTO instances are not drawn (noted). Y is up, units are metres.

mod classic;
mod geom;
mod scene;
mod xmlenc;

use std::collections::HashMap;

use crate::builder::{Affine, Builder};
use crate::{Error, ImageData, Limits, Material, Mesh, Resolver, Result, Unit, Up};
use geom::{Look, TexXf, rotate};
use scene::Scene;

pub(crate) fn read_xml(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    read(bytes, resolver, limits)
}

pub(crate) fn read_classic(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    read(bytes, resolver, limits)
}

/// The text of a file: gunzipped when it starts with gzip's magic.
fn unpack<'a>(bytes: &'a [u8], limits: &Limits) -> Result<std::borrow::Cow<'a, [u8]>> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let out = specular_png::inflate_gzip(bytes, limits.max_unpacked).map_err(|e| match e {
            specular_png::InflateError::NeedsMemory(_) => {
                Error::TooLarge("the gzipped scene unpacks too large".into())
            }
            e => Error::Malformed(format!("gzip: {e}")),
        })?;
        Ok(std::borrow::Cow::Owned(out))
    } else {
        Ok(std::borrow::Cow::Borrowed(bytes))
    }
}

/// Parses one file (either encoding, by its content) into `scene`; its root nodes.
fn parse_into(bytes: &[u8], scene: &mut Scene, limits: &Limits) -> Result<Vec<u32>> {
    let text = unpack(bytes, limits)?;
    let t = text.strip_prefix(&[0xef, 0xbb, 0xbf][..]).unwrap_or(&text);
    let t = t.trim_ascii_start();
    if t.starts_with(b"#VRML V1.") {
        return Err(Error::Unsupported(
            "VRML 1.0 files are not supported; VRML 97 (2.0) and X3D are".into(),
        ));
    }
    if t.starts_with(b"<") || t.starts_with(&[0xff, 0xfe]) || t.starts_with(&[0xfe, 0xff]) {
        let max_nodes = scene::MAX_NODES.saturating_sub(scene.nodes.len());
        let root = crate::xml::parse(&text, max_nodes)?;
        if root.name != "X3D" && root.name != "Scene" {
            return Err(Error::Malformed(format!(
                "not an X3D document (root element <{}>)",
                root.name
            )));
        }
        xmlenc::convert(&root, scene)
    } else {
        classic::parse(t, scene)
    }
}

fn read(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    let mut scene = Scene::default();
    let roots = parse_into(bytes, &mut scene, limits)?;
    let mut b = Builder::new(limits);
    b.up = Up::Y;
    b.unit = Unit::Metre;
    let mut w = Walk {
        scene,
        b,
        resolver,
        limits: *limits,
        looks: HashMap::new(),
        images: HashMap::new(),
        inline_stack: Vec::new(),
        inlines: 0,
        inlined: HashMap::new(),
        skipped: Vec::new(),
        visits: 0,
    };
    for r in roots {
        w.walk(r, &Affine::IDENTITY, 0)?;
    }
    let Walk {
        scene,
        mut b,
        skipped,
        ..
    } = w;
    for s in &skipped {
        b.note(s.clone());
    }
    if b.triangle_count() == 0 {
        let what = if skipped.is_empty() {
            "no shapes".to_owned()
        } else {
            format!("no shapes to show ({})", skipped.join("; "))
        };
        return Err(Error::Unsupported(format!("this scene has {what}")));
    }
    let world = |n: &scene::SNode| n.kind == "WorldInfo";
    b.title = scene.title.clone().or_else(|| {
        scene
            .nodes
            .iter()
            .position(world)
            .map(|i| scene.strings(i as u32, "title"))
            .and_then(|t| t.into_iter().next())
            .filter(|t| !t.trim().is_empty())
    });
    b.generator = scene.generator.clone();
    b.finish()
}

/// Deepest nesting of grouping nodes walked (and of Inline files).
const MAX_DEPTH: usize = 256;
const MAX_INLINE_DEPTH: usize = 8;
const MAX_INLINES: usize = 256;

struct Walk<'a> {
    scene: Scene,
    b: Builder,
    resolver: &'a dyn Resolver,
    limits: Limits,
    /// Appearance node → its look.
    looks: HashMap<u32, Look>,
    /// Texture name → image index.
    images: HashMap<String, Option<u32>>,
    inline_stack: Vec<String>,
    inlines: usize,
    /// Inline file name → its root nodes, parsed once.
    inlined: HashMap<String, Vec<u32>>,
    /// What was not drawn, once each.
    skipped: Vec<String>,
    /// Nodes walked so far ([`MAX_VISITS`]).
    visits: usize,
}

/// Nodes walked per file, at most: `USE` may share a group that uses the next one twice,
/// which would otherwise walk exponentially many nodes.
const MAX_VISITS: usize = 1 << 21;

fn translate(t: [f64; 3]) -> Affine {
    Affine([
        [1.0, 0.0, 0.0, t[0]],
        [0.0, 1.0, 0.0, t[1]],
        [0.0, 0.0, 1.0, t[2]],
    ])
}

fn rotation(r: [f64; 4]) -> Affine {
    let col = |e: [f64; 3]| rotate([r[0], r[1], r[2]], r[3], e);
    let (x, y, z) = (
        col([1.0, 0.0, 0.0]),
        col([0.0, 1.0, 0.0]),
        col([0.0, 0.0, 1.0]),
    );
    Affine([
        [x[0], y[0], z[0], 0.0],
        [x[1], y[1], z[1], 0.0],
        [x[2], y[2], z[2], 0.0],
    ])
}

fn scaling(s: [f64; 3]) -> Affine {
    Affine([
        [s[0], 0.0, 0.0, 0.0],
        [0.0, s[1], 0.0, 0.0],
        [0.0, 0.0, s[2], 0.0],
    ])
}

/// A Transform's matrix: `T × C × R × SR × S × −SR × −C`.
fn transform_of(sc: &Scene, n: u32) -> Affine {
    let t = sc.vec(n, "translation", [0.0; 3]);
    let c = sc.vec(n, "center", [0.0; 3]);
    let r = sc.vec(n, "rotation", [0.0, 0.0, 1.0, 0.0]);
    let s = sc.vec(n, "scale", [1.0; 3]);
    let so = sc.vec(n, "scaleOrientation", [0.0, 0.0, 1.0, 0.0]);
    let neg = |v: [f64; 3]| v.map(|x| -x);
    translate(t)
        .then_after(&translate(c))
        .then_after(&rotation(r))
        .then_after(&rotation(so))
        .then_after(&scaling(s))
        .then_after(&rotation([so[0], so[1], so[2], -so[3]]))
        .then_after(&translate(neg(c)))
}

impl Walk<'_> {
    fn skip(&mut self, what: &str) {
        if !self.skipped.iter().any(|s| s == what) {
            self.skipped.push(what.to_owned());
        }
    }

    fn walk_all(&mut self, kids: Vec<u32>, xf: &Affine, depth: usize) -> Result<()> {
        for k in kids {
            self.walk(k, xf, depth + 1)?;
        }
        Ok(())
    }

    fn walk(&mut self, n: u32, xf: &Affine, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            self.skip("groups nested too deeply");
            return Ok(());
        }
        self.visits += 1;
        if self.visits > MAX_VISITS {
            self.skip("groups used too many times");
            return Ok(());
        }
        let sc = &self.scene;
        let kind = sc.kind(n).to_owned();
        match kind.as_str() {
            "Transform" | "CADPart" => {
                let x = xf.then_after(&transform_of(sc, n));
                let kids = sc.nodes(n, "children");
                self.walk_all(kids, &x, depth)
            }
            "Group" | "StaticGroup" | "Collision" | "Anchor" | "Billboard" | "CADAssembly"
            | "CADLayer" | "Scene" | "LayerSet" | "Layer" => {
                let mut kids = sc.nodes(n, "children");
                kids.extend(sc.nodes(n, "layers"));
                self.walk_all(kids, xf, depth)
            }
            "CADFace" => {
                let kids = sc.nodes(n, "shape");
                self.walk_all(kids, xf, depth)
            }
            "Switch" => {
                let which = sc.num(n, "whichChoice", -1.0);
                let mut kids = sc.nodes(n, "choice");
                if kids.is_empty() {
                    kids = sc.nodes(n, "children");
                }
                match usize::try_from(which as i64).ok().and_then(|i| kids.get(i)) {
                    Some(&k) => self.walk(k, xf, depth + 1),
                    None => Ok(()),
                }
            }
            "LOD" => {
                let mut kids = sc.nodes(n, "level");
                if kids.is_empty() {
                    kids = sc.nodes(n, "children");
                }
                match kids.first() {
                    Some(&k) => self.walk(k, xf, depth + 1),
                    None => Ok(()),
                }
            }
            "Shape" => self.shape(n, xf),
            "Inline" => self.inline(n, xf, depth),
            _ if sc.protos.contains(&kind) => {
                self.skip("PROTO instances are not shown");
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn shape(&mut self, n: u32, xf: &Affine) -> Result<()> {
        let Some(g) = self.scene.node(n, "geometry") else {
            return Ok(());
        };
        let look = match self.scene.node(n, "appearance") {
            Some(a) => self.look(a),
            None => Look::default(),
        };
        if !geom::geometry(&self.scene, &mut self.b, g, xf, &look)? {
            let what = match self.scene.kind(g) {
                "PointSet" | "IndexedLineSet" | "LineSet" | "Polyline2D" | "Polypoint2D"
                | "Arc2D" | "Circle2D" => "lines and points are not shown",
                "Text" => "text is not shown",
                k if k.starts_with("Nurbs") => "NURBS surfaces are not shown",
                k if self.scene.protos.contains(k) => "PROTO instances are not shown",
                _ => "some geometry types are not shown",
            };
            self.skip(what);
        }
        Ok(())
    }

    /// The material of an Appearance node, made once.
    fn look(&mut self, a: u32) -> Look {
        if let Some(l) = self.looks.get(&a) {
            return *l;
        }
        let sc = &self.scene;
        let mut base: Option<[f32; 4]> = None;
        let name = sc.nodes[a as usize].name.clone();
        if let Some(m) = sc.node(a, "material") {
            let alpha = (1.0 - sc.num(m, "transparency", 0.0)).clamp(0.0, 1.0);
            let rgb = match sc.kind(m) {
                "Material" | "TwoSidedMaterial" => {
                    let d = sc.vec(m, "diffuseColor", [0.8; 3]);
                    let e = sc.vec(m, "emissiveColor", [0.0; 3]);
                    // Unlit scenes give their colour as emissive alone.
                    if d.iter().all(|&v| v < 1e-3) && e.iter().any(|&v| v > 1e-3) {
                        e
                    } else {
                        d
                    }
                }
                "PhysicalMaterial" => sc.vec(m, "baseColor", [1.0; 3]),
                "UnlitMaterial" => sc.vec(m, "emissiveColor", [1.0; 3]),
                _ => [0.8; 3],
            };
            base = Some([rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, alpha as f32]);
        }
        let mut texture = None;
        if let Some(mut t) = sc.node(a, "texture") {
            if sc.kind(t) == "MultiTexture" {
                t = sc.nodes(t, "texture").first().copied().unwrap_or(t);
            }
            match sc.kind(t) {
                "ImageTexture" => {
                    let urls = sc.strings(t, "url");
                    texture = self.image(&urls);
                }
                "PixelTexture" => texture = self.pixel_texture(t),
                "MovieTexture" => self.skip("movie textures are not shown"),
                _ => {}
            }
        }
        let sc = &self.scene;
        let tex = sc.node(a, "textureTransform").and_then(|t| {
            (sc.kind(t) == "TextureTransform").then(|| TexXf {
                center: sc.vec(t, "center", [0.0; 2]),
                scale: sc.vec(t, "scale", [1.0; 2]),
                rotation: sc.num(t, "rotation", 0.0),
                translation: sc.vec(t, "translation", [0.0; 2]),
            })
        });
        let material = if base.is_some() || texture.is_some() {
            let mut c = base.unwrap_or([1.0; 4]);
            if texture.is_some() {
                // An RGB texture replaces the diffuse colour.
                c = [1.0, 1.0, 1.0, c[3]];
            }
            Some(self.b.add_material(Material {
                name: name.unwrap_or_default(),
                base_color: c,
                texture,
            }))
        } else {
            None
        };
        let look = Look { material, tex };
        self.looks.insert(a, look);
        look
    }

    /// A PixelTexture's SFImage (`width height components` then one integer per pixel, rows
    /// from the bottom, the components packed most significant first) as a PNG.
    fn pixel_texture(&mut self, t: u32) -> Option<u32> {
        let v = self.scene.ints(t, "image");
        let (w, h, comps) = match v.get(..3)? {
            &[w, h, c] => (w, h, c),
            _ => return None,
        };
        let pixels = usize::try_from(w)
            .ok()?
            .checked_mul(usize::try_from(h).ok()?)?;
        if pixels == 0 {
            return None;
        }
        if !(1..=4).contains(&comps) {
            self.skip("a pixel texture has an unknown number of components");
            return None;
        }
        if pixels > self.limits.max_side_file / 4 || v.len() < 3 + pixels {
            self.skip("a pixel texture is cut short or too large");
            return None;
        }
        let (w, h, comps) = (w as usize, h as usize, comps as u32);
        let mut rgba = vec![0u8; pixels * 4];
        for (i, &p) in v[3..3 + pixels].iter().enumerate() {
            let p = p.clamp(0, i64::from(u32::MAX)) as u32;
            let c = |k: u32| ((p >> (8 * (comps - 1 - k))) & 0xff) as u8;
            let px = match comps {
                1 => [c(0), c(0), c(0), 255],
                2 => [c(0), c(0), c(0), c(1)],
                3 => [c(0), c(1), c(2), 255],
                _ => [c(0), c(1), c(2), c(3)],
            };
            // Row 0 of the file is the bottom of the image.
            let (x, y) = (i % w, h - 1 - i / w);
            rgba[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&px);
        }
        let bytes =
            specular_png::encode(w as u32, h as u32, specular_png::ColorType::Rgba, &rgba).ok()?;
        self.b.add_image(ImageData::Bytes {
            bytes,
            mime: Some("image/png".into()),
        })
    }

    /// The first of `urls` that reads, else a missing image named after the first.
    fn image(&mut self, urls: &[String]) -> Option<u32> {
        let key = urls.join("\n");
        if let Some(i) = self.images.get(&key) {
            return *i;
        }
        let mut data = None;
        let mut first_err = None;
        for u in urls {
            let name = u.split('#').next().unwrap_or(u);
            let name = name.strip_prefix("./").unwrap_or(name);
            if name.is_empty() {
                continue;
            }
            match self.resolver.read(name, self.limits.max_side_file) {
                Ok(bytes) => {
                    data = Some(ImageData::Bytes { bytes, mime: None });
                    break;
                }
                Err(e) => {
                    first_err.get_or_insert((name.to_owned(), e));
                }
            }
        }
        let data = match (data, first_err) {
            (Some(d), _) => d,
            (None, Some((name, reason))) => ImageData::Missing { name, reason },
            (None, None) => return None,
        };
        let i = self.b.add_image(data);
        self.images.insert(key, i);
        i
    }

    /// An Inline: the first of its files (in the model's own folder) that reads.
    fn inline(&mut self, n: u32, xf: &Affine, depth: usize) -> Result<()> {
        if !self.scene.boolean(n, "load", true) {
            return Ok(());
        }
        if self.inline_stack.len() >= MAX_INLINE_DEPTH || self.inlines > MAX_INLINES {
            self.skip("too many Inline files; the rest are not shown");
            return Ok(());
        }
        let urls = self.scene.strings(n, "url");
        let mut outside = false;
        for u in &urls {
            let name = u.split('#').next().unwrap_or(u);
            let name = name.strip_prefix("./").unwrap_or(name);
            if name.is_empty() {
                continue;
            }
            // Same folder only: no folders, no URLs, no absolute paths.
            if name.contains(['/', '\\', ':']) {
                outside = true;
                continue;
            }
            if self.inline_stack.iter().any(|s| s == name) {
                self.skip("an Inline file includes itself");
                return Ok(());
            }
            let roots = match self.inlined.get(name) {
                Some(r) => r.clone(),
                None => {
                    let Ok(bytes) = self.resolver.read(name, self.limits.max_side_file) else {
                        continue;
                    };
                    self.inlines += 1;
                    let roots = match parse_into(&bytes, &mut self.scene, &self.limits) {
                        Ok(r) => r,
                        Err(Error::TooLarge(m)) => return Err(Error::TooLarge(m)),
                        Err(_) => {
                            self.skip("an Inline file could not be read");
                            return Ok(());
                        }
                    };
                    self.inlined.insert(name.to_owned(), roots.clone());
                    roots
                }
            };
            self.inline_stack.push(name.to_owned());
            let r = self.walk_all(roots, xf, depth);
            self.inline_stack.pop();
            return r;
        }
        if outside {
            self.skip("Inline files outside the model's folder are not read");
        } else if !urls.is_empty() {
            self.skip("an Inline file is missing");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
