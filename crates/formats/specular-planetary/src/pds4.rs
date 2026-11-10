//! PDS4 products: an XML label (`Product_Observational`) naming the data files of its
//! `File_Area_Observational` sections and the arrays in them. `Array_2D*` and `Array_3D*`
//! arrays (images, maps, spectra, movies) become images: element type, byte offset, the
//! axes in storage order ("Last Index Fastest"), scaling and offset, special constants as
//! no-data, and the display directions of the display dictionary.

use roxmltree::{Document, Node};

use crate::raster::{Interleave, Layout, NoData};
use crate::sample::{Real, Sample};
use crate::{Error, Facts, MapFacts, Result, meaningful, metres};

/// Largest label read, in bytes.
pub const MAX_LABEL: usize = 64 << 20;
/// Bytes of a file looked at to recognise a label.
pub const SNIFF: usize = 8192;

/// One array of a product.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// The array's class ("Array_2D_Image") and its `local_identifier`, when it has one.
    pub class: String,
    pub name: Option<String>,
    /// The data file, as the label names it (beside the label).
    pub file: String,
    pub layout: Layout,
    pub unit: Option<String>,
    /// Axis names: the band axis's, when there are bands.
    pub band_axis: Option<String>,
}

/// An array the reader cannot show, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Refused {
    pub class: String,
    pub reason: String,
}

/// A parsed PDS4 label.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Product {
    pub images: Vec<Image>,
    pub refused: Vec<Refused>,
    pub facts: Facts,
    /// Label elements with text, as (path, text), in document order.
    pub keywords: Vec<(String, String)>,
}

/// Whether `head` is a PDS4 label: XML with a `Product_…` root in the PDS4 namespace.
pub fn sniff(head: &[u8]) -> bool {
    let h = &head[..head.len().min(SNIFF)];
    let t = h.trim_ascii_start();
    (t.starts_with(b"<?xml") || t.starts_with(b"<Product_") || t.starts_with(b"\xef\xbb\xbf"))
        && find(h, b"<Product_").is_some()
        && find(h, b"pds.nasa.gov/pds4/pds/v1").is_some()
}

/// Whether `head` is a PDS4 observational product's label.
pub fn sniff_observational(head: &[u8]) -> bool {
    sniff(head) && find(&head[..head.len().min(SNIFF)], b"<Product_Observational").is_some()
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn children<'a, 'i: 'a>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

fn descendant<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.descendants()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn text(n: Option<Node>) -> Option<String> {
    let t = n?.text()?;
    let t = t.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    meaningful(&t).filter(|t| !t.starts_with("${"))
}

fn num(n: Option<Node>) -> Option<f64> {
    let t = text(n)?;
    parse_number(&t)
}

/// A number in decimal, `0x` hexadecimal or `base#digits#` form.
fn parse_number(t: &str) -> Option<f64> {
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u64::from_str_radix(h, 16).ok().map(|v| v as f64);
    }
    crate::odl::number(t)
}

/// The sample type of a PDS4 `data_type`.
pub fn data_type(t: &str) -> std::result::Result<Sample, String> {
    let int = |bytes: u8, signed: bool, big_endian: bool| Sample::Int {
        bytes,
        signed,
        big_endian,
    };
    Ok(match t {
        "UnsignedByte" => int(1, false, true),
        "SignedByte" => int(1, true, true),
        "UnsignedMSB2" => int(2, false, true),
        "UnsignedLSB2" => int(2, false, false),
        "SignedMSB2" => int(2, true, true),
        "SignedLSB2" => int(2, true, false),
        "UnsignedMSB4" => int(4, false, true),
        "UnsignedLSB4" => int(4, false, false),
        "SignedMSB4" => int(4, true, true),
        "SignedLSB4" => int(4, true, false),
        "UnsignedMSB8" => int(8, false, true),
        "UnsignedLSB8" => int(8, false, false),
        "SignedMSB8" => int(8, true, true),
        "SignedLSB8" => int(8, true, false),
        "IEEE754MSBSingle" => Sample::Float {
            bytes: 4,
            real: Real::IeeeBig,
        },
        "IEEE754LSBSingle" => Sample::Float {
            bytes: 4,
            real: Real::IeeeLittle,
        },
        "IEEE754MSBDouble" => Sample::Float {
            bytes: 8,
            real: Real::IeeeBig,
        },
        "IEEE754LSBDouble" => Sample::Float {
            bytes: 8,
            real: Real::IeeeLittle,
        },
        "ComplexMSB8" => Sample::Complex {
            bytes: 4,
            real: Real::IeeeBig,
        },
        "ComplexLSB8" => Sample::Complex {
            bytes: 4,
            real: Real::IeeeLittle,
        },
        "ComplexMSB16" => Sample::Complex {
            bytes: 8,
            real: Real::IeeeBig,
        },
        "ComplexLSB16" => Sample::Complex {
            bytes: 8,
            real: Real::IeeeLittle,
        },
        t if t.contains("BitString") => return Err("bit-string elements are not supported".into()),
        t => return Err(format!("element type {t} is not supported")),
    })
}

