//! Sentinel-2 products in their SAFE layout: the product metadata (`MTD_MSIL1C.xml`,
//! `MTD_MSIL2A.xml`: level, spacecraft, sensing time, processing baseline, the band files,
//! the quantification value and per-band offsets, cloud cover) and the tile metadata
//! (`GRANULE/*/MTD_TL.xml`: the CRS and each resolution's grid), and how a band is brought
//! to another resolution.
//!
//! Grids of one tile nest exactly (10980, 5490 and 1830 pixels for 10, 20 and 60 m, with
//! the same corner), so resampling is by whole factors: to a coarser grid the block mean of
//! the valid samples, to a finer one each sample repeated.

use roxmltree::{Document, Node};

use crate::geotiff::Affine2;

/// The thirteen bands: name, native resolution (m), central wavelength (nm, Sentinel-2A).
pub const BANDS: [(&str, u32, f64); 13] = [
    ("B01", 60, 442.7),
    ("B02", 10, 492.7),
    ("B03", 10, 559.8),
    ("B04", 10, 664.6),
    ("B05", 20, 704.1),
    ("B06", 20, 740.5),
    ("B07", 20, 782.8),
    ("B08", 10, 832.8),
    ("B8A", 20, 864.7),
    ("B09", 60, 945.1),
    ("B10", 60, 1373.5),
    ("B11", 20, 1613.7),
    ("B12", 20, 2202.4),
];

/// Resolutions a product offers, coarse first.
pub const RESOLUTIONS: [u32; 3] = [60, 20, 10];

/// Processing level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Top-of-atmosphere reflectance.
    L1C,
    /// Bottom-of-atmosphere reflectance with a scene classification.
    L2A,
}

/// What the product metadata says.
#[derive(Clone, Debug)]
pub struct Product {
    pub level: Level,
    pub spacecraft: String,
    pub start_time: String,
    pub baseline: String,
    pub product_type: String,
    /// Reflectance = (DN + offset) / quantification.
    pub quantification: f64,
    /// Per band (in [`BANDS`] order), 0 before processing baseline 04.00.
    pub offsets: [f64; 13],
    /// Cloud cover of the tile, percent.
    pub cloud_cover: Option<f64>,
    /// The band files as the metadata lists them (paths in the SAFE folder, no extension).
    pub files: Vec<String>,
}

/// What the tile metadata says.
#[derive(Clone, Debug, Default)]
pub struct Tile {
    pub epsg: Option<u32>,
    pub cs_name: String,
    pub tile_id: String,
    pub sensing_time: String,
    /// (resolution, columns, rows).
    pub sizes: Vec<(u32, u32, u32)>,
    /// (resolution, upper-left x, upper-left y, x step, y step).
    pub positions: Vec<(u32, f64, f64, f64, f64)>,
}

fn parse(text: &str) -> Result<Document<'_>, String> {
    if !specular_limits::xml_within_depth(text.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err("the metadata nests too deeply".into());
    }
    Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            ..Default::default()
        },
    )
    .map_err(|e| format!("unreadable metadata: {e}"))
}

fn named<'a>(doc: &'a Document, name: &str) -> impl Iterator<Item = Node<'a, 'a>> {
    let name = name.to_owned();
    doc.descendants()
        .filter(move |n| n.is_element() && n.tag_name().name() == name)
}

