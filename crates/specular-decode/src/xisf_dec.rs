//! XISF (PixInsight, `.xisf`) through `specular-xisf`: monolithic files, one page per image.
//! Channels form a channel axis (red, green and blue for an RGB image) and a 3-D image's
//! depth a cube axis; planes come out as real values (integers as stored, floats as they
//! are). World coordinates come from the image's `FITSKeyword` elements, else from the
//! astrometric solution PixInsight 1.8.9-2 and later store as `PCL:AstrometricSolution:*`
//! properties ([`solution_cards`]). A one-shot colour frame is debayered like a FITS one,
//! from its `<ColorFilterArray>` or its `BAYERPAT` keyword ([`xisf_cfa`]).

use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_xisf as xisf;

use crate::science::{self, AxisKind, NdAxis, NdPlane, PlaneCache, ScienceSummary};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

pub struct XisfDecoder;

fn map_err(e: xisf::Error) -> DecodeError {
    match e {
        xisf::Error::NotXisf => DecodeError::UnsupportedFormat,
        xisf::Error::Truncated(s) => DecodeError::Truncated(s.into()),
        xisf::Error::Malformed(s) | xisf::Error::Codec(s) => DecodeError::Malformed(s),
        xisf::Error::Unsupported(s) => DecodeError::Unsupported(s),
        xisf::Error::TooLarge => DecodeError::TooLarge,
    }
}

