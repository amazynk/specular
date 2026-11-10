use super::*;

fn ptm_lrgb(w: u32, h: u32, coefs: &[[u8; 6]], rgb: &[[u8; 3]]) -> Vec<u8> {
    let mut out =
        format!("PTM_1.2\nPTM_FORMAT_LRGB\n{w}\n{h}\n2.0 2.0 2.0 1.0 1.0 1.0 \n10 10 10 0 0 0 \n")
            .into_bytes();
    for c in coefs {
        out.extend_from_slice(c);
    }
    for c in rgb {
        out.extend_from_slice(c);
    }
    out
}

#[test]
fn ptm_lrgb_is_read_bottom_up() {
    // Two rows: the file's first row is the bottom one.
    let coefs = [[11, 12, 13, 14, 15, 16], [21, 22, 23, 24, 25, 26]];
    let rgb = [[1, 2, 3], [4, 5, 6]];
    let r = Rti::read(&ptm_lrgb(1, 2, &coefs, &rgb)).unwrap();
    assert_eq!((r.width, r.height, r.colour), (1, 2, Colour::Lrgb));
    // Top pixel is the file's second.
    assert_eq!(r.layers[0].data[..3], [4, 5, 6]);
    assert_eq!(r.layers[1].data[..3], [21, 22, 23]);
    assert_eq!(r.layers[2].data[3..6], [14, 15, 16]);
    // (21 − 10) · 2 / 255
    assert!((r.coef(1, 0, 0)[0] - 22.0 / 255.0).abs() < 1e-6);
    // a5 = (26 − 0) · 1
    assert!((r.coef(2, 0, 0)[2] - 26.0 / 255.0).abs() < 1e-6);
}

#[test]
fn ptm_head_on_light_is_a5_times_colour() {
    let coefs = [[0, 0, 0, 0, 0, 200]];
    let rgb = [[255, 128, 0]];
    let r = Rti::read(&ptm_lrgb(1, 1, &coefs, &rgb)).unwrap();
    let c = r.relight_image(&Look::default())[0];
    let l = 200.0 / 255.0;
    assert!((c[0] - l).abs() < 1e-5, "{c:?}");
    assert!((c[1] - 128.0 / 255.0 * l).abs() < 1e-5);
    assert_eq!(c[2], 0.0);
}

#[test]
fn ptm_truncated_and_foreign_are_errors() {
    let mut d = ptm_lrgb(2, 2, &[[0; 6]; 4], &[[0; 3]; 4]);
    d.truncate(d.len() - 1);
    assert!(matches!(Rti::read(&d), Err(Error::Truncated(_))));
    assert!(matches!(Rti::read(b"GIF89a"), Err(Error::NotRti)));
    let lum = b"PTM_1.2\nPTM_FORMAT_LUM\n1 1\n1 1 1 1 1 1\n0 0 0 0 0 0\n";
    assert!(matches!(Rti::read(lum), Err(Error::Unsupported(_))));
}

fn rti_hsh(w: u32, h: u32, terms: usize, px: impl Fn(usize, usize, usize) -> u8) -> Vec<u8> {
    let mut out = format!("#HSH test\n3\n{w} {h} 3\n{terms} 2 1\n").into_bytes();
    for k in 0..terms {
        out.extend_from_slice(&(1.0f32 + k as f32).to_le_bytes());
    }
    for k in 0..terms {
        out.extend_from_slice(&(-0.5f32 * k as f32).to_le_bytes());
    }
    for i in 0..(w * h) as usize {
        for c in 0..3 {
            for k in 0..terms {
                out.push(px(i, c, k));
            }
        }
    }
    out
}

#[test]
fn hsh_rti_header_and_values() {
    let d = rti_hsh(3, 2, 9, |i, c, k| (i * 30 + c * 7 + k) as u8);
    assert!(Rti::probe(&d));
    let r = Rti::read(&d).unwrap();
    assert_eq!(r.basis, Basis::Hsh { terms: 9 });
    assert_eq!(r.layers.len(), 9);
    // pixel 4 = (1, 1), green, term 5: byte 4·30 + 7 + 5 = 132 → 132/255 · 6 − 2.5
    let v = r.coef(5, 1, 1)[1];
    assert!((v - (132.0 / 255.0 * 6.0 - 2.5)).abs() < 1e-5, "{v}");
}

/// The coefficient layers are reserved from the memory budget before they are allocated:
/// 27 MiB of HSH layers are refused within 16 MiB and read within 64 MiB.
#[test]
fn layers_reserve_from_the_budget() {
    let d = rti_hsh(1024, 1024, 9, |_, _, _| 0);
    let r = specular_core::budget::with_limit(16 << 20, || Rti::read(&d));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{:?}", r.err());
    let r = specular_core::budget::with_limit(64 << 20, || Rti::read(&d)).unwrap();
    assert_eq!(r.layer_bytes(), 27 << 20);
}

