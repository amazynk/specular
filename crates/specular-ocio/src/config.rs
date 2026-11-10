//! The config: colour spaces, roles, displays and views, view transforms, and the
//! transforms they hold, from the YAML document.

use std::path::{Path, PathBuf};

use crate::yaml::{self, Value};
use crate::{Error, Result};

/// A transform as the config writes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Transform {
    Group(Vec<Transform>, bool),
    Matrix {
        m: [f64; 16],
        offset: [f64; 4],
        inverse: bool,
    },
    Exponent {
        value: [f64; 4],
        style: String,
        inverse: bool,
    },
    ExponentWithLinear {
        gamma: [f64; 4],
        offset: [f64; 4],
        style: String,
        inverse: bool,
    },
    Log {
        base: f64,
        inverse: bool,
    },
    LogAffine {
        base: f64,
        log_slope: [f64; 3],
        log_offset: [f64; 3],
        lin_slope: [f64; 3],
        lin_offset: [f64; 3],
        inverse: bool,
    },
    LogCamera {
        base: f64,
        log_slope: [f64; 3],
        log_offset: [f64; 3],
        lin_slope: [f64; 3],
        lin_offset: [f64; 3],
        lin_break: [f64; 3],
        linear_slope: Option<[f64; 3]>,
        inverse: bool,
    },
    Range {
        min_in: Option<f64>,
        max_in: Option<f64>,
        min_out: Option<f64>,
        max_out: Option<f64>,
        clamp: bool,
        inverse: bool,
    },
    Cdl {
        slope: [f64; 3],
        offset: [f64; 3],
        power: [f64; 3],
        sat: f64,
        clamp: bool,
        inverse: bool,
    },
    File {
        src: String,
        interpolation: String,
        inverse: bool,
    },
    ColorSpace {
        src: String,
        dst: String,
        inverse: bool,
    },
    DisplayView {
        src: String,
        display: String,
        view: String,
        inverse: bool,
    },
    Builtin {
        style: String,
        inverse: bool,
    },
    /// A transform this reader doesn't know, by its tag (refused when used).
    Unknown(String),
}

/// One colour space.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorSpace {
    pub name: String,
    pub aliases: Vec<String>,
    pub family: String,
    pub description: String,
    pub is_data: bool,
    /// Display-referred (in `display_colorspaces`).
    pub display: bool,
    pub to_ref: Option<Transform>,
    pub from_ref: Option<Transform>,
}

/// A view of a display.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub name: String,
    /// The colour space the view shows in (display colour space with a view transform, or
    /// any colour space without one). `<USE_DISPLAY_NAME>` stands for the display's name.
    pub colorspace: String,
    pub view_transform: Option<String>,
    pub looks: String,
}

/// A view transform.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewTransform {
    pub name: String,
    pub from_scene: Option<Transform>,
    pub to_scene: Option<Transform>,
    pub from_display: Option<Transform>,
    pub to_display: Option<Transform>,
}

/// A parsed config.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub version: String,
    pub search_paths: Vec<PathBuf>,
    pub roles: Vec<(String, String)>,
    pub colorspaces: Vec<ColorSpace>,
    /// Displays in file order, each with its views (shared views resolved).
    pub displays: Vec<(String, Vec<View>)>,
    pub active_displays: Vec<String>,
    pub active_views: Vec<String>,
    pub view_transforms: Vec<ViewTransform>,
    pub default_view_transform: Option<String>,
    /// Where the config is, for relative search paths.
    pub dir: PathBuf,
}

fn num4(v: Option<&Value>, default: f64) -> [f64; 4] {
    let mut out = [default; 4];
    match v {
        Some(Value::Seq(_)) | Some(Value::Tagged(..)) => {
            let n = v.map(Value::numbers).unwrap_or_default();
            for (o, x) in out.iter_mut().zip(n.iter()) {
                *o = *x;
            }
            if n.len() == 3 {
                out[3] = default;
            }
        }
        Some(s) => {
            if let Some(x) = s.as_f64() {
                out = [x, x, x, default];
            }
        }
        None => {}
    }
    out
}