/// Parses a label.
pub fn parse(bytes: &[u8]) -> Result<Product> {
    if bytes.len() > MAX_LABEL {
        return Err(Error::Unsupported("the PDS4 label is too large".into()));
    }
    let text_ = std::str::from_utf8(bytes)
        .map_err(|_| Error::Malformed("the PDS4 label is not UTF-8".into()))?;
    // The XML parser recurses per element level.
    if !specular_limits::xml_within_depth(bytes, specular_limits::MAX_XML_DEPTH) {
        return Err(Error::Malformed("the PDS4 label nests too deeply".into()));
    }
    let opts = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..Default::default()
    };
    let doc = Document::parse_with_options(text_.trim_start_matches('\u{feff}'), opts)
        .map_err(|e| Error::Malformed(format!("PDS4 label: {e}")))?;
    let root = doc.root_element();
    let class = root.tag_name().name();
    if !class.starts_with("Product_") {
        return Err(Error::NotThisFormat);
    }
    if class != "Product_Observational" {
        return Err(Error::Unsupported(format!(
            "this PDS4 label is a {class}, not an observational product with images"
        )));
    }
    let facts = facts(root);
    let mut images = Vec::new();
    let mut refused = Vec::new();
    for area in children(root, "File_Area_Observational") {
        let Some(file) = child(area, "File").and_then(|f| text(child(f, "file_name"))) else {
            continue;
        };
        for arr in area.children().filter(|c| c.is_element()) {
            let class = arr.tag_name().name();
            if !(class.starts_with("Array_2D") || class.starts_with("Array_3D")) {
                continue;
            }
            match array(root, arr, &file) {
                Ok(img) => images.push(img),
                Err(reason) => refused.push(Refused {
                    class: class.to_owned(),
                    reason,
                }),
            }
        }
    }
    let mut keywords = Vec::new();
    collect(root, "", 0, &mut keywords);
    Ok(Product {
        images,
        refused,
        facts,
        keywords,
    })
}

/// Label nesting followed for keywords: deeper elements are left out (a hostile label nests
/// without end, and each level holds its path).
const MAX_KEYWORD_DEPTH: usize = 64;

/// Label elements with text, as (`Parent/element`, text).
fn collect(n: Node, path: &str, depth: usize, out: &mut Vec<(String, String)>) {
    if out.len() >= 1 << 14 || depth >= MAX_KEYWORD_DEPTH {
        return;
    }
    for c in n.children().filter(|c| c.is_element()) {
        let name = c.tag_name().name();
        let p = if path.is_empty() {
            name.to_owned()
        } else {
            format!("{path}/{name}")
        };
        let has_elements = c.children().any(|x| x.is_element());
        if has_elements {
            collect(c, &p, depth + 1, out);
        } else if let Some(t) = text(Some(c)) {
            let t = match c.attribute("unit") {
                Some(u) => format!("{t} {u}"),
                None => t,
            };
            out.push((p, t));
        }
    }
}

/// An axis of an array.
struct Axis {
    name: String,
    elements: u64,
    seq: u64,
}

