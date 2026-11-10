//! Lens corrections from Lensfun's lens database.
//!
//! Only Lensfun's **data files** are used (CC BY-SA 3.0, `data/PROVENANCE.md`); this crate's
//! parser, matching and models are our own, written from Lensfun's published documentation
//! of the database format (<https://lensfun.github.io/manual/latest/dbformat.html> and the
//! element pages it links) and of the correction models (see [`model`]). The Lensfun
//! library (LGPL) is not used.
//!
//! - [`Db`]: mounts, cameras and lenses with their calibrations, indexed by maker and mount.
//! - [`bundled`]: the database shipped with Specular, decompressed and parsed once, on first
//!   use; [`preload`] starts that on a background thread.
//! - [`lookup`]: a shot's camera and lens names to the matching profiles.
//! - [`model`]: the distortion, TCA and vignetting models and their interpolation.

pub mod lookup;
pub mod model;
pub mod names;

use std::collections::HashMap;
use std::sync::OnceLock;

use model::{Distortion, DistortionCal, Tca, TcaCal, VignettingCal};

/// The database shipped with Specular: Lensfun's `data/db/*.xml`, each as
/// `name\0content\0` in name order, zstd-compressed (`tools/fetch-lensfun.sh`).
pub const BUNDLE: &[u8] = include_bytes!("../data/lensfun-db.zst");

/// A lens mount and the mounts whose lenses fit it (directly or with an adapter).
#[derive(Clone, Debug, PartialEq)]
pub struct Mount {
    pub name: String,
    pub compat: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// As the Exif data has them (identification).
    pub maker: String,
    pub model: String,
    /// The English names for display (the plain ones when there are none).
    pub maker_en: String,
    pub model_en: String,
    pub variant: Option<String>,
    pub mount: String,
    pub crop: f64,
}

/// A lens's projection (`<type>`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LensType {
    #[default]
    Rectilinear,
    Fisheye,
    Panoramic,
    Equirectangular,
    Orthographic,
    Stereographic,
    Equisolid,
    FisheyeThoby,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lens {
    pub maker: String,
    pub model: String,
    pub maker_en: String,
    pub model_en: String,
    pub mounts: Vec<String>,
    /// The crop factor and aspect ratio (long over short side) of the calibration camera.
    pub crop: f64,
    pub aspect: f64,
    pub kind: LensType,
    /// Focal range in mm: `<focal>`, else the name's, else the calibrations'.
    pub focal: Option<(f64, f64)>,
    pub aperture: Option<(f64, f64)>,
    pub distortion: Vec<DistortionCal>,
    pub tca: Vec<TcaCal>,
    pub vignetting: Vec<VignettingCal>,
    /// [`names::words`] of the model, sorted, each once.
    pub words: Vec<String>,
    /// [`names::words`] of the maker, sorted, each once.
    pub maker_words: Vec<String>,
    /// [`names::plain`] maker and model (one lens model, whatever its calibration).
    pub(crate) plain_key: String,
    /// [`Lens::display_name`] in lower case (sorting).
    pub(crate) sort_name: String,
}

impl Lens {
    /// "Maker model" for display, without repeating the maker.
    pub fn display_name(&self) -> String {
        let m = self.maker_en.trim();
        if m.is_empty() || self.model_en.to_lowercase().starts_with(&m.to_lowercase()) {
            self.model_en.clone()
        } else {
            format!("{m} {}", self.model_en)
        }
    }

    /// A key that names this lens in an edit: maker, model and calibration crop factor.
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.maker, self.model, self.crop)
    }

    /// Whether any calibration is present.
    pub fn has_calibration(&self) -> bool {
        !(self.distortion.is_empty() && self.tca.is_empty() && self.vignetting.is_empty())
    }

    /// The corrections for a shot at `focal` mm, f-number `aperture` and focus distance
    /// `distance` m on a camera of crop factor `camera_crop`.
    pub fn profile(
        &self,
        focal: f64,
        aperture: f64,
        distance: f64,
        camera_crop: f64,
    ) -> model::Profile {
        model::Profile::new(
            model::distortion_at(&self.distortion, focal),
            model::tca_at(&self.tca, focal),
            model::vignetting_at(&self.vignetting, focal, aperture, distance),
            self.crop,
            self.aspect,
            camera_crop,
        )
    }
}

/// The lens database.
#[derive(Clone, Debug, Default)]
pub struct Db {
    pub mounts: Vec<Mount>,
    pub cameras: Vec<Camera>,
    pub lenses: Vec<Lens>,
    /// Files that could not be read, with why.
    pub errors: Vec<String>,
    /// Camera indices by [`names::plain`] maker (the identifying and English names).
    cameras_by_maker: HashMap<String, Vec<usize>>,
    /// Lens indices by [`names::plain`] mount.
    lenses_by_mount: HashMap<String, Vec<usize>>,
    /// Mount index by [`names::plain`] name.
    mount_index: HashMap<String, usize>,
}