fn num3(v: Option<&Value>, default: f64) -> [f64; 3] {
    let n = num4(v, default);
    [n[0], n[1], n[2]]
}

fn inverse(v: &Value) -> bool {
    v.get("direction")
        .and_then(Value::as_str)
        .is_some_and(|d| d.eq_ignore_ascii_case("inverse"))
}

fn text(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// A transform node (tagged).
pub fn transform(v: &Value) -> Transform {
    let Some(tag) = v.tag() else {
        return Transform::Unknown("untagged".into());
    };
    let inv = inverse(v);
    match tag {
        "GroupTransform" => Transform::Group(
            v.get("children")
                .map(|c| c.as_seq().iter().map(transform).collect())
                .unwrap_or_default(),
            inv,
        ),
        "MatrixTransform" => {
            let mut m = [
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
            ];
            if let Some(n) = v.get("matrix").map(Value::numbers)
                && n.len() == 16
            {
                m.copy_from_slice(&n);
            }
            Transform::Matrix {
                m,
                offset: num4(v.get("offset"), 0.0),
                inverse: inv,
            }
        }
        "ExponentTransform" => Transform::Exponent {
            value: num4(v.get("value"), 1.0),
            style: text(v, "style"),
            inverse: inv,
        },
        "ExponentWithLinearTransform" => Transform::ExponentWithLinear {
            gamma: num4(v.get("gamma"), 1.0),
            offset: num4(v.get("offset"), 0.0),
            style: text(v, "style"),
            inverse: inv,
        },
        "LogTransform" => Transform::Log {
            base: v.get("base").and_then(Value::as_f64).unwrap_or(2.0),
            inverse: inv,
        },
        "LogAffineTransform" => Transform::LogAffine {
            base: v.get("base").and_then(Value::as_f64).unwrap_or(2.0),
            log_slope: num3(v.get("log_side_slope"), 1.0),
            log_offset: num3(v.get("log_side_offset"), 0.0),
            lin_slope: num3(v.get("lin_side_slope"), 1.0),
            lin_offset: num3(v.get("lin_side_offset"), 0.0),
            inverse: inv,
        },
        "LogCameraTransform" => Transform::LogCamera {
            base: v.get("base").and_then(Value::as_f64).unwrap_or(2.0),
            log_slope: num3(v.get("log_side_slope"), 1.0),
            log_offset: num3(v.get("log_side_offset"), 0.0),
            lin_slope: num3(v.get("lin_side_slope"), 1.0),
            lin_offset: num3(v.get("lin_side_offset"), 0.0),
            lin_break: num3(v.get("lin_side_break"), 0.0),
            linear_slope: v.get("linear_slope").map(|s| num3(Some(s), 1.0)),
            inverse: inv,
        },
        "RangeTransform" => Transform::Range {
            min_in: v.get("min_in_value").and_then(Value::as_f64),
            max_in: v.get("max_in_value").and_then(Value::as_f64),
            min_out: v.get("min_out_value").and_then(Value::as_f64),
            max_out: v.get("max_out_value").and_then(Value::as_f64),
            clamp: text(v, "style") != "noClamp",
            inverse: inv,
        },
        "CDLTransform" => Transform::Cdl {
            slope: num3(v.get("slope"), 1.0),
            offset: num3(v.get("offset"), 0.0),
            power: num3(v.get("power"), 1.0),
            sat: v.get("sat").and_then(Value::as_f64).unwrap_or(1.0),
            // OCIO's default is no clamping; `asc` clamps as ASC CDL v1.2 does.
            clamp: text(v, "style").eq_ignore_ascii_case("asc"),
            inverse: inv,
        },
        "FileTransform" => Transform::File {
            src: text(v, "src"),
            interpolation: text(v, "interpolation"),
            inverse: inv,
        },
        "ColorSpaceTransform" => Transform::ColorSpace {
            src: text(v, "src"),
            dst: text(v, "dst"),
            inverse: inv,
        },
        "DisplayViewTransform" => Transform::DisplayView {
            src: text(v, "src"),
            display: text(v, "display"),
            view: text(v, "view"),
            inverse: inv,
        },
        "BuiltinTransform" => Transform::Builtin {
            style: text(v, "style"),
            inverse: inv,
        },
        other => Transform::Unknown(other.to_owned()),
    }
}

fn colorspace(v: &Value, display: bool) -> Option<ColorSpace> {
    let name = text(v, "name");
    if name.is_empty() {
        return None;
    }
    let t = |a: &str, b: &str| v.get(a).or_else(|| v.get(b)).map(transform);
    let (to_ref, from_ref) = if display {
        (
            t("to_display_reference", "to_reference"),
            t("from_display_reference", "from_reference"),
        )
    } else {
        (
            t("to_scene_reference", "to_reference"),
            t("from_scene_reference", "from_reference"),
        )
    };
    Some(ColorSpace {
        aliases: v
            .get("aliases")
            .map(|a| {
                a.as_seq()
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        family: text(v, "family"),
        description: text(v, "description"),
        is_data: text(v, "isdata") == "true",
        display,
        to_ref,
        from_ref,
        name,
    })
}

fn view(v: &Value) -> View {
    View {
        name: text(v, "name"),
        colorspace: {
            let c = text(v, "colorspace");
            if c.is_empty() {
                text(v, "display_colorspace")
            } else {
                c
            }
        },
        view_transform: Some(text(v, "view_transform")).filter(|s| !s.is_empty()),
        looks: text(v, "looks"),
    }
}

fn names(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Str(s)) => s
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect(),
        Some(v) => v
            .as_seq()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        None => Vec::new(),
    }
}

impl Config {
    /// Reads a config file.
    pub fn load(path: &Path) -> Result<Config> {
        let meta = std::fs::metadata(path)
            .map_err(|e| Error::Config(format!("can't read the config: {e}")))?;
        if meta.len() > 64 << 20 {
            return Err(Error::Config("the config file is too large".into()));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("can't read the config: {e}")))?;
        Config::parse(&text, path.parent().unwrap_or(Path::new(".")))
    }

    /// Parses a config's text; `dir` is where it is (for its search paths).
    pub fn parse(src: &str, dir: &Path) -> Result<Config> {
        let doc =
            yaml::parse(src).map_err(|e| Error::Config(format!("not an OCIO config: {e}")))?;
        let version = doc
            .get("ocio_profile_version")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Config("not an OCIO config: no ocio_profile_version".into()))?
            .to_owned();
        let mut c = Config {
            version,
            dir: dir.to_path_buf(),
            ..Default::default()
        };
        let sp: Vec<String> = match doc.get("search_path") {
            Some(Value::Str(s)) => s.split(':').map(str::to_owned).collect(),
            Some(v) => names(Some(v)),
            None => Vec::new(),
        };
        c.search_paths = sp
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(|s| dir.join(s))
            .collect();
        c.search_paths.push(dir.to_path_buf());
        c.roles = doc
            .get("roles")
            .map(|r| {
                r.as_map()
                    .iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        for (key, display) in [("colorspaces", false), ("display_colorspaces", true)] {
            if let Some(list) = doc.get(key) {
                c.colorspaces
                    .extend(list.as_seq().iter().filter_map(|v| colorspace(v, display)));
            }
        }
        if let Some(list) = doc.get("view_transforms") {
            for v in list.as_seq() {
                let t = |k: &str| v.get(k).map(transform);
                c.view_transforms.push(ViewTransform {
                    name: text(v, "name"),
                    from_scene: t("from_scene_reference").or_else(|| t("from_reference")),
                    to_scene: t("to_scene_reference").or_else(|| t("to_reference")),
                    from_display: t("from_display_reference"),
                    to_display: t("to_display_reference"),
                });
            }
        }
        let shared: Vec<View> = doc
            .get("shared_views")
            .map(|s| s.as_seq().iter().map(view).collect())
            .unwrap_or_default();
        if let Some(d) = doc.get("displays") {
            for (name, views) in d.as_map() {
                let mut out = Vec::new();
                for v in views.as_seq() {
                    match v.tag() {
                        Some("Views") => {
                            for n in v.as_seq().iter().filter_map(Value::as_str) {
                                if let Some(s) = shared.iter().find(|s| s.name == n) {
                                    let mut s = s.clone();
                                    if s.colorspace == "<USE_DISPLAY_NAME>" {
                                        s.colorspace = name.clone();
                                    }
                                    out.push(s);
                                }
                            }
                        }
                        _ => out.push(view(v)),
                    }
                }
                c.displays.push((name.clone(), out));
            }
        }
        c.active_displays = names(doc.get("active_displays"));
        c.active_views = names(doc.get("active_views"));
        c.default_view_transform = doc
            .get("default_view_transform")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if c.colorspaces.is_empty() {
            return Err(Error::Config("the config has no colour spaces".into()));
        }
        Ok(c)
    }

    /// The colour space named `n` (by name, alias or role), case-insensitively.
    pub fn colorspace(&self, n: &str) -> Option<&ColorSpace> {
        let n = self
            .roles
            .iter()
            .find(|(r, _)| r.eq_ignore_ascii_case(n))
            .map_or(n, |(_, cs)| cs.as_str());
        self.colorspaces.iter().find(|c| {
            c.name.eq_ignore_ascii_case(n) || c.aliases.iter().any(|a| a.eq_ignore_ascii_case(n))
        })
    }

    pub fn view_transform(&self, n: &str) -> Option<&ViewTransform> {
        self.view_transforms
            .iter()
            .find(|v| v.name.eq_ignore_ascii_case(n))
    }

    /// The displays to offer: the active ones (in their order), else all.
    pub fn display_names(&self) -> Vec<String> {
        let all: Vec<String> = self.displays.iter().map(|(n, _)| n.clone()).collect();
        let active: Vec<String> = self
            .active_displays
            .iter()
            .filter(|a| all.contains(a))
            .cloned()
            .collect();
        if active.is_empty() { all } else { active }
    }

    /// The views of `display` to offer (active ones only, when the config lists them).
    pub fn views(&self, display: &str) -> Vec<&View> {
        let Some((_, vs)) = self.displays.iter().find(|(n, _)| n == display) else {
            return Vec::new();
        };
        vs.iter()
            .filter(|v| self.active_views.is_empty() || self.active_views.contains(&v.name))
            .collect()
    }

    /// The scene-referred colour spaces to offer as inputs (not data, not display).
    pub fn input_names(&self) -> Vec<String> {
        self.colorspaces
            .iter()
            .filter(|c| !c.display)
            .map(|c| c.name.clone())
            .collect()
    }

    /// The colour space a role names (`scene_linear`, `default`).
    pub fn role(&self, role: &str) -> Option<&str> {
        self.roles
            .iter()
            .find(|(r, _)| r == role)
            .map(|(_, c)| c.as_str())
    }

    /// A LUT file's path, through the search paths.
    pub fn find_file(&self, src: &str) -> Result<PathBuf> {
        if src.contains('$') {
            return Err(Error::Unsupported(format!(
                "the LUT name {src} uses context variables, which are not supported"
            )));
        }
        let p = Path::new(src);
        if p.is_absolute() {
            return p
                .is_file()
                .then(|| p.to_path_buf())
                .ok_or_else(|| Error::File(format!("the LUT file {src} is missing")));
        }
        self.search_paths
            .iter()
            .map(|d| d.join(src))
            .find(|p| p.is_file())
            .ok_or_else(|| Error::File(format!("the LUT file {src} is missing")))
    }
}
