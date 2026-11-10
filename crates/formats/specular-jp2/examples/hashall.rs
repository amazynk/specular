//! Regression helper: `hashall dir...` prints a hash of the 16-bit decode of every JPEG 2000
//! file under the given directories at reduce 0 and 1 and for a region, to compare decoder
//! changes for bit-exactness.

use specular_jp2::{DecodeOptions, Depth, Pixels, Region};
use std::hash::{Hash, Hasher};

fn walk(p: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if p.is_dir() {
        let mut v: Vec<_> = std::fs::read_dir(p)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        v.sort();
        for e in v {
            walk(&e, out);
        }
    } else if let Some(e) = p.extension().and_then(|e| e.to_str())
        && matches!(
            e.to_ascii_lowercase().as_str(),
            "jp2" | "j2k" | "j2c" | "jpc" | "jpx" | "jpf" | "jph" | "jhc"
        )
    {
        out.push(p.to_path_buf());
    }
}

fn main() {
    let mut files = Vec::new();
    for a in std::env::args().skip(1) {
        walk(std::path::Path::new(&a), &mut files);
    }
    for f in files {
        let data = std::fs::read(&f).unwrap();
        let mut line = format!("{}", f.display());
        let info = specular_jp2::read_info(&data);
        for k in 0..3 {
            let region = match (&info, k) {
                (Ok(i), 2) if i.width > 4 && i.height > 4 => Some(Region {
                    x: i.width / 3,
                    y: i.height / 4,
                    w: i.width / 2,
                    h: i.height / 2,
                }),
                (_, 2) => continue,
                _ => None,
            };
            let r = specular_jp2::decode(
                &data,
                &DecodeOptions {
                    reduce: (k == 1) as u8,
                    region,
                    depth: Depth::Sixteen,
                    ..Default::default()
                },
            );
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match r {
                Ok(img) => {
                    (img.width, img.height, img.truncated).hash(&mut h);
                    if let Pixels::U16(v) = &img.pixels {
                        v.hash(&mut h);
                    }
                    line += &format!(" {:016x}", h.finish());
                }
                Err(e) => line += &format!(" err({e})"),
            }
        }
        println!("{line}");
    }
}
