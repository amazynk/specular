//! Generated clouds for tests, goldens and timings elsewhere: a small lidar-like scene as a
//! LAS file (point format 3: colour, intensity, classes, returns).

/// A `side` × `side` m patch of rolling ground (class 2) with box houses (class 6) and
/// round trees (class 5), about `side`² × `density` points, as LAS 1.2 point format 3 with
/// 16-bit colour, in a projected frame far from the origin (as survey data is).
pub fn terrain_las(side: f64, density: f64) -> Vec<u8> {
    let mut pts: Vec<([f64; 3], [u16; 3], u16, u8)> = Vec::new();
    let mut s = 0x9E37_79B9u32;
    let mut rnd = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        f64::from(s) / f64::from(u32::MAX)
    };
    let ground =
        |x: f64, y: f64| 10.0 + 3.0 * (x / side * 5.0).sin() * (y / side * 3.0).cos() + 0.04 * x;
    let houses: Vec<[f64; 4]> = (0..6)
        .map(|i| {
            let f = f64::from(i);
            [
                side * (0.15 + 0.13 * f),
                side * (0.2 + 0.11 * ((f * 2.3).sin() + 1.0) * 2.5),
                side * 0.06,
                6.0 + 2.0 * (f * 1.7).cos(),
            ]
        })
        .collect();
    let trees: Vec<[f64; 3]> = (0..24)
        .map(|i| {
            let f = f64::from(i);
            [
                side * (0.05 + 0.9 * ((f * 0.618).fract())),
                side * (0.05 + 0.9 * ((f * 0.7549 + 0.2).fract())),
                2.0 + 1.5 * (f * 0.9).sin().abs(),
            ]
        })
        .collect();
    let n = (side * side * density) as usize;
    for _ in 0..n {
        let (x, y) = (rnd() * side, rnd() * side);
        let g = ground(x, y);
        let house = houses
            .iter()
            .find(|h| (x - h[0]).abs() < h[2] && (y - h[1]).abs() < h[2] * 0.7);
        if let Some(h) = house {
            // A pitched roof.
            let z = g + h[3] - 2.5 * ((y - h[1]).abs() / (h[2] * 0.7));
            let shade = (180.0 + 40.0 * rnd()) as u16;
            pts.push(([x, y, z], [shade, 60, 50].map(|v| v * 257), 900, 6));
            continue;
        }
        let tree = trees
            .iter()
            .find(|t| (x - t[0]).powi(2) + (y - t[1]).powi(2) < t[2] * t[2]);
        if let Some(t) = tree {
            let d = ((x - t[0]).powi(2) + (y - t[1]).powi(2)).sqrt() / t[2];
            let z = g + 3.0 + t[2] * 1.8 * (1.0 - d * d).sqrt() * (0.8 + 0.2 * rnd());
            let gr = (90.0 + 60.0 * rnd()) as u16;
            pts.push(([x, y, z], [40, gr, 35].map(|v| v * 257), 300, 5));
            continue;
        }
        let k = (120.0 + 50.0 * rnd()) as u16;
        let rgb = [k, k - 15, k - 50].map(|v| v * 257);
        let intensity = (400.0 + 600.0 * rnd()) as u16;
        pts.push(([x, y, g], rgb, intensity, 2));
    }
    write_las(&pts)
}

fn write_las(pts: &[([f64; 3], [u16; 3], u16, u8)]) -> Vec<u8> {
    const HEADER: usize = 227;
    const REC: usize = 34;
    let origin = [512_000.0f64, 5_402_000.0, 0.0];
    let scale = 0.001f64;
    let mut b = vec![0u8; HEADER];
    b[..4].copy_from_slice(b"LASF");
    b[24] = 1;
    b[25] = 2;
    b[58..58 + 8].copy_from_slice(b"specular");
    b[94..96].copy_from_slice(&(HEADER as u16).to_le_bytes());
    b[96..100].copy_from_slice(&(HEADER as u32).to_le_bytes());
    b[104] = 3;
    b[105..107].copy_from_slice(&(REC as u16).to_le_bytes());
    b[107..111].copy_from_slice(&(pts.len() as u32).to_le_bytes());
    for k in 0..3 {
        b[131 + 8 * k..139 + 8 * k].copy_from_slice(&scale.to_le_bytes());
        b[155 + 8 * k..163 + 8 * k].copy_from_slice(&origin[k].to_le_bytes());
        let max = pts.iter().map(|p| p.0[k]).fold(f64::MIN, f64::max) + origin[k];
        let min = pts.iter().map(|p| p.0[k]).fold(f64::MAX, f64::min) + origin[k];
        b[179 + 16 * k..187 + 16 * k].copy_from_slice(&max.to_le_bytes());
        b[187 + 16 * k..195 + 16 * k].copy_from_slice(&min.to_le_bytes());
    }
    b.reserve(pts.len() * REC);
    for (p, rgb, intensity, class) in pts {
        let mut r = [0u8; REC];
        for k in 0..3 {
            let v = (p[k] / scale).round() as i32;
            r[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
        }
        r[12..14].copy_from_slice(&intensity.to_le_bytes());
        r[14] = 1 | (1 << 3);
        r[15] = *class;
        for k in 0..3 {
            r[28 + 2 * k..30 + 2 * k].copy_from_slice(&rgb[k].to_le_bytes());
        }
        b.extend_from_slice(&r);
    }
    b
}

#[cfg(test)]
mod tests {
    use crate::{Limits, PointFormat, read};

    #[test]
    fn terrain_reads_with_its_classes() {
        let c = read(
            PointFormat::Las,
            &super::terrain_las(40.0, 20.0),
            &Limits::default(),
        )
        .unwrap();
        assert_eq!(c.len(), 32_000);
        let classes: Vec<u8> = c.info.classes.iter().map(|c| c.0).collect();
        assert_eq!(classes, [2, 5, 6]);
        assert!(c.info.channels.rgb);
    }
}
