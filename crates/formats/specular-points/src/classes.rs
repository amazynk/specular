//! ASPRS point classes: colours for the classification view and the standard classes' keys.

/// The colour (sRGB) of each class: the standard classes (0–22 of LAS 1.4 R15) by convention
/// (ground brown, vegetation greens, buildings red, water blue, noise magenta), the others
/// spread around the hue circle.
pub const CLASS_COLOURS: [[u8; 3]; 256] = {
    let mut t = [[0u8; 3]; 256];
    let fixed: [[u8; 3]; 23] = [
        [150, 150, 150], // 0 created, never classified
        [205, 205, 205], // 1 unclassified
        [166, 118, 62],  // 2 ground
        [178, 223, 138], // 3 low vegetation
        [92, 178, 72],   // 4 medium vegetation
        [27, 110, 42],   // 5 high vegetation
        [214, 72, 56],   // 6 building
        [255, 0, 255],   // 7 low point (noise)
        [255, 200, 0],   // 8 model key point (reserved in 1.4)
        [38, 110, 222],  // 9 water
        [124, 64, 150],  // 10 rail
        [92, 92, 100],   // 11 road surface
        [255, 170, 200], // 12 overlap (reserved in 1.4)
        [245, 245, 120], // 13 wire guard
        [240, 225, 30],  // 14 wire conductor
        [200, 135, 40],  // 15 transmission tower
        [230, 180, 90],  // 16 wire connector
        [110, 150, 175], // 17 bridge deck
        [255, 60, 200],  // 18 high noise
        [180, 168, 150], // 19 overhead structure
        [140, 108, 76],  // 20 ignored ground
        [235, 245, 255], // 21 snow
        [150, 150, 210], // 22 temporal exclusion
    ];
    let mut i = 0;
    while i < 256 {
        if i < 23 {
            t[i] = fixed[i];
        } else {
            // Golden-angle hues at medium lightness.
            let h = (i as u32 * 137) % 360;
            let sector = h / 60;
            let f = (h % 60) * 255 / 60;
            let (hi, lo) = (220u32, 70u32);
            let up = lo + (hi - lo) * f / 255;
            let down = hi - (hi - lo) * f / 255;
            let (r, g, b) = match sector {
                0 => (hi, up, lo),
                1 => (down, hi, lo),
                2 => (lo, hi, up),
                3 => (lo, down, hi),
                4 => (up, lo, hi),
                _ => (hi, lo, down),
            };
            t[i] = [r as u8, g as u8, b as u8];
        }
        i += 1;
    }
    t
};

/// A class's colour.
pub fn class_colour(class: u8) -> [u8; 3] {
    CLASS_COLOURS[usize::from(class)]
}

/// The standard classes with a name (0–22): the app shows them by name.
pub fn is_standard_class(class: u8) -> bool {
    class <= 22
}
