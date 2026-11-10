//! Bundled fonts (SIL Open Font License, see the `fonts/` directory). CJK and other scripts
//! come from system fonts through fontique's fallback.

/// Inter (variable, Latin/Greek/Cyrillic): the interface font.
pub static INTER: &[u8] = include_bytes!("../fonts/InterVariable.ttf");
/// Noto Sans Arabic (variable).
pub static NOTO_SANS_ARABIC: &[u8] = include_bytes!("../fonts/NotoSansArabic-Variable.ttf");
/// Noto Sans Hebrew (variable).
pub static NOTO_SANS_HEBREW: &[u8] = include_bytes!("../fonts/NotoSansHebrew-Variable.ttf");

/// Family names as registered.
pub const UI_FAMILY: &str = "Inter Variable";
pub const ARABIC_FAMILY: &str = "Noto Sans Arabic";
pub const HEBREW_FAMILY: &str = "Noto Sans Hebrew";

/// All bundled font files.
pub fn all() -> [&'static [u8]; 3] {
    [INTER, NOTO_SANS_ARABIC, NOTO_SANS_HEBREW]
}
