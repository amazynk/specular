//! Channel isolation (DESIGN.md §6.2): which single channel the viewer shows, in grey.
//!
//! The image shader does the work; this is the state machine behind `Shift+1…5` and the
//! code the shader switches on. Colour channels are shown unpremultiplied and after the
//! current exposure and tone mapping, so the grey is the channel as it is displayed; the
//! colour under fully transparent pixels shows too (the GPU keeps straight colour).

/// What the image view shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChannelView {
    /// The image as usual.
    #[default]
    Normal,
    Red,
    Green,
    Blue,
    /// Alpha as grey: black is transparent, white opaque.
    Alpha,
    /// The image over a high-contrast checkerboard, to judge the alpha edges.
    AlphaChecker,
    /// The image over a solid matte colour.
    AlphaMatte,
    /// Relative luminance.
    Luma,
}

/// A channel key (`Shift+1…5`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelKey {
    Red,
    Green,
    Blue,
    Alpha,
    Luma,
}

impl ChannelView {
    /// The view after pressing `key`. Pressing the key of the view on screen goes back to
    /// the normal view; the alpha key steps grey → checkerboard → matte → normal.
    pub fn press(self, key: ChannelKey) -> Self {
        use ChannelView as V;
        match (key, self) {
            (ChannelKey::Red, V::Red)
            | (ChannelKey::Green, V::Green)
            | (ChannelKey::Blue, V::Blue)
            | (ChannelKey::Luma, V::Luma)
            | (ChannelKey::Alpha, V::AlphaMatte) => V::Normal,
            (ChannelKey::Alpha, V::Alpha) => V::AlphaChecker,
            (ChannelKey::Alpha, V::AlphaChecker) => V::AlphaMatte,
            (ChannelKey::Alpha, _) => V::Alpha,
            (ChannelKey::Red, _) => V::Red,
            (ChannelKey::Green, _) => V::Green,
            (ChannelKey::Blue, _) => V::Blue,
            (ChannelKey::Luma, _) => V::Luma,
        }
    }

    /// Whether this is one of the alpha views.
    pub fn is_alpha(self) -> bool {
        matches!(self, Self::Alpha | Self::AlphaChecker | Self::AlphaMatte)
    }

    /// The view actually drawn for an image with or without an alpha channel: the alpha
    /// views have nothing to show on an opaque image, so they fall back to the normal view
    /// (the choice is kept for the next image that has alpha).
    pub fn effective(self, has_alpha: bool) -> Self {
        if self.is_alpha() && !has_alpha {
            Self::Normal
        } else {
            self
        }
    }

    /// The mode the image shader switches on (`Img.chan.x`): 0 shows the composited image
    /// (also for the checkerboard and matte views, which only change the backdrop), 1‥3 a
    /// colour channel, 4 alpha, 5 luminance.
    pub fn shader_mode(self) -> u32 {
        match self {
            Self::Normal | Self::AlphaChecker | Self::AlphaMatte => 0,
            Self::Red => 1,
            Self::Green => 2,
            Self::Blue => 3,
            Self::Alpha => 4,
            Self::Luma => 5,
        }
    }

    /// Reference for the shader: the grey level (linear) that `shader_mode` shows for a
    /// straight (unpremultiplied) linear BT.709 pixel. Colour shows whatever the alpha,
    /// also under fully transparent pixels. `None` for the composited views.
    pub fn grey(self, px: [f32; 4]) -> Option<f32> {
        let (rgb, a) = ([px[0], px[1], px[2]], px[3]);
        let v = match self.shader_mode() {
            1 => rgb[0],
            2 => rgb[1],
            3 => rgb[2],
            4 => a,
            5 => crate::luma(rgb),
            _ => return None,
        };
        Some(v.max(0.0))
    }
}

/// How the colour channels of a multichannel image (a composite of fluorescence or other
/// channels stored as red, green and blue) are coloured on screen (DESIGN.md §6.2). Red–green
/// is the stored colour; the magenta presets keep the channels apart for red–green colour
/// vision deficiencies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChannelColours {
    /// Red, green and blue as stored.
    #[default]
    RedGreen,
    /// Two-channel overlays: the first channel magenta, the second green (a third channel is
    /// not shown; magenta–yellow–cyan is the preset for three).
    MagentaGreen,
    /// Three channels in magenta, yellow and cyan, at half intensity each so that all three
    /// at full make white.
    MagentaYellowCyan,
}

impl ChannelColours {
    pub const ALL: [ChannelColours; 3] =
        [Self::RedGreen, Self::MagentaGreen, Self::MagentaYellowCyan];

    pub fn id(self) -> &'static str {
        match self {
            Self::RedGreen => "red-green",
            Self::MagentaGreen => "magenta-green",
            Self::MagentaYellowCyan => "magenta-yellow-cyan",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == id)
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&c| c == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// The display colour (linear) of each stored channel: `out = Σ ch_i · colours[i]`.
    pub fn colours(self) -> [[f32; 3]; 3] {
        match self {
            Self::RedGreen => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Self::MagentaGreen => [[1.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]],
            Self::MagentaYellowCyan => [[0.5, 0.0, 0.5], [0.5, 0.5, 0.0], [0.0, 0.5, 0.5]],
        }
    }

    /// The matrix (rows) the image shader applies to straight linear RGB; `None` for the
    /// stored colours.
    pub fn matrix(self) -> Option<[[f32; 3]; 3]> {
        if self == Self::RedGreen {
            return None;
        }
        let c = self.colours();
        Some([0, 1, 2].map(|row| [c[0][row], c[1][row], c[2][row]]))
    }

    /// Reference for the shader.
    pub fn apply(self, rgb: [f32; 3]) -> [f32; 3] {
        match self.matrix() {
            None => rgb,
            Some(m) => m.map(|r| r[0] * rgb[0] + r[1] * rgb[1] + r[2] * rgb[2]),
        }
    }
}
