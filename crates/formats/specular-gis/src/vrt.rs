//! GDAL virtual rasters (`.vrt`): XML naming other rasters and where their pixels go.
//! Read here: the size, SRS and geotransform, and per band its data type, no-data value,
//! colour interpretation and colour table, and its simple and complex sources (file, band,
//! source and destination windows, a complex source's no-data and linear scaling).
//! Warped, derived (pixel function) and pansharpened rasters and other source kinds are
//! named and refused.

use crate::geotiff::Affine2;

/// One source of a band.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub file: String,
    /// The file is named relative to the VRT's folder.
    pub relative: bool,
    /// 1-based.
    pub band: u32,
    /// x, y, width, height in source pixels.
    pub src_rect: Option<[f64; 4]>,
    /// x, y, width, height in the VRT's pixels.
    pub dst_rect: Option<[f64; 4]>,
    /// Source values equal to this are left out (complex sources).
    pub nodata: Option<f64>,
    /// value × ratio + offset (complex sources).
    pub scale_ratio: f64,
    pub scale_offset: f64,
}

/// One band.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub data_type: String,
    pub nodata: Option<f64>,
    pub color_interp: Option<String>,
    pub description: Option<String>,
    pub unit: Option<String>,
    /// RGBA entries of a palette band.
    pub palette: Vec<[u8; 4]>,
    pub sources: Vec<Source>,
}

/// A virtual raster.
#[derive(Clone, Debug, PartialEq)]
pub struct Vrt {
    pub width: u32,
    pub height: u32,
    pub srs: Option<String>,
    pub transform: Option<Affine2>,
    pub bands: Vec<Band>,
}

/// Whether the bytes are a VRT raster (an XML root `VRTDataset`).
pub fn sniff(head: &[u8]) -> bool {
    let t = String::from_utf8_lossy(&head[..head.len().min(1024)]);
    let t = t.trim_start_matches('\u{feff}').trim_start();
    let t = match t.strip_prefix("<?xml") {
        Some(r) => r.split_once("?>").map_or("", |(_, r)| r).trim_start(),
        None => t,
    };
    t.starts_with("<VRTDataset") && t[11..].starts_with([' ', '>', '\t', '\r', '\n'])
}

/// The bytes of a sample of a GDAL data type, and whether it is supported.
pub fn type_bytes(t: &str) -> Option<usize> {
    Some(match t {
        "Byte" | "Int8" => 1,
        "UInt16" | "Int16" => 2,
        "UInt32" | "Int32" | "Float32" => 4,
        "UInt64" | "Int64" | "Float64" => 8,
        _ => return None,
    })
}

/// A value as stored in a band of type `t`: rounded and clamped for integer types.
pub fn to_type(t: &str, v: f64) -> f64 {
    let clamp = |lo: f64, hi: f64| {
        if v.is_nan() {
            0.0
        } else {
            v.round().clamp(lo, hi)
        }
    };
    match t {
        "Byte" => clamp(0.0, 255.0),
        "Int8" => clamp(-128.0, 127.0),
        "UInt16" => clamp(0.0, 65535.0),
        "Int16" => clamp(-32768.0, 32767.0),
        "UInt32" => clamp(0.0, 4_294_967_295.0),
        "Int32" => clamp(-2_147_483_648.0, 2_147_483_647.0),
        "UInt64" => clamp(0.0, 1.8446744073709552e19),
        "Int64" => clamp(-9.223372036854776e18, 9.223372036854776e18),
        "Float32" => f64::from(v as f32),
        _ => v,
    }
}

