//! Design tokens: colours for the dark and light themes, sizes, motion (springs and easing)
//! and the bundled fonts. Everything visual that is not an image comes from here.

pub mod color;
pub mod fonts;
pub mod motion;

pub use color::Color;
pub use motion::{Spring, SpringParams};

/// Light or dark appearance of the chrome. Images are never altered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    #[default]
    Dark,
    Light,
}

/// Colour tokens. All colours are sRGB with straight alpha; the renderer converts them.
#[derive(Clone, Debug)]
pub struct Palette {
    pub mode: Mode,
    /// Window background behind the image.
    pub canvas: Color,
    /// Floating chrome (toolbar, panels): drawn over a blurred copy of the image.
    pub glass: Color,
    pub glass_border: Color,
    /// Opaque surfaces (settings window, palette list).
    pub surface: Color,
    pub surface_raised: Color,
    /// Navigation column of a sheet (the Settings sections). Opaque, so its contrast with
    /// text doesn't depend on how translucent colours blend.
    pub sidebar: Color,
    pub surface_hover: Color,
    pub surface_pressed: Color,
    pub separator: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_faint: Color,
    /// The accent as text, marks and rings on the surfaces (AA on each, hovered too).
    pub accent: Color,
    /// The accent filled behind `accent_text`: default buttons, ticked boxes, badges.
    pub accent_fill: Color,
    /// Text and marks on `accent_fill` (and on `danger_fill`).
    pub accent_text: Color,
    pub accent_soft: Color,
    /// Errors and destructive actions as text on the surfaces.
    pub danger: Color,
    /// A destructive button's fill, behind `accent_text`.
    pub danger_fill: Color,
    /// Good news as text and marks: available, chosen, the normal state.
    pub success: Color,
    /// Worth knowing as text: not measured yet, on battery.
    pub warning: Color,
    pub focus_ring: Color,
    pub shadow: Color,
    /// Dims the image behind modal sheets.
    pub scrim: Color,
    pub tooltip_bg: Color,
    pub tooltip_text: Color,
    pub checker_a: Color,
    pub checker_b: Color,
    pub selection: Color,
    pub hud_text: Color,
    /// The settings.toml editor.
    pub syntax: Syntax,
}

/// Colours of the built-in text editor (settings.toml): the page, its gutter, and one
/// colour per kind of token. Every token colour reads at WCAG AA (4.5:1) against the page
/// and the current-line highlight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Syntax {
    /// The page behind the text.
    pub background: Color,
    pub gutter: Color,
    pub line_number: Color,
    pub line_number_current: Color,
    /// Drawn over the background on the caret's line.
    pub current_line: Color,
    /// Behind both of a matching pair of brackets or quotes.
    pub bracket_match: Color,
    /// Behind find matches (the current one uses `selection`).
    pub find_match: Color,
    /// Plain text and anything the lexer does not recognise.
    pub text: Color,
    /// `[table]` and `[[array.of.tables]]` headers.
    pub table: Color,
    pub key: Color,
    pub string: Color,
    /// `\n`, `\u00e9` inside basic strings.
    pub escape: Color,
    pub number: Color,
    pub boolean: Color,
    pub datetime: Color,
    pub comment: Color,
    /// `=`, `,`, `.`, brackets and braces.
    pub punctuation: Color,
    /// Squiggles and gutter marks.
    pub error: Color,
    pub warning: Color,
    pub info: Color,
}

impl Syntax {
    pub fn dark() -> Syntax {
        Syntax {
            background: Color::hex(0x19191c),
            gutter: Color::hex(0x151518),
            line_number: Color::hex(0x8c8c95),
            line_number_current: Color::hex(0xd6d6dc),
            current_line: Color::rgba(1.0, 1.0, 1.0, 0.045),
            bracket_match: Color::hex(0x5b8cff).with_alpha(0.16),
            find_match: Color::hex(0xffc857).with_alpha(0.26),
            text: Color::hex(0xe6e6ea),
            table: Color::hex(0xc9a8ff),
            key: Color::hex(0x7cc4ff),
            string: Color::hex(0xa5de8c),
            escape: Color::hex(0xffd37a),
            number: Color::hex(0xffab70),
            boolean: Color::hex(0xff8fbd),
            datetime: Color::hex(0x6fe0d2),
            comment: Color::hex(0x9c9ca6),
            punctuation: Color::hex(0xa9a9b2),
            error: Color::hex(0xff6b5e),
            warning: Color::hex(0xf5b84a),
            info: Color::hex(0x7cc4ff),
        }
    }

