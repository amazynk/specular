//! Colour signalling of a decoded HEIC image and the YCbCr → RGB
//! defaults derived from it.
//!
//! Three sources can describe an item's colour: the container `nclx`
//! (`colr` property), an ICC profile (`colr` `prof` / `rICC`), and the
//! HEVC SPS VUI `video_signal_type`. [`Colour`] carries all three
//! unresolved so the caller's colour layer decides; [`Colour::ycbcr`]
//! and [`Colour::cicp`] give the defaults a reader without one uses,
//! matching libheif's precedence: the `nclx` wins, else the VUI, else
//! H.273 "unspecified" (BT.601 matrix, limited range — the VUI's own
//! inference when `video_signal_type` is absent).

use heif::Colr;

/// CICP code points (ISO/IEC 23091-2 / ITU-T H.273).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cicp {
    /// `colour_primaries`.
    pub primaries: u16,
    /// `transfer_characteristics`.
    pub transfer: u16,
    /// `matrix_coefficients`.
    pub matrix: u16,
    /// `full_range_flag` / `video_full_range_flag`.
    pub full_range: bool,
}

/// The HEVC SPS VUI colour signalling of the decoded picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VuiColour {
    /// `video_full_range_flag`.
    pub full_range: bool,
    /// `(colour_primaries, transfer_characteristics, matrix_coeffs)`
    /// when `colour_description_present_flag` is set.
    pub description: Option<(u8, u8, u8)>,
}

impl VuiColour {
    pub(crate) fn from_hevc(s: &hevc::Signal) -> Self {
        Self {
            full_range: s.full_range,
            description: s.colour,
        }
    }

    pub(crate) fn from_signal(s: &h265::VideoSignalType) -> Self {
        Self {
            full_range: s.video_full_range_flag,
            description: s.colour_description.map(|d| {
                (
                    d.colour_primaries,
                    d.transfer_characteristics,
                    d.matrix_coeffs,
                )
            }),
        }
    }
}

/// How YCbCr samples become R'G'B'.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ycbcr {
    /// H.273 `matrix_coefficients` (0 = identity / GBR; 2 is never
    /// returned — unspecified resolves to 6, BT.601).
    pub matrix: u16,
    /// Full-range samples (else limited / "video" range).
    pub full_range: bool,
}

/// Everything the file and the bitstream say about an image's colour.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Colour {
    /// The item's `nclx` `colr` property, when present.
    pub nclx: Option<Cicp>,
    /// The item's ICC profile (`prof` / `rICC` `colr`), when present.
    pub icc: Option<Vec<u8>>,
    /// The coded picture's VUI signalling, when present (a grid takes
    /// its first tile's).
    pub vui: Option<VuiColour>,
}

/// H.273 "unspecified" (2) and the reserved / unconvertible code
/// points resolve to BT.601 (6), like libheif.
fn resolve_matrix(m: u16) -> u16 {
    match m {
        0 | 1 | 4 | 5 | 6 | 7 | 9 => m,
        _ => 6,
    }
}

impl Colour {
    /// The samples are R, G and B (an RGB-coded JPEG or `unci` item, stored as G / B / R
    /// planes): the identity matrix at full range, whatever the `nclx` matrix says; its
    /// primaries and transfer stay.
    pub fn mark_rgb(&mut self) {
        if let Some(n) = &mut self.nclx {
            n.matrix = 0;
            n.full_range = true;
        }
        self.vui = Some(VuiColour {
            full_range: true,
            description: Some((2, 2, 0)),
        });
    }

    /// The YCbCr → R'G'B' parameters: `nclx` matrix and range when the
    /// item has an `nclx`, else the VUI's, else BT.601 limited range.
    /// An `nclx` with an unspecified matrix (2) takes the VUI's matrix.
    pub fn ycbcr(&self) -> Ycbcr {
        let vui_matrix = self
            .vui
            .and_then(|v| v.description)
            .map(|(_, _, m)| m as u16);
        match (self.nclx, self.vui) {
            (Some(n), _) => {
                let m = if n.matrix == 2 {
                    vui_matrix.unwrap_or(2)
                } else {
                    n.matrix
                };
                Ycbcr {
                    matrix: resolve_matrix(m),
                    full_range: n.full_range,
                }
            }
            (None, Some(v)) => Ycbcr {
                matrix: resolve_matrix(vui_matrix.unwrap_or(2)),
                full_range: v.full_range,
            },
            (None, None) => Ycbcr {
                matrix: 6,
                full_range: false,
            },
        }
    }

