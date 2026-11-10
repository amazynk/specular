//! Export recipes: saved ways to export (format, size, colour, sharpening, metadata,
//! watermark, naming, folder), run several at once on a selection.
//!
//! - [`Recipe`]: the choices, with a plain text form ([`Recipe::to_text`],
//!   [`Recipe::parse_all`]) and the shipped starters ([`Recipe::starters`]).
//! - [`size`]: the output size of each resize mode.
//! - [`space`]: the output colour spaces and their ICC profiles.
//! - [`finish`]: from linear light at any size to the file's bytes (resize, colour, output
//!   sharpening, watermark, encode with the chosen metadata).
//! - [`meta`]: what metadata goes into the file.
//! - [`run`]: names for every image and recipe settled first, then the images rendered once
//!   each (at the largest size any recipe needs) and written per recipe, in parallel, with
//!   progress, cancel and an end report.
//!
//! The text form, one section per recipe:
//!
//! ```text
//! [Web, 2048 px]
//! id = web
//! format = jpeg
//! quality = 82
//! size = long_edge 2048
//! sharpen = screen standard
//! metadata = copyright_contact
//! strip_gps = true
//! pattern = {name}
//! folder = subfolder web
//! ```
//!
//! Unknown keys are ignored and bad values keep the default, so a file from a newer
//! Specular still loads.

pub mod finish;
pub mod meta;
pub mod run;
pub mod size;
pub mod space;
pub mod watermark;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use crate::Format;

pub use run::{
    Facts, Files, Item, Output, Plan, PlanError, Planned, RecipeReport, Render, Rendered, execute,
    folder, name_for, plan,
};
pub use size::Size;
pub use space::Space;

/// Output sharpening: what the file is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Medium {
    Off,
    Screen,
    Matte,
    Glossy,
}

/// How much output sharpening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Strength {
    Low,
    Standard,
    High,
}

/// Output sharpening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sharpen {
    pub medium: Medium,
    pub strength: Strength,
}

impl Sharpen {
    pub const OFF: Sharpen = Sharpen {
        medium: Medium::Off,
        strength: Strength::Standard,
    };
}

/// Which metadata goes into the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Keep {
    All,
    Copyright,
    CopyrightContact,
    None,
}

/// Metadata choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Metadata {
    pub keep: Keep,
    /// Leave out GPS and place names.
    pub strip_gps: bool,
    /// Leave out the people shown and face regions.
    pub strip_people: bool,
}

/// Where a watermark goes: a 3 × 3 grid, row by row from the top start corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Position {
    TopLeft,
    Top,
    TopRight,
    Left,
    Centre,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Position {
    pub const ALL: [Position; 9] = [
        Position::TopLeft,
        Position::Top,
        Position::TopRight,
        Position::Left,
        Position::Centre,
        Position::Right,
        Position::BottomLeft,
        Position::Bottom,
        Position::BottomRight,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Position::TopLeft => "top_left",
            Position::Top => "top",
            Position::TopRight => "top_right",
            Position::Left => "left",
            Position::Centre => "centre",
            Position::Right => "right",
            Position::BottomLeft => "bottom_left",
            Position::Bottom => "bottom",
            Position::BottomRight => "bottom_right",
        }
    }

    pub fn from_id(s: &str) -> Option<Position> {
        Position::ALL.into_iter().find(|p| p.id() == s)
    }

    /// Column and row in the grid, 0–2 each.
    pub fn cell(self) -> (u32, u32) {
        let i = Position::ALL.iter().position(|p| *p == self).unwrap_or(8) as u32;
        (i % 3, i / 3)
    }
}

/// What the watermark is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    None,
    Text(String),
    /// An image file (PNG with transparency, SVG, or any format Specular opens).
    Image(PathBuf),
}

/// A watermark.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Watermark {
    pub kind: MarkKind,
    pub position: Position,
    /// 1–100 %.
    pub opacity: u8,
    /// Its width as a share of the picture's width, 1–100 %.
    pub scale: u8,
}

impl Default for Watermark {
    fn default() -> Watermark {
        Watermark {
            kind: MarkKind::None,
            position: Position::BottomRight,
            opacity: 60,
            scale: 20,
        }
    }
}

/// Where the files go.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Destination {
    /// Next to the original.
    Same,
    /// A folder of this name next to the original.
    Subfolder(String),
    /// This folder, whatever the original.
    Fixed(PathBuf),
    /// Asked for when the recipe runs.
    Ask,
}

/// What to do when the file name exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Collision {
    /// Add `-2`, `-3`, … to the name.
    Unique,
    /// Replace the file (asked first; an original is never replaced).
    Overwrite,
    /// Leave the existing file and export nothing.
    Skip,
}