    pub fn light() -> Syntax {
        Syntax {
            background: Color::hex(0xffffff),
            gutter: Color::hex(0xf4f4f7),
            line_number: Color::hex(0x6e6e78),
            line_number_current: Color::hex(0x1b1b1f),
            current_line: Color::rgba(0.0, 0.0, 0.0, 0.035),
            bracket_match: Color::hex(0x2f6bf2).with_alpha(0.12),
            find_match: Color::hex(0xffb000).with_alpha(0.30),
            text: Color::hex(0x1b1b1f),
            table: Color::hex(0x7339c9),
            key: Color::hex(0x0b5fb3),
            string: Color::hex(0x2c7a1c),
            escape: Color::hex(0x925400),
            number: Color::hex(0xb0440a),
            boolean: Color::hex(0xbd2468),
            datetime: Color::hex(0x00776a),
            comment: Color::hex(0x67676f),
            punctuation: Color::hex(0x55555e),
            error: Color::hex(0xd93b2e),
            warning: Color::hex(0xa86400),
            info: Color::hex(0x2f6bf2),
        }
    }

    /// The token colours, for contrast checks.
    pub fn tokens(&self) -> [(&'static str, Color); 10] {
        [
            ("text", self.text),
            ("table", self.table),
            ("key", self.key),
            ("string", self.string),
            ("escape", self.escape),
            ("number", self.number),
            ("boolean", self.boolean),
            ("datetime", self.datetime),
            ("comment", self.comment),
            ("punctuation", self.punctuation),
        ]
    }
}

/// Side of a transparency checkerboard cell in logical px: the viewer and the gallery's
/// thumbnails draw the same screen-fixed pattern.
pub const CHECKER_CELL: f32 = 8.0;

impl Palette {
    pub fn dark() -> Palette {
        Palette {
            mode: Mode::Dark,
            canvas: Color::hex(0x141414),
            glass: Color::hex(0x26262a).with_alpha(0.72),
            glass_border: Color::rgba(1.0, 1.0, 1.0, 0.10),
            surface: Color::hex(0x1f1f22),
            surface_raised: Color::hex(0x2a2a2e),
            sidebar: Color::hex(0x18181b),
            surface_hover: Color::rgba(1.0, 1.0, 1.0, 0.08),
            surface_pressed: Color::rgba(1.0, 1.0, 1.0, 0.14),
            separator: Color::rgba(1.0, 1.0, 1.0, 0.09),
            text: Color::hex(0xf2f2f4),
            text_muted: Color::hex(0xacacb3),
            text_faint: Color::hex(0x76767e),
            accent: Color::hex(0x7ea6ff),
            accent_fill: Color::hex(0x305ecc),
            accent_text: Color::hex(0xffffff),
            accent_soft: Color::hex(0x5b8cff).with_alpha(0.22),
            danger: Color::hex(0xff8478),
            danger_fill: Color::hex(0xc0362c),
            success: Color::hex(0x5cc98a),
            warning: Color::hex(0xf5b84a),
            focus_ring: Color::hex(0x8fb0ff),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.45),
            scrim: Color::rgba(0.0, 0.0, 0.0, 0.40),
            tooltip_bg: Color::hex(0x0e0e10).with_alpha(0.94),
            tooltip_text: Color::hex(0xf2f2f4),
            checker_a: Color::hex(0x3a3a3e),
            checker_b: Color::hex(0x2c2c30),
            selection: Color::hex(0x5b8cff).with_alpha(0.30),
            hud_text: Color::hex(0xc8f7c5),
            syntax: Syntax::dark(),
        }
    }

