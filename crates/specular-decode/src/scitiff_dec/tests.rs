use super::*;

#[test]
fn readable_lengths() {
    assert_eq!(readable((2.5e-9, 2.5e-9)).1, "nm");
    let ((x, _), u) = readable((4e-7, 4e-7));
    assert_eq!(u, "nm");
    assert!((x - 400.0).abs() < 1e-9);
    assert_eq!(readable((1.5e-6, 1.5e-6)).1, "µm");
}

#[test]
fn grey_and_ramp_tables() {
    let mut t: ColourTable = [[0; 3]; 256];
    for (i, e) in t.iter_mut().enumerate() {
        *e = [i as u8; 3];
    }
    assert!(is_grey(&t));
    for (i, e) in t.iter_mut().enumerate() {
        *e = [0, i as u8, 0];
    }
    assert!(!is_grey(&t));
    assert_eq!(lut_colour(&t), [0.0, 1.0, 0.0]);
}
