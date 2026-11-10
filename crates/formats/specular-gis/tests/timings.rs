//! Timings (run by hand: `cargo test -p specular-gis --test timings -- --ignored
//! --nocapture`): the EPSG table's first read, conversions to WGS 84, PMTiles lookups and
//! vector tile decoding.

use std::time::Instant;

use specular_gis::{crs, fixtures, mvt, pmtiles, tiles::TileFormat, tiles::TileStore};

#[test]
#[ignore = "a measurement"]
fn timings() {
    let t = Instant::now();
    let c = crs::from_epsg(27700).unwrap();
    eprintln!(
        "EPSG table, first lookup: {:.1} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    let t = Instant::now();
    for code in specular_gis::epsg::codes() {
        let _ = crs::from_epsg(code);
    }
    eprintln!("every EPSG code: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let n = 1_000_000;
    let t = Instant::now();
    let mut s = 0.0;
    for i in 0..n {
        let (lon, lat) = c
            .to_wgs84(
                400_000.0 + f64::from(i % 1000),
                300_000.0 + f64::from(i / 1000),
            )
            .unwrap();
        s += lon + lat;
    }
    eprintln!(
        "British National Grid to WGS 84 (TM + Helmert): {:.0} ns a point ({s:.0})",
        t.elapsed().as_secs_f64() * 1e9 / f64::from(n)
    );
    let file = fixtures::pmtiles(TileFormat::Png, 6, 2, true);
    let p = pmtiles::PmTiles::open(Box::new(file)).unwrap();
    let t = Instant::now();
    let mut k = 0;
    for y in 0..64u32 {
        for x in 0..64u32 {
            k += p.tile(6, x, y).unwrap().map_or(0, |b| b.len());
        }
    }
    eprintln!(
        "PMTiles (leaf directories), 4096 tiles: {:.2} µs a tile ({k} bytes)",
        t.elapsed().as_secs_f64() * 1e6 / 4096.0
    );
    let tile = fixtures::mvt_tile();
    let t = Instant::now();
    for _ in 0..10_000 {
        let mut layers = Vec::new();
        let mut index = std::collections::HashMap::new();
        mvt::decode_into(&tile, (0, 0, 0), &mut layers, &mut index).unwrap();
    }
    eprintln!(
        "MVT decode (3 features): {:.2} µs a tile",
        t.elapsed().as_secs_f64() * 1e6 / 10_000.0
    );
}
