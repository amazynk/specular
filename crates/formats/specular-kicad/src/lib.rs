//! KiCad 6 and later boards and schematics, read with our own S-expression parser
//! ([`sexpr`]) into `specular-vector` scenes:
//!
//! * [`pcb`]: `.kicad_pcb` boards: tracks, arcs, vias, pads, zone fills, footprint and board
//!   graphics and text, one layer per KiCad layer in KiCad's default colours.
//! * [`sch`]: `.kicad_sch` schematics: wires, buses, junctions, labels, text, symbols drawn
//!   from the embedded symbol library, and hierarchical sheets, each sheet a page.
//!
//! KiCad 5 and older files (the legacy formats) are recognised and refused by name
//! ([`legacy_kind`]).

pub mod geom;
pub mod pcb;
pub mod sch;
pub mod sexpr;

/// Why a file could not be read.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum Error {
    /// Not this format at all (the next decoder may try).
    #[error("not this format")]
    NotThisFormat,
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("{0}")]
    Malformed(String),
    /// A variant we refuse, said in plain English.
    #[error("{0}")]
    Unsupported(String),
    #[error("too large")]
    TooLarge,
}

/// Limits on what one file may make.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Items (shapes and texts) in a scene; past it the rest is left out with a warning.
    pub items: usize,
    /// Schematic sheets (pages) read, the root included.
    pub sheets: usize,
    /// List nesting in the S-expression text.
    pub depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            items: 4_000_000,
            sheets: 1000,
            depth: 256,
        }
    }
}

/// Refusal of a KiCad 4 or 5 board.
pub const LEGACY_BOARD: &str =
    "KiCad 5 and older boards are not supported: open the board in KiCad 6 or later and save it.";
/// Refusal of a KiCad 4 or 5 schematic.
pub const LEGACY_SCHEMATIC: &str = "KiCad 5 and older schematics are not supported: open the schematic in KiCad 6 or later and save it.";

fn skip_ws(head: &[u8]) -> &[u8] {
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    let n = head
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(head.len());
    &head[n..]
}

/// Whether `head` (the start of a file) is a KiCad S-expression board.
pub fn is_kicad_pcb(head: &[u8]) -> bool {
    starts_with_token(skip_ws(head), b"(kicad_pcb")
}

/// Whether `head` is a KiCad S-expression schematic.
pub fn is_kicad_sch(head: &[u8]) -> bool {
    starts_with_token(skip_ws(head), b"(kicad_sch")
}

fn starts_with_token(s: &[u8], tok: &[u8]) -> bool {
    s.starts_with(tok)
        && s.get(tok.len())
            .is_none_or(|b| b.is_ascii_whitespace() || *b == b')' || *b == b'(')
}

/// The file's `(version N)` from its first few hundred bytes.
pub fn head_version(head: &[u8]) -> Option<u64> {
    let s = &head[..head.len().min(512)];
    let at = s.windows(9).position(|w| w == b"(version ")? + 9;
    let digits: Vec<u8> = s[at..]
        .iter()
        .copied()
        .skip_while(|b| *b == b' ')
        .take_while(u8::is_ascii_digit)
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

/// What a KiCad 4/5 legacy file is ("KiCad 5 board", "KiCad 5 schematic"), so the decoder
/// can refuse it by name: `PCBNEW-BOARD` boards, S-expression boards older than KiCad 6
/// (version before 2020) and `EESchema Schematic File` schematics.
pub fn legacy_kind(head: &[u8]) -> Option<&'static str> {
    let s = skip_ws(head);
    if s.starts_with(b"PCBNEW-BOARD") {
        return Some("KiCad 4 board");
    }
    if s.starts_with(b"EESchema Schematic File") {
        return Some("KiCad 5 schematic");
    }
    if s.starts_with(b"EESchema-LIBRARY") {
        return Some("KiCad 5 symbol library");
    }
    if is_kicad_pcb(s) && head_version(s).is_some_and(|v| v < BOARD_V6_DEV) {
        return Some("KiCad 5 board");
    }
    None
}

/// Board versions from this date on are KiCad 6 (development builds included).
pub const BOARD_V6_DEV: u64 = 20_200_000;

/// The oldest KiCad release that opens a board of `version`.
pub fn board_release(version: u64) -> &'static str {
    release(version, [20211014, 20221018, 20240108, 20241229])
}

/// The oldest KiCad release that opens a schematic of `version`.
pub fn schematic_release(version: u64) -> &'static str {
    release(version, [20211123, 20230121, 20231120, 20250114])
}

/// The first of KiCad 6, 7, 8, 9 whose file version (`last`) is at least `version`, else
/// KiCad 10.
fn release(version: u64, last: [u64; 4]) -> &'static str {
    const NAMES: [&str; 4] = ["KiCad 6", "KiCad 7", "KiCad 8", "KiCad 9"];
    last.iter()
        .position(|&l| version <= l)
        .map_or("KiCad 10", |i| NAMES[i])
}

/// Adds `w` to `warnings` unless it is there already.
pub(crate) fn warn(warnings: &mut Vec<String>, w: impl Into<String>) {
    let w = w.into();
    if !warnings.contains(&w) && warnings.len() < 64 {
        warnings.push(w);
    }
}

/// The text of a file: UTF-8, else Latin-1 (KiCad writes UTF-8; a broken byte does not
/// stop the read).
pub(crate) fn text_of(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    match std::str::from_utf8(bytes) {
        Ok(s) => std::borrow::Cow::Borrowed(s),
        Err(_) => std::borrow::Cow::Owned(String::from_utf8_lossy(bytes).into_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_and_names_legacy_files() {
        assert!(is_kicad_pcb(b"  \n(kicad_pcb (version 20240108)"));
        assert!(!is_kicad_pcb(b"(kicad_pcbx"));
        assert!(is_kicad_sch(b"\xEF\xBB\xBF(kicad_sch (version 1)"));
        assert_eq!(
            head_version(b"(kicad_pcb (version 20171130) (host"),
            Some(20171130)
        );
        assert_eq!(
            legacy_kind(b"(kicad_pcb (version 20171130) (host pcbnew 5)"),
            Some("KiCad 5 board")
        );
        assert_eq!(legacy_kind(b"(kicad_pcb (version 20211014)"), None);
        assert_eq!(
            legacy_kind(b"EESchema Schematic File Version 4\n"),
            Some("KiCad 5 schematic")
        );
        assert_eq!(
            legacy_kind(b"PCBNEW-BOARD Version 1"),
            Some("KiCad 4 board")
        );
        assert_eq!(board_release(20240108), "KiCad 8");
        assert_eq!(board_release(20210722), "KiCad 6");
        assert_eq!(schematic_release(20230819), "KiCad 8");
        assert_eq!(schematic_release(20250610), "KiCad 10");
    }
}
