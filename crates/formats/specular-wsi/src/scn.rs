//! Leica SCN: an XML ImageDescription listing a collection (the slide, sizes in nanometres)
//! and its images, each with its pixels' resolutions (`dimension` elements naming their IFD)
//! and its view on the slide. The macro image covers the whole collection; the others are
//! scanned regions.

use crate::xml::{self, Element};
use crate::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ScnImage {
    pub name: String,
    pub macro_image: bool,
    /// (width, height, IFD) per resolution, largest first (channel 0, z 0).
    pub dims: Vec<(u32, u32, u32)>,
    /// Nanometres per pixel at full resolution.
    pub nm_per_px: Option<f64>,
    pub objective: Option<f64>,
    pub device: Option<String>,
    pub date: Option<String>,
    pub illumination: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Scn {
    pub barcode: Option<String>,
    pub images: Vec<ScnImage>,
}

pub(crate) fn is_scn(desc: &str) -> bool {
    let head = crate::tiffslide::head(desc, 512);
    head.contains("<scn") && head.contains("leica")
}

fn num(e: &Element, k: &str) -> Option<u64> {
    e.attr(k)?.trim().parse().ok()
}

pub(crate) fn parse(desc: &str) -> Result<Scn> {
    let root = xml::parse(desc)?;
    let coll = root
        .child("collection")
        .ok_or_else(|| Error::Malformed("SCN without a collection".into()))?;
    let (cw, ch) = (num(coll, "sizeX"), num(coll, "sizeY"));
    let mut images = Vec::new();
    for im in coll.all("image") {
        let Some(px) = im.child("pixels") else {
            continue;
        };
        let mut dims: Vec<(u32, u32, u32)> = px
            .all("dimension")
            .filter(|d| num(d, "c").unwrap_or(0) == 0 && num(d, "z").unwrap_or(0) == 0)
            .filter_map(|d| {
                Some((
                    u32::try_from(num(d, "sizeX")?).ok()?,
                    u32::try_from(num(d, "sizeY")?).ok()?,
                    u32::try_from(num(d, "ifd")?).ok()?,
                ))
            })
            .filter(|d| d.0 > 0 && d.1 > 0)
            .collect();
        dims.sort_by_key(|d| std::cmp::Reverse(d.0));
        dims.dedup_by_key(|d| d.0);
        let Some(&(w, _, _)) = dims.first() else {
            continue;
        };
        let view = im.child("view");
        let (vw, vh, vx, vy) = view.map_or((None, None, None, None), |v| {
            (
                num(v, "sizeX"),
                num(v, "sizeY"),
                num(v, "offsetX"),
                num(v, "offsetY"),
            )
        });
        let macro_image = vx == Some(0) && vy == Some(0) && vw.is_some() && vw == cw && vh == ch;
        let scan = im.find("objective");
        let device = im.child("device").and_then(|d| {
            let s = [d.attr("model"), d.attr("version")]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            (!s.trim().is_empty()).then(|| s.trim().to_owned())
        });
        images.push(ScnImage {
            name: im.attr("name").unwrap_or("").to_owned(),
            macro_image,
            dims,
            nm_per_px: vw.map(|vw| vw as f64 / f64::from(w)).filter(|v| *v > 0.0),
            objective: scan.and_then(|o| o.text.trim().parse().ok()),
            device,
            date: im.child_text("creationDate").map(str::to_owned),
            illumination: im.find("illuminationSource").map(|e| e.text.clone()),
        });
    }
    if images.is_empty() {
        return Err(Error::Malformed("SCN without images".into()));
    }
    Ok(Scn {
        barcode: coll.child_text("barcode").map(str::to_owned),
        images,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_collection_and_images() {
        let x = r#"<?xml version="1.0"?><scn xmlns="http://www.leica-microsystems.com/scn/2010/10/01">
<collection name="s" uuid="u" sizeX="76000000" sizeY="26000000"><barcode>X1</barcode>
<image name="macro" uuid="m"><creationDate>2010-10-21T09:00:00Z</creationDate><device model="Leica SCN400" version="1.4"/>
<pixels sizeX="1616" sizeY="552"><dimension sizeX="1616" sizeY="552" r="0" ifd="0"/></pixels>
<view sizeX="76000000" sizeY="26000000" offsetX="0" offsetY="0" spacingZ="0"/></image>
<image name="main" uuid="i"><creationDate>2010-10-21T09:10:00Z</creationDate><device model="Leica SCN400" version="1.4"/>
<pixels sizeX="4668" sizeY="2944"><dimension sizeX="4668" sizeY="2944" r="0" ifd="1"/><dimension sizeX="1167" sizeY="736" r="1" ifd="2"/></pixels>
<view sizeX="2333894" sizeY="1471919" offsetX="1" offsetY="2"/><scanSettings><objectiveSettings><objective>20</objective></objectiveSettings></scanSettings></image>
</collection></scn>"#;
        assert!(is_scn(x));
        let s = parse(x).unwrap();
        assert_eq!(s.barcode.as_deref(), Some("X1"));
        assert!(s.images[0].macro_image);
        let m = &s.images[1];
        assert!(!m.macro_image);
        assert_eq!(m.dims, vec![(4668, 2944, 1), (1167, 736, 2)]);
        assert_eq!(m.objective, Some(20.0));
        assert!((m.nm_per_px.unwrap() - 499.977).abs() < 0.01);
    }
}