impl Db {
    /// Parses database files in order; a later definition of the same camera or lens replaces
    /// an earlier one ("How database files are found and loaded"). A file that is not a
    /// lens database is listed in [`Db::errors`].
    pub fn from_files<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Db {
        let mut db = Db::default();
        for (name, xml) in files {
            if let Err(e) = db.add_xml(xml) {
                db.errors.push(format!("{name}: {e}"));
            }
        }
        db.index();
        db
    }

    /// One XML document.
    pub fn from_xml(xml: &str) -> Result<Db, String> {
        let mut db = Db::default();
        db.add_xml(xml)?;
        db.index();
        Ok(db)
    }

    /// Decompresses and parses a bundle in [`BUNDLE`]'s form.
    pub fn from_bundle(zst: &[u8]) -> Result<Db, String> {
        use std::io::Read;
        let mut dec = ruzstd::decoding::StreamingDecoder::new(zst).map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        dec.read_to_end(&mut raw).map_err(|e| e.to_string())?;
        let parts: Vec<&[u8]> = raw.split(|&b| b == 0).collect();
        let mut files = Vec::new();
        for pair in parts.chunks_exact(2) {
            let name = std::str::from_utf8(pair[0]).map_err(|e| e.to_string())?;
            let text = std::str::from_utf8(pair[1]).map_err(|e| e.to_string())?;
            files.push((name, text));
        }
        if files.is_empty() {
            return Err("empty bundle".into());
        }
        Ok(Db::from_files(files))
    }

