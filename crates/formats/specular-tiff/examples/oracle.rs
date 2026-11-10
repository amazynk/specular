//! Spot-check a real TIFF against the `tiff` crate, chunk by chunk (memory-light: only a few
//! strips / tiles are decoded by either side, ours in bands of rows).
//! `cargo run --release -p specular-tiff --example oracle -- <file.tif>...`

use std::io::BufReader;

use specular_tiff::{DecodeOptions, Region, Tiff};
use tiff::decoder::{ChunkType, Decoder, DecodingResult, Limits};

fn main() {
    for path in std::env::args().skip(1) {
        println!("== {path}");
        let file = std::fs::File::open(&path).expect("open");
        let data = std::fs::read(&path).expect("read");
        let t = Tiff::open(&data).expect("specular-tiff open");
        for (i, p) in t.pages().iter().enumerate() {
            println!(
                "page {i}: {}x{} {} levels {:?} orientation {} icc {}",
                p.width(),
                p.height(),
                p.ifd().description(),
                p.level_sizes(),
                p.orientation(),
                p.icc_profile(&data).map_or(0, |b| b.len())
            );
        }
        let mut dec = Decoder::new(BufReader::new(file))
            .expect("tiff open")
            .with_limits(Limits::unlimited());
        let (w, h) = dec.dimensions().unwrap();
        let ct = dec.colortype().unwrap();
        let (cw, ch) = dec.chunk_dimensions();
        let tiled = dec.get_chunk_type() == ChunkType::Tile;
        let n = if tiled {
            dec.tile_count().unwrap()
        } else {
            dec.strip_count().unwrap()
        };
        println!("oracle: {w}x{h} {ct:?} chunks {n} of {cw}x{ch}");
        let across = w.div_ceil(cw);
        let picks: Vec<u32> = [0, 1, n / 3, n / 2, n * 2 / 3, n - 2, n - 1]
            .into_iter()
            .filter(|&i| i < n)
            .collect();
        let mut worst = 0u8;
        for idx in picks {
            let (dw, dh) = dec.chunk_data_dimensions(idx);
            let (x0, y0) = if tiled {
                (idx % across * cw, idx / across * ch)
            } else {
                (0, idx * ch)
            };
            let (dw, dh) = (dw.min(w - x0), dh.min(h - y0));
            let want = match dec.read_chunk(idx).expect("oracle chunk") {
                DecodingResult::U8(v) => v,
                DecodingResult::U16(v) => v
                    .iter()
                    .map(|&x| ((x as u32 * 255 + 32767) / 65535) as u8)
                    .collect(),
                other => panic!("oracle sample type {:?}", std::mem::discriminant(&other)),
            };
            let spp = want.len()
                / (dec.chunk_data_dimensions(idx).0 as usize
                    * dec.chunk_data_dimensions(idx).1 as usize);
            let stride_src = dec.chunk_data_dimensions(idx).0 as usize * spp;
            // Compare in bands of rows so a huge strip never needs a huge decode on our side.
            let mut maxd = 0u8;
            for by in (0..dh).step_by(512) {
                let bh = 512.min(dh - by);
                let got = t
                    .decode(
                        &data,
                        0,
                        &DecodeOptions {
                            region: Some(Region {
                                x: x0,
                                y: y0 + by,
                                w: dw,
                                h: bh,
                            }),
                            ..Default::default()
                        },
                    )
                    .expect("specular-tiff decode");
                let bpp = got.format.bytes_per_pixel();
                for y in 0..bh as usize {
                    for x in 0..dw as usize {
                        let s = &want[(by as usize + y) * stride_src + x * spp..][..spp];
                        let g = &got.data[y * got.stride + x * bpp..][..bpp];
                        for c in 0..spp.min(bpp) {
                            maxd = maxd.max(s[c].abs_diff(g[c]));
                        }
                    }
                }
            }
            println!("  chunk {idx} at ({x0},{y0}) {dw}x{dh}: max diff {maxd}");
            worst = worst.max(maxd);
        }
        println!("worst diff {worst}");
    }
}
