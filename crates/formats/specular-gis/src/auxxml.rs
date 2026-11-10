//! GDAL's persistent auxiliary metadata (`<file>.aux.xml`, a `PAMDataset`): the CRS
//! (`SRS`), the geotransform, dataset metadata and, per band, the description, no-data
//! value, unit, scale and offset, metadata items (statistics among them) and category
//! names.

use crate::crs::Crs;
use crate::geotiff::{Affine2, GdalItem, GdalMetadata, parse_nodata};

/// One `PAMRasterBand`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuxBand {
    /// 1-based band number.
    pub band: u32,
    pub description: Option<String>,
    pub nodata: Option<f64>,
    pub unit: Option<String>,
    pub offset: Option<f64>,
    pub scale: Option<f64>,
    /// `STATISTICS_*` and other metadata items: (domain, key, value).
    pub metadata: Vec<(String, String, String)>,
    pub categories: Vec<String>,
}

impl AuxBand {
    /// A statistics item (`MINIMUM`, `MAXIMUM`, `MEAN`, `STDDEV`, `VALID_PERCENT`).
    pub fn statistic(&self, name: &str) -> Option<f64> {
        let key = format!("STATISTICS_{name}");
        self.metadata
            .iter()
            .find(|(_, k, _)| k.eq_ignore_ascii_case(&key))
            .and_then(|(_, _, v)| v.trim().parse().ok())
    }
}

/// A parsed `.aux.xml`.
#[derive(Clone, Debug, Default)]
pub struct AuxXml {
    pub crs: Option<Crs>,
    /// The SRS text as written.
    pub srs: Option<String>,
    /// Pixel corner to map (GDAL's geotransform).
    pub transform: Option<Affine2>,
    /// Dataset metadata: (domain, key, value).
    pub metadata: Vec<(String, String, String)>,
    pub bands: Vec<AuxBand>,
}

fn text(n: roxmltree::Node) -> String {
    n.descendants()
        .filter(|d| d.is_text())
        .filter_map(|d| d.text())
        .collect::<String>()
        .trim()
        .to_string()
}

fn metadata(n: roxmltree::Node, out: &mut Vec<(String, String, String)>) {
    for md in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "Metadata")
    {
        let domain = md.attribute("domain").unwrap_or("").to_string();
        // `format="xml"` domains hold a document, not items.
        for mdi in md
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "MDI")
            .take(4096)
        {
            out.push((
                domain.clone(),
                mdi.attribute("key").unwrap_or("").to_string(),
                text(mdi),
            ));
        }
    }
}

/// Parses a `PAMDataset` document; `None` when it is not one.
pub fn parse(xml: &str) -> Option<AuxXml> {
    if !specular_limits::xml_within_depth(xml.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return None;
    }
    let doc = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}')).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "PAMDataset" {
        return None;
    }
    let mut aux = AuxXml::default();
    for c in root.children().filter(|c| c.is_element()) {
        match c.tag_name().name() {
            "SRS" => {
                let s = text(c);
                if !s.is_empty() {
                    aux.crs = crate::worldfile::parse_prj(&s);
                    aux.srs = Some(s);
                }
            }
            "GeoTransform" => {
                let v: Vec<f64> = text(c)
                    .split(',')
                    .filter_map(|t| t.trim().parse().ok())
                    .collect();
                if let [c0, a, b, f, d, e] = v[..]
                    && a * e - b * d != 0.0
                    && v.iter().all(|x| x.is_finite())
                {
                    aux.transform = Some(Affine2 {
                        a,
                        b,
                        c: c0,
                        d,
                        e,
                        f,
                    });
                }
            }
            "PAMRasterBand" => {
                let mut b = AuxBand {
                    band: c
                        .attribute("band")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0),
                    ..Default::default()
                };
                for e in c.children().filter(|e| e.is_element()) {
                    let t = text(e);
                    match e.tag_name().name() {
                        "Description" => b.description = Some(t).filter(|s| !s.is_empty()),
                        "NoDataValue" => b.nodata = parse_nodata(&t),
                        "UnitType" => b.unit = Some(t).filter(|s| !s.is_empty()),
                        "Offset" => b.offset = t.parse().ok(),
                        "Scale" => b.scale = t.parse().ok(),
                        "CategoryNames" => {
                            b.categories = e
                                .children()
                                .filter(|c| c.is_element())
                                .take(65536)
                                .map(text)
                                .collect()
                        }
                        _ => {}
                    }
                }
                metadata(c, &mut b.metadata);
                aux.bands.push(b);
            }
            _ => {}
        }
    }
    metadata(root, &mut aux.metadata);
    aux.bands.sort_by_key(|b| b.band);
    Some(aux)
}

