//! Export presets: what a file looks like (format, depth, quality, space, size, metadata)
//! and where it goes (naming pattern, folder, what to do with a taken name).
//!
//! Shipped presets have a stable [`Preset::id`] (`jpeg`, `web`, `tiff16`, `exr`) used by the
//! command line and translated for display by the app; the user's own have only a name.
//! Presets are kept as plain text, one section per preset:
//!
//! ```text
//! [Web, 2048 px]
//! format = jpeg
//! depth = 8
//! quality = 85
//! space = srgb
//! size = long_edge 2048
//! metadata = no_location
//! pattern = {name}
//! folder = subfolder export
//! conflict = suffix
//! ```
//!
//! Unknown keys are ignored and bad values keep the default, so a file from a newer
//! Specular still loads.

use std::path::PathBuf;

use specular_convert::{Conflict, MetaMode, OutputDir, Resize};

use super::Space;
use super::hdr::HdrOutput;

/// File type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileFormat {
    Jpeg,
    Png,
    Avif,
    Jxl,
    Tiff,
    Exr,
}

impl FileFormat {
    pub const ALL: [FileFormat; 6] = [
        FileFormat::Jpeg,
        FileFormat::Png,
        FileFormat::Avif,
        FileFormat::Jxl,
        FileFormat::Tiff,
        FileFormat::Exr,
    ];

    pub fn id(self) -> &'static str {
        match self {
            FileFormat::Jpeg => "jpeg",
            FileFormat::Png => "png",
            FileFormat::Avif => "avif",
            FileFormat::Jxl => "jxl",
            FileFormat::Tiff => "tiff",
            FileFormat::Exr => "exr",
        }
    }

    pub fn from_id(s: &str) -> Option<FileFormat> {
        let s = s.to_ascii_lowercase();
        let s = match s.as_str() {
            "jpg" => "jpeg",
            "tif" => "tiff",
            o => o,
        };
        FileFormat::ALL.into_iter().find(|f| f.id() == s)
    }

    /// File extension (without the dot).
    pub fn ext(self) -> &'static str {
        match self {
            FileFormat::Jpeg => "jpg",
            FileFormat::Png => "png",
            FileFormat::Avif => "avif",
            FileFormat::Jxl => "jxl",
            FileFormat::Tiff => "tif",
            FileFormat::Exr => "exr",
        }
    }

    /// The depths it holds, the default first.
    pub fn depths(self) -> &'static [Depth] {
        match self {
            FileFormat::Jpeg | FileFormat::Avif => &[Depth::Eight],
            FileFormat::Png | FileFormat::Jxl => &[Depth::Eight, Depth::Sixteen],
            FileFormat::Tiff => &[Depth::Sixteen, Depth::Eight, Depth::Float],
            FileFormat::Exr => &[Depth::Half, Depth::Float],
        }
    }

    /// Has a quality setting (lossy).
    pub fn lossy(self) -> bool {
        matches!(self, FileFormat::Jpeg | FileFormat::Avif | FileFormat::Jxl)
    }
}

/// Sample depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Depth {
    Eight,
    Sixteen,
    /// 16-bit float (EXR).
    Half,
    /// 32-bit float.
    Float,
}

impl Depth {
    pub fn id(self) -> &'static str {
        match self {
            Depth::Eight => "8",
            Depth::Sixteen => "16",
            Depth::Half => "half",
            Depth::Float => "float",
        }
    }

    pub fn from_id(s: &str) -> Option<Depth> {
        [Depth::Eight, Depth::Sixteen, Depth::Half, Depth::Float]
            .into_iter()
            .find(|d| d.id() == s)
    }

    /// Linear float samples.
    pub fn float(self) -> bool {
        matches!(self, Depth::Half | Depth::Float)
    }
}

/// One way to export.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// Stable id of a shipped preset (empty for the user's).
    pub id: String,
    /// The user's name for it (shipped presets are named by the app from `id`).
    pub name: String,
    pub format: FileFormat,
    pub depth: Depth,
    /// 1–100, for JPEG, AVIF and lossy JPEG XL.
    pub quality: u8,
    /// JPEG XL without loss.
    pub lossless: bool,
    pub space: Space,
    /// HDR output: off (SDR), PQ or HLG (BT.2100), or an SDR file with a gain map.
    pub hdr: HdrOutput,
    pub size: Resize,
    /// Allow making images larger.
    pub upscale: bool,
    pub metadata: MetaMode,
    /// File name pattern (`specular_convert::Pattern`).
    pub pattern: String,
    pub folder: OutputDir,
    pub conflict: Conflict,
}

