//! Timing probe for the open sequence: `cargo run --release --example jpegtimes -- file.jpg`.
use specular_jpeg::*;
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let src = specular_core::Source::open(&path).unwrap();
    let data = src.bytes();
    let t = Instant::now();
    let info = read_info(data).unwrap();
    println!(
        "info {:.2} ms  {}x{} {:?} rst={}",
        ms(t),
        info.width,
        info.height,
        info.coding,
        info.restart_interval
    );
    if let Some(r) = info.exif_thumbnail.clone() {
        let t = Instant::now();
        let d = decode(&data[r], &DecodeOptions::default()).unwrap();
        println!("thumb {:.2} ms {}x{}", ms(t), d.width, d.height);
    }
    for s in [Scale::Eighth, Scale::Quarter] {
        let t = Instant::now();
        let d = decode(
            data,
            &DecodeOptions {
                scale: s,
                ..Default::default()
            },
        )
        .unwrap();
        println!("decode {:?} {:.1} ms {}x{}", s, ms(t), d.width, d.height);
    }
    let t = Instant::now();
    let (d, ix) = decode_and_index(
        data,
        &DecodeOptions {
            scale: Scale::Eighth,
            ..Default::default()
        },
    )
    .unwrap();
    println!(
        "decode_and_index Eighth {:.1} ms {}x{} index {} KB",
        ms(t),
        d.width,
        d.height,
        ix.memory_bytes() / 1024
    );
    let t = Instant::now();
    let ix2 = build_index(data).unwrap();
    println!("build_index {:.1} ms ({})", ms(t), ix2.len());
    for s in [Scale::Eighth, Scale::Quarter, Scale::Half] {
        let t = Instant::now();
        let d = decode_with_index(
            data,
            &ix,
            &DecodeOptions {
                scale: s,
                ..Default::default()
            },
        )
        .unwrap();
        println!(
            "with_index {:?} {:.1} ms {}x{}",
            s,
            ms(t),
            d.width,
            d.height
        );
    }
    // The probe regions are sized for large photos; clamp them so small images work too.
    let reg = clamp(
        Region {
            x: info.width / 2,
            y: info.height / 3,
            w: 2600,
            h: 1500,
        },
        &info,
    );
    let t = Instant::now();
    let d = decode_with_index(
        data,
        &ix,
        &DecodeOptions {
            scale: Scale::Full,
            region: Some(reg),
            ..Default::default()
        },
    )
    .unwrap();
    println!(
        "region full {:.1} ms {}x{} at {:?}",
        ms(t),
        d.width,
        d.height,
        d.origin
    );
    let reg = clamp(
        Region {
            x: 0,
            y: info.height / 3,
            w: info.width,
            h: 528,
        },
        &info,
    );
    let t = Instant::now();
    let d = decode_with_index(
        data,
        &ix,
        &DecodeOptions {
            scale: Scale::Full,
            region: Some(reg),
            ..Default::default()
        },
    )
    .unwrap();
    println!("band full {:.1} ms {}x{}", ms(t), d.width, d.height);
    let reg = clamp(
        Region {
            x: 4096,
            y: info.height / 3,
            w: 528,
            h: 528,
        },
        &info,
    );
    let t = Instant::now();
    let d = decode_with_index(
        data,
        &ix,
        &DecodeOptions {
            scale: Scale::Full,
            region: Some(reg),
            ..Default::default()
        },
    )
    .unwrap();
    println!("tile full {:.1} ms {}x{}", ms(t), d.width, d.height);
    let t = Instant::now();
    let d = decode(
        data,
        &DecodeOptions {
            scale: Scale::Full,
            region: Some(reg),
            ..Default::default()
        },
    )
    .unwrap();
    println!(
        "tile full no index {:.1} ms {}x{}",
        ms(t),
        d.width,
        d.height
    );
}

/// Moves `r` inside the image and shrinks it to fit (at least 1×1).
fn clamp(r: Region, info: &JpegInfo) -> Region {
    let w = r.w.clamp(1, info.width.max(1));
    let h = r.h.clamp(1, info.height.max(1));
    Region {
        x: r.x.min(info.width.saturating_sub(w)),
        y: r.y.min(info.height.saturating_sub(h)),
        w,
        h,
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
