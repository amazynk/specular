pub(super) fn gamut_map(
    r: &mut [f32],
    g: &mut [f32],
    b: &mut [f32],
    luminances: [f32; 3],
    saturation_factor: f32,
) {
    assert_eq!(r.len(), g.len());
    assert_eq!(g.len(), b.len());

    // generic
    for ((r, g), b) in r.iter_mut().zip(g).zip(b) {
        let mapped =
            crate::color::gamut::map_gamut_generic([*r, *g, *b], luminances, saturation_factor);
        *r = mapped[0];
        *g = mapped[1];
        *b = mapped[2];
    }
}