impl Default for Preset {
    fn default() -> Preset {
        Preset {
            id: String::new(),
            name: String::new(),
            format: FileFormat::Jpeg,
            depth: Depth::Eight,
            quality: 90,
            lossless: false,
            space: Space::Srgb,
            hdr: HdrOutput::Off,
            size: Resize::None,
            upscale: false,
            metadata: MetaMode::All,
            pattern: "{name}".into(),
            folder: OutputDir::Subfolder("export".into()),
            conflict: Conflict::Suffix,
        }
    }
}

impl Preset {
    /// The shipped presets, in the order the dialog lists them.
    pub fn builtin() -> Vec<Preset> {
        let p = |id: &str| Preset {
            id: id.into(),
            ..Preset::default()
        };
        vec![
            p("jpeg"),
            Preset {
                quality: 85,
                size: Resize::LongEdge(2048),
                metadata: MetaMode::NoLocation,
                ..p("web")
            },
            Preset {
                format: FileFormat::Tiff,
                depth: Depth::Sixteen,
                space: Space::AdobeRgb,
                ..p("tiff16")
            },
            Preset {
                format: FileFormat::Exr,
                depth: Depth::Half,
                space: Space::Rec2020,
                ..p("exr")
            },
        ]
    }

    /// A shipped preset by id, or one of `user` by name (ignoring case).
    pub fn find(name: &str, user: &[Preset]) -> Option<Preset> {
        Preset::builtin()
            .into_iter()
            .find(|p| p.id == name)
            .or_else(|| {
                user.iter()
                    .find(|p| p.name.eq_ignore_ascii_case(name.trim()))
                    .cloned()
            })
    }

    /// The same choices with every value the format can hold (a JPEG is 8-bit, an EXR
    /// float, quality within 1–100).
    pub fn normalised(mut self) -> Preset {
        let d = self.format.depths();
        if !d.contains(&self.depth) {
            self.depth = d[0];
        }
        self.quality = self.quality.clamp(1, 100);
        self.lossless &= self.format == FileFormat::Jxl;
        if !self.hdr.fits(self.format) {
            self.hdr = HdrOutput::Off;
        }
        match self.hdr {
            // BT.2100: Rec. 2020 primaries, 16-bit where the format has a choice.
            HdrOutput::Pq | HdrOutput::Hlg => {
                self.space = Space::Rec2020;
                self.depth = *d.last().unwrap_or(&self.depth);
                self.lossless = false;
            }
            HdrOutput::GainMap => {
                self.depth = Depth::Eight;
                self.lossless = false;
            }
            HdrOutput::Off => {}
        }
        if self.pattern.trim().is_empty() {
            self.pattern = "{name}".into();
        }
        self
    }

    /// Whether two presets make the same files (names and ids aside).
    pub fn same_choices(&self, o: &Preset) -> bool {
        let strip = |p: &Preset| Preset {
            id: String::new(),
            name: String::new(),
            ..p.clone().normalised()
        };
        strip(self) == strip(o)
    }

    /// The `specular_convert` options for the integer formats.
    pub fn convert_options(&self) -> specular_convert::Options {
        use specular_convert::{Format, Options};
        let format = match self.format {
            FileFormat::Jpeg => Format::Jpeg,
            FileFormat::Png => Format::Png,
            FileFormat::Avif => Format::Avif,
            FileFormat::Jxl => Format::Jxl,
            FileFormat::Tiff | FileFormat::Exr => Format::Tiff,
        };
        let q = self.quality.clamp(1, 100);
        Options {
            format,
            jpeg_quality: q,
            jpeg_full_chroma: q >= 95,
            avif_quality: q,
            jxl_quality: (!self.lossless).then_some(q),
            ..Options::default()
        }
    }

    /// The text form of one preset (see the module docs).
    pub fn to_text(&self) -> String {
        let size = match self.size {
            Resize::None => "full".to_owned(),
            Resize::LongEdge(n) => format!("long_edge {n}"),
            Resize::Fit { width, height } => format!("fit {width} {height}"),
            Resize::Percent(p) => format!("percent {p}"),
        };
        let folder = match &self.folder {
            OutputDir::Subfolder(s) if s.trim().is_empty() => "same".to_owned(),
            OutputDir::Subfolder(s) => format!("subfolder {s}"),
            OutputDir::Folder(p) => format!("path {}", p.display()),
        };
        let mut s = format!("[{}]\n", self.name.replace(['[', ']', '\n'], " "));
        if !self.id.is_empty() {
            s.push_str(&format!("id = {}\n", self.id));
        }
        for (k, v) in [
            ("format", self.format.id().to_owned()),
            ("depth", self.depth.id().to_owned()),
            ("quality", self.quality.to_string()),
            ("lossless", self.lossless.to_string()),
            ("space", self.space.id().to_owned()),
            ("hdr", self.hdr.id().to_owned()),
            ("size", size),
            ("upscale", self.upscale.to_string()),
            ("metadata", meta_id(self.metadata).to_owned()),
            ("pattern", self.pattern.replace('\n', " ")),
            ("folder", folder.replace('\n', " ")),
            (
                "conflict",
                match self.conflict {
                    Conflict::Suffix => "suffix",
                    Conflict::Skip => "skip",
                }
                .to_owned(),
            ),
        ] {
            s.push_str(&format!("{k} = {v}\n"));
        }
        s
    }