fn rect(n: roxmltree::Node<'_, '_>) -> Option<[f64; 4]> {
    let a = |k: &str| n.attribute(k)?.trim().parse::<f64>().ok();
    Some([a("xOff")?, a("yOff")?, a("xSize")?, a("ySize")?])
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn text_of(n: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    let t = child(n, name)?.text()?.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

fn number(s: &str) -> Option<f64> {
    match s.trim().to_ascii_lowercase().as_str() {
        "nan" => Some(f64::NAN),
        "inf" | "+inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        t => t.parse().ok(),
    }
}

/// Source elements read here, and those named as unsupported.
const SOURCES: &[&str] = &["SimpleSource", "ComplexSource"];
const OTHER_SOURCES: &[&str] = &[
    "AveragedSource",
    "KernelFilteredSource",
    "NoDataFromMaskSource",
    "ArraySource",
];
/// Children of a complex source that change values in ways not done here.
const COMPLEX_UNSUPPORTED: &[&str] = &[
    "LUT",
    "Exponent",
    "SrcMin",
    "SrcMax",
    "DstMin",
    "DstMax",
    "ColorTableComponent",
    "UseMaskBand",
];

/// Parses a VRT.
pub fn parse(text: &str) -> Result<Vrt, String> {
    if !specular_limits::xml_within_depth(text.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err("the XML nests too deeply".into());
    }
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("not valid XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "VRTDataset" {
        return Err("not a GDAL virtual raster".into());
    }
    if let Some(sub) = root.attribute("subClass") {
        return Err(format!("GDAL {sub} rasters are not supported"));
    }
    let dim = |k: &str| {
        root.attribute(k)
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|v| *v > 0)
            .ok_or_else(|| format!("no {k}"))
    };
    let (width, height) = (dim("rasterXSize")?, dim("rasterYSize")?);
    let transform = text_of(root, "GeoTransform").and_then(|g| {
        let v: Vec<f64> = g
            .split(',')
            .map(|s| s.trim().parse().ok())
            .collect::<Option<_>>()?;
        (v.len() == 6).then(|| Affine2 {
            a: v[1],
            b: v[2],
            c: v[0],
            d: v[4],
            e: v[5],
            f: v[3],
        })
    });
    let mut bands = Vec::new();
    for b in root
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "VRTRasterBand")
    {
        // A sourced band (the default) may say so; derived bands (pixel functions) are not
        // read.
        if let Some(sub) = b.attribute("subClass")
            && sub != "VRTSourcedRasterBand"
        {
            return Err(format!("GDAL {sub} bands are not supported"));
        }
        let data_type = b.attribute("dataType").unwrap_or("Byte").to_owned();
        if type_bytes(&data_type).is_none() {
            return Err(format!("{data_type} bands are not supported"));
        }
        let mut sources = Vec::new();
        for s in b.children().filter(|c| c.is_element()) {
            let kind = s.tag_name().name();
            if OTHER_SOURCES.contains(&kind) {
                return Err(format!("{kind}s are not supported"));
            }
            if !SOURCES.contains(&kind) {
                continue;
            }
            let complex = kind == "ComplexSource";
            if complex
                && let Some(u) = s
                    .children()
                    .find(|c| c.is_element() && COMPLEX_UNSUPPORTED.contains(&c.tag_name().name()))
            {
                return Err(format!(
                    "complex sources with {} are not supported",
                    u.tag_name().name()
                ));
            }
            let f = child(s, "SourceFilename").ok_or("a source without a file")?;
            let file = f.text().unwrap_or("").trim().to_owned();
            let relative = f
                .attribute("relativeToVRT")
                .is_some_and(|v| v.trim() == "1");
            if sources.len() >= 65_536 {
                return Err("too many sources".into());
            }
            sources.push(Source {
                file,
                relative,
                band: text_of(s, "SourceBand")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1),
                src_rect: child(s, "SrcRect").and_then(rect),
                dst_rect: child(s, "DstRect").and_then(rect),
                nodata: if complex {
                    text_of(s, "NODATA").and_then(|v| number(&v))
                } else {
                    None
                },
                scale_ratio: text_of(s, "ScaleRatio")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1.0),
                scale_offset: text_of(s, "ScaleOffset")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0),
            });
        }
        let palette = child(b, "ColorTable")
            .map(|t| {
                t.children()
                    .filter(|e| e.is_element() && e.tag_name().name() == "Entry")
                    .take(65_536)
                    .map(|e| {
                        let c = |k: &str| {
                            e.attribute(k)
                                .and_then(|v| v.trim().parse::<u8>().ok())
                                .unwrap_or(255)
                        };
                        [c("c1"), c("c2"), c("c3"), c("c4")]
                    })
                    .collect()
            })
            .unwrap_or_default();
        bands.push(Band {
            data_type,
            nodata: text_of(b, "NoDataValue").and_then(|v| number(&v)),
            color_interp: text_of(b, "ColorInterp"),
            description: text_of(b, "Description"),
            unit: text_of(b, "UnitType"),
            palette,
            sources,
        });
    }
    if bands.is_empty() {
        return Err("no bands".into());
    }
    Ok(Vrt {
        width,
        height,
        srs: text_of(root, "SRS"),
        transform,
        bands,
    })
}