    fn add_xml(&mut self, xml: &str) -> Result<(), String> {
        let opts = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        };
        let doc = roxmltree::Document::parse_with_options(xml, opts).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        if root.tag_name().name() != "lensdatabase" {
            return Err("not a lens database".into());
        }
        for n in root.children().filter(roxmltree::Node::is_element) {
            match n.tag_name().name() {
                "mount" => self.mounts.extend(parse_mount(n)),
                "camera" => self.cameras.extend(parse_camera(n)),
                "lens" => self.lenses.extend(parse_lens(n)),
                _ => {}
            }
        }
        Ok(())
    }

    /// Keeps the last of each camera, lens and mount defined more than once, in order.
    fn dedup(&mut self) {
        fn last<T>(v: &mut Vec<T>, key: impl Fn(&T) -> String) {
            let mut at = HashMap::new();
            for (i, x) in v.iter().enumerate() {
                at.insert(key(x), i);
            }
            let mut i = 0;
            v.retain(|x| {
                let keep = at.get(&key(x)) == Some(&i);
                i += 1;
                keep
            });
        }
        last(&mut self.mounts, |m| names::plain(&m.name));
        last(&mut self.cameras, |c| {
            format!(
                "{}\0{}\0{:?}",
                names::plain(&c.maker),
                names::plain(&c.model),
                c.variant
            )
        });
        last(&mut self.lenses, |l| {
            format!(
                "{}\0{}\0{}\0{:?}",
                names::plain(&l.maker),
                names::plain(&l.model),
                l.crop,
                l.mounts
            )
        });
    }

    fn index(&mut self) {
        self.dedup();
        self.cameras_by_maker.clear();
        self.lenses_by_mount.clear();
        self.mount_index.clear();
        for (i, c) in self.cameras.iter().enumerate() {
            let mut keys = vec![names::plain(&c.maker), names::plain(&c.maker_en)];
            keys.dedup();
            for k in keys {
                self.cameras_by_maker.entry(k).or_default().push(i);
            }
        }
        for (i, l) in self.lenses.iter().enumerate() {
            for m in &l.mounts {
                let v = self.lenses_by_mount.entry(names::plain(m)).or_default();
                if v.last() != Some(&i) {
                    v.push(i);
                }
            }
        }
        for (i, m) in self.mounts.iter().enumerate() {
            self.mount_index.insert(names::plain(&m.name), i);
        }
    }

    /// The cameras of a maker (any spelling [`names::plain`] makes equal).
    pub fn cameras_of(&self, maker: &str) -> impl Iterator<Item = &Camera> {
        self.camera_indices(maker).iter().map(|&i| &self.cameras[i])
    }

    /// Indices into [`Db::cameras`] of a maker's cameras.
    pub fn camera_indices(&self, maker: &str) -> &[usize] {
        self.cameras_by_maker
            .get(&names::plain(maker))
            .map_or(&[], Vec::as_slice)
    }

    /// Every maker key of the camera index.
    pub fn camera_makers(&self) -> impl Iterator<Item = &str> {
        self.cameras_by_maker.keys().map(String::as_str)
    }

    pub fn mount(&self, name: &str) -> Option<&Mount> {
        self.mount_index
            .get(&names::plain(name))
            .map(|&i| &self.mounts[i])
    }

    /// The mounts whose lenses fit a camera of mount `name`: itself and its `<compat>` ones
    /// (not further: "recursion doesn't work here").
    pub fn fitting_mounts(&self, name: &str) -> Vec<String> {
        let mut v = vec![names::plain(name)];
        if let Some(m) = self.mount(name) {
            v.extend(m.compat.iter().map(|c| names::plain(c)));
        }
        v.dedup();
        v
    }

    /// The lenses of the mounts `mounts` ([`names::plain`] names), each once, in database
    /// order.
    pub fn lenses_on(&self, mounts: &[String]) -> Vec<usize> {
        let mut v: Vec<usize> = mounts
            .iter()
            .filter_map(|m| self.lenses_by_mount.get(m))
            .flatten()
            .copied()
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// The text of `n`'s child `tag`: the one without `lang` (identification) and the English
/// one (display; the plain one when absent).
fn text_child(n: roxmltree::Node, tag: &str) -> Option<(String, String)> {
    let mut plain = None;
    let mut en = None;
    for c in n.children().filter(|c| c.has_tag_name(tag)) {
        let t = c.text().unwrap_or("").trim().to_string();
        match c.attribute("lang") {
            None => plain = plain.or(Some(t)),
            Some("en") => en = en.or(Some(t)),
            Some(_) => {}
        }
    }
    let plain = plain?;
    let en = en.unwrap_or_else(|| plain.clone());
    Some((plain, en))
}

fn number(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn attr(n: roxmltree::Node, name: &str) -> Option<f64> {
    n.attribute(name).and_then(number)
}

fn child_number(n: roxmltree::Node, tag: &str) -> Option<f64> {
    n.children()
        .find(|c| c.has_tag_name(tag))
        .and_then(|c| c.text())
        .and_then(number)
}

fn parse_mount(n: roxmltree::Node) -> Option<Mount> {
    let (name, _) = text_child(n, "name")?;
    let compat = n
        .children()
        .filter(|c| c.has_tag_name("compat"))
        .filter_map(|c| c.text().map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .collect();
    Some(Mount { name, compat })
}

fn parse_camera(n: roxmltree::Node) -> Option<Camera> {
    let (maker, maker_en) = text_child(n, "maker")?;
    let (model, model_en) = text_child(n, "model")?;
    let (mount, _) = text_child(n, "mount")?;
    let variant = text_child(n, "variant")
        .map(|v| v.0)
        .filter(|v| !v.is_empty());
    let crop = child_number(n, "cropfactor")
        .filter(|c| *c > 0.0)
        .unwrap_or(1.0);
    Some(Camera {
        maker,
        model,
        maker_en,
        model_en,
        variant,
        mount,
        crop,
    })
}

/// "1.5" or "3:2".
fn aspect(s: &str) -> Option<f64> {
    match s.split_once(':') {
        Some((a, b)) => {
            let (a, b) = (number(a)?, number(b)?);
            (a > 0.0 && b > 0.0).then(|| a.max(b) / a.min(b))
        }
        None => number(s).filter(|v| *v > 0.0).map(|v| v.max(1.0 / v)),
    }
}

/// `<focal>` or `<aperture>`: `value`, or `min` and `max`.
fn range(n: roxmltree::Node, tag: &str) -> Option<(f64, f64)> {
    let c = n.children().find(|c| c.has_tag_name(tag))?;
    if let Some(v) = attr(c, "value") {
        return Some((v, v));
    }
    let (lo, hi) = (attr(c, "min"), attr(c, "max"));
    match (lo, hi) {
        (Some(a), Some(b)) => Some((a.min(b), a.max(b))),
        (Some(a), None) | (None, Some(a)) => Some((a, a)),
        _ => None,
    }
}

fn parse_lens(n: roxmltree::Node) -> Option<Lens> {
    let (maker, maker_en) = text_child(n, "maker").unwrap_or_default();
    let (model, model_en) = text_child(n, "model")?;
    let mounts: Vec<String> = n
        .children()
        .filter(|c| c.has_tag_name("mount"))
        .filter_map(|c| c.text().map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
        .collect();
    let crop = child_number(n, "cropfactor")
        .filter(|c| *c > 0.0)
        .unwrap_or(1.0);
    let asp = n
        .children()
        .find(|c| c.has_tag_name("aspect-ratio"))
        .and_then(|c| c.text())
        .and_then(aspect)
        .unwrap_or(1.5);
    let kind = match text_child(n, "type").map(|t| t.0) {
        Some(t) => match t.as_str() {
            "fisheye" => LensType::Fisheye,
            "panoramic" => LensType::Panoramic,
            "equirectangular" => LensType::Equirectangular,
            "orthographic" => LensType::Orthographic,
            "stereographic" => LensType::Stereographic,
            "equisolid" => LensType::Equisolid,
            "fisheye_thoby" => LensType::FisheyeThoby,
            _ => LensType::Rectilinear,
        },
        None => LensType::Rectilinear,
    };
    let mut distortion = Vec::new();
    let mut tca = Vec::new();
    let mut vignetting = Vec::new();
    for cal in n.children().filter(|c| c.has_tag_name("calibration")) {
        for e in cal.children().filter(roxmltree::Node::is_element) {
            let Some(focal) = attr(e, "focal").filter(|f| *f > 0.0) else {
                continue;
            };
            let a = |name: &str, d: f64| attr(e, name).unwrap_or(d);
            let model = e.attribute("model").unwrap_or("none");
            match e.tag_name().name() {
                "distortion" => {
                    let m = match model {
                        "ptlens" => Distortion::PtLens {
                            a: a("a", 0.0),
                            b: a("b", 0.0),
                            c: a("c", 0.0),
                        },
                        "poly3" => Distortion::Poly3 { k1: a("k1", 0.0) },
                        "poly5" => Distortion::Poly5 {
                            k1: a("k1", 0.0),
                            k2: a("k2", 0.0),
                        },
                        _ => continue,
                    };
                    distortion.push(DistortionCal { focal, model: m });
                }
                "tca" => {
                    let m = match model {
                        "linear" => Tca::linear(a("kr", 1.0), a("kb", 1.0)),
                        "poly3" => Tca {
                            red: [a("br", 0.0), a("cr", 0.0), a("vr", 1.0)],
                            blue: [a("bb", 0.0), a("cb", 0.0), a("vb", 1.0)],
                        },
                        _ => continue,
                    };
                    tca.push(TcaCal { focal, model: m });
                }
                "vignetting" => {
                    if model != "pa" {
                        continue;
                    }
                    let Some(aperture) = attr(e, "aperture").filter(|v| *v > 0.0) else {
                        continue;
                    };
                    vignetting.push(VignettingCal {
                        focal,
                        aperture,
                        distance: attr(e, "distance").filter(|v| *v > 0.0).unwrap_or(1000.0),
                        k: [a("k1", 0.0), a("k2", 0.0), a("k3", 0.0)],
                    });
                }
                _ => {}
            }
        }
    }
    let calibrated = || {
        let fs = distortion
            .iter()
            .map(|d| d.focal)
            .chain(tca.iter().map(|t| t.focal))
            .chain(vignetting.iter().map(|v| v.focal));
        let (lo, hi) = fs.fold((f64::MAX, f64::MIN), |(lo, hi), f| (lo.min(f), hi.max(f)));
        (lo <= hi).then_some((lo, hi))
    };
    let focal = range(n, "focal")
        .or_else(|| names::focal_of_name(&model))
        .or_else(calibrated);
    let aperture = range(n, "aperture");
    let set = |s: &str| {
        let mut w = names::words(s);
        w.sort_unstable();
        w.dedup();
        w
    };
    let words = set(&model);
    let maker_words = set(&maker);
    let plain_key = format!("{}\0{}", names::plain(&maker), names::plain(&model));
    let mut l = Lens {
        maker,
        model,
        maker_en,
        model_en,
        mounts,
        crop,
        aspect: asp,
        kind,
        focal,
        aperture,
        distortion,
        tca,
        vignetting,
        words,
        maker_words,
        plain_key,
        sort_name: String::new(),
    };
    l.sort_name = l.display_name().to_lowercase();
    Some(l)
}

static BUNDLED: OnceLock<Option<Db>> = OnceLock::new();

/// The database shipped with Specular, decompressed and parsed on first use (which blocks
/// the caller: call it off the UI thread, or [`preload`] early). `None` if the bundle
/// cannot be read.
pub fn bundled() -> Option<&'static Db> {
    BUNDLED
        .get_or_init(|| Db::from_bundle(BUNDLE).ok())
        .as_ref()
}

/// The shipped database if it is loaded, without waiting.
pub fn bundled_if_loaded() -> Option<&'static Db> {
    BUNDLED.get().and_then(Option::as_ref)
}

/// Starts loading the shipped database on a background thread (nothing when it is loaded or
/// loading).
pub fn preload() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if BUNDLED.get().is_some() {
        return;
    }
    STARTED.get_or_init(|| {
        specular_thread::spawn("dec-lensfun", || {
            bundled();
        });
    });
}

#[cfg(test)]
mod tests;