impl Decoder for XisfDecoder {
    fn format(&self) -> Format {
        Format::Xisf
    }
    fn probe(&self, head: &[u8]) -> bool {
        xisf::sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["xisf"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let x = xisf::parse(source.bytes()).map_err(map_err)?;
        if x.images.is_empty() {
            return Err(DecodeError::Unsupported("no image in the XISF file".into()));
        }
        Ok(Arc::new(XisfSession::new(source, Arc::new(x), 0)?))
    }
}

struct XisfSession {
    source: Arc<Source>,
    file: Arc<xisf::Xisf>,
    page: u32,
    info: ImageInfo,
    summary: ScienceSummary,
    /// The decompressed pixel block of a compressed image, kept while planes are read.
    block: Mutex<Option<Arc<Vec<u8>>>>,
    planes: PlaneCache,
    levels: Levels,
}

impl XisfSession {
    fn new(source: Arc<Source>, file: Arc<xisf::Xisf>, page: u32) -> Result<XisfSession> {
        let img = &file.images[page as usize];
        if img.width == 0 || img.height == 0 {
            return Err(DecodeError::Malformed("empty image".into()));
        }
        let header = specular_fits::Header::from_pairs(
            img.fits_keywords
                .iter()
                .map(|k| (k.name.as_str(), k.value.as_str(), k.comment.as_str())),
        );
        let mut dims = vec![img.width as usize, img.height as usize];
        if img.depth > 1 {
            dims.push(img.depth as usize);
        }
        let mut wcs = specular_fits::wcs::Wcs::from_header(&header, &dims);
        if wcs.as_ref().is_none_or(|w| w.celestial.is_none())
            && let Some(cards) = solution_cards(img)
        {
            let h = specular_fits::Header::from_pairs(
                cards.iter().map(|(k, v)| (k.as_str(), v.as_str(), "")),
            );
            wcs = specular_fits::wcs::Wcs::from_header(&h, &dims).map(|mut w| {
                if spline(img) {
                    w.unsupported = Some(specular_fits::wcs::Unsupported::Distortion(
                        "PixInsight spline".into(),
                    ));
                }
                w
            });
        }
        let wcs = wcs.map(Arc::new);
        let mut axes = Vec::new();
        let mut channel_axis = None;
        if img.channels > 1 {
            let rgb = img.color_space == xisf::ColorSpace::Rgb && img.channels >= 3;
            if rgb {
                channel_axis = Some(0);
            }
            axes.push(NdAxis {
                name: String::new(),
                len: img.channels,
                unit: String::new(),
                labels: if rgb {
                    ["R", "G", "B"].map(String::from).to_vec()
                } else {
                    Vec::new()
                },
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Channel,
            });
        }
        if img.depth > 1 {
            axes.push(NdAxis {
                name: String::new(),
                len: img.depth,
                unit: String::new(),
                labels: Vec::new(),
                wcs_axis: wcs
                    .as_ref()
                    .is_some_and(|w| w.axis_value(2, 0).is_some())
                    .then_some(2),
                linear: None,
                kind: AxisKind::Cube,
            });
        }
        let mut sample = img.sample.name().to_owned();
        if let Some(c) = &img.compression {
            sample += &format!(", {:?}", c.codec).to_ascii_lowercase();
            if c.shuffle_item.is_some() {
                sample += "+shuffle";
            }
        }
        let mut summary = ScienceSummary {
            width: img.width,
            height: img.height,
            axes,
            channel_axis,
            wcs,
            sample,
            ..Default::default()
        };
        summary.describe(&header, None);
        if img.channels == 1 && img.depth == 1 {
            summary.cfa = xisf_cfa(img, &header);
        }
        if summary.object.is_none() {
            summary.object = text_property(img, &["Observation:Object:Name"]);
        }
        if summary.telescope.is_none() {
            summary.telescope = text_property(img, &["Instrument:Telescope:Name"]);
        }
        if summary.instrument.is_none() {
            summary.instrument = text_property(img, &["Instrument:Camera:Name"]);
        }
        if summary.filter.is_none() {
            summary.filter = text_property(img, &["Instrument:Filter:Name"]);
        }
        let info = ImageInfo {
            format: Format::Xisf,
            width: img.width,
            height: img.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: (img.sample.bytes() * 8).min(64) as u8,
            progressive: false,
            coding: summary.sample.clone(),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(XisfSession {
            source,
            file,
            page,
            info,
            summary,
            block: Mutex::new(None),
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn image(&self) -> &xisf::Image {
        &self.file.images[self.page as usize]
    }

    fn read(&self, index: &[u32]) -> Result<NdPlane> {
        let img = self.image();
        let (mut channel, mut slice) = (0, 0);
        for (a, &i) in self.summary.axes.iter().zip(index) {
            if i >= a.len {
                return Err(DecodeError::Malformed("no such plane".into()));
            }
            match a.kind {
                AxisKind::Channel => channel = i,
                _ => slice = i,
            }
        }
        let values = if img.compression.is_some() {
            let block = {
                let mut g = self.block.lock();
                match &*g {
                    Some(b) => b.clone(),
                    None => {
                        let b = Arc::new(
                            img.pixel_bytes(self.source.bytes())
                                .map_err(map_err)?
                                .into_owned(),
                        );
                        *g = Some(b.clone());
                        b
                    }
                }
            };
            img.plane_from_bytes(&block, channel, slice)
        } else {
            img.plane_f32(self.source.bytes(), channel, slice)
        }
        .map_err(map_err)?;
        Ok(NdPlane {
            width: img.width,
            height: img.height,
            values,
            truncated: false,
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        self.planes.get_or(index, cancel, || self.read(index))
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let n = self.summary.axes.len();
        let planes = match self.summary.channel_axis {
            Some(c) => (0..self.summary.axes[c].len.min(3))
                .map(|k| {
                    let mut idx = vec![0; n];
                    idx[c] = k;
                    self.plane(&idx, cancel)
                })
                .collect::<Result<Vec<_>>>()?,
            None => vec![self.plane(&vec![0; n], cancel)?],
        };
        // The compressed block is only needed while the default planes are read.
        self.block.lock().take();
        Ok(science::auto_raster(&planes, self.summary.cfa))
    }
}

/// The FITS WCS cards of the image's `PCL:AstrometricSolution:*` properties, as PCL's
/// `AstrometricMetadata::ComputeWCSKeywords` derives them: the projection (`Gnomonic` or
/// its code `TAN`, …), the reference celestial coordinates (`CRVAL`), the reference image
/// coordinates (0-based, y down: `CRPIX1 = x + ½`, `CRPIX2 = height + ½ − y`), the linear
/// transformation from image to projection plane (degrees; its second column negated for
/// FITS's upward y, `CD`), the reference native coordinates (`PV1_1`, `PV1_2`) and the
/// celestial pole's longitude (`LONPOLE`); `Observation:CelestialReferenceSystem` and
/// `Observation:Equinox` give `RADESYS` and `EQUINOX`. `None` without a complete solution.
fn solution_cards(img: &xisf::Image) -> Option<Vec<(String, String)>> {
    const P: &str = "PCL:AstrometricSolution:";
    let get = |id: &str| img.property(&format!("{P}{id}")).map(|p| &p.value);
    let vector = |id: &str| match get(id)? {
        xisf::PropertyValue::Vector(v) if v.len() == 2 && v.iter().all(|x| x.is_finite()) => {
            Some((v[0], v[1]))
        }
        _ => None,
    };
    let code = match get("ProjectionSystem")? {
        xisf::PropertyValue::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "gnomonic" | "tan" => "TAN",
            "stereographic" | "stg" => "STG",
            "platecarree" | "car" => "CAR",
            "mercator" | "mer" => "MER",
            "hammeraitoff" | "ait" => "AIT",
            "zenithalequalarea" | "zea" => "ZEA",
            "orthographic" | "sin" => "SIN",
            _ => return None,
        },
        _ => return None,
    };
    let (ra, dec) = vector("ReferenceCelestialCoordinates")?;
    let (x0, y0) = vector("ReferenceImageCoordinates")?;
    let m = match get("LinearTransformationMatrix")? {
        xisf::PropertyValue::Matrix {
            rows: 2,
            columns: 2,
            values,
        } if values.iter().all(|x| x.is_finite()) => values.clone(),
        _ => return None,
    };
    let num = |k: &str, v: f64| (k.to_owned(), format!("{v:?}"));
    let mut cards = vec![
        ("CTYPE1".to_owned(), format!("'RA---{code}'")),
        ("CTYPE2".to_owned(), format!("'DEC--{code}'")),
        num("CRVAL1", ra),
        num("CRVAL2", dec),
        num("CRPIX1", x0 + 0.5),
        num("CRPIX2", f64::from(img.height) + 0.5 - y0),
        num("CD1_1", m[0]),
        num("CD1_2", -m[1]),
        num("CD2_1", m[2]),
        num("CD2_2", -m[3]),
    ];
    if let Some((phi0, theta0)) = vector("ReferenceNativeCoordinates") {
        cards.push(num("PV1_1", phi0));
        cards.push(num("PV1_2", theta0));
    }
    // The celestial pole's native longitude is LONPOLE. Its second value is left out: it is
    // unverified whether it is LATPOLE, and the default picks the same pole in all but
    // ambiguous cases.
    if let Some((lonpole, _)) = vector("CelestialPoleNativeCoordinates") {
        cards.push(num("LONPOLE", lonpole));
    }
    if let Some(s) = text_property(img, &["Observation:CelestialReferenceSystem"]) {
        cards.push(("RADESYS".into(), format!("'{}'", s.replace('\'', ""))));
    }
    if let Some(xisf::PropertyValue::Number(e)) =
        img.property("Observation:Equinox").map(|p| &p.value)
    {
        cards.push(num("EQUINOX", *e));
    }
    Some(cards)
}

/// Whether the solution carries a spline distortion (PCL's
/// `SplineWorldTransformation` properties), which is not read: its coordinates are not
/// shown rather than shown approximately.
fn spline(img: &xisf::Image) -> bool {
    img.properties.iter().any(|p| {
        p.id.starts_with("PCL:AstrometricSolution:SplineWorldTransformation")
            || p.id == "Transformation_ImageToProjection"
    })
}

/// The colour filter array of a one-shot colour frame: the image's 2×2
/// `<ColorFilterArray>` pattern, else the `BAYERPAT` (or `COLORTYP`) keyword shifted by
/// `XBAYROFF` / `YBAYROFF`, as for FITS but from the first stored pixel, XISF rows being
/// stored top row first.
fn xisf_cfa(img: &xisf::Image, h: &specular_fits::Header) -> Option<specular_core::nd::Cfa> {
    if let Some((pattern, 2, 2)) = &img.color_filter
        && let Some(c) = specular_core::nd::Cfa::parse(pattern)
    {
        return Some(c);
    }
    // An odd height leaves the pattern unflipped: XISF rows are stored as shown.
    crate::fits_dec::fits_cfa(h, 1)
}

fn text_property(img: &xisf::Image, ids: &[&str]) -> Option<String> {
    ids.iter().find_map(|id| match &img.property(id)?.value {
        xisf::PropertyValue::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        _ => None,
    })
}

impl Session for XisfSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.auto(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        let block = self
            .image()
            .compression
            .as_ref()
            .map_or(0, |c| c.uncompressed);
        px * 4 * u64::from(self.image().channels.min(3))
            + block
            + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
            + self.planes.memory_bytes()
            + self.block.lock().as_ref().map_or(0, |b| b.len())
    }
    fn trim_memory(&self) -> usize {
        let b = self.block.lock().take().map_or(0, |b| b.len());
        self.levels.clear() + self.planes.clear() + b
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn page_count(&self) -> u32 {
        self.file.images.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.file.images.len() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: Some(u32::from(self.info.bit_depth)),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.file.images.len() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(XisfSession::new(
            self.source.clone(),
            self.file.clone(),
            page,
        )?))
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(match self.image().color_space {
            xisf::ColorSpace::Rgb => "RGB",
            xisf::ColorSpace::CieLab => "Lab",
            xisf::ColorSpace::Gray if self.image().channels > 1 => "Multichannel",
            xisf::ColorSpace::Gray if self.summary.cfa.is_some() => "Bayer",
            xisf::ColorSpace::Gray => "Greyscale",
        })
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// A monolithic XISF of one 40 × 30 UInt16 image (inline pixels) with `children`.
    fn file(children: &str) -> Vec<u8> {
        let px: Vec<u8> = (0..40 * 30u32)
            .flat_map(|i| ((i % 40) as u16 * 100 + (i / 40 % 2) as u16 * 7).to_le_bytes())
            .collect();
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xisf version=\"1.0\" \
             xmlns=\"http://www.pixinsight.com/xisf\"><Image geometry=\"40:30:1\" \
             sampleFormat=\"UInt16\" location=\"inline:hex\">{children}{}</Image></xisf>",
            hex(&px)
        );
        let mut v = b"XISF0100".to_vec();
        v.extend((xml.len() as u32).to_le_bytes());
        v.extend([0; 4]);
        v.extend(xml.as_bytes());
        v
    }

    fn open(bytes: Vec<u8>) -> Arc<dyn Session> {
        XisfDecoder
            .open(Arc::new(Source::from_vec("a.xisf", bytes)))
            .unwrap()
    }

    fn f64s(v: &[f64]) -> String {
        hex(&v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>())
    }

    /// A solution in PixInsight's properties gives the same sky as the FITS keywords
    /// PixInsight derives from it (CRPIX from the 0-based, y-down reference point, CD with
    /// its second column negated).
    #[test]
    fn plate_solution_properties() {
        let keys = "<FITSKeyword name=\"CTYPE1\" value=\"'RA---TAN'\" comment=\"\"/>\
            <FITSKeyword name=\"CTYPE2\" value=\"'DEC--TAN'\" comment=\"\"/>\
            <FITSKeyword name=\"CRVAL1\" value=\"150.0\" comment=\"\"/>\
            <FITSKeyword name=\"CRVAL2\" value=\"2.2\" comment=\"\"/>\
            <FITSKeyword name=\"CRPIX1\" value=\"20.5\" comment=\"\"/>\
            <FITSKeyword name=\"CRPIX2\" value=\"15.5\" comment=\"\"/>\
            <FITSKeyword name=\"CD1_1\" value=\"-0.0002\" comment=\"\"/>\
            <FITSKeyword name=\"CD1_2\" value=\"0.0001\" comment=\"\"/>\
            <FITSKeyword name=\"CD2_1\" value=\"0.0001\" comment=\"\"/>\
            <FITSKeyword name=\"CD2_2\" value=\"0.0002\" comment=\"\"/>\
            <FITSKeyword name=\"RADESYS\" value=\"'ICRS'\" comment=\"\"/>";
        let v = |id: &str, x: &[f64]| {
            format!(
                "<Property id=\"PCL:AstrometricSolution:{id}\" type=\"F64Vector\" length=\"2\" \
                 location=\"inline:hex\">{}</Property>",
                f64s(x)
            )
        };
        let props = format!(
            "<Property id=\"PCL:AstrometricSolution:ProjectionSystem\" type=\"String\">Gnomonic</Property>\
             {}{}{}{}\
             <Property id=\"PCL:AstrometricSolution:LinearTransformationMatrix\" type=\"F64Matrix\" \
             rows=\"2\" columns=\"2\" location=\"inline:hex\">{}</Property>\
             <Property id=\"Observation:CelestialReferenceSystem\" type=\"String\">ICRS</Property>\
             <Property id=\"Observation:Equinox\" type=\"Float64\" value=\"2000\"/>",
            v("ReferenceCelestialCoordinates", &[150.0, 2.2]),
            v("ReferenceImageCoordinates", &[20.0, 15.0]),
            v("ReferenceNativeCoordinates", &[0.0, 90.0]),
            v("CelestialPoleNativeCoordinates", &[180.0, 2.2]),
            f64s(&[-0.0002, -0.0001, 0.0001, -0.0002]),
        );
        let a = open(file(keys));
        let b = open(file(&props));
        let wa = a.science().unwrap().wcs.clone().unwrap();
        let wb = b.science().unwrap().wcs.clone().unwrap();
        assert!(wb.unsupported.is_none(), "{:?}", wb.unsupported);
        for (x, y) in [(0.0, 0.0), (20.0, 15.0), (39.5, 3.25), (7.0, 29.0)] {
            let (p, q) = (wa.sky_at(x, y, &[]).unwrap(), wb.sky_at(x, y, &[]).unwrap());
            assert!(
                (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9,
                "{x},{y}: {p:?} {q:?}"
            );
        }
        // The reference point is at the image point the solution names.
        let c = wb.display_of(150.0, 2.2, &[]).unwrap();
        assert!(
            (c.0 - 20.0).abs() < 1e-9 && (c.1 - 15.0).abs() < 1e-9,
            "{c:?}"
        );
        // With a spline distortion the coordinates are not shown.
        let spline = props.clone()
            + "<Property id=\"PCL:AstrometricSolution:SplineWorldTransformation:Version\" \
               type=\"Int32\" value=\"1\"/>";
        let s = open(file(&spline));
        assert!(
            s.science()
                .unwrap()
                .wcs
                .as_ref()
                .unwrap()
                .unsupported
                .is_some()
        );
        // Incomplete or unknown solutions give none.
        let other = props.replace("Gnomonic", "Bonne");
        assert!(open(file(&other)).science().unwrap().wcs.is_none());
    }

    /// One-shot colour frames: the `<ColorFilterArray>` element, else `BAYERPAT` with its
    /// offsets, from the first stored (top) row; other array sizes are not Bayer.
    #[test]
    fn colour_filter_array() {
        use specular_core::nd::Cfa;
        let cfa = |children: &str| open(file(children)).science().unwrap().cfa;
        assert_eq!(
            cfa("<ColorFilterArray pattern=\"GBRG\" width=\"2\" height=\"2\"/>"),
            Some(Cfa::GBRG)
        );
        assert_eq!(
            cfa("<FITSKeyword name=\"BAYERPAT\" value=\"'RGGB'\" comment=\"\"/>"),
            Some(Cfa::RGGB)
        );
        assert_eq!(
            cfa(
                "<FITSKeyword name=\"BAYERPAT\" value=\"'RGGB'\" comment=\"\"/>\
                 <FITSKeyword name=\"XBAYROFF\" value=\"1\" comment=\"\"/>"
            ),
            Some(Cfa::GRBG)
        );
        // The element wins over the keyword; a 6 × 6 X-Trans array is not a Bayer one.
        assert_eq!(
            cfa(
                "<ColorFilterArray pattern=\"BGGR\" width=\"2\" height=\"2\"/>\
                 <FITSKeyword name=\"BAYERPAT\" value=\"'RGGB'\" comment=\"\"/>"
            ),
            Some(Cfa::BGGR)
        );
        assert_eq!(
            cfa(
                "<ColorFilterArray pattern=\"GGRGGBGGBGGRBRGRBGGGBGGRGGRGGBRBGBRG\" \
                 width=\"6\" height=\"6\"/>"
            ),
            None
        );
        assert_eq!(cfa(""), None);
        let s = open(file(
            "<FITSKeyword name=\"BAYERPAT\" value=\"'RGGB'\" comment=\"\"/>",
        ));
        assert_eq!(s.colour_model(), Some("Bayer"));
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!((r.width, r.height), (40, 30));
    }
}