/// How a source's pixels map onto the VRT's: for each VRT column (row) in the destination
/// window, the source column (row) it takes, nearest neighbour as GDAL picks it.
pub fn spans(dst_off: f64, dst_size: f64, src_off: f64, src_size: f64, n: u32) -> Vec<(u32, i64)> {
    if dst_size.is_nan()
        || src_size.is_nan()
        || dst_size <= 0.0
        || src_size <= 0.0
        || !dst_off.is_finite()
        || !src_off.is_finite()
    {
        return Vec::new();
    }
    let ratio = src_size / dst_size;
    let start = dst_off.max(0.0).floor() as i64;
    let end = (dst_off + dst_size).min(f64::from(n)).ceil() as i64;
    (start..end.max(start))
        .filter_map(|d| {
            let centre = d as f64 + 0.5;
            if centre < dst_off || centre >= dst_off + dst_size {
                return None;
            }
            let s = (src_off + (centre - dst_off) * ratio).floor() as i64;
            Some((d as u32, s))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VRT: &str = r#"<VRTDataset rasterXSize="4" rasterYSize="3">
  <SRS>EPSG:4326</SRS>
  <GeoTransform>10, 0.5, 0, 50, 0, -0.5</GeoTransform>
  <VRTRasterBand dataType="Int16" band="1">
    <NoDataValue>-99</NoDataValue>
    <ColorInterp>Gray</ColorInterp>
    <SimpleSource>
      <SourceFilename relativeToVRT="1">a.tif</SourceFilename>
      <SourceBand>2</SourceBand>
      <SrcRect xOff="1" yOff="1" xSize="2" ySize="2"/>
      <DstRect xOff="0" yOff="0" xSize="4" ySize="3"/>
    </SimpleSource>
    <ComplexSource>
      <SourceFilename relativeToVRT="0">/data/b.tif</SourceFilename>
      <ScaleOffset>1.5</ScaleOffset><ScaleRatio>2</ScaleRatio><NODATA>nan</NODATA>
    </ComplexSource>
  </VRTRasterBand>
</VRTDataset>"#;

    #[test]
    fn parses() {
        assert!(sniff(VRT.as_bytes()));
        assert!(sniff(
            b"<?xml version=\"1.0\"?>\n<VRTDataset rasterXSize=\"1\">"
        ));
        assert!(!sniff(b"<OGRVRTDataSource>"));
        let v = parse(VRT).unwrap();
        assert_eq!((v.width, v.height), (4, 3));
        assert_eq!(v.transform.unwrap().apply(2.0, 2.0), (11.0, 49.0));
        let b = &v.bands[0];
        assert_eq!(b.nodata, Some(-99.0));
        assert_eq!(b.sources[0].band, 2);
        assert!(b.sources[0].relative && !b.sources[1].relative);
        assert_eq!(b.sources[0].src_rect, Some([1.0, 1.0, 2.0, 2.0]));
        assert_eq!(b.sources[1].scale_ratio, 2.0);
        assert!(b.sources[1].nodata.unwrap().is_nan());
        assert!(
            parse(&VRT.replace("<VRTDataset", "<VRTDataset subClass=\"VRTWarpedDataset\""))
                .is_err()
        );
        assert!(parse(&VRT.replace("<ScaleOffset>", "<LUT>0:0</LUT><ScaleOffset>")).is_err());
        let band =
            |sub: &str| VRT.replace("band=\"1\">", &format!("band=\"1\" subClass=\"{sub}\">"));
        assert!(parse(&band("VRTSourcedRasterBand")).is_ok());
        assert!(parse(&band("VRTDerivedRasterBand")).is_err());
        assert_eq!(to_type("Byte", 300.4), 255.0);
        assert_eq!(to_type("Int16", -2.5), -3.0);
    }

    #[test]
    fn nearest_spans() {
        // Halving: each VRT pixel takes every other source pixel.
        assert_eq!(spans(0.0, 2.0, 0.0, 4.0, 2), [(0, 1), (1, 3)]);
        // Doubling.
        assert_eq!(
            spans(0.0, 4.0, 0.0, 2.0, 4),
            [(0, 0), (1, 0), (2, 1), (3, 1)]
        );
        // Clipped to the raster, and an offset window.
        assert_eq!(spans(3.0, 2.0, 5.0, 2.0, 4), [(3, 5)]);
        assert!(spans(0.0, 0.0, 0.0, 1.0, 4).is_empty());
    }
}
