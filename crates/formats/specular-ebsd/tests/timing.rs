//! Open time of a million-point map (`cargo test -p specular-ebsd --test timing -- --ignored
//! --nocapture`).

use std::fmt::Write;
use std::time::Instant;

#[test]
#[ignore = "timing; run explicitly"]
fn million_points() {
    let (w, h) = (1000usize, 1000usize);
    let mut s = String::from(
        "# TEM_PIXperUM 1.0\n# Phase 1\n# MaterialName Nickel\n# Symmetry 43\n\
         # GRID: SqrGrid\n# XSTEP: 0.1\n# YSTEP: 0.1\n# NCOLS_ODD: 1000\n# NCOLS_EVEN: 1000\n\
         # NROWS: 1000\n#\n",
    );
    for y in 0..h {
        for x in 0..w {
            let k = (x * 7 + y * 13) % 997;
            let _ = writeln!(
                s,
                "  {:.5}   {:.5}   {:.5}     {:.5}     {:.5}  {:.1}  {:.3}  1  {:.3}  {:.3}",
                (k as f64) * 0.0063,
                (k as f64) * 0.0031,
                (k as f64) * 0.0057,
                x as f64 * 0.1,
                y as f64 * 0.1,
                1000.0 + k as f64,
                (k % 100) as f64 / 100.0,
                k as f64 * 0.5,
                (k % 30) as f64 / 10.0
            );
        }
    }
    let bytes = s.into_bytes();
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let t = Instant::now();
        let d = specular_ebsd::read_ang(&bytes).unwrap();
        best = best.min(t.elapsed().as_secs_f64());
        assert_eq!((d.pages[0].width, d.pages[0].height), (w, h));
    }
    eprintln!(
        "1M-point .ang ({} MB): opened in {:.0} ms (best of 3)",
        bytes.len() >> 20,
        best * 1e3
    );
}
