//! The periodic table as far as drawing needs it: symbols, CPK colours (the Jmol palette),
//! covalent radii (Cordero et al. 2008, low-spin for the first-row transition metals) and
//! van der Waals radii (Bondi 1964, filled from Alvarez 2013; 2.0 Å where neither gives one).

/// Atomic number; 0 for an unknown element.
pub type Element = u8;

const SYMBOLS: [&str; 119] = [
    "X", "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S",
    "Cl", "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge",
    "As", "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd",
    "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd",
    "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg",
    "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm",
    "Bk", "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn",
    "Nh", "Fl", "Mc", "Lv", "Ts", "Og",
];

/// Jmol's CPK colours, 0xRRGGBB, to Lr; heavier elements use the last.
const CPK: [u32; 104] = [
    0xFF1493, 0xFFFFFF, 0xD9FFFF, 0xCC80FF, 0xC2FF00, 0xFFB5B5, 0x909090, 0x3050F8, 0xFF0D0D,
    0x90E050, 0xB3E3F5, 0xAB5CF2, 0x8AFF00, 0xBFA6A6, 0xF0C8A0, 0xFF8000, 0xFFFF30, 0x1FF01F,
    0x80D1E3, 0x8F40D4, 0x3DFF00, 0xE6E6E6, 0xBFC2C7, 0xA6A6AB, 0x8A99C7, 0x9C7AC7, 0xE06633,
    0xF090A0, 0x50D050, 0xC88033, 0x7D80B0, 0xC28F8F, 0x668F8F, 0xBD80E3, 0xFFA100, 0xA62929,
    0x5CB8D1, 0x702EB0, 0x00FF00, 0x94FFFF, 0x94E0E0, 0x73C2C9, 0x54B5B5, 0x3B9E9E, 0x248F8F,
    0x0A7D8C, 0x006985, 0xC0C0C0, 0xFFD98F, 0xA67573, 0x668080, 0x9E63B5, 0xD47A00, 0x940094,
    0x429EB0, 0x57178F, 0x00C900, 0x70D4FF, 0xFFFFC7, 0xD9FFC7, 0xC7FFC7, 0xA3FFC7, 0x8FFFC7,
    0x61FFC7, 0x45FFC7, 0x30FFC7, 0x1FFFC7, 0x00FF9C, 0x00E675, 0x00D452, 0x00BF38, 0x00AB24,
    0x4DC2FF, 0x4DA6FF, 0x2194D6, 0x267DAB, 0x266696, 0x175487, 0xD0D0E0, 0xFFD123, 0xB8B8D0,
    0xA6544D, 0x575961, 0x9E4FB5, 0xAB5C00, 0x754F45, 0x428296, 0x420066, 0x007D00, 0x70ABFA,
    0x00BAFF, 0x00A1FF, 0x008FFF, 0x0080FF, 0x006BFF, 0x545CF2, 0x785CE3, 0x8A4FE3, 0xA136D4,
    0xB31FD4, 0xB31FBA, 0xB30DA6, 0xBD0D87, 0xC70066,
];

/// Covalent radii in hundredths of an ångström, to Cm; heavier elements use 1.70 Å.
const COVALENT: [u16; 97] = [
    150, 31, 28, 128, 96, 84, 76, 71, 66, 57, 58, 166, 141, 121, 111, 107, 105, 102, 106, 203, 176,
    170, 160, 153, 139, 139, 132, 126, 124, 132, 122, 122, 120, 119, 120, 120, 116, 220, 195, 190,
    175, 164, 154, 147, 146, 142, 139, 145, 144, 142, 139, 139, 138, 139, 140, 244, 215, 207, 204,
    203, 201, 199, 198, 198, 196, 194, 192, 192, 189, 190, 187, 187, 175, 170, 162, 151, 144, 141,
    136, 136, 132, 145, 146, 148, 140, 150, 150, 260, 221, 215, 206, 200, 196, 190, 187, 180, 169,
];

/// Van der Waals radii in hundredths of an ångström, to U; 0 where neither source gives one.
const VDW: [u16; 93] = [
    0, 120, 140, 182, 153, 192, 170, 155, 152, 147, 154, 227, 173, 184, 210, 180, 180, 175, 188,
    275, 231, 211, 0, 0, 0, 0, 0, 0, 163, 140, 139, 187, 211, 185, 190, 185, 202, 303, 249, 0, 0,
    0, 0, 0, 0, 0, 163, 172, 158, 193, 217, 206, 206, 198, 216, 343, 268, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 175, 166, 155, 196, 202, 207, 197, 202, 220, 348, 283,
    0, 0, 0, 186,
];

/// The element's symbol ("X" for unknown).
pub fn symbol(e: Element) -> &'static str {
    SYMBOLS.get(usize::from(e)).copied().unwrap_or("X")
}

/// The element for a symbol, case-insensitively (D and T are hydrogen).
pub fn by_symbol(s: &[u8]) -> Option<Element> {
    let s = trim(s);
    match s.len() {
        1 | 2 => {}
        _ => return None,
    }
    let up0 = s[0].to_ascii_uppercase();
    let lo1 = s.get(1).map(u8::to_ascii_lowercase);
    if s.len() == 1 && (up0 == b'D' || up0 == b'T') {
        return Some(1);
    }
    SYMBOLS.iter().enumerate().skip(1).find_map(|(i, sym)| {
        let b = sym.as_bytes();
        (b[0] == up0 && b.get(1).copied() == lo1).then_some(i as u8)
    })
}

/// sRGB colour, opaque.
pub fn cpk(e: Element) -> [u8; 4] {
    let c = CPK[usize::from(e).min(CPK.len() - 1)];
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}

/// Covalent radius (Å).
pub fn covalent(e: Element) -> f32 {
    f32::from(COVALENT.get(usize::from(e)).copied().unwrap_or(170)) / 100.0
}

/// Van der Waals radius (Å).
pub fn vdw(e: Element) -> f32 {
    match VDW.get(usize::from(e)).copied().unwrap_or(0) {
        0 => 2.0,
        r => f32::from(r) / 100.0,
    }
}

pub fn is_metal(e: Element) -> bool {
    matches!(e, 3 | 4 | 11..=13 | 19..=32 | 37..=51 | 55..=84 | 87..=118)
}

pub(crate) fn trim(s: &[u8]) -> &[u8] {
    s.trim_ascii()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_round_trip() {
        for e in 1..=118u8 {
            assert_eq!(by_symbol(symbol(e).as_bytes()), Some(e), "{}", symbol(e));
        }
        assert_eq!(by_symbol(b"FE"), Some(26));
        assert_eq!(by_symbol(b" c"), Some(6));
        assert_eq!(by_symbol(b"D"), Some(1));
        assert_eq!(by_symbol(b"Qq"), None);
        assert_eq!(by_symbol(b""), None);
        assert_eq!(symbol(200), "X");
        assert!((covalent(6) - 0.76).abs() < 1e-6);
        assert!((vdw(6) - 1.70).abs() < 1e-6);
        assert!((vdw(26) - 2.0).abs() < 1e-6);
        assert_eq!(cpk(8), [255, 13, 13, 255]);
        assert!(is_metal(30) && !is_metal(6));
    }
}