/// What happens after the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum After {
    Nothing,
    OpenFolder,
}

/// Sample depth of integer files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bits {
    Eight,
    Sixteen,
}

/// One saved way to export.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    /// Stable id of a shipped starter (`web`, `print`, `email`), kept after edits; empty
    /// for the user's own.
    pub id: String,
    /// The name shown (a starter's is translated from `id` while it is empty).
    pub name: String,
    pub format: Format,
    /// 1–100 for the lossy formats.
    pub quality: u8,
    /// WebP and JPEG XL without loss.
    pub lossless: bool,
    pub bits: Bits,
    pub space: Space,
    /// Embed the colour profile (without it, readers assume sRGB).
    pub embed_icc: bool,
    pub size: Size,
    /// Allow making images larger.
    pub enlarge: bool,
    pub sharpen: Sharpen,
    pub metadata: Metadata,
    pub watermark: Watermark,
    /// File name pattern ([`crate::Pattern`]).
    pub pattern: String,
    /// What `{text}` in the pattern becomes.
    pub text: String,
    pub destination: Destination,
    pub collision: Collision,
    pub after: After,
}

impl Default for Recipe {
    fn default() -> Recipe {
        Recipe {
            id: String::new(),
            name: String::new(),
            format: Format::Jpeg,
            quality: 90,
            lossless: false,
            bits: Bits::Eight,
            space: Space::Srgb,
            embed_icc: true,
            size: Size::Full,
            enlarge: false,
            sharpen: Sharpen::OFF,
            metadata: Metadata {
                keep: Keep::All,
                strip_gps: false,
                strip_people: false,
            },
            watermark: Watermark::default(),
            pattern: "{name}".into(),
            text: String::new(),
            destination: Destination::Subfolder("export".into()),
            collision: Collision::Unique,
            after: After::Nothing,
        }
    }
}

/// The formats recipes write (HEIF has no encoder in Specular yet).
pub const FORMATS: [Format; 6] = [
    Format::Jpeg,
    Format::Png,
    Format::Tiff,
    Format::Webp,
    Format::Avif,
    Format::Jxl,
];

/// Whether a format has a quality setting (lossy, or lossy unless `lossless`).
pub fn lossy(f: Format) -> bool {
    matches!(f, Format::Jpeg | Format::Webp | Format::Avif | Format::Jxl)
}

/// Whether a format can also be lossless.
pub fn has_lossless(f: Format) -> bool {
    matches!(f, Format::Webp | Format::Jxl)
}

impl Recipe {
    /// The shipped starters: web, print full size, email small.
    pub fn starters() -> Vec<Recipe> {
        vec![
            Recipe {
                id: "web".into(),
                quality: 82,
                size: Size::LongEdge(2048),
                sharpen: Sharpen {
                    medium: Medium::Screen,
                    strength: Strength::Standard,
                },
                metadata: Metadata {
                    keep: Keep::CopyrightContact,
                    strip_gps: true,
                    strip_people: true,
                },
                destination: Destination::Subfolder("web".into()),
                ..Recipe::default()
            },
            Recipe {
                id: "print".into(),
                format: Format::Tiff,
                bits: Bits::Sixteen,
                space: Space::AdobeRgb,
                sharpen: Sharpen {
                    medium: Medium::Glossy,
                    strength: Strength::Standard,
                },
                destination: Destination::Subfolder("print".into()),
                ..Recipe::default()
            },
            Recipe {
                id: "email".into(),
                quality: 75,
                size: Size::LongEdge(1280),
                sharpen: Sharpen {
                    medium: Medium::Screen,
                    strength: Strength::Low,
                },
                metadata: Metadata {
                    keep: Keep::Copyright,
                    strip_gps: true,
                    strip_people: true,
                },
                pattern: "{name}-small".into(),
                destination: Destination::Subfolder("email".into()),
                ..Recipe::default()
            },
        ]
    }

    /// The same choices with every value the format can hold.
    pub fn normalised(mut self) -> Recipe {
        if !FORMATS.contains(&self.format) {
            self.format = Format::Jpeg;
        }
        self.quality = self.quality.clamp(1, 100);
        self.lossless &= has_lossless(self.format);
        if !self.format.sixteen() {
            self.bits = Bits::Eight;
        }
        self.watermark.opacity = self.watermark.opacity.clamp(1, 100);
        self.watermark.scale = self.watermark.scale.clamp(1, 100);
        self.size = self.size.normalised();
        if self.pattern.trim().is_empty() {
            self.pattern = "{name}".into();
        }
        if let Destination::Subfolder(s) = &self.destination
            && s.trim().is_empty()
        {
            self.destination = Destination::Same;
        }
        self
    }

