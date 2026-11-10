//! The bilinear Bayer demosaic against colour-demosaicing, pinned in
//! `tests/data/debayer_oracle.tsv` (`gen/debayer_oracle.py`): every pattern, odd and even
//! sizes, every pixel, edges included; `debayer_row` and `debayer_at` alike.

use specular_core::nd::Cfa;

fn nums(s: &str) -> Vec<f64> {
    s.split(',').map(|v| v.parse().unwrap()).collect()
}

#[test]
fn bilinear_matches_colour_demosaicing() {
    let text = include_str!("data/debayer_oracle.tsv");
    let mut case: Option<(Cfa, usize, usize)> = None;
    let mut raw: Vec<f32> = Vec::new();
    let mut rows: Vec<Vec<[f32; 3]>> = Vec::new();
    let (mut cases, mut pixels) = (0, 0);
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "case" => {
                let cfa = Cfa::parse(f[1]).unwrap();
                let (w, h) = (f[2].parse().unwrap(), f[3].parse().unwrap());
                case = Some((cfa, w, h));
                cases += 1;
            }
            "raw" => {
                let (cfa, w, h) = case.unwrap();
                raw = nums(f[1]).into_iter().map(|v| v as f32).collect();
                assert_eq!(raw.len(), w * h);
                rows = (0..h)
                    .map(|y| {
                        let mut row = vec![[0.0; 3]; w];
                        cfa.debayer_row(&raw, w, h, y, |x, c| row[x] = c);
                        row
                    })
                    .collect();
            }
            "rgb" => {
                let (cfa, w, h) = case.unwrap();
                let (x, y): (usize, usize) = (f[1].parse().unwrap(), f[2].parse().unwrap());
                let want = nums(f[3]);
                let at = cfa.debayer_at(&raw, w, h, x, y);
                for ch in 0..3 {
                    let tol = 1e-3 + want[ch].abs() * 1e-6;
                    for (how, got) in [("at", at[ch]), ("row", rows[y][x][ch])] {
                        assert!(
                            (f64::from(got) - want[ch]).abs() <= tol,
                            "{} {w}x{h} ({x},{y}) channel {ch} {how}: {got} vs {}",
                            cfa.name(),
                            want[ch]
                        );
                    }
                }
                pixels += 1;
            }
            other => panic!("unknown line {other}"),
        }
    }
    assert_eq!(cases, 12);
    assert!(pixels > 600);
}