fn array(root: Node, arr: Node, file: &str) -> std::result::Result<Image, String> {
    let class = arr.tag_name().name().to_owned();
    let name = text(child(arr, "local_identifier"));
    let offset = num(child(arr, "offset")).unwrap_or(0.0);
    if !(0.0..2f64.powi(62)).contains(&offset) {
        return Err("its offset is out of range".into());
    }
    let order = text(child(arr, "axis_index_order")).unwrap_or_default();
    if !order.is_empty() && !order.eq_ignore_ascii_case("Last Index Fastest") {
        return Err(format!("axis order “{order}” is not supported"));
    }
    let el = child(arr, "Element_Array").ok_or("it has no element type")?;
    let sample = data_type(&text(child(el, "data_type")).unwrap_or_default())?;
    let mut axes: Vec<Axis> = children(arr, "Axis_Array")
        .map(|a| Axis {
            name: text(child(a, "axis_name")).unwrap_or_default(),
            elements: num(child(a, "elements"))
                .filter(|v| *v >= 0.0)
                .unwrap_or(0.0) as u64,
            seq: num(child(a, "sequence_number")).unwrap_or(0.0) as u64,
        })
        .collect();
    axes.sort_by_key(|a| a.seq);
    let is = |a: &Axis, n: &[&str]| n.iter().any(|x| a.name.eq_ignore_ascii_case(x));
    let lines = ["Line", "Lines", "Row", "Rows", "y"];
    let samples = ["Sample", "Samples", "Column", "Columns", "x"];
    // (band, line, sample) positions in storage order, slowest first.
    let (b, y, x) = match axes.len() {
        2 => (None, 0, 1),
        3 => {
            let find = |n: &[&str]| axes.iter().position(|a| is(a, n));
            match (find(&lines), find(&samples)) {
                (Some(y), Some(x)) if y != x => (Some(3 - x - y), y, x),
                // Unnamed axes: planes, then lines, then samples.
                _ => (Some(0), 1, 2),
            }
        }
        _ => return Err(format!("{} axes are not supported", axes.len())),
    };
    let s = sample.bytes() as u64;
    let n = axes.len();
    let mut strides = vec![s; n];
    for i in (0..n.saturating_sub(1)).rev() {
        strides[i] = strides[i + 1].saturating_mul(axes[i + 1].elements);
    }
    let dim = |i: usize, what: &str| -> std::result::Result<u32, String> {
        match axes[i].elements {
            0 => Err(format!("it has no {what}")),
            e => u32::try_from(e).map_err(|_| format!("too many {what}")),
        }
    };
    let width = dim(x, "samples")?;
    let height = dim(y, "lines")?;
    let bands = match b {
        Some(b) => dim(b, "bands")?,
        None => 1,
    };
    let interleave = match b {
        None | Some(0) => Interleave::Bsq,
        Some(2) => Interleave::Bip,
        Some(_) => Interleave::Bil,
    };
    let mut layout = Layout::new(
        width,
        height,
        bands,
        sample,
        interleave,
        offset as u64,
        0,
        0,
    );
    layout.sample_stride = strides[x];
    layout.line_stride = strides[y];
    layout.band_stride = b.map_or(0, |b| strides[b]);
    layout.scale = num(child(el, "scaling_factor"))
        .filter(|v| *v != 0.0)
        .unwrap_or(1.0);
    layout.add = num(child(el, "value_offset")).unwrap_or(0.0);
    let mut nd = NoData::default();
    if let Some(sc) = child(arr, "Special_Constants") {
        for k in [
            "missing_constant",
            "invalid_constant",
            "unknown_constant",
            "not_applicable_constant",
            "error_constant",
        ] {
            let Some(t) = text(child(sc, k)) else {
                continue;
            };
            let bits = t
                .strip_prefix("0x")
                .or_else(|| t.strip_prefix("0X"))
                .and_then(|h| u64::from_str_radix(h, 16).ok())
                .or_else(|| crate::odl::based(&t).map(|(b, _)| b));
            match bits {
                Some(b) => nd.push(sample.from_bits(b)),
                None => {
                    if let Some(v) = parse_number(&t) {
                        nd.push(v);
                    }
                }
            }
        }
    }
    layout.nodata = nd;
    // Display directions for this array (disp:Display_Settings naming it).
    let id = name.clone();
    if let Some(dir) = display_direction(root, id.as_deref()) {
        let (h_axis, h_dir, v_axis, v_dir) = dir;
        if is(&axes[x], &[h_axis.as_str()]) || h_axis.is_empty() {
            layout.flip_x = h_dir.eq_ignore_ascii_case("Right to Left");
            layout.flip_y = v_dir.eq_ignore_ascii_case("Bottom to Top");
        } else if is(&axes[y], &[h_axis.as_str()]) && is(&axes[x], &[v_axis.as_str()]) {
            return Err("its display axes are transposed, which is not supported".into());
        }
    }
    Ok(Image {
        class,
        name,
        file: file.to_owned(),
        layout,
        unit: text(child(el, "unit")),
        band_axis: b.map(|b| axes[b].name.clone()).filter(|n| !n.is_empty()),
    })
}

