//! Colour filter array description.

/// Colour of a CFA cell.
pub const RED: u8 = 0;
pub const GREEN: u8 = 1;
pub const BLUE: u8 = 2;

/// Repeating colour filter pattern, `colors[row * width + col]` with 0 = red, 1 = green,
/// 2 = blue (other values: cyan, magenta, yellow, ... as in the DNG `CFAPlaneColor` space,
/// unsupported by the demosaicer). The pattern is anchored at the top-left of the *stored* raw
/// image.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cfa {
    pub width: u32,
    pub height: u32,
    pub colors: Vec<u8>,
}

impl Cfa {
    pub fn new(width: u32, height: u32, colors: Vec<u8>) -> Option<Cfa> {
        (width >= 1
            && height >= 1
            && width <= 16
            && height <= 16
            && colors.len() == (width * height) as usize)
            .then_some(Cfa {
                width,
                height,
                colors,
            })
    }

    /// A 2x2 Bayer pattern from its name, e.g. `"RGGB"`.
    pub fn bayer(name: &str) -> Option<Cfa> {
        let c: Vec<u8> = name
            .bytes()
            .map(|b| match b {
                b'R' => Some(RED),
                b'G' => Some(GREEN),
                b'B' => Some(BLUE),
                _ => None,
            })
            .collect::<Option<_>>()?;
        Cfa::new(2, 2, c)
    }

    pub fn color(&self, row: u32, col: u32) -> u8 {
        self.colors[((row % self.height) * self.width + col % self.width) as usize]
    }

    /// The pattern as seen from an image whose origin is `(x, y)` in this pattern's image.
    pub fn shifted(&self, x: u32, y: u32) -> Cfa {
        let mut colors = Vec::with_capacity(self.colors.len());
        for r in 0..self.height {
            for c in 0..self.width {
                colors.push(self.color(r + y, c + x));
            }
        }
        Cfa {
            width: self.width,
            height: self.height,
            colors,
        }
    }

    /// For a 2x2 Bayer pattern (one red, two greens, one blue): the colours at the four
    /// positions `(y & 1) * 2 + (x & 1)`. 1x1/2x1/1x2 patterns are rejected.
    pub fn bayer2x2(&self) -> Option<[u8; 4]> {
        let p = if self.width == 2 && self.height == 2 {
            [
                self.colors[0],
                self.colors[1],
                self.colors[2],
                self.colors[3],
            ]
        } else if self.width.is_multiple_of(2) && self.height.is_multiple_of(2) {
            // Larger repeat that is really 2x2 (some DNGs write 4x4 or 2x2 repeated).
            let p = [
                self.color(0, 0),
                self.color(0, 1),
                self.color(1, 0),
                self.color(1, 1),
            ];
            for r in 0..self.height {
                for c in 0..self.width {
                    if self.color(r, c) != p[((r & 1) * 2 + (c & 1)) as usize] {
                        return None;
                    }
                }
            }
            p
        } else {
            return None;
        };
        let mut n = [0u8; 3];
        for &c in &p {
            *n.get_mut(c as usize)? += 1;
        }
        // Greens must sit on a diagonal for the demosaicer.
        let diag = (p[0] == GREEN && p[3] == GREEN) || (p[1] == GREEN && p[2] == GREEN);
        (n == [1, 2, 1] && diag).then_some(p)
    }

    /// Whether [`crate::develop`] can demosaic this pattern: 2x2 Bayer, or an RGB pattern such
    /// as X-Trans whose every 3x3 window holds red, green and blue.
    pub fn developable(&self) -> bool {
        self.bayer2x2().is_some() || crate::xtrans::supported(self)
    }

    /// Short name such as `"RGGB"` (first 2x2 cells for larger patterns).
    pub fn name(&self) -> String {
        let mut s = String::new();
        for i in 0..self.colors.len().min(4) {
            let (r, c) = (i as u32 / 2, i as u32 % 2);
            s.push(match self.color(r, c) {
                RED => 'R',
                GREEN => 'G',
                BLUE => 'B',
                _ => '?',
            });
        }
        if !(self.width == 2 && self.height == 2) {
            s.push_str(&format!(" ({}x{})", self.width, self.height));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayer_shift() {
        let c = Cfa::bayer("RGGB").unwrap();
        assert_eq!(c.shifted(1, 0).name(), "GRBG");
        assert_eq!(c.shifted(0, 1).name(), "GBRG");
        assert_eq!(c.shifted(1, 1).name(), "BGGR");
        assert_eq!(c.bayer2x2(), Some([RED, GREEN, GREEN, BLUE]));
        assert!(Cfa::bayer("RGBG").unwrap().bayer2x2().is_none());
        assert!(Cfa::bayer("RRGB").is_some_and(|c| c.bayer2x2().is_none()));
    }
}
