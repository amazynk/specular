//! The font context of one book: the bundled fonts, the platform's when asked for, and the
//! book's own (`@font-face`), each registered under a family name of its own so a book's
//! "Garamond" never mixes with an installed one.

use std::collections::HashMap;
use std::sync::Arc;

use fontique::{
    Blob, Collection, CollectionOptions, FontInfoOverride, FontStyle, FontWeight, SourceCache,
};
use parley::FontContext;

use crate::css::FontFace;
use crate::package::resolve;

/// Bundled families, the last resort of every family list.
pub const BUNDLED: [&str; 3] = [
    specular_theme::fonts::UI_FAMILY,
    specular_theme::fonts::ARABIC_FAMILY,
    specular_theme::fonts::HEBREW_FAMILY,
];

/// Most font files a book may register.
const MAX_BOOK_FONTS: usize = 64;

pub struct BookFonts {
    pub fcx: FontContext,
    /// CSS family (lower case) to the family it was registered as.
    families: HashMap<String, String>,
    /// Font files already registered, by archive path and face descriptor.
    loaded: HashMap<(String, u32, bool), ()>,
    /// The memory budget's share of the registered files (the collection keeps them).
    held: Vec<specular_core::Reservation>,
}

impl BookFonts {
    pub fn new(system: bool) -> BookFonts {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: system,
        });
        for data in specular_theme::fonts::all() {
            collection.register_fonts(Blob::new(Arc::new(data)), None);
        }
        BookFonts {
            fcx: FontContext {
                collection,
                source_cache: SourceCache::default(),
            },
            families: HashMap::new(),
            loaded: HashMap::new(),
            held: Vec::new(),
        }
    }

    /// The reservations of the registered font files, for whoever keeps their data.
    pub fn into_held(self) -> Vec<specular_core::Reservation> {
        self.held
    }

    /// Registers the faces of `rules` whose files `read` returns (TrueType/OpenType, and
    /// WOFF/WOFF2 unwrapped by `specular-font`).
    pub fn register(&mut self, rules: &[FontFace], read: &dyn Fn(&str) -> Option<crate::Part>) {
        for f in rules {
            if self.loaded.len() >= MAX_BOOK_FONTS {
                return;
            }
            let fam = f.family.to_ascii_lowercase();
            let weight = f.weight.unwrap_or(400.0);
            for src in &f.src {
                let Some((path, _)) = resolve(&f.base, src) else {
                    continue;
                };
                let key = (path.clone(), weight as u32, f.italic);
                if self.loaded.contains_key(&key) {
                    self.families
                        .entry(fam.clone())
                        .or_insert_with(|| format!("epub {fam}"));
                    break;
                }
                let Some(part) = read(&path) else { continue };
                let (data, held) = part.into_parts();
                // WOFF and WOFF2 are unwrapped to the SFNT the text stack reads.
                let data = if is_sfnt(&data) {
                    data
                } else {
                    match specular_font::open(&data, Default::default()) {
                        Ok(f) if !f.faces.is_empty() && f.faces[0].index() == 0 => {
                            f.faces[0].sfnt().as_ref().clone()
                        }
                        _ => continue,
                    }
                };
                let name = format!("epub {fam}");
                let got = self.fcx.collection.register_fonts(
                    Blob::new(Arc::new(data)),
                    Some(FontInfoOverride {
                        family_name: Some(&name),
                        weight: Some(FontWeight::new(weight)),
                        style: Some(if f.italic {
                            FontStyle::Italic
                        } else {
                            FontStyle::Normal
                        }),
                        ..Default::default()
                    }),
                );
                if !got.is_empty() {
                    self.held.push(held);
                    self.loaded.insert(key, ());
                    self.families.insert(fam.clone(), name);
                    break;
                }
            }
        }
    }

    /// A CSS family list as parley's source: book families mapped to their registered
    /// names, generic families kept, the bundled families last.
    pub fn stack(&self, families: &[String]) -> String {
        let mut out = String::new();
        for f in families {
            let name = match self.families.get(f) {
                Some(n) => n.clone(),
                None => f.clone(),
            };
            if !out.is_empty() {
                out.push_str(", ");
            }
            if matches!(
                name.as_str(),
                "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui"
            ) {
                out.push_str(&name);
            } else {
                out.push('"');
                out.push_str(&name.replace('"', ""));
                out.push('"');
            }
        }
        for b in BUNDLED {
            if !out.is_empty() {
                out.push_str(", ");
            }
            out.push('"');
            out.push_str(b);
            out.push('"');
        }
        out
    }
}

fn is_sfnt(d: &[u8]) -> bool {
    d.len() > 12 && matches!(&d[..4], [0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf")
}