/// (horizontal axis, its direction, vertical axis, its direction) of the display settings
/// referring to `id` (or the only ones, when there is one).
fn display_direction(root: Node, id: Option<&str>) -> Option<(String, String, String, String)> {
    let settings: Vec<Node> = root
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Display_Settings")
        .collect();
    let s = settings
        .iter()
        .find(|s| {
            id.is_some()
                && descendant(**s, "local_identifier_reference")
                    .and_then(|r| r.text())
                    .map(str::trim)
                    == id
        })
        .or_else(|| (settings.len() == 1).then(|| &settings[0]))?;
    let d = descendant(*s, "Display_Direction")?;
    Some((
        text(child(d, "horizontal_display_axis")).unwrap_or_default(),
        text(child(d, "horizontal_display_direction")).unwrap_or_default(),
        text(child(d, "vertical_display_axis")).unwrap_or_default(),
        text(child(d, "vertical_display_direction")).unwrap_or_default(),
    ))
}

fn facts(root: Node) -> Facts {
    let ident = child(root, "Identification_Area");
    let obs = child(root, "Observation_Area");
    let time = obs.and_then(|o| child(o, "Time_Coordinates"));
    let component = |kinds: &[&str]| -> Option<String> {
        let sys = obs.and_then(|o| child(o, "Observing_System"))?;
        let names: Vec<String> = children(sys, "Observing_System_Component")
            .filter(|c| {
                text(child(*c, "type"))
                    .is_some_and(|t| kinds.iter().any(|k| t.eq_ignore_ascii_case(k)))
            })
            .filter_map(|c| text(child(c, "name")))
            .collect();
        (!names.is_empty()).then(|| names.join(", "))
    };
    let join = |parent: &str, name: &str| -> Option<String> {
        let o = obs?;
        let v: Vec<String> = children(o, parent)
            .filter_map(|c| text(child(c, name)))
            .collect();
        (!v.is_empty()).then(|| v.join(", "))
    };
    Facts {
        mission: join("Investigation_Area", "name"),
        spacecraft: component(&[
            "Spacecraft",
            "Host",
            "Rover",
            "Lander",
            "Telescope",
            "Observatory",
        ]),
        instrument: component(&["Instrument"]),
        target: join("Target_Identification", "name"),
        start: time.and_then(|t| text(child(t, "start_date_time"))),
        stop: time.and_then(|t| text(child(t, "stop_date_time"))),
        product_id: ident.and_then(|i| text(child(i, "logical_identifier"))),
        title: ident.and_then(|i| text(child(i, "title"))),
        map: obs.and_then(|o| descendant(o, "Cartography")).and_then(map),
    }
}

/// A value with its `unit` attribute as metres (`km`, `m`; per pixel).
fn metres_of(n: Option<Node>) -> Option<f64> {
    let n = n?;
    metres(num(Some(n))?, n.attribute("unit"), 1.0)
}