#[test]
fn hsh_legacy_header() {
    let mut d = b"#comment\n".to_vec();
    for v in [2i32, 1, 3, 2] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    for k in 0..4 {
        d.extend_from_slice(&(-(k as f32)).to_le_bytes());
    }
    for k in 0..4 {
        d.extend_from_slice(&(k as f32 + 1.0).to_le_bytes());
    }
    d.extend((0..2 * 3 * 4).map(|i| i as u8 * 10));
    assert!(Rti::probe(&d));
    let r = Rti::read(&d).unwrap();
    assert_eq!(r.container.name(), "HSH");
    // pixel 1, blue (c 2), term 3: byte (12 + 8 + 3) · 10 = 230 → 230/255 · (4 − −3) − 3
    let v = r.coef(3, 1, 0)[2];
    assert!((v - (230.0 / 255.0 * 7.0 - 3.0)).abs() < 1e-5, "{v}");
}

#[test]
fn probes_reject_other_text() {
    assert!(!Rti::probe(b"# a comment\nhello world\n"));
    assert!(!Rti::probe(b"12 13\n"));
    assert!(!Rti::probe(b"P6\n1 1\n255\n"));
}

/// An SH image whose luminance is exactly `n · l` (terms 1..3 are x, z, y).
fn lambertian(n: [f32; 3]) -> Rti {
    let a = 0.5 * (3.0 / std::f32::consts::PI).sqrt();
    let coef = [0.0, n[0] / a, n[2] / a, n[1] / a];
    Rti {
        width: 1,
        height: 1,
        container: Container::Relight {
            kind: "sh".into(),
            colorspace: "rgb".into(),
            planes: 12,
            quality: None,
            lights: 0,
        },
        basis: Basis::Sh { terms: 4 },
        colour: Colour::Rgb,
        planes: 12,
        layers: coef
            .iter()
            .map(|&c| Layer {
                data: vec![0; 3],
                scale: [0.0; 3],
                offset: [c; 3],
            })
            .collect(),
    }
}

#[test]
fn normals_of_a_lambertian_surface_are_exact() {
    let n = {
        let v = [0.3f32, -0.4, 0.8];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        v.map(|c| c / l)
    };
    let r = lambertian(n);
    let look = Look {
        mode: Mode::Normals,
        ..Look::default()
    };
    let c = r.relight_image(&look)[0];
    for k in 0..3 {
        assert!((c[k] - (n[k] * 0.5 + 0.5)).abs() < 1e-4, "{c:?} vs {n:?}");
    }
    // Default: the luminance is n · l; diffuse gain changes nothing on a Lambertian surface.
    let light = Look::light_from_disk(0.5, 0.2);
    let d = r.relight_image(&Look {
        light,
        ..Look::default()
    })[0];
    let g = r.relight_image(&Look {
        light,
        mode: Mode::DiffuseGain,
        ..Look::default()
    })[0];
    let dot = n[0] * light[0] + n[1] * light[1] + n[2] * light[2];
    assert!((d[0] - dot).abs() < 1e-4);
    assert!((g[0] - d[0]).abs() < 1e-4);
    // Specular: brightest when the half vector is the normal.
    let s = |l: [f32; 3]| {
        r.relight_image(&Look {
            light: l,
            mode: Mode::Specular,
            ..Look::default()
        })[0][0]
    };
    let mirror = [
        2.0 * n[2] * n[0],
        2.0 * n[2] * n[1],
        2.0 * n[2] * n[2] - 1.0,
    ];
    assert!(s(mirror) > s([0.0, 0.0, 1.0]));
}

#[test]
fn downscale_averages() {
    let d = rti_hsh(3, 3, 4, |i, _, _| if i % 2 == 0 { 100 } else { 50 });
    let r = Rti::read(&d).unwrap().downscaled(2);
    assert_eq!((r.width, r.height), (2, 2));
    // top-left 2×2 block: pixels 0, 1, 3, 4 → 100, 50, 50, 100
    assert_eq!(r.layers[0].data[0], 75);
    // bottom-right: pixel 8 alone
    assert_eq!(r.layers[0].data[9], 100);
}

#[test]
fn modes_cycle() {
    let mut m = Mode::Default;
    for _ in 0..4 {
        m = m.next();
    }
    assert_eq!(m, Mode::Default);
    assert_eq!(Look::light_from_disk(2.0, 0.0), [1.0, 0.0, 0.0]);
}

/// A 13-byte JPEG header claiming an 8192×8192 coefficient plane: the plane was decoded
/// (64 MB, and 65535×65535 = 4 GB is as easy) before its size was compared. Checked first
/// now: the wrong size is refused, the right size from too few bytes is truncated.
#[test]
fn jpeg_plane_header_checked_before_decoding() {
    let mut j = vec![0xff, 0xd8, 0xff, 0xc0, 0, 11, 8];
    j.extend(8192u16.to_be_bytes());
    j.extend(8192u16.to_be_bytes());
    j.extend([1, 1, 0x11, 0]);
    j.extend([0xff, 0xdb, 0, 67, 0]);
    j.extend([1; 64]);
    for class in [0x00, 0x10] {
        j.extend([0xff, 0xc4, 0, 20, class, 1]);
        j.extend([0; 15]);
        j.push(0);
    }
    j.extend([0xff, 0xda, 0, 8, 1, 1, 0, 0, 63, 0]);
    j.extend([0xff, 0xd9]);
    let r = crate::ptm::decode_plane(&j, 100, 100);
    assert!(matches!(r, Err(Error::Malformed(_))), "{:?}", r.err());
    assert!(matches!(
        crate::ptm::decode_plane(&j, 8192, 8192),
        Err(Error::Truncated(_))
    ));
}