    /// Sets one `key = value` of the text form; false when the key or value is not known.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let v = value.trim();
        let num = |s: &str| s.trim().parse::<u32>().ok().filter(|&n| n > 0);
        match key.trim() {
            "id" => self.id = v.to_owned(),
            "format" => match FileFormat::from_id(v) {
                Some(f) => self.format = f,
                None => return false,
            },
            "depth" => match Depth::from_id(v) {
                Some(d) => self.depth = d,
                None => return false,
            },
            "quality" => match v.parse::<u8>() {
                Ok(q) if (1..=100).contains(&q) => self.quality = q,
                _ => return false,
            },
            "lossless" => match v.parse() {
                Ok(b) => self.lossless = b,
                Err(_) => return false,
            },
            "upscale" => match v.parse() {
                Ok(b) => self.upscale = b,
                Err(_) => return false,
            },
            "space" => match Space::from_id(v) {
                Some(s) => self.space = s,
                None => return false,
            },
            "hdr" => match HdrOutput::from_id(v) {
                Some(h) => self.hdr = h,
                None => return false,
            },
            "size" => {
                let mut it = v.split_whitespace();
                let size = match (it.next(), it.next(), it.next()) {
                    (Some("full"), None, None) => Some(Resize::None),
                    (Some("long_edge"), Some(n), None) => num(n).map(Resize::LongEdge),
                    (Some("percent"), Some(n), None) => {
                        num(n).filter(|&p| p <= 400).map(Resize::Percent)
                    }
                    (Some("fit"), Some(w), Some(h)) => match (num(w), num(h)) {
                        (Some(width), Some(height)) => Some(Resize::Fit { width, height }),
                        _ => None,
                    },
                    _ => None,
                };
                match size {
                    Some(s) => self.size = s,
                    None => return false,
                }
            }
            "metadata" => match v {
                "all" => self.metadata = MetaMode::All,
                "no_location" => self.metadata = MetaMode::NoLocation,
                "strip" => self.metadata = MetaMode::Strip,
                _ => return false,
            },
            "pattern" => self.pattern = v.to_owned(),
            "folder" => {
                let (kind, rest) = v.split_once(' ').unwrap_or((v, ""));
                match kind {
                    "same" => self.folder = OutputDir::Subfolder(String::new()),
                    "subfolder" if !rest.trim().is_empty() => {
                        self.folder = OutputDir::Subfolder(rest.trim().to_owned())
                    }
                    "path" if !rest.trim().is_empty() => {
                        self.folder = OutputDir::Folder(PathBuf::from(rest.trim()))
                    }
                    _ => return false,
                }
            }
            "conflict" => match v {
                "suffix" => self.conflict = Conflict::Suffix,
                "skip" => self.conflict = Conflict::Skip,
                _ => return false,
            },
            _ => return false,
        }
        true
    }

    /// Every preset in `text` (the form of [`Preset::to_text`], several one after the
    /// other). Lines before the first section, comments (`#`) and bad lines are skipped.
    pub fn parse_all(text: &str) -> Vec<Preset> {
        let mut out: Vec<Preset> = Vec::new();
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if let Some(name) = l.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                out.push(Preset {
                    name: name.trim().to_owned(),
                    ..Preset::default()
                });
                continue;
            }
            if let (Some(p), Some((k, v))) = (out.last_mut(), l.split_once('=')) {
                p.set(k, v);
            }
        }
        out.into_iter().map(Preset::normalised).collect()
    }

    /// The text form of several presets.
    pub fn write_all(presets: &[Preset]) -> String {
        presets
            .iter()
            .map(Preset::to_text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn meta_id(m: MetaMode) -> &'static str {
    match m {
        MetaMode::All => "all",
        MetaMode::NoLocation => "no_location",
        MetaMode::Strip => "strip",
    }
}