impl AuxXml {
    /// A band by its 1-based number.
    pub fn band(&self, n: u32) -> Option<&AuxBand> {
        self.bands.iter().find(|b| b.band == n)
    }

    /// The items as GDAL metadata (what the info panel lists for GeoTIFF's GDAL_METADATA):
    /// dataset items with no sample, band items with `sample` = band − 1, and the band's
    /// description, unit, scale and offset as roles.
    pub fn to_gdal(&self) -> GdalMetadata {
        let mut items: Vec<GdalItem> = self
            .metadata
            .iter()
            .filter(|(d, _, _)| d.is_empty())
            .map(|(_, k, v)| GdalItem {
                name: k.clone(),
                sample: None,
                role: None,
                value: v.clone(),
            })
            .collect();
        for b in &self.bands {
            let s = Some(b.band.saturating_sub(1));
            let mut role = |r: &str, v: String| {
                items.push(GdalItem {
                    name: r.to_uppercase(),
                    sample: s,
                    role: Some(r.into()),
                    value: v,
                })
            };
            if let Some(d) = &b.description {
                role("description", d.clone());
            }
            if let Some(u) = &b.unit {
                role("unittype", u.clone());
            }
            if let Some(v) = b.scale {
                role("scale", v.to_string());
            }
            if let Some(v) = b.offset {
                role("offset", v.to_string());
            }
            for (d, k, v) in &b.metadata {
                if d.is_empty() {
                    items.push(GdalItem {
                        name: k.clone(),
                        sample: s,
                        role: None,
                        value: v.clone(),
                    });
                }
            }
        }
        GdalMetadata { items }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUX: &str = r#"<PAMDataset>
  <SRS dataAxisToSRSAxisMapping="2,1">GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,298.257223563,AUTHORITY["EPSG","7030"]],AUTHORITY["EPSG","6326"]],PRIMEM["Greenwich",0],UNIT["degree",0.0174532925199433],AUTHORITY["EPSG","4326"]]</SRS>
  <GeoTransform>  1.0000000000000000e+01,  1.0000000000000000e-01,  0.0000000000000000e+00,  5.0000000000000000e+01,  0.0000000000000000e+00, -1.0000000000000000e-01</GeoTransform>
  <Metadata>
    <MDI key="AREA_OR_POINT">Area</MDI>
  </Metadata>
  <PAMRasterBand band="1">
    <Description>elevation</Description>
    <NoDataValue>-9.99900000000000E+03</NoDataValue>
    <UnitType>m</UnitType>
    <Offset>0</Offset>
    <Scale>0.5</Scale>
    <Metadata>
      <MDI key="STATISTICS_MAXIMUM">812</MDI>
      <MDI key="STATISTICS_MINIMUM">-3.5</MDI>
    </Metadata>
  </PAMRasterBand>
</PAMDataset>"#;

    #[test]
    fn pam() {
        let a = parse(AUX).unwrap();
        assert_eq!(a.crs.as_ref().unwrap().epsg, Some(4326));
        let t = a.transform.unwrap();
        assert_eq!(t.apply(10.0, 10.0), (11.0, 49.0));
        assert_eq!(a.metadata[0].1, "AREA_OR_POINT");
        let b = a.band(1).unwrap();
        assert_eq!(b.nodata, Some(-9999.0));
        assert_eq!(b.unit.as_deref(), Some("m"));
        assert_eq!(b.scale, Some(0.5));
        assert_eq!(b.statistic("MAXIMUM"), Some(812.0));
        assert_eq!(b.statistic("MINIMUM"), Some(-3.5));
        let g = a.to_gdal();
        assert_eq!(g.band_role(0, "unittype"), Some("m"));
        assert_eq!(g.named("STATISTICS_MAXIMUM", Some(0)), Some("812"));
        assert!(parse("<GDALMetadata/>").is_none());
    }
}