    /// The effective CICP description for colour management: the
    /// `nclx`, else the VUI colour description, with "unspecified"
    /// primaries / transfer resolved to BT.709 / sRGB (what libheif and
    /// browsers assume for HEIC). `None` when an ICC profile is present
    /// and no `nclx` is (the profile governs).
    pub fn cicp(&self) -> Option<Cicp> {
        if self.nclx.is_none() && self.icc.is_some() {
            return None;
        }
        let y = self.ycbcr();
        let (p, t) = match (self.nclx, self.vui.and_then(|v| v.description)) {
            (Some(n), _) => (n.primaries, n.transfer),
            (None, Some((p, t, _))) => (p as u16, t as u16),
            (None, None) => (2, 2),
        };
        Some(Cicp {
            primaries: if p == 2 { 1 } else { p },
            transfer: if t == 2 { 13 } else { t },
            matrix: y.matrix,
            full_range: y.full_range,
        })
    }

    /// The `colr` value the reference YCbCr → RGB kernel
    /// ([`heif::rgb`]) is driven with.
    pub(crate) fn kernel_colr(&self) -> Colr {
        let y = self.ycbcr();
        Colr::Nclx {
            primaries: 1,
            transfer: 13,
            matrix: y.matrix,
            full_range: y.full_range,
        }
    }

    pub(crate) fn from_props(props: &heif::ItemProperties) -> Self {
        let nclx = props.nclx().and_then(|c| match c {
            Colr::Nclx {
                primaries,
                transfer,
                matrix,
                full_range,
            } => Some(Cicp {
                primaries: *primaries,
                transfer: *transfer,
                matrix: *matrix,
                full_range: *full_range,
            }),
            _ => None,
        });
        Self {
            nclx,
            icc: props.icc_profile().map(<[u8]>::to_vec),
            vui: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nclx(matrix: u16, full_range: bool) -> Option<Cicp> {
        Some(Cicp {
            primaries: 1,
            transfer: 13,
            matrix,
            full_range,
        })
    }

    #[test]
    fn precedence() {
        let vui = Some(VuiColour {
            full_range: true,
            description: Some((9, 16, 9)),
        });
        // nclx wins.
        let c = Colour {
            nclx: nclx(1, false),
            icc: None,
            vui,
        };
        assert_eq!(
            c.ycbcr(),
            Ycbcr {
                matrix: 1,
                full_range: false
            }
        );
        // nclx with unspecified matrix takes the VUI's.
        let c = Colour {
            nclx: nclx(2, true),
            icc: None,
            vui,
        };
        assert_eq!(c.ycbcr().matrix, 9);
        // VUI only.
        let c = Colour {
            nclx: None,
            icc: None,
            vui,
        };
        assert_eq!(
            c.ycbcr(),
            Ycbcr {
                matrix: 9,
                full_range: true
            }
        );
        assert_eq!(c.cicp().unwrap().transfer, 16);
        // VUI range without description: unspecified matrix -> 601.
        let c = Colour {
            nclx: None,
            icc: None,
            vui: Some(VuiColour {
                full_range: true,
                description: None,
            }),
        };
        assert_eq!(
            c.ycbcr(),
            Ycbcr {
                matrix: 6,
                full_range: true
            }
        );
        // Nothing: BT.601 limited.
        let c = Colour::default();
        assert_eq!(
            c.ycbcr(),
            Ycbcr {
                matrix: 6,
                full_range: false
            }
        );
        // Identity (lossless GBR) from the VUI.
        let c = Colour {
            nclx: None,
            icc: Some(vec![1]),
            vui: Some(VuiColour {
                full_range: true,
                description: Some((1, 13, 0)),
            }),
        };
        assert_eq!(c.ycbcr().matrix, 0);
        assert!(c.cicp().is_none(), "ICC governs");
    }
}
