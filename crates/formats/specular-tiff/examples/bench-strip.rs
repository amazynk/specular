//! One large single-strip CMYK LZW TIFF (6000×4000 by default) decoded at 1/1, 1/4 and 1/8
//! through the colour-managed CMYK hook (generic profile, as specular-decode installs it).
//!
//! `cargo run --release -p specular-tiff --example bench-strip -- [runs] [w] [h]`

#[path = "../tests/common/mod.rs"]
mod common;

use common::{Comp, Spec, pattern8, write};
use specular_color::{CmykConverter, Intent};
use specular_tiff::{CmykRowConverter, DecodeOptions, Tiff, set_cmyk_hook};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

fn hook(_: Option<&[u8]>) -> Option<CmykRowConverter> {
    static C: OnceLock<Arc<CmykConverter>> = OnceLock::new();
    let c = C
        .get_or_init(|| Arc::new(CmykConverter::generic(Intent::Relative, true)))
        .clone();
    Some(Arc::new(move |ink: &[u8], rgba: &mut [u8]| {
        c.convert_row(ink, rgba)
    }))
}

fn main() {
    let arg = |i: usize, d: u32| {
        std::env::args()
            .nth(i)
            .and_then(|a| a.parse().ok())
            .unwrap_or(d)
    };
    let (runs, w, h) = (arg(1, 7) as usize, arg(2, 6000), arg(3, 4000));
    set_cmyk_hook(Some(hook));
    let mut s = Spec::new(w, h, 4, 8, 5, pattern8(w, h, 4, 5));
    s.rows_per_strip = h;
    s.comp = Comp::Lzw;
    let f = write(&[s], false, true);
    let t = Tiff::open(&f).unwrap();
    print!("{w}×{h} CMYK LZW one strip, median of {runs} (ms):");
    for scale in [1, 4, 8] {
        let o = DecodeOptions {
            scale,
            ..Default::default()
        };
        let mut v: Vec<f64> = (0..runs)
            .map(|_| {
                let t0 = Instant::now();
                std::hint::black_box(t.decode(&f, 0, &o).unwrap());
                t0.elapsed().as_secs_f64() * 1e3
            })
            .collect();
        v.sort_by(f64::total_cmp);
        print!(" 1/{scale} {:.0} (min {:.0})", v[v.len() / 2], v[0]);
    }
    println!();
}
