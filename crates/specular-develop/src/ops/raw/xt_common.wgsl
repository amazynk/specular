// Raw CA helpers shared by the `xtg` and `ca` passes: the mosaic (source A) and X-Trans
// colour lookups.
fn mo(x: i32, y: i32) -> f32 {
    let r = refl(y, i32(uu(U_H)));
    return srca[u32(r - ui(U_A_Y0)) * uu(U_W) + u32(refl(x, i32(uu(U_W))))];
}
// 6x6 patterns: the packed colours of H_XT6 (loaded once per pixel), looked up without
// divisions by the pattern's size; other patterns through ccolor.
var<private> XT: vec3<u32>;
var<private> XT_ON: bool;
fn xt_load() {
    XT_ON = du(H_XT6_ON) == 1u;
    XT = vec3<u32>(du(H_XT6), du(H_XT6 + 1u), du(H_XT6 + 2u));
}
// Colour at domain (x, y), already reflected into the image.
fn xc(x: i32, y: i32) -> u32 {
    if (!XT_ON) { return ccolor(x, y); }
    let rx = u32(x) % 6u;
    let ry = u32(y) % 6u;
    let wd = select(select(XT.z, XT.y, ry < 4u), XT.x, ry < 2u);
    return (wd >> ((ry & 1u) * 12u + rx * 2u)) & 3u;
}
// Green (pattern sensors) at q: its sites in the 4x4 pixels around, tent-weighted
// (radius 2).
fn xt_green(q: vec2<f32>) -> f32 {
    let ix = i32(floor(q.x));
    let iy = i32(floor(q.y));
    let w = i32(uu(U_W));
    let h = i32(uu(U_H));
    var s = 0.0;
    var n = 0.0;
    for (var j = -1; j <= 2; j += 1) {
        let sy = iy + j;
        let wy = max(0.0, 1.0 - abs(f32(sy) - q.y) * 0.5);
        for (var i = -1; i <= 2; i += 1) {
            let sx = ix + i;
            if (xc(refl(sx, w), refl(sy, h)) != 1u) { continue; }
            let wt = wy * max(0.0, 1.0 - abs(f32(sx) - q.x) * 0.5);
            s += wt * mo(sx, sy);
            n += wt;
        }
    }
    return s / max(n, 1e-6);
}
