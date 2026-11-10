//! Read speed of long channels (run with `--ignored --nocapture`; the timings are printed).

use std::time::Instant;

fn segment(objs: &[(&str, u32, u64)], raw: &[u8], toc: u32) -> Vec<u8> {
    let mut meta = Vec::new();
    meta.extend_from_slice(&(objs.len() as u32).to_le_bytes());
    for (path, code, count) in objs {
        meta.extend_from_slice(&(path.len() as u32).to_le_bytes());
        meta.extend_from_slice(path.as_bytes());
        meta.extend_from_slice(&20u32.to_le_bytes());
        meta.extend_from_slice(&code.to_le_bytes());
        meta.extend_from_slice(&1u32.to_le_bytes());
        meta.extend_from_slice(&count.to_le_bytes());
        meta.extend_from_slice(&0u32.to_le_bytes());
    }
    let mut d = b"TDSm".to_vec();
    d.extend_from_slice(&(toc | 0x2 | 0x4 | 0x8).to_le_bytes());
    d.extend_from_slice(&4713u32.to_le_bytes());
    d.extend_from_slice(&((meta.len() + raw.len()) as u64).to_le_bytes());
    d.extend_from_slice(&(meta.len() as u64).to_le_bytes());
    d.extend_from_slice(&meta);
    d.extend_from_slice(raw);
    d
}

#[test]
#[ignore]
fn long_channels() {
    let n = 10_000_000u64;
    // 10M f64 in 100 segments of 100k samples.
    let mut file = Vec::new();
    let per = n / 100;
    for s in 0..100 {
        let raw: Vec<u8> = (0..per)
            .flat_map(|i| ((s * per + i) as f64 * 0.5).to_le_bytes())
            .collect();
        file.extend(segment(&[("/'g'/'x'", 10, per)], &raw, 0));
    }
    let t = Instant::now();
    let f = specular_tdms::parse(&file).unwrap();
    let parse = t.elapsed();
    let c = &f.groups[0].channels[0];
    assert_eq!(c.len, n);
    let mut out = vec![0f64; n as usize];
    let t = Instant::now();
    c.read(&file, 0, &mut out).unwrap();
    let read = t.elapsed();
    assert_eq!(out[n as usize - 1], (n - 1) as f64 * 0.5);
    println!(
        "10M f64 in 100 segments: parse {:.2} ms, read {:.1} ms ({:.0} Msamples/s)",
        parse.as_secs_f64() * 1e3,
        read.as_secs_f64() * 1e3,
        n as f64 / read.as_secs_f64() / 1e6
    );
    // 4 interleaved i16 channels, 2.5M samples each, one segment.
    let m = 2_500_000u64;
    let raw: Vec<u8> = (0..m)
        .flat_map(|i| (0..4).flat_map(move |k| ((i as i64 * 4 + k) as i16).to_le_bytes()))
        .collect();
    let paths: Vec<String> = (0..4).map(|k| format!("/'g'/'c{k}'")).collect();
    let objs: Vec<(&str, u32, u64)> = paths.iter().map(|p| (p.as_str(), 2, m)).collect();
    let file = segment(&objs, &raw, 0x20);
    let f = specular_tdms::parse(&file).unwrap();
    let mut out = vec![0f64; m as usize];
    let t = Instant::now();
    for c in &f.groups[0].channels {
        c.read(&file, 0, &mut out).unwrap();
    }
    let read = t.elapsed();
    assert_eq!(out[1], 7.0);
    println!(
        "4 × 2.5M interleaved i16: read {:.1} ms ({:.0} Msamples/s)",
        read.as_secs_f64() * 1e3,
        4.0 * m as f64 / read.as_secs_f64() / 1e6
    );
}