fn text_of(doc: &Document, name: &str) -> Option<String> {
    named(doc, name)
        .next()
        .and_then(|n| n.text())
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// Whether the start of a file is a Sentinel-2 product's metadata.
pub fn sniff_product(head: &[u8]) -> bool {
    let h = &head[..head.len().min(4096)];
    let has = |s: &[u8]| h.windows(s.len()).any(|w| w == s);
    has(b"<?xml") && (has(b"Level-2A_User_Product") || has(b"Level-1C_User_Product"))
}

/// Parses `MTD_MSIL1C.xml` or `MTD_MSIL2A.xml`.
pub fn parse_product(text: &str) -> Result<Product, String> {
    let doc = parse(text)?;
    let root = doc.root_element().tag_name().name().to_owned();
    let level = if root.contains("Level-2A") {
        Level::L2A
    } else if root.contains("Level-1C") {
        Level::L1C
    } else {
        return Err("not a Sentinel-2 L1C or L2A product".into());
    };
    let quant = |n: &str| text_of(&doc, n).and_then(|v| v.parse::<f64>().ok());
    let quantification = match level {
        Level::L2A => quant("BOA_QUANTIFICATION_VALUE"),
        Level::L1C => quant("QUANTIFICATION_VALUE"),
    }
    .filter(|q| *q > 0.0)
    .unwrap_or(10000.0);
    let mut offsets = [0.0; 13];
    let offset_tag = match level {
        Level::L2A => "BOA_ADD_OFFSET",
        Level::L1C => "RADIO_ADD_OFFSET",
    };
    for n in named(&doc, offset_tag) {
        if let (Some(id), Some(v)) = (
            n.attribute("band_id").and_then(|v| v.parse::<usize>().ok()),
            n.text().and_then(|t| t.trim().parse::<f64>().ok()),
        ) && id < 13
        {
            offsets[id] = v;
        }
    }
    let files = named(&doc, "IMAGE_FILE")
        .filter_map(|n| n.text())
        .map(|t| t.trim().trim_start_matches('/').to_owned())
        .filter(|t| !t.is_empty() && !t.contains(".."))
        .collect::<Vec<_>>();
    if files.is_empty() {
        return Err("the product lists no band files".into());
    }
    Ok(Product {
        level,
        spacecraft: text_of(&doc, "SPACECRAFT_NAME").unwrap_or_default(),
        start_time: text_of(&doc, "PRODUCT_START_TIME").unwrap_or_default(),
        baseline: text_of(&doc, "PROCESSING_BASELINE").unwrap_or_default(),
        product_type: text_of(&doc, "PRODUCT_TYPE").unwrap_or_default(),
        quantification,
        offsets,
        cloud_cover: text_of(&doc, "Cloud_Coverage_Assessment").and_then(|v| v.parse().ok()),
        files,
    })
}

/// Parses a granule's `MTD_TL.xml`.
pub fn parse_tile(text: &str) -> Result<Tile, String> {
    let doc = parse(text)?;
    let mut t = Tile {
        epsg: text_of(&doc, "HORIZONTAL_CS_CODE")
            .and_then(|c| c.rsplit(':').next().and_then(|v| v.parse().ok())),
        cs_name: text_of(&doc, "HORIZONTAL_CS_NAME").unwrap_or_default(),
        tile_id: text_of(&doc, "TILE_ID").unwrap_or_default(),
        sensing_time: text_of(&doc, "SENSING_TIME").unwrap_or_default(),
        ..Default::default()
    };
    let child = |n: Node, name: &str| -> Option<f64> {
        n.children()
            .find(|c| c.tag_name().name() == name)?
            .text()?
            .trim()
            .parse()
            .ok()
    };
    for n in named(&doc, "Size") {
        if let (Some(r), Some(rows), Some(cols)) = (
            n.attribute("resolution").and_then(|v| v.parse().ok()),
            child(n, "NROWS"),
            child(n, "NCOLS"),
        ) && (1.0..=65536.0).contains(&rows)
            && (1.0..=65536.0).contains(&cols)
        {
            t.sizes.push((r, cols as u32, rows as u32));
        }
    }
    for n in named(&doc, "Geoposition") {
        if let (Some(r), Some(x), Some(y), Some(dx), Some(dy)) = (
            n.attribute("resolution").and_then(|v| v.parse().ok()),
            child(n, "ULX"),
            child(n, "ULY"),
            child(n, "XDIM"),
            child(n, "YDIM"),
        ) {
            t.positions.push((r, x, y, dx, dy));
        }
    }
    if t.sizes.is_empty() {
        return Err("the tile metadata gives no grid".into());
    }
    Ok(t)
}

impl Tile {
    /// The grid at resolution `r`: columns and rows, from the metadata or from another
    /// resolution's grid scaled.
    pub fn size(&self, r: u32) -> Option<(u32, u32)> {
        if let Some(&(_, w, h)) = self.sizes.iter().find(|s| s.0 == r) {
            return Some((w, h));
        }
        let &(r0, w, h) = self.sizes.first()?;
        let scale = |v: u32| (u64::from(v) * u64::from(r0)).div_ceil(u64::from(r)) as u32;
        Some((scale(w), scale(h)))
    }

    /// Pixel corner to map at resolution `r`.
    pub fn transform(&self, r: u32) -> Option<Affine2> {
        let &(_, x, y, _, _) = self
            .positions
            .iter()
            .find(|p| p.0 == r)
            .or(self.positions.first())?;
        let s = f64::from(r);
        Some(Affine2 {
            a: s,
            b: 0.0,
            c: x,
            d: 0.0,
            e: -s,
            f: y,
        })
    }
}

impl Product {
    /// The file to read band `band` ("B04", "SCL", "TCI") from for a grid of resolution
    /// `r`, and that file's resolution: the file at `r` when the product has one (L2A),
    /// else the nearest finer one, else the nearest coarser one.
    pub fn band_file(&self, band: &str, r: u32) -> Option<(&str, u32)> {
        let native = BANDS
            .iter()
            .find(|b| b.0 == band)
            .map(|b| b.1)
            .unwrap_or(20);
        let mut found: Vec<(&str, u32)> = self
            .files
            .iter()
            .filter_map(|f| {
                let name = f.rsplit('/').next()?;
                let mut parts = name.rsplit('_');
                let last = parts.next()?;
                if last == band {
                    return Some((f.as_str(), native));
                }
                let res = last.strip_suffix('m')?.parse::<u32>().ok()?;
                (parts.next()? == band).then_some((f.as_str(), res))
            })
            .collect();
        found.sort_by_key(|&(_, fr)| {
            if fr == r {
                (0, 0)
            } else if fr < r {
                (1, r - fr)
            } else {
                (2, fr - r)
            }
        });
        found.first().copied()
    }

    /// The granule folder (`GRANULE/<name>`) the band files are in.
    pub fn granule(&self) -> Option<&str> {
        let f = self.files.first()?;
        let mut it = f.match_indices('/');
        let (_, _) = it.next()?;
        let (end, _) = it.next()?;
        Some(&f[..end])
    }

    /// Band index in [`BANDS`].
    pub fn band_index(band: &str) -> Option<usize> {
        BANDS.iter().position(|b| b.0 == band)
    }
}

/// Brings `src` (`sw × sh` samples at resolution `sr`) to the grid of resolution `r`
/// (`w × h`): block means of the finite samples to a coarser grid, repeated samples to a
/// finer one. Factors must divide (`r / sr` or `sr / r` whole).
pub fn resample(
    src: &[f32],
    (sw, sh): (u32, u32),
    sr: u32,
    r: u32,
    (w, h): (u32, u32),
) -> Vec<f32> {
    let (sw, sh, w, h) = (sw as usize, sh as usize, w as usize, h as usize);
    let mut out = vec![f32::NAN; w * h];
    if sr == r {
        for y in 0..h.min(sh) {
            out[y * w..y * w + w.min(sw)].copy_from_slice(&src[y * sw..y * sw + w.min(sw)]);
        }
    } else if sr < r {
        let k = (r / sr.max(1)).max(1) as usize;
        for y in 0..h {
            for x in 0..w {
                let (mut sum, mut n) = (0f64, 0u32);
                for yy in y * k..((y + 1) * k).min(sh) {
                    for &v in &src[yy * sw + (x * k).min(sw)..yy * sw + ((x + 1) * k).min(sw)] {
                        if v.is_finite() {
                            sum += f64::from(v);
                            n += 1;
                        }
                    }
                }
                if n > 0 {
                    out[y * w + x] = (sum / f64::from(n)) as f32;
                }
            }
        }
    } else {
        let k = (sr / r.max(1)).max(1) as usize;
        for y in 0..h {
            let sy = y / k;
            if sy >= sh {
                break;
            }
            for x in 0..w {
                let sx = x / k;
                if sx < sw {
                    out[y * w + x] = src[sy * sw + sx];
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const L2A: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<n1:Level-2A_User_Product xmlns:n1="https://psd-14.sentinel2.eo.esa.int/PSD/User_Product_Level-2A.xsd">
<n1:General_Info><Product_Info><PRODUCT_START_TIME>2015-07-04T10:10:06.027Z</PRODUCT_START_TIME>
<PROCESSING_LEVEL>Level-2A</PROCESSING_LEVEL><PRODUCT_TYPE>S2MSI2A</PRODUCT_TYPE>
<PROCESSING_BASELINE>05.00</PROCESSING_BASELINE>
<Datatake><SPACECRAFT_NAME>Sentinel-2A</SPACECRAFT_NAME></Datatake>
<Product_Organisation><Granule_List><Granule>
<IMAGE_FILE>GRANULE/L2A_T1/IMG_DATA/R10m/T33TUN_X_B04_10m</IMAGE_FILE>
<IMAGE_FILE>GRANULE/L2A_T1/IMG_DATA/R20m/T33TUN_X_B04_20m</IMAGE_FILE>
<IMAGE_FILE>GRANULE/L2A_T1/IMG_DATA/R20m/T33TUN_X_SCL_20m</IMAGE_FILE>
<IMAGE_FILE>GRANULE/L2A_T1/IMG_DATA/R60m/T33TUN_X_B01_60m</IMAGE_FILE>
</Granule></Granule_List></Product_Organisation></Product_Info>
<Product_Image_Characteristics><QUANTIFICATION_VALUES_LIST>
<BOA_QUANTIFICATION_VALUE unit="none">10000</BOA_QUANTIFICATION_VALUE></QUANTIFICATION_VALUES_LIST>
<BOA_ADD_OFFSET_VALUES_LIST><BOA_ADD_OFFSET band_id="3">-1000</BOA_ADD_OFFSET></BOA_ADD_OFFSET_VALUES_LIST>
</Product_Image_Characteristics></n1:General_Info>
<n1:Quality_Indicators_Info><Cloud_Coverage_Assessment>12.5</Cloud_Coverage_Assessment></n1:Quality_Indicators_Info>
</n1:Level-2A_User_Product>"#;

    const TL: &str = r#"<?xml version="1.0"?><n1:Level-2A_Tile_ID xmlns:n1="x">
<n1:General_Info><TILE_ID>T1</TILE_ID><SENSING_TIME>2015-07-04T10:17:36Z</SENSING_TIME></n1:General_Info>
<n1:Geometric_Info><Tile_Geocoding><HORIZONTAL_CS_NAME>WGS84 / UTM zone 33N</HORIZONTAL_CS_NAME>
<HORIZONTAL_CS_CODE>EPSG:32633</HORIZONTAL_CS_CODE>
<Size resolution="10"><NROWS>10980</NROWS><NCOLS>10980</NCOLS></Size>
<Size resolution="60"><NROWS>1830</NROWS><NCOLS>1830</NCOLS></Size>
<Geoposition resolution="10"><ULX>300000</ULX><ULY>5300040</ULY><XDIM>10</XDIM><YDIM>-10</YDIM></Geoposition>
</Tile_Geocoding></n1:Geometric_Info></n1:Level-2A_Tile_ID>"#;

    #[test]
    fn product_and_tile() {
        assert!(sniff_product(L2A.as_bytes()));
        let p = parse_product(L2A).unwrap();
        assert_eq!(p.level, Level::L2A);
        assert_eq!(p.spacecraft, "Sentinel-2A");
        assert_eq!(p.offsets[3], -1000.0);
        assert_eq!(p.offsets[0], 0.0);
        assert_eq!(p.cloud_cover, Some(12.5));
        assert_eq!(p.granule(), Some("GRANULE/L2A_T1"));
        assert_eq!(p.band_file("B04", 20).unwrap().1, 20);
        assert_eq!(p.band_file("B04", 60).unwrap().1, 20);
        assert_eq!(p.band_file("B04", 10).unwrap().1, 10);
        assert_eq!(p.band_file("SCL", 10).unwrap().1, 20);
        assert_eq!(p.band_file("B01", 10).unwrap().1, 60);
        assert!(p.band_file("B08", 10).is_none());
        let t = parse_tile(TL).unwrap();
        assert_eq!(t.epsg, Some(32633));
        assert_eq!(t.size(20), Some((5490, 5490)));
        assert_eq!(t.size(60), Some((1830, 1830)));
        let a = t.transform(60).unwrap();
        assert_eq!((a.a, a.c, a.e, a.f), (60.0, 300000.0, -60.0, 5300040.0));
    }

    #[test]
    fn resampling() {
        // 4 × 2 at 10 m to 20 m: block means skipping NaN.
        let s = [1.0, 3.0, 5.0, f32::NAN, 1.0, 3.0, 7.0, f32::NAN];
        assert_eq!(resample(&s, (4, 2), 10, 20, (2, 1)), [2.0, 6.0]);
        // 2 × 1 at 20 m to 10 m.
        assert_eq!(
            resample(&[1.0, 2.0], (2, 1), 20, 10, (4, 2)),
            [1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0]
        );
        assert_eq!(resample(&[4.0], (1, 1), 60, 60, (1, 1)), [4.0]);
    }
}
