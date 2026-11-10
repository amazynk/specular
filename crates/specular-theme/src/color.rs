/// An sRGB colour with straight alpha, components in 0..=1.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    pub const fn hex(v: u32) -> Color {
        Color {
            r: ((v >> 16) & 0xff) as f32 / 255.0,
            g: ((v >> 8) & 0xff) as f32 / 255.0,
            b: (v & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Parse `#RRGGBB` or `#RRGGBBAA`.
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim().strip_prefix('#')?;
        let v = u32::from_str_radix(s, 16).ok()?;
        match s.len() {
            6 => Some(Color::hex(v)),
            8 => Some(Color::hex(v >> 8).with_alpha((v & 0xff) as f32 / 255.0)),
            _ => None,
        }
    }

    pub const fn with_alpha(self, a: f32) -> Color {
        Color { a, ..self }
    }

    pub fn fade(self, f: f32) -> Color {
        Color {
            a: self.a * f,
            ..self
        }
    }

    /// Linear-light, premultiplied RGBA for the GPU.
    pub fn to_linear_premul(self) -> [f32; 4] {
        let l = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        [
            l(self.r) * self.a,
            l(self.g) * self.a,
            l(self.b) * self.a,
            self.a,
        ]
    }

    /// Relative luminance (for picking text contrast).
    pub fn luminance(self) -> f32 {
        let l = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * l(self.r) + 0.7152 * l(self.g) + 0.0722 * l(self.b)
    }

    /// Mix in sRGB space.
    pub fn lerp(self, o: Color, t: f32) -> Color {
        Color {
            r: self.r + (o.r - self.r) * t,
            g: self.g + (o.g - self.g) * t,
            b: self.b + (o.b - self.b) * t,
            a: self.a + (o.a - self.a) * t,
        }
    }
}