fn map(cart: Node) -> Option<MapFacts> {
    let projection = descendant(cart, "map_projection_name")
        .and_then(|n| text(Some(n)))
        .or_else(|| {
            // Geographic (latitude/longitude) grids have no projection element.
            descendant(cart, "Geographic").map(|_| "Geographic".to_owned())
        })?;
    let d = |name: &str| num(descendant(cart, name)).filter(|v| v.is_finite());
    let pair = |a: &str, b: &str| d(a).zip(d(b));
    let radius =
        descendant(cart, "a_axis_radius").or_else(|| descendant(cart, "semi_major_radius"));
    Some(MapFacts {
        projection,
        metres_per_pixel: metres_of(descendant(cart, "pixel_resolution_x")),
        pixels_per_degree: d("pixel_scale_x").filter(|v| *v > 0.0),
        centre: pair(
            "latitude_of_projection_origin",
            "longitude_of_central_meridian",
        ),
        latitudes: pair("south_bounding_coordinate", "north_bounding_coordinate"),
        longitudes: pair("west_bounding_coordinate", "east_bounding_coordinate"),
        radius_km: metres_of(radius).map(|m| m / 1000.0),
        positive_longitude: text(descendant(cart, "longitude_direction")),
    })
}

/// A label for tests and fixtures: one array of `class` with `axes` (name, elements) in
/// storage order, of `data_type`, in `file` at `offset`, with `extra` XML inside the array.
pub fn write_label(
    class: &str,
    file: &str,
    offset: u64,
    data_type: &str,
    axes: &[(&str, u64)],
    extra: &str,
) -> String {
    let axes_xml: String = axes
        .iter()
        .enumerate()
        .map(|(i, (n, e))| {
            format!(
                "<Axis_Array><axis_name>{n}</axis_name><elements>{e}</elements>\
                 <sequence_number>{}</sequence_number></Axis_Array>",
                i + 1
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <Product_Observational xmlns=\"http://pds.nasa.gov/pds4/pds/v1\">\n\
         <Identification_Area><logical_identifier>urn:nasa:pds:test:data:x</logical_identifier>\
         <title>Test</title></Identification_Area>\n\
         <Observation_Area><Time_Coordinates><start_date_time>2020-01-01T00:00:00Z</start_date_time>\
         </Time_Coordinates><Investigation_Area><name>Test Mission</name><type>Mission</type>\
         </Investigation_Area><Observing_System><Observing_System_Component><name>Cam</name>\
         <type>Instrument</type></Observing_System_Component></Observing_System>\
         <Target_Identification><name>Moon</name><type>Satellite</type></Target_Identification>\
         </Observation_Area>\n\
         <File_Area_Observational><File><file_name>{file}</file_name></File>\
         <{class}><offset unit=\"byte\">{offset}</offset><axes>{}</axes>\
         <axis_index_order>Last Index Fastest</axis_index_order>\
         <Element_Array><data_type>{data_type}</data_type></Element_Array>{axes_xml}{extra}</{class}>\
         </File_Area_Observational>\n</Product_Observational>\n",
        axes.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A label nested 100 000 elements deep is refused before the XML parser (which recurses
    /// per level) sees it; one nested 200 deep is read, its keywords stopping at their cap.
    #[test]
    fn deep_label() {
        let depth = 100_000;
        let mut l = String::from("<Product_Observational>");
        l.push_str(&"<a>".repeat(depth));
        l.push('x');
        l.push_str(&"</a>".repeat(depth));
        l.push_str("</Product_Observational>");
        let r = specular_thread::on_stack(256 * 1024, || parse(l.as_bytes()));
        assert!(matches!(r, Err(Error::Malformed(_))));
        let depth = 200;
        let mut l = String::from("<Product_Observational>");
        l.push_str(&"<a>".repeat(depth));
        l.push('x');
        l.push_str(&"</a>".repeat(depth));
        l.push_str("</Product_Observational>");
        let p = specular_thread::on_stack(256 * 1024, || parse(l.as_bytes())).unwrap();
        assert!(p.keywords.is_empty());
    }

    #[test]
    fn arrays_and_axes() {
        let s = data_type("SignedMSB2").unwrap();
        // Band, Line, Sample (BSQ): 2 × 3 × 4 with scaling and a missing constant.
        let l = write_label(
            "Array_3D_Image",
            "x.img",
            10,
            "SignedMSB2",
            &[("Band", 2), ("Line", 3), ("Sample", 4)],
            "<Special_Constants><missing_constant>7</missing_constant>\
             <saturated_constant>9</saturated_constant></Special_Constants>",
        )
        .replace(
            "<data_type>SignedMSB2</data_type>",
            "<data_type>SignedMSB2</data_type><scaling_factor>0.5</scaling_factor>\
             <value_offset>1</value_offset>",
        );
        assert!(sniff_observational(l.as_bytes()));
        let p = parse(l.as_bytes()).unwrap();
        assert_eq!(p.facts.target.as_deref(), Some("Moon"));
        assert_eq!(p.facts.instrument.as_deref(), Some("Cam"));
        assert_eq!(p.facts.mission.as_deref(), Some("Test Mission"));
        let img = &p.images[0];
        assert_eq!(img.file, "x.img");
        let mut d = vec![0u8; 10];
        for v in 0..24i16 {
            d.extend(v.to_be_bytes());
        }
        let (v, t) = img.layout.band(&d, 1);
        assert!(!t);
        assert_eq!(v[0], 7.0);
        let (v, _) = img.layout.band(&d, 0);
        assert!(v[7].is_nan());
        assert_eq!(v[9], 5.5);
        assert_eq!(s.bytes(), 2);
        // Line, Sample, Band (BIP) and Line, Band, Sample (BIL).
        for (axes, il) in [
            ([("Line", 3), ("Sample", 4), ("Band", 2)], Interleave::Bip),
            ([("Line", 3), ("Band", 2), ("Sample", 4)], Interleave::Bil),
        ] {
            let l = write_label("Array_3D_Spectrum", "x.dat", 0, "UnsignedByte", &axes, "");
            let p = parse(l.as_bytes()).unwrap();
            let lay = &p.images[0].layout;
            assert_eq!(
                (lay.width, lay.height, lay.bands, lay.interleave),
                (4, 3, 2, il)
            );
            let want = Layout::new(4, 3, 2, Sample::U8, il, 0, 0, 0);
            assert_eq!(
                (lay.band_stride, lay.line_stride, lay.sample_stride),
                (want.band_stride, want.line_stride, want.sample_stride)
            );
        }
    }

    #[test]
    fn refusals() {
        let other =
            "<?xml version=\"1.0\"?><Product_Browse xmlns=\"http://pds.nasa.gov/pds4/pds/v1\"/>";
        assert!(sniff(other.as_bytes()) && !sniff_observational(other.as_bytes()));
        let e = parse(other.as_bytes()).unwrap_err();
        assert!(matches!(e, Error::Unsupported(m) if m.contains("Product_Browse")));
        let l = write_label(
            "Array_2D_Image",
            "x",
            0,
            "UnsignedBitString",
            &[("Line", 2), ("Sample", 2)],
            "",
        );
        assert!(
            parse(l.as_bytes()).unwrap().refused[0]
                .reason
                .contains("bit-string")
        );
        assert!(parse(b"<a/>").is_err());
        assert!(
            parse(b"<?xml version='1.0'?><!DOCTYPE x [<!ENTITY a 'b'>]><Product_Observational/>")
                .is_err()
        );
    }

    #[test]
    fn display_flips() {
        let l = write_label("Array_2D_Image", "x", 0, "UnsignedByte", &[("Line", 2), ("Sample", 2)], "")
            .replace(
                "</Observation_Area>",
                "<Discipline_Area><disp:Display_Settings xmlns:disp=\"d\"><disp:Display_Direction>\
                 <disp:horizontal_display_axis>Sample</disp:horizontal_display_axis>\
                 <disp:horizontal_display_direction>Left to Right</disp:horizontal_display_direction>\
                 <disp:vertical_display_axis>Line</disp:vertical_display_axis>\
                 <disp:vertical_display_direction>Bottom to Top</disp:vertical_display_direction>\
                 </disp:Display_Direction></disp:Display_Settings></Discipline_Area></Observation_Area>",
            );
        let p = parse(l.as_bytes()).unwrap();
        let lay = &p.images[0].layout;
        assert!(lay.flip_y && !lay.flip_x);
        let (v, _) = lay.band(&[1, 2, 3, 4], 0);
        assert_eq!(v, [3.0, 4.0, 1.0, 2.0]);
    }
}