    pub fn light() -> Palette {
        Palette {
            mode: Mode::Light,
            canvas: Color::hex(0xececee),
            glass: Color::hex(0xfafafc).with_alpha(0.74),
            glass_border: Color::rgba(0.0, 0.0, 0.0, 0.10),
            surface: Color::hex(0xf7f7f9),
            surface_raised: Color::hex(0xffffff),
            sidebar: Color::hex(0xececf0),
            surface_hover: Color::rgba(0.0, 0.0, 0.0, 0.06),
            surface_pressed: Color::rgba(0.0, 0.0, 0.0, 0.11),
            separator: Color::rgba(0.0, 0.0, 0.0, 0.09),
            text: Color::hex(0x1b1b1f),
            text_muted: Color::hex(0x5d5d66),
            text_faint: Color::hex(0x8e8e96),
            accent: Color::hex(0x2050be),
            accent_fill: Color::hex(0x2050be),
            accent_text: Color::hex(0xffffff),
            accent_soft: Color::hex(0x2f6bf2).with_alpha(0.16),
            danger: Color::hex(0xb02a22),
            danger_fill: Color::hex(0xb02a22),
            success: Color::hex(0x17692f),
            warning: Color::hex(0x8a5000),
            focus_ring: Color::hex(0x2f6bf2),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.22),
            scrim: Color::rgba(0.0, 0.0, 0.0, 0.25),
            tooltip_bg: Color::hex(0x1f1f23).with_alpha(0.94),
            tooltip_text: Color::hex(0xf5f5f7),
            checker_a: Color::hex(0xffffff),
            checker_b: Color::hex(0xe3e3e6),
            selection: Color::hex(0x2f6bf2).with_alpha(0.22),
            hud_text: Color::hex(0x0a5d12),
            syntax: Syntax::light(),
        }
    }

    pub fn for_mode(mode: Mode) -> Palette {
        match mode {
            Mode::Dark => Palette::dark(),
            Mode::Light => Palette::light(),
        }
    }
}

/// Sizes in logical pixels.
pub mod metrics {
    pub const RADIUS_SM: f32 = 6.0;
    pub const RADIUS_MD: f32 = 10.0;
    pub const RADIUS_LG: f32 = 14.0;
    pub const TOOLBAR_HEIGHT: f32 = 46.0;
    pub const TOOLBAR_MARGIN: f32 = 16.0;
    pub const BUTTON: f32 = 34.0;
    pub const ICON: f32 = 18.0;
    pub const ICON_STROKE: f32 = 1.6;
    pub const GAP: f32 = 8.0;
    pub const PAD: f32 = 12.0;
    pub const PANEL_WIDTH: f32 = 340.0;
    pub const EDGE_ARROW: f32 = 44.0;
    pub const SHADOW_BLUR: f32 = 24.0;
    pub const SHADOW_Y: f32 = 8.0;

    pub const FONT_SMALL: f32 = 12.0;
    pub const FONT_BODY: f32 = 13.5;
    pub const FONT_LABEL: f32 = 13.0;
    pub const FONT_TITLE: f32 = 16.0;
    pub const FONT_LARGE: f32 = 20.0;
    pub const FONT_MONO: f32 = 12.0;
}

/// Durations in seconds.
pub mod durations {
    pub const TOOLTIP_DELAY: f32 = 0.55;
    pub const TOOLBAR_HIDE: f32 = 1.6;
    pub const FADE: f32 = 0.16;
    pub const TOAST: f32 = 5.0;
    pub const HQ_SETTLE: f32 = 0.10;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG contrast ratio of two opaque colours.
    fn contrast(a: Color, b: Color) -> f32 {
        let (la, lb) = (a.luminance(), b.luminance());
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Source-over in sRGB (how the highlight is drawn over the page).
    fn over(top: Color, bottom: Color) -> Color {
        bottom.lerp(top.with_alpha(1.0), top.a).with_alpha(1.0)
    }

    /// Status text (the hardware report's values, the chosen path's uses) reads at AA on the
    /// sheet, on its cards and on the chosen path's accent band.
    #[test]
    fn status_colours_meet_aa() {
        for p in [Palette::dark(), Palette::light()] {
            let pages = [
                ("surface", p.surface),
                ("surface_raised", p.surface_raised),
                ("accent band", over(p.accent_soft, p.surface_raised)),
            ];
            for (name, c) in [("success", p.success), ("warning", p.warning)] {
                for (page, bg) in pages {
                    let r = contrast(c, bg);
                    assert!(r >= 4.5, "{:?} {name} on {page}: {r:.2}", p.mode);
                }
            }
        }
    }

    #[test]
    fn syntax_colours_meet_aa() {
        for p in [Palette::dark(), Palette::light()] {
            let s = p.syntax;
            let pages = [
                ("background", s.background),
                ("current line", over(s.current_line, s.background)),
                ("bracket match", over(s.bracket_match, s.background)),
            ];
            for (name, c) in s.tokens() {
                for (page, bg) in pages {
                    let r = contrast(c, bg);
                    assert!(r >= 4.5, "{:?} {name} on {page}: {r:.2}", p.mode);
                }
            }
            // Squiggles are 3:1 like other non-text marks.
            for c in [s.error, s.warning, s.info] {
                assert!(contrast(c, s.background) >= 3.0, "{:?} {c:?}", p.mode);
            }
            assert!(contrast(s.line_number_current, s.gutter) >= 4.5);
            assert!(contrast(s.line_number, s.gutter) >= 4.5);
        }
    }
}
