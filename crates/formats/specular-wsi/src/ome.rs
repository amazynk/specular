//! OME-XML (the OME-TIFF ImageDescription and OME-Zarr's `OME/METADATA.ome.xml`): images,
//! their pixels (sizes, dimension order, type, physical sizes with units), channels (names,
//! colours, samples per pixel) and the TiffData blocks that map planes to IFDs and files.

use crate::xml::{self, Element};
use crate::{Error, Result};

/// One TiffData block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TiffData {
    pub ifd: u32,
    pub first_c: u32,
    pub first_z: u32,
    pub first_t: u32,
    pub plane_count: Option<u32>,
    pub ifd_given: bool,
    /// The file holding the planes (`UUID FileName`), when another than the described one.
    pub file: Option<String>,
    /// The file's UUID.
    pub uuid: Option<String>,
    /// The file is named with a path (outside the folder): its planes are never read.
    pub outside: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OmeChannel {
    pub name: String,
    pub colour: Option<[u8; 3]>,
    pub samples: u32,
    pub emission: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OmeImage {
    pub id: String,
    pub name: Option<String>,
    pub size_x: u32,
    pub size_y: u32,
    pub size_z: u32,
    /// SizeC as stored (samples included).
    pub size_c: u32,
    pub size_t: u32,
    /// "XYZCT" and the like.
    pub order: String,
    pub pixel_type: String,
    /// Physical pixel sizes in microns (x, y, z).
    pub physical: [Option<f64>; 3],
    /// Seconds between time points.
    pub time_increment: Option<f64>,
    pub channels: Vec<OmeChannel>,
    pub tiff_data: Vec<TiffData>,
    pub acquired: Option<String>,
    pub objective: Option<f64>,
    pub microscope: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ome {
    pub images: Vec<OmeImage>,
    pub creator: Option<String>,
    /// The described file's own UUID.
    pub uuid: Option<String>,
}

/// Microns in one `unit` (OME's UnitsLength; microns when absent).
pub fn microns(unit: Option<&str>) -> Option<f64> {
    Some(match unit.unwrap_or("µm").trim() {
        "µm" | "um" | "μm" | "micron" | "microns" | "micrometer" | "micrometre" => 1.0,
        "nm" | "nanometer" | "nanometre" => 1e-3,
        "pm" => 1e-6,
        "Å" | "angstrom" => 1e-4,
        "mm" | "millimeter" | "millimetre" => 1e3,
        "cm" | "centimeter" => 1e4,
        "m" | "meter" | "metre" => 1e6,
        "in" | "inch" => 25_400.0,
        _ => return None,
    })
}

/// Seconds in one `unit` (OME's UnitsTime; seconds when absent).
pub fn seconds(unit: Option<&str>) -> Option<f64> {
    Some(match unit.unwrap_or("s").trim() {
        "s" | "second" | "seconds" => 1.0,
        "ms" | "millisecond" | "milliseconds" => 1e-3,
        "µs" | "us" | "μs" | "microsecond" | "microseconds" => 1e-6,
        "ns" => 1e-9,
        "min" | "minute" | "minutes" => 60.0,
        "h" | "hour" | "hours" => 3600.0,
        _ => return None,
    })
}

/// OME's signed 32-bit RGBA colour.
fn colour(v: &str) -> Option<[u8; 3]> {
    let n: i64 = v.trim().parse().ok()?;
    let u = n as i32 as u32;
    Some([(u >> 24) as u8, (u >> 16) as u8, (u >> 8) as u8])
}

fn uint(e: &Element, k: &str) -> Option<u32> {
    e.attr(k)?.trim().parse().ok()
}

/// A file name of the same folder only: no separators, no parent references.
pub fn plain_name(name: &str) -> Option<&str> {
    let n = name.trim();
    (!n.is_empty() && !n.contains(['/', '\\']) && n != "." && n != "..").then_some(n)
}

/// A "BinaryOnly" OME-XML document (one file of a set whose metadata is a companion file):
/// the companion's name when it is a plain name in the same folder, and the document's UUID.
pub fn binary_only(src: &str) -> Option<(String, Option<String>)> {
    let root = xml::parse(src).ok()?;
    if root.name != "OME" {
        return None;
    }
    let b = root.child("BinaryOnly")?;
    let name = plain_name(b.attr("MetadataFile")?)?;
    Some((name.to_owned(), root.attr("UUID").map(str::to_owned)))
}

/// Parse an OME-XML document.
pub fn parse(src: &str) -> Result<Ome> {
    let root = xml::parse(src)?;
    if root.name != "OME" {
        return Err(Error::Malformed("not OME-XML".into()));
    }
    let instruments: Vec<&Element> = root.all("Instrument").collect();
    let mut images = Vec::new();
    for im in root.all("Image") {
        let Some(px) = im.child("Pixels") else {
            continue;
        };
        let dim = |k: &str| uint(px, k).unwrap_or(1).max(1);
        let size = |k: &str, u: &str| {
            let v = px.num(k)?;
            let f = microns(px.attr(u))?;
            Some(v * f).filter(|v| *v > 0.0 && v.is_finite())
        };
        let channels = px
            .all("Channel")
            .map(|c| OmeChannel {
                name: c.attr("Name").unwrap_or("").to_owned(),
                colour: c.attr("Color").and_then(colour),
                samples: uint(c, "SamplesPerPixel").unwrap_or(1).max(1),
                emission: c.num("EmissionWavelength"),
            })
            .collect();
        let tiff_data = px
            .all("TiffData")
            .map(|t| {
                let u = t.child("UUID");
                TiffData {
                    ifd: uint(t, "IFD").unwrap_or(0),
                    ifd_given: t.attr("IFD").is_some(),
                    first_c: uint(t, "FirstC").unwrap_or(0),
                    first_z: uint(t, "FirstZ").unwrap_or(0),
                    first_t: uint(t, "FirstT").unwrap_or(0),
                    plane_count: uint(t, "PlaneCount"),
                    file: u
                        .and_then(|u| u.attr("FileName"))
                        .and_then(plain_name)
                        .map(str::to_owned),
                    uuid: u.map(|u| u.text.clone()).filter(|t| !t.is_empty()),
                    outside: u
                        .and_then(|u| u.attr("FileName"))
                        .is_some_and(|n| plain_name(n).is_none()),
                }
            })
            .collect();
        // The instrument and objective the image refers to.
        let inst = im
            .child("InstrumentRef")
            .and_then(|r| r.attr("ID"))
            .and_then(|id| instruments.iter().find(|i| i.attr("ID") == Some(id)))
            .or(instruments.first())
            .copied();
        let objective = inst.and_then(|i| {
            let wanted = im.child("ObjectiveSettings").and_then(|o| o.attr("ID"));
            let objs: Vec<&Element> = i.all("Objective").collect();
            wanted
                .and_then(|w| objs.iter().find(|o| o.attr("ID") == Some(w)))
                .or(objs.first())
                .and_then(|o| {
                    o.num("NominalMagnification")
                        .or(o.num("CalibratedMagnification"))
                })
        });
        let microscope = inst.and_then(|i| i.child("Microscope")).and_then(|m| {
            let s = [m.attr("Manufacturer"), m.attr("Model")]
                .into_iter()
                .flatten()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            (!s.is_empty()).then_some(s)
        });
        images.push(OmeImage {
            id: im.attr("ID").unwrap_or("").to_owned(),
            name: im.attr("Name").map(str::to_owned).filter(|n| !n.is_empty()),
            size_x: dim("SizeX"),
            size_y: dim("SizeY"),
            size_z: dim("SizeZ"),
            size_c: dim("SizeC"),
            size_t: dim("SizeT"),
            order: px.attr("DimensionOrder").unwrap_or("XYZCT").to_owned(),
            pixel_type: px.attr("Type").unwrap_or("").to_owned(),
            physical: [
                size("PhysicalSizeX", "PhysicalSizeXUnit"),
                size("PhysicalSizeY", "PhysicalSizeYUnit"),
                size("PhysicalSizeZ", "PhysicalSizeZUnit"),
            ],
            time_increment: px
                .num("TimeIncrement")
                .and_then(|v| Some(v * seconds(px.attr("TimeIncrementUnit"))?)),
            channels,
            tiff_data,
            acquired: im.child_text("AcquisitionDate").map(str::to_owned),
            objective,
            microscope,
        });
    }
    if images.is_empty() {
        return Err(Error::Malformed("OME-XML without images".into()));
    }
    Ok(Ome {
        images,
        creator: root.attr("Creator").map(str::to_owned),
        uuid: root.attr("UUID").map(str::to_owned),
    })
}

/// Where a plane is stored: the file (0 is the described one, else 1 + index into the file
/// list) and its main-chain IFD.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaneRef {
    pub file: usize,
    pub ifd: u32,
}

impl OmeImage {
    /// Samples per pixel of the channels (3 for an RGB image stored interleaved).
    pub fn samples(&self) -> u32 {
        self.channels.first().map_or(1, |c| c.samples).max(1)
    }

    /// Channels as planes: SizeC over samples per pixel.
    pub fn effective_c(&self) -> u32 {
        (self.size_c / self.samples()).max(1)
    }

    /// Number of planes.
    pub fn plane_count(&self) -> u64 {
        u64::from(self.effective_c())
            .saturating_mul(u64::from(self.size_z))
            .saturating_mul(u64::from(self.size_t))
    }

    /// The position of `(c, z, t)` in the dimension order's plane sequence (saturating: past
    /// the last plane when out of range).
    pub fn plane_index(&self, c: u32, z: u32, t: u32) -> u64 {
        let (sc, sz) = (u64::from(self.effective_c()), u64::from(self.size_z));
        let st = u64::from(self.size_t);
        let (c, z, t) = (u64::from(c), u64::from(z), u64::from(t));
        // a + sa × (b + sb × c)
        let mix = |a: u64, sa: u64, b: u64, sb: u64, c: u64| {
            a.saturating_add(sa.saturating_mul(b.saturating_add(sb.saturating_mul(c))))
        };
        match self.order.as_str() {
            "XYCZT" => mix(c, sc, z, sz, t),
            "XYZTC" => mix(z, sz, t, st, c),
            "XYCTZ" => mix(c, sc, t, st, z),
            "XYTZC" => mix(t, st, z, sz, c),
            "XYTCZ" => mix(t, st, c, sc, z),
            // XYZCT, the default.
            _ => mix(z, sz, c, sc, t),
        }
    }

    /// The (c, z, t) of the plane at `i` in the dimension order.
    pub fn plane_at(&self, i: u64) -> (u32, u32, u32) {
        let (sc, sz) = (u64::from(self.effective_c()), u64::from(self.size_z));
        let st = u64::from(self.size_t);
        let (a, b, cc) = match self.order.as_str() {
            "XYCZT" => (sc, sz, st),
            "XYZTC" => (sz, st, sc),
            "XYCTZ" => (sc, st, sz),
            "XYTZC" => (st, sz, sc),
            "XYTCZ" => (st, sc, sz),
            _ => (sz, sc, st),
        };
        let (i0, i1, i2) = (i % a, (i / a) % b, (i / a / b) % cc);
        let (i0, i1, i2) = (i0 as u32, i1 as u32, i2 as u32);
        match self.order.as_str() {
            "XYCZT" => (i0, i1, i2),
            "XYZTC" => (i2, i0, i1),
            "XYCTZ" => (i0, i2, i1),
            "XYTZC" => (i2, i1, i0),
            "XYTCZ" => (i1, i2, i0),
            _ => (i1, i0, i2),
        }
    }

    /// Where each plane is stored, by `c + C × (z + Z × t)`, with the other files the image
    /// needs. `own` tells whether a TiffData's file (by name or UUID) is the described one.
    pub fn plane_map(
        &self,
        own: &dyn Fn(Option<&str>, Option<&str>) -> bool,
        first_ifd: u32,
    ) -> (Vec<Option<PlaneRef>>, Vec<String>) {
        let total = self.plane_count().min(1 << 20) as usize;
        let mut map = vec![None; total];
        let mut files: Vec<String> = Vec::new();
        let canon = |i: u64| -> usize {
            let (c, z, t) = self.plane_at(i);
            let zt =
                u64::from(z).saturating_add(u64::from(self.size_z).saturating_mul(u64::from(t)));
            u64::from(c).saturating_add(u64::from(self.effective_c()).saturating_mul(zt)) as usize
        };
        if self.tiff_data.is_empty() {
            for (i, m) in (0..total as u64).map(|i| (i, canon(i))) {
                if let (Some(slot), Some(ifd)) = (map.get_mut(m), first_ifd.checked_add(i as u32)) {
                    *slot = Some(PlaneRef { file: 0, ifd });
                }
            }
            return (map, files);
        }
        let single = self.tiff_data.len() == 1;
        for td in self.tiff_data.iter().filter(|td| !td.outside) {
            let file = if own(td.file.as_deref(), td.uuid.as_deref()) {
                0
            } else if let Some(name) = &td.file {
                match files.iter().position(|f| f == name) {
                    Some(i) => i + 1,
                    None => {
                        files.push(name.clone());
                        files.len()
                    }
                }
            } else {
                0
            };
            let start = self.plane_index(td.first_c, td.first_z, td.first_t);
            let count = td.plane_count.map_or_else(
                || {
                    if single && !td.ifd_given {
                        total as u64
                    } else if single {
                        total as u64 - start.min(total as u64)
                    } else {
                        1
                    }
                },
                u64::from,
            );
            for k in 0..count.min(total as u64) {
                let i = start + k;
                if i >= total as u64 {
                    break;
                }
                let m = canon(i);
                if let (Some(slot), Some(ifd)) = (map.get_mut(m), td.ifd.checked_add(k as u32)) {
                    *slot = Some(PlaneRef { file, ifd });
                }
            }
        }
        (map, files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OME xmlns="http://www.openmicroscopy.org/Schemas/OME/2016-06" UUID="urn:uuid:a" Creator="test">
 <Instrument ID="Instrument:0"><Microscope Manufacturer="Zeiss" Model="Axio"/>
  <Objective ID="Objective:0" NominalMagnification="40"/></Instrument>
 <Image ID="Image:0" Name="cells"><AcquisitionDate>2020-01-02T03:04:05</AcquisitionDate>
  <InstrumentRef ID="Instrument:0"/><ObjectiveSettings ID="Objective:0"/>
  <Pixels ID="Pixels:0" DimensionOrder="XYCZT" Type="uint16" SizeX="64" SizeY="32" SizeZ="2" SizeC="3" SizeT="1"
   PhysicalSizeX="325" PhysicalSizeXUnit="nm" PhysicalSizeY="0.325" PhysicalSizeZ="2">
   <Channel ID="Channel:0:0" Name="DAPI" Color="65535" SamplesPerPixel="1"/>
   <Channel ID="Channel:0:1" Name="GFP" Color="16711935"/>
   <Channel ID="Channel:0:2" Name="RFP" Color="-16776961"/>
   <TiffData IFD="0" PlaneCount="3"><UUID FileName="a.ome.tif">urn:uuid:a</UUID></TiffData>
   <TiffData IFD="0" FirstZ="1" PlaneCount="3"><UUID FileName="b.ome.tif">urn:uuid:b</UUID></TiffData>
  </Pixels></Image></OME>"#;

    #[test]
    fn reads_pixels_channels_and_plane_map() {
        let o = parse(XML).unwrap();
        let im = &o.images[0];
        assert_eq!((im.size_x, im.size_y, im.size_z, im.size_c), (64, 32, 2, 3));
        assert!((im.physical[0].unwrap() - 0.325).abs() < 1e-9);
        assert_eq!(im.physical[2], Some(2.0));
        assert_eq!(im.channels[0].colour, Some([0, 0, 255]));
        assert_eq!(im.channels[1].colour, Some([0, 255, 0]));
        assert_eq!(im.channels[2].colour, Some([255, 0, 0]));
        assert_eq!(im.objective, Some(40.0));
        assert_eq!(im.microscope.as_deref(), Some("Zeiss Axio"));
        let own = |_: Option<&str>, u: Option<&str>| u == Some("urn:uuid:a");
        let (map, files) = im.plane_map(&own, 0);
        assert_eq!(files, vec!["b.ome.tif".to_owned()]);
        // c + 3 (z): z = 1 planes are in the second file.
        assert_eq!(map[0], Some(PlaneRef { file: 0, ifd: 0 }));
        assert_eq!(map[2], Some(PlaneRef { file: 0, ifd: 2 }));
        assert_eq!(map[4], Some(PlaneRef { file: 1, ifd: 1 }));
        for i in 0..6 {
            let (c, z, t) = im.plane_at(i);
            assert_eq!(im.plane_index(c, z, t), i);
        }
    }

    #[test]
    fn binary_only_names_its_companion() {
        let x = r#"<OME UUID="urn:uuid:z2"><BinaryOnly MetadataFile="set.companion.ome" UUID="urn:uuid:m"/></OME>"#;
        assert_eq!(
            binary_only(x),
            Some((
                "set.companion.ome".to_owned(),
                Some("urn:uuid:z2".to_owned())
            ))
        );
        let outside = r#"<OME><BinaryOnly MetadataFile="../set.companion.ome"/></OME>"#;
        assert_eq!(binary_only(outside), None);
        assert_eq!(binary_only(XML), None);
    }

    #[test]
    fn rejects_paths_outside_the_folder() {
        assert_eq!(plain_name("../x.tif"), None);
        assert_eq!(plain_name("/etc/x"), None);
        assert_eq!(plain_name("a.tif"), Some("a.tif"));
    }
}
