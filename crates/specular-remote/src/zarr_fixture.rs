//! Generated OME-Zarr stores for tests and screenshots (with [`crate::testing`]): an image
//! as Zarr v2 (NGFF 0.4, uncompressed chunks) or Zarr v3 (NGFF 0.5, sharded), a plate of
//! wells and a bioformats2raw layout, and a handler that serves a store with ranges.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::testing::{TestRequest, TestResponse};

/// Keys and their bytes.
pub type Files = BTreeMap<String, Vec<u8>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Zarr v2, NGFF 0.4, one file per chunk.
    V2,
    /// Zarr v3, NGFF 0.5, shards of 2×2 chunks with the index at the end (CRC32C).
    V3Sharded,
}

/// An image: `channels` × `planes` (Z) × `size` × `size` 16-bit samples, `levels` levels
/// (each half the last), chunks of `chunk` × `chunk`.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub layout: Layout,
    pub channels: u32,
    pub planes: u32,
    pub size: u32,
    pub levels: u32,
    pub chunk: u32,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            layout: Layout::V2,
            channels: 2,
            planes: 3,
            size: 512,
            levels: 3,
            chunk: 128,
        }
    }
}

/// Level 0's sample at channel `c`, plane `z`, row `y`, column `x`: soft blobs and a grid of
/// small cells in channel 0, rings in channel 1, shifting with `z`.
pub fn sample(c: u32, z: u32, y: u32, x: u32, size: u32) -> u16 {
    let s = size as f32;
    let (fx, fy) = (x as f32 / s, y as f32 / s);
    let v = if c.is_multiple_of(2) {
        let mut a = 0.0f32;
        for (cx, cy, r) in [(0.3, 0.35, 0.12), (0.68, 0.6, 0.18), (0.45, 0.78, 0.08)] {
            let d2 = (fx - cx - 0.03 * z as f32).powi(2) + (fy - cy).powi(2);
            a += (-d2 / (r * r)).exp();
        }
        // Small "cells" on a grid: detail only the finest level shows.
        if (x + 3 * z) % 24 < 4 && y % 24 < 4 {
            1.0
        } else {
            a.min(1.0)
        }
    } else {
        let d = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt();
        0.5 + 0.5 * (d * 40.0 - z as f32).cos() * (1.0 - d).max(0.0)
    };
    (v * 4000.0) as u16 + 100
}

/// One level's samples, C order (c, z, y, x).
fn level(spec: &Spec, l: u32) -> (u32, Vec<u16>) {
    let n = spec.size >> l;
    let (c, z) = (spec.channels, spec.planes);
    let mut out = vec![0u16; (c * z * n * n) as usize];
    let f = 1u32 << l;
    for ci in 0..c {
        for zi in 0..z {
            for y in 0..n {
                for x in 0..n {
                    let mut sum = 0u32;
                    for dy in 0..f {
                        for dx in 0..f {
                            sum += u32::from(sample(ci, zi, y * f + dy, x * f + dx, spec.size));
                        }
                    }
                    out[(((ci * z + zi) * n + y) * n + x) as usize] = (sum / (f * f)) as u16;
                }
            }
        }
    }
    (n, out)
}

