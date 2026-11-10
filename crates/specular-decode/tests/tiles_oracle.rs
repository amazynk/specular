//! MBTiles and PMTiles against GDAL (rasterio, pyogrio) and the PMTiles reference reader
//! (python `pmtiles`): this test writes the fixtures and what Specular reads from them
//! into SPECULAR_TILES_ORACLE (a folder), and `specular-gis/gen/tiles_oracle.py` compares.
//! Skips without the variable.

use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::geo::specular_gis::{fixtures, tiles::TileFormat};
use specular_decode::{DecodeRequest, Registry};

#[test]
fn write_for_the_oracle() {
    let Some(dir) = std::env::var_os("SPECULAR_TILES_ORACLE") else {
        eprintln!("SPECULAR_TILES_ORACLE not set; skipping");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut manifest = Vec::new();
    // Raster MBTiles: the mosaic's size, geotransform and pixels on a 9 × 9 grid.
    let mb = dir.join("raster.mbtiles");
    fixtures::mbtiles(&mb, TileFormat::Png, 3);
    let s = Registry::default()
        .open(Arc::new(Source::open(&mb).unwrap()))
        .unwrap();
    let g = s.geo().unwrap();
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    let (w, h) = (r.width as usize, r.height as usize);
    let mut samples = Vec::new();
    for j in 0..9 {
        for i in 0..9 {
            let (x, y) = ((i * w + w / 2) / 9, (j * h + h / 2) / 9);
            let o = y * r.planes[0].stride + x * 4;
            let p = &r.planes[0].data[o..o + 3];
            samples.push(format!("[{x},{y},{},{},{}]", p[0], p[1], p[2]));
        }
    }
    let t = &g.to_map;
    manifest.push(format!(
        r#"{{"kind":"raster","file":"raster.mbtiles","width":{w},"height":{h},"transform":[{},{},{},{},{},{}],"samples":[{}]}}"#,
        t.a,
        t.b,
        t.c,
        t.d,
        t.e,
        t.f,
        samples.join(",")
    ));
    // PMTiles: every tile's bytes, by its hash.
    for (name, internal, leaves) in [("gzip.pmtiles", 2u8, false), ("leaves.pmtiles", 2, true)] {
        let file = fixtures::pmtiles(TileFormat::Png, 3, internal, leaves);
        std::fs::write(dir.join(name), &file).unwrap();
        let p = specular_decode::open_tile_file(&dir.join(name)).unwrap();
        let mut tiles = Vec::new();
        for z in 0..=3u8 {
            for y in 0..(1u32 << z) {
                for x in 0..(1u32 << z) {
                    let b = p.tile(z, x, y).unwrap().unwrap();
                    tiles.push(format!("[{z},{x},{y},{}]", b.len()));
                }
            }
        }
        manifest.push(format!(
            r#"{{"kind":"pmtiles","file":"{name}","tiles":[{}]}}"#,
            tiles.join(",")
        ));
    }
    // Vector tiles in both containers: features per layer at the zoom Specular reads.
    let vmb = dir.join("vector.mbtiles");
    fixtures::mbtiles(&vmb, TileFormat::Mvt, 1);
    std::fs::write(
        dir.join("vector.pmtiles"),
        fixtures::pmtiles(TileFormat::Mvt, 1, 2, false),
    )
    .unwrap();
    for name in ["vector.mbtiles", "vector.pmtiles"] {
        let s = Registry::default()
            .open(Arc::new(Source::open(dir.join(name)).unwrap()))
            .unwrap();
        let g = s.geo().unwrap();
        let ds = &g.features.as_ref().unwrap().dataset;
        let layers: Vec<String> = ds
            .layers
            .iter()
            .map(|l| format!(r#""{}":{}"#, l.name, l.features.len()))
            .collect();
        manifest.push(format!(
            r#"{{"kind":"vector","file":"{name}","zoom":1,"layers":{{{}}}}}"#,
            layers.join(",")
        ));
    }
    std::fs::write(dir.join("specular.jsonl"), manifest.join("\n")).unwrap();
}