    /// The encoder options for this recipe's format.
    pub fn options(&self) -> crate::Options {
        let q = self.quality.clamp(1, 100);
        crate::Options {
            format: self.format,
            jpeg_quality: q,
            jpeg_full_chroma: q >= 90,
            webp_quality: (!self.lossless).then_some(q),
            jxl_quality: (!self.lossless).then_some(q),
            jxl_effort: if self.lossless { 1 } else { 3 },
            avif_quality: q,
            avif_effort: 3,
            tiff_deflate: true,
            ..crate::Options::default()
        }
    }

    /// The text form of one recipe (see the module docs).
    pub fn to_text(&self) -> String {
        let mut s = format!("[{}]\n", one_line(&self.name).replace(['[', ']'], " "));
        if !self.id.is_empty() {
            s.push_str(&format!("id = {}\n", self.id));
        }
        let sharpen = match self.sharpen.medium {
            Medium::Off => "off".to_owned(),
            m => format!("{} {}", medium_id(m), strength_id(self.sharpen.strength)),
        };
        let (mark, mark_value) = match &self.watermark.kind {
            MarkKind::None => ("none", String::new()),
            MarkKind::Text(t) => ("text", one_line(t)),
            MarkKind::Image(p) => ("image", one_line(&p.to_string_lossy())),
        };
        let folder = match &self.destination {
            Destination::Same => "same".to_owned(),
            Destination::Subfolder(n) => format!("subfolder {}", one_line(n)),
            Destination::Fixed(p) => format!("path {}", one_line(&p.to_string_lossy())),
            Destination::Ask => "ask".to_owned(),
        };
        for (k, v) in [
            ("format", self.format.id().to_owned()),
            ("quality", self.quality.to_string()),
            ("lossless", self.lossless.to_string()),
            (
                "bits",
                match self.bits {
                    Bits::Eight => "8",
                    Bits::Sixteen => "16",
                }
                .to_owned(),
            ),
            ("space", self.space.id().to_owned()),
            ("embed_profile", self.embed_icc.to_string()),
            ("size", self.size.to_text()),
            ("enlarge", self.enlarge.to_string()),
            ("sharpen", sharpen),
            ("metadata", keep_id(self.metadata.keep).to_owned()),
            ("strip_gps", self.metadata.strip_gps.to_string()),
            ("strip_people", self.metadata.strip_people.to_string()),
            (
                "watermark",
                if mark_value.is_empty() {
                    mark.to_owned()
                } else {
                    format!("{mark} {mark_value}")
                },
            ),
            (
                "watermark_position",
                self.watermark.position.id().to_owned(),
            ),
            ("watermark_opacity", self.watermark.opacity.to_string()),
            ("watermark_scale", self.watermark.scale.to_string()),
            ("pattern", one_line(&self.pattern)),
            ("text", one_line(&self.text)),
            ("folder", folder),
            ("collision", collision_id(self.collision).to_owned()),
            (
                "after",
                match self.after {
                    After::Nothing => "nothing",
                    After::OpenFolder => "open_folder",
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
        let flag = |v: &str| v.parse::<bool>().ok();
        let pct = |v: &str| v.parse::<u8>().ok().filter(|n| (1..=100).contains(n));
        match key.trim() {
            "id" => self.id = v.to_owned(),
            "format" => match crate::Format::from_id(v).filter(|f| FORMATS.contains(f)) {
                Some(f) => self.format = f,
                None => return false,
            },
            "quality" => match pct(v) {
                Some(q) => self.quality = q,
                None => return false,
            },
            "lossless" => match flag(v) {
                Some(b) => self.lossless = b,
                None => return false,
            },
            "bits" => match v {
                "8" => self.bits = Bits::Eight,
                "16" => self.bits = Bits::Sixteen,
                _ => return false,
            },
            "space" => match Space::from_id(v) {
                Some(s) => self.space = s,
                None => return false,
            },
            "embed_profile" => match flag(v) {
                Some(b) => self.embed_icc = b,
                None => return false,
            },
            "size" => match Size::parse(v) {
                Some(s) => self.size = s,
                None => return false,
            },
            "enlarge" => match flag(v) {
                Some(b) => self.enlarge = b,
                None => return false,
            },
            "sharpen" => {
                let mut it = v.split_whitespace();
                match (it.next(), it.next()) {
                    (Some("off"), None) => self.sharpen = Sharpen::OFF,
                    (Some(m), Some(s)) => match (medium_from(m), strength_from(s)) {
                        (Some(medium), Some(strength)) if medium != Medium::Off => {
                            self.sharpen = Sharpen { medium, strength }
                        }
                        _ => return false,
                    },
                    _ => return false,
                }
            }
            "metadata" => match keep_from(v) {
                Some(k) => self.metadata.keep = k,
                None => return false,
            },
            "strip_gps" => match flag(v) {
                Some(b) => self.metadata.strip_gps = b,
                None => return false,
            },
            "strip_people" => match flag(v) {
                Some(b) => self.metadata.strip_people = b,
                None => return false,
            },
            "watermark" => {
                let (kind, rest) = v.split_once(' ').unwrap_or((v, ""));
                let rest = rest.trim();
                self.watermark.kind = match kind {
                    "none" => MarkKind::None,
                    "text" if !rest.is_empty() => MarkKind::Text(rest.to_owned()),
                    "image" if !rest.is_empty() => MarkKind::Image(PathBuf::from(rest)),
                    _ => return false,
                };
            }
            "watermark_position" => match Position::from_id(v) {
                Some(p) => self.watermark.position = p,
                None => return false,
            },
            "watermark_opacity" => match pct(v) {
                Some(n) => self.watermark.opacity = n,
                None => return false,
            },
            "watermark_scale" => match pct(v) {
                Some(n) => self.watermark.scale = n,
                None => return false,
            },
            "pattern" => self.pattern = v.to_owned(),
            "text" => self.text = v.to_owned(),
            "folder" => {
                let (kind, rest) = v.split_once(' ').unwrap_or((v, ""));
                let rest = rest.trim();
                self.destination = match kind {
                    "same" => Destination::Same,
                    "ask" => Destination::Ask,
                    "subfolder" if !rest.is_empty() => Destination::Subfolder(rest.to_owned()),
                    "path" if !rest.is_empty() => Destination::Fixed(PathBuf::from(rest)),
                    _ => return false,
                };
            }
            "collision" => match collision_from(v) {
                Some(c) => self.collision = c,
                None => return false,
            },
            "after" => match v {
                "nothing" => self.after = After::Nothing,
                "open_folder" => self.after = After::OpenFolder,
                _ => return false,
            },
            _ => return false,
        }
        true
    }

    /// Every recipe in `text` (the form of [`Recipe::to_text`], several one after the
    /// other). Lines before the first section, comments (`#`) and bad lines are skipped.
    pub fn parse_all(text: &str) -> Vec<Recipe> {
        let mut out: Vec<Recipe> = Vec::new();
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if let Some(name) = l.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                out.push(Recipe {
                    name: name.trim().to_owned(),
                    ..Recipe::default()
                });
                continue;
            }
            if let (Some(r), Some((k, v))) = (out.last_mut(), l.split_once('=')) {
                r.set(k, v);
            }
        }
        out.into_iter().map(Recipe::normalised).collect()
    }

    /// The text form of several recipes.
    pub fn write_all(recipes: &[Recipe]) -> String {
        let mut s = String::from("# Specular export recipes\n\n");
        s.push_str(
            &recipes
                .iter()
                .map(Recipe::to_text)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        s
    }
}

/// `s` on one line (the text form is line based).
fn one_line(s: &str) -> String {
    s.replace(['\n', '\r'], " ").trim().to_owned()
}

pub fn medium_id(m: Medium) -> &'static str {
    match m {
        Medium::Off => "off",
        Medium::Screen => "screen",
        Medium::Matte => "matte",
        Medium::Glossy => "glossy",
    }
}

fn medium_from(s: &str) -> Option<Medium> {
    [Medium::Off, Medium::Screen, Medium::Matte, Medium::Glossy]
        .into_iter()
        .find(|m| medium_id(*m) == s)
}

pub fn strength_id(s: Strength) -> &'static str {
    match s {
        Strength::Low => "low",
        Strength::Standard => "standard",
        Strength::High => "high",
    }
}

fn strength_from(s: &str) -> Option<Strength> {
    [Strength::Low, Strength::Standard, Strength::High]
        .into_iter()
        .find(|m| strength_id(*m) == s)
}

pub fn keep_id(k: Keep) -> &'static str {
    match k {
        Keep::All => "all",
        Keep::Copyright => "copyright",
        Keep::CopyrightContact => "copyright_contact",
        Keep::None => "none",
    }
}

fn keep_from(s: &str) -> Option<Keep> {
    [
        Keep::All,
        Keep::Copyright,
        Keep::CopyrightContact,
        Keep::None,
    ]
    .into_iter()
    .find(|k| keep_id(*k) == s)
}

pub fn collision_id(c: Collision) -> &'static str {
    match c {
        Collision::Unique => "unique",
        Collision::Overwrite => "overwrite",
        Collision::Skip => "skip",
    }
}

fn collision_from(s: &str) -> Option<Collision> {
    [Collision::Unique, Collision::Overwrite, Collision::Skip]
        .into_iter()
        .find(|c| collision_id(*c) == s)
}
