//! Static HTML client galleries: a folder anyone can open
//! from disk or put on any static host. No uploads, no server, no network.
//!
//! - [`build`] renders the photos through Develop's export queue (the saved edit, and the
//!   watermark text through Develop's borders-and-watermark module), then writes each one as
//!   a resized JPEG or WebP and a thumbnail, location removed unless asked for.
//! - [`site`] writes `index.html`, `assets/gallery.css` and `assets/gallery.js`: a grid, a
//!   lightbox with keys, and a heart on each photo. Nothing is fetched from anywhere else.
//! - [`picks`] reads what the client sends back (`picks.txt`, `picks.json` or a pasted list)
//!   and matches it to photos by file name.
//!
//! A static gallery cannot be protected by a password: anyone with the folder or its address
//! sees everything in it. The page says nothing to suggest otherwise.

pub mod build;
pub mod picks;
pub mod site;

/// The page's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    Light,
    Dark,
    /// White, no frames, wide gaps.
    Minimal,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Light, Theme::Dark, Theme::Minimal];

    pub fn id(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::Minimal => "minimal",
        }
    }

    pub fn from_id(s: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|t| t.id() == s)
    }
}

/// The photos' file type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Jpeg,
    Webp,
}

impl ImageFormat {
    pub const ALL: [ImageFormat; 2] = [ImageFormat::Jpeg, ImageFormat::Webp];

    pub fn ext(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
        }
    }

    pub fn from_id(s: &str) -> Option<ImageFormat> {
        match s {
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "webp" => Some(ImageFormat::Webp),
            _ => None,
        }
    }
}

/// What shows under each photo (and in the lightbox).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Captions {
    Off,
    /// The original file name.
    FileName,
    /// The photo's title (XMP `dc:title`), when it has one.
    Title,
    /// The file name, the title and the caption (XMP `dc:description`).
    All,
}

impl Captions {
    pub const ALL: [Captions; 4] = [
        Captions::Off,
        Captions::FileName,
        Captions::Title,
        Captions::All,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Captions::Off => "off",
            Captions::FileName => "name",
            Captions::Title => "title",
            Captions::All => "all",
        }
    }

    pub fn from_id(s: &str) -> Option<Captions> {
        Captions::ALL.into_iter().find(|c| c.id() == s)
    }
}

/// Long edges offered for the photos, in pixels.
pub const SIZES: [u32; 5] = [1280, 1600, 2048, 2560, 3200];
/// The thumbnails' long edge (twice the grid's cell, for sharp tiles on dense screens).
pub const THUMB_EDGE: u32 = 600;
/// Quality of the photos and thumbnails (1–100).
pub const QUALITY: u8 = 85;

/// The page's words, in the language Specular showed when the gallery was made.
/// `{n}`, `{i}` are filled in by the page's script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    /// BCP 47 language tag for `<html lang>`.
    pub lang: String,
    pub rtl: bool,
    /// "24 photos", already counted.
    pub count: String,
    pub hint: String,
    /// "Picked: {n}".
    pub picked: String,
    pub pick: String,
    pub copy: String,
    pub copied: String,
    /// When the browser does not let the page copy: the list is shown selected.
    pub copy_by_hand: String,
    pub download_txt: String,
    pub download_json: String,
    pub clear: String,
    pub clear_ask: String,
    /// The browser keeps nothing for this page (private window, storage off).
    pub no_storage: String,
    pub noscript: String,
    pub previous: String,
    pub next: String,
    pub close: String,
    /// "{i} of {n}".
    pub position: String,
    /// The lightbox's name for screen readers.
    pub viewer: String,
    /// The picks bar's name for screen readers.
    pub picks: String,
}

impl Default for Words {
    /// English (tests and a missing catalogue).
    fn default() -> Words {
        Words {
            lang: "en".into(),
            rtl: false,
            count: String::new(),
            hint: "Tap the heart on the photos you want. Your picks stay in this browser until you send them.".into(),
            picked: "Picked: {n}".into(),
            pick: "Pick".into(),
            copy: "Copy my picks".into(),
            copied: "Copied. Paste the list in a message.".into(),
            copy_by_hand: "Copy the selected list and paste it in a message.".into(),
            download_txt: "Download picks.txt".into(),
            download_json: "Download picks.json".into(),
            clear: "Clear picks".into(),
            clear_ask: "Clear all your picks in this browser?".into(),
            no_storage: "This browser will not keep your picks. Send them before you close the page.".into(),
            noscript: "Picking needs JavaScript. The photos still open.".into(),
            previous: "Previous".into(),
            next: "Next".into(),
            close: "Close".into(),
            position: "{i} of {n}".into(),
            viewer: "Photo viewer".into(),
            picks: "Your picks".into(),
        }
    }
}

/// What the gallery holds and how it looks.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub title: String,
    pub theme: Theme,
    /// The photos' long edge (no photo is made larger).
    pub long_edge: u32,
    pub format: ImageFormat,
    pub captions: Captions,
    /// Camera, lens, exposure and date in the lightbox.
    pub exif: bool,
    /// Keep GPS and place names in the photos' metadata (off: removed).
    pub keep_location: bool,
    /// Text drawn on every photo (empty: none), through Develop's watermark.
    pub watermark: String,
    pub words: Words,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            title: String::new(),
            theme: Theme::Light,
            long_edge: 2048,
            format: ImageFormat::Jpeg,
            captions: Captions::FileName,
            exif: false,
            keep_location: false,
            watermark: String::new(),
            words: Words::default(),
        }
    }
}