/// The chunk at (c, z, cy, cx) of a level, little-endian, `chunk` × `chunk` (zero-padded).
fn chunk_bytes(spec: &Spec, n: u32, data: &[u16], c: u32, z: u32, cy: u32, cx: u32) -> Vec<u8> {
    let k = spec.chunk;
    let mut b = Vec::with_capacity((k * k * 2) as usize);
    for y in 0..k {
        for x in 0..k {
            let (gy, gx) = (cy * k + y, cx * k + x);
            let v = if gy < n && gx < n {
                data[(((c * spec.planes + z) * n + gy) * n + gx) as usize]
            } else {
                0
            };
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    b
}

/// CRC32C (Castagnoli).
pub fn crc32c(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0x82F6_3B78
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// The `multiscales` and `omero` members of an image group's attributes.
fn multiscales(spec: &Spec, version: &str, name: &str) -> String {
    let datasets: Vec<String> = (0..spec.levels)
        .map(|l| {
            let s = 0.5 * f64::from(1u32 << l);
            format!(
                r#"{{"path":"{l}","coordinateTransformations":[{{"type":"scale","scale":[1.0,2.0,{s},{s}]}}]}}"#
            )
        })
        .collect();
    let channels: Vec<String> = (0..spec.channels)
        .map(|c| {
            let (label, colour) = if c.is_multiple_of(2) {
                ("DAPI", "0000FF")
            } else {
                ("GFP", "00FF00")
            };
            format!(
                r#"{{"label":"{label}","color":"{colour}","window":{{"start":100,"end":4100,"min":0,"max":65535}},"active":true}}"#
            )
        })
        .collect();
    format!(
        r#""multiscales":[{{"version":"{version}","name":"{name}","axes":[{{"name":"c","type":"channel"}},{{"name":"z","type":"space","unit":"micrometer"}},{{"name":"y","type":"space","unit":"micrometer"}},{{"name":"x","type":"space","unit":"micrometer"}}],"datasets":[{}]}}],"omero":{{"name":"{name}","channels":[{}]}}"#,
        datasets.join(","),
        channels.join(",")
    )
}

/// An OME-Zarr image at `prefix` ("" or ending in '/') of `files`, named `name`.
pub fn add_image(files: &mut Files, prefix: &str, spec: &Spec, name: &str) {
    let ms = multiscales(
        spec,
        if spec.layout == Layout::V2 {
            "0.4"
        } else {
            "0.5"
        },
        name,
    );
    let shape = |n: u32| format!("[{},{},{n},{n}]", spec.channels, spec.planes);
    match spec.layout {
        Layout::V2 => {
            files.insert(format!("{prefix}.zgroup"), br#"{"zarr_format":2}"#.to_vec());
            files.insert(format!("{prefix}.zattrs"), format!("{{{ms}}}").into_bytes());
        }
        Layout::V3Sharded => {
            files.insert(
                format!("{prefix}zarr.json"),
                format!(
                    r#"{{"zarr_format":3,"node_type":"group","attributes":{{"ome":{{"version":"0.5",{}}}}}}}"#,
                    ms
                )
                .into_bytes(),
            );
        }
    }
    for l in 0..spec.levels {
        let (n, data) = level(spec, l);
        let k = spec.chunk;
        let grid = n.div_ceil(k);
        let dir = format!("{prefix}{l}/");
        match spec.layout {
            Layout::V2 => {
                files.insert(
                    format!("{dir}.zarray"),
                    format!(
                        r#"{{"zarr_format":2,"shape":{},"chunks":[1,1,{k},{k}],"dtype":"<u2","compressor":null,"fill_value":0,"order":"C","filters":null,"dimension_separator":"/"}}"#,
                        shape(n)
                    )
                    .into_bytes(),
                );
                for c in 0..spec.channels {
                    for z in 0..spec.planes {
                        for cy in 0..grid {
                            for cx in 0..grid {
                                files.insert(
                                    format!("{dir}{c}/{z}/{cy}/{cx}"),
                                    chunk_bytes(spec, n, &data, c, z, cy, cx),
                                );
                            }
                        }
                    }
                }
            }
            Layout::V3Sharded => {
                let s = 2 * k;
                files.insert(
                    format!("{dir}zarr.json"),
                    format!(
                        r#"{{"zarr_format":3,"node_type":"array","shape":{},"data_type":"uint16","chunk_grid":{{"name":"regular","configuration":{{"chunk_shape":[1,1,{s},{s}]}}}},"chunk_key_encoding":{{"name":"default","configuration":{{"separator":"/"}}}},"fill_value":0,"codecs":[{{"name":"sharding_indexed","configuration":{{"chunk_shape":[1,1,{k},{k}],"codecs":[{{"name":"bytes","configuration":{{"endian":"little"}}}}],"index_codecs":[{{"name":"bytes","configuration":{{"endian":"little"}}}},{{"name":"crc32c"}}],"index_location":"end"}}}}],"dimension_names":["c","z","y","x"]}}"#,
                        shape(n)
                    )
                    .into_bytes(),
                );
                let sgrid = n.div_ceil(s);
                for c in 0..spec.channels {
                    for z in 0..spec.planes {
                        for sy in 0..sgrid {
                            for sx in 0..sgrid {
                                let mut body = Vec::new();
                                let mut index = Vec::new();
                                for iy in 0..2 {
                                    for ix in 0..2 {
                                        let (cy, cx) = (sy * 2 + iy, sx * 2 + ix);
                                        if cy >= grid || cx >= grid {
                                            index.extend_from_slice(&u64::MAX.to_le_bytes());
                                            index.extend_from_slice(&u64::MAX.to_le_bytes());
                                            continue;
                                        }
                                        let b = chunk_bytes(spec, n, &data, c, z, cy, cx);
                                        index.extend_from_slice(&(body.len() as u64).to_le_bytes());
                                        index.extend_from_slice(&(b.len() as u64).to_le_bytes());
                                        body.extend_from_slice(&b);
                                    }
                                }
                                let crc = crc32c(&index);
                                body.extend_from_slice(&index);
                                body.extend_from_slice(&crc.to_le_bytes());
                                files.insert(format!("{dir}c/{c}/{z}/{sy}/{sx}"), body);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One OME-Zarr image at the root.
pub fn ome_zarr(spec: &Spec) -> Files {
    let mut f = Files::new();
    add_image(&mut f, "", spec, "synthetic");
    f
}

/// A plate (v2): rows A–B, columns 1–2, wells A1 and B2 with two fields each.
pub fn plate(spec: &Spec) -> Files {
    let mut f = Files::new();
    f.insert(".zgroup".into(), br#"{"zarr_format":2}"#.to_vec());
    f.insert(
        ".zattrs".into(),
        br#"{"plate":{"version":"0.4","name":"P1","rows":[{"name":"A"},{"name":"B"}],"columns":[{"name":"1"},{"name":"2"}],"wells":[{"path":"A/1","rowIndex":0,"columnIndex":0},{"path":"B/2","rowIndex":1,"columnIndex":1}],"field_count":2}}"#
            .to_vec(),
    );
    for w in ["A/1", "B/2"] {
        f.insert(format!("{w}/.zgroup"), br#"{"zarr_format":2}"#.to_vec());
        f.insert(
            format!("{w}/.zattrs"),
            br#"{"well":{"version":"0.4","images":[{"path":"0"},{"path":"1"}]}}"#.to_vec(),
        );
        for field in 0..2 {
            add_image(
                &mut f,
                &format!("{w}/{field}/"),
                spec,
                &format!("{w} {field}"),
            );
        }
    }
    f
}

/// A bioformats2raw layout with `n` series and an `OME` group listing them.
pub fn bioformats2raw(spec: &Spec, n: u32) -> Files {
    let mut f = Files::new();
    f.insert(".zgroup".into(), br#"{"zarr_format":2}"#.to_vec());
    f.insert(".zattrs".into(), br#"{"bioformats2raw.layout":3}"#.to_vec());
    f.insert("OME/.zgroup".into(), br#"{"zarr_format":2}"#.to_vec());
    let list: Vec<String> = (0..n).map(|k| format!("\"{k}\"")).collect();
    f.insert(
        "OME/.zattrs".into(),
        format!(r#"{{"series":[{}]}}"#, list.join(",")).into_bytes(),
    );
    for k in 0..n {
        add_image(&mut f, &format!("{k}/"), spec, &format!("series {k}"));
    }
    f
}

/// Serves `files` under the URL path `prefix` ("/data/img.zarr/"), with ranges; 404 for
/// anything else.
pub fn handler(
    files: Arc<Files>,
    prefix: &'static str,
) -> impl Fn(&TestRequest) -> TestResponse + Send + Sync + 'static {
    move |r| {
        let Some(key) = r.path().strip_prefix(prefix) else {
            return TestResponse::status(404);
        };
        match files.get(key) {
            Some(d) if r.method == "HEAD" => {
                TestResponse::ok(Vec::new()).header("Content-Length", d.len().to_string())
            }
            Some(d) => TestResponse::ranged(r, d, "\"z1\""),
            None => TestResponse::status(404),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32c_check_value() {
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
    }

    #[test]
    fn stores_have_their_chunks() {
        let s = Spec {
            size: 64,
            chunk: 16,
            levels: 2,
            ..Spec::default()
        };
        let f = ome_zarr(&s);
        assert!(f.contains_key("0/1/2/3/3"));
        assert!(f.contains_key("1/.zarray"));
        let f = ome_zarr(&Spec {
            layout: Layout::V3Sharded,
            ..s
        });
        // 64/32 = 2 shards a side at level 0.
        assert!(f.contains_key("0/c/1/2/1/1"));
        assert_eq!(f["0/c/0/0/0/0"].len(), 4 * 16 * 16 * 2 + 4 * 16 + 4);
    }
}
