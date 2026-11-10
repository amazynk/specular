//! Online map services: what a server offers and the addresses to ask it for. WMS and WMTS
//! capabilities (as Web Mercator tiles for the base map), WFS capabilities and OGC API
//! collections (features as GeoJSON), OGC API Tiles, and STAC catalogues (a search, then
//! the Cloud-Optimized GeoTIFFs of each item).
//!
//! Nothing here fetches anything: the app asks the user first, then fetches through
//! `specular-remote` and hands the answers to [`parse`].

use crate::basemap::TileKey;

/// Half the Web Mercator world, metres.
pub const MERCATOR_HALF: f64 = 20_037_508.342_789_244;
/// The scale denominator of zoom 0 in 256-pixel tiles (OGC's 0.28 mm pixel).
const SCALE0: f64 = 559_082_264.028_717_8;

/// A kind of service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Wms,
    Wmts,
    Wfs,
    OgcApi,
    Stac,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Wms, Kind::Wmts, Kind::Wfs, Kind::OgcApi, Kind::Stac];

    /// Its id in the saved servers' file.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Wms => "wms",
            Kind::Wmts => "wmts",
            Kind::Wfs => "wfs",
            Kind::OgcApi => "ogcapi",
            Kind::Stac => "stac",
        }
    }

    pub fn from_id(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == s)
    }

    /// Its name for display (the standard's own).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Wms => "WMS",
            Kind::Wmts => "WMTS",
            Kind::Wfs => "WFS",
            Kind::OgcApi => "OGC API",
            Kind::Stac => "STAC",
        }
    }
}

/// Tiles in Web Mercator rows and columns (row 0 at the north edge), 256 px.
#[derive(Clone, Debug, PartialEq)]
pub enum Tiles {
    /// `{z}`, `{x}` and `{y}` in the address.
    Xyz(String),
    /// A WMTS or OGC API tile matrix set: `{TileMatrix}`, `{TileRow}`, `{TileCol}` in the
    /// address; the matrix's identifier per zoom (index = zoom, `None` for a zoom it lacks).
    Matrix {
        url: String,
        matrices: Vec<Option<String>>,
    },
    /// A WMS GetMap address with `{bbox}` for the tile's bounds in EPSG:3857.
    Wms(String),
}

impl Tiles {
    /// The address of a tile.
    pub fn url(&self, (z, x, y): TileKey) -> Option<String> {
        match self {
            Tiles::Xyz(t) => crate::basemap::tile_url(t, (z, x, y)),
            Tiles::Matrix { url, matrices } => {
                let id = matrices.get(usize::from(z))?.as_ref()?;
                Some(
                    url.replace("{TileMatrix}", &encode(id))
                        .replace("{TileRow}", &y.to_string())
                        .replace("{TileCol}", &x.to_string()),
                )
            }
            Tiles::Wms(t) => {
                let b = tile_bounds((z, x, y));
                let bbox = format!("{:.3},{:.3},{:.3},{:.3}", b[0], b[1], b[2], b[3]);
                Some(t.replace("{bbox}", &bbox))
            }
        }
    }

    /// The deepest zoom served.
    pub fn max_zoom(&self) -> u8 {
        match self {
            Tiles::Matrix { matrices, .. } => matrices
                .iter()
                .rposition(Option::is_some)
                .map_or(0, |z| z as u8),
            _ => 19,
        }
    }
}

/// A service layer chosen as the base map, as the settings keep it: its tiles, its name
/// and the credit the server asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceTiles {
    pub tiles: Tiles,
    pub name: String,
    pub credit: String,
}

impl ServiceTiles {
    /// Tab-separated: `xyz`/`wms`/`matrix`, the address, the name, the credit, then a matrix
    /// set's identifiers per zoom (empty for a zoom it lacks).
    pub fn encode(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        let (kind, url, ids) = match &self.tiles {
            Tiles::Xyz(u) => ("xyz", u.as_str(), String::new()),
            Tiles::Wms(u) => ("wms", u.as_str(), String::new()),
            Tiles::Matrix { url, matrices } => (
                "matrix",
                url.as_str(),
                matrices
                    .iter()
                    .map(|m| m.as_deref().map(encode).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        };
        format!(
            "{kind}\t{}\t{}\t{}\t{ids}",
            clean(url),
            clean(&self.name),
            clean(&self.credit)
        )
    }

    pub fn decode(s: &str) -> Option<ServiceTiles> {
        let mut f = s.trim_end_matches(['\n', '\r']).split('\t');
        let (kind, url) = (f.next()?, f.next()?.to_owned());
        let name = f.next().unwrap_or("").to_owned();
        let credit = f.next().unwrap_or("").to_owned();
        let ids = f.next().unwrap_or("");
        let lower = url.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return None;
        }
        let tiles = match kind {
            "xyz" => Tiles::Xyz(url),
            "wms" => Tiles::Wms(url),
            "matrix" => Tiles::Matrix {
                url,
                matrices: ids
                    .split(',')
                    .map(|i| (!i.is_empty()).then(|| decode(i)))
                    .collect(),
            },
            _ => return None,
        };
        Some(ServiceTiles {
            tiles,
            name,
            credit,
        })
    }
}

/// Undoes [`encode`].
pub fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A tile's bounds in EPSG:3857 metres: west, south, east, north.
pub fn tile_bounds((z, x, y): TileKey) -> [f64; 4] {
    let n = f64::from(1u32 << z.min(30));
    let size = 2.0 * MERCATOR_HALF / n;
    let west = -MERCATOR_HALF + f64::from(x) * size;
    let north = MERCATOR_HALF - f64::from(y) * size;
    [west, north - size, west + size, north]
}

/// Why a layer cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Not offered in Web Mercator (EPSG:3857) with 256-pixel tiles.
    NoWebMercator,
    /// No PNG or JPEG images.
    NoImageFormat,
    /// Features not offered as GeoJSON.
    NoGeoJson,
}

/// What a layer gives.
#[derive(Clone, Debug, PartialEq)]
pub enum Offer {
    /// Tiles for the base map.
    Tiles(Tiles),
    /// Features as GeoJSON, from this address (`{bbox}`: west,south,east,north in degrees;
    /// `{limit}`: the most features).
    Features(String),
    /// A STAC search (`{bbox}`, `{limit}`, `{datetime}`).
    Search(String),
    Refused(Refusal),
}

/// One layer, feature type or collection of a server.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: String,
    pub title: String,
    /// Where it has data: west, south, east, north in degrees.
    pub bbox: Option<[f64; 4]>,
    pub offer: Offer,
    /// The credit the server asks for.
    pub attribution: Option<String>,
}

/// What a server offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Capabilities {
    pub kind: Kind,
    pub title: String,
    pub layers: Vec<Layer>,
    /// For an OGC API or STAC landing page: where its collections are listed (to fetch
    /// next), and a STAC search.
    pub collections: Option<String>,
}

/// Why an answer is not a service description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Neither XML capabilities nor an OGC API, STAC or GeoJSON document.
    Unknown,
    /// The server answered with an exception report: its text.
    Exception(String),
    /// Broken XML or JSON.
    Malformed(String),
}

/// The addresses to try for a server's description, in order: the address as given, then
/// the GetCapabilities requests of WMS, WMTS and WFS when it has no query.
pub fn probe_urls(url: &str) -> Vec<String> {
    let u = url.trim().to_owned();
    let mut v = vec![u.clone()];
    let lower = u.to_ascii_lowercase();
    if lower.contains("request=getcapabilities") || lower.ends_with(".json") {
        return v;
    }
    for service in ["WMS", "WMTS", "WFS"] {
        v.push(with_query(
            &u,
            &format!("SERVICE={service}&REQUEST=GetCapabilities"),
        ));
    }
    v
}

/// `url` with `query` appended (`?` or `&`).
pub fn with_query(url: &str, query: &str) -> String {
    let sep = if url.contains('?') {
        if url.ends_with('?') || url.ends_with('&') {
            ""
        } else {
            "&"
        }
    } else {
        "?"
    };
    format!("{url}{sep}{query}")
}

/// Percent-encodes a query value.
pub fn encode(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b':' => {
                o.push(b as char)
            }
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// Joins a relative link to the address it came from.
pub fn resolve(base: &str, href: &str) -> String {
    if href.contains("://") {
        return href.to_owned();
    }
    let (scheme, rest) = base.split_once("://").unwrap_or(("https", base));
    let host_end = rest.find('/').unwrap_or(rest.len());
    let host = &rest[..host_end];
    if let Some(abs) = href.strip_prefix('/') {
        return format!("{scheme}://{host}/{abs}");
    }
    let path = rest[host_end..].split(['?', '#']).next().unwrap_or("");
    let dir = path.rfind('/').map_or("/", |i| &path[..=i]);
    format!("{scheme}://{host}{dir}{href}")
}

/// Reads a server's answer from `url`.
pub fn parse(url: &str, body: &[u8]) -> Result<Capabilities, ParseError> {
    let text = std::str::from_utf8(body)
        .map_err(|_| ParseError::Malformed("not UTF-8".into()))?
        .trim_start_matches('\u{feff}')
        .trim_start();
    if text.starts_with('<') {
        return parse_xml(url, text);
    }
    if text.starts_with('{') {
        let j: serde_json::Value =
            serde_json::from_str(text).map_err(|e| ParseError::Malformed(e.to_string()))?;
        return parse_json(url, &j);
    }
    Err(ParseError::Unknown)
}

// ----- XML ---------------------------------------------------------------------------

type Node<'a, 'i> = roxmltree::Node<'a, 'i>;

fn kids<'a, 'i>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

fn kid<'a, 'i>(n: Node<'a, 'i>, name: &'a str) -> Option<Node<'a, 'i>> {
    kids(n, name).next()
}

fn text_of(n: Node, name: &str) -> Option<String> {
    kid(n, name)
        .and_then(|c| c.text())
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

fn path<'a, 'i>(n: Node<'a, 'i>, names: &[&'a str]) -> Option<Node<'a, 'i>> {
    let mut cur = n;
    for name in names {
        cur = kid(cur, name)?;
    }
    Some(cur)
}

fn href(n: Node) -> Option<String> {
    n.attributes()
        .find(|a| a.name() == "href")
        .map(|a| a.value().trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn web_mercator(crs: &str) -> bool {
    let c = crs.to_ascii_uppercase();
    ["3857", "900913", "102100", "102113", "GOOGLEMAPSCOMPATIBLE"]
        .iter()
        .any(|k| c.ends_with(k) || c.contains(&format!(":{k}")))
}

/// Deepest WMS `<Layer>` nesting followed (real servers nest a few levels).
const MAX_LAYER_DEPTH: usize = 32;

fn parse_xml(url: &str, text: &str) -> Result<Capabilities, ParseError> {
    if !specular_limits::xml_within_depth(text.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err(ParseError::Malformed("nested too deeply".into()));
    }
    let opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opt)
        .map_err(|e| ParseError::Malformed(e.to_string()))?;
    let root = doc.root_element();
    match root.tag_name().name() {
        "WMS_Capabilities" | "WMT_MS_Capabilities" => Ok(wms(url, root)),
        "Capabilities" => Ok(wmts(url, root)),
        "WFS_Capabilities" => Ok(wfs(url, root)),
        "ServiceExceptionReport" | "ExceptionReport" => {
            let msg = root
                .descendants()
                .filter(|n| n.is_element())
                .filter(|n| matches!(n.tag_name().name(), "ServiceException" | "ExceptionText"))
                .filter_map(|n| n.text())
                .map(str::trim)
                .find(|t| !t.is_empty())
                .unwrap_or("")
                .to_owned();
            Err(ParseError::Exception(msg))
        }
        _ => Err(ParseError::Unknown),
    }
}

fn service_title(root: Node) -> String {
    kid(root, "Service")
        .and_then(|s| text_of(s, "Title"))
        .or_else(|| kid(root, "ServiceIdentification").and_then(|s| text_of(s, "Title")))
        .unwrap_or_default()
}

/// The GET address of an operation in WMS capabilities (`Capability/Request/<op>`).
fn wms_get(root: Node, op: &str) -> Option<String> {
    let o = path(root, &["Capability", "Request", op])?;
    o.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Get")
        .and_then(|g| kid(g, "OnlineResource"))
        .and_then(href)
}

fn wms(url: &str, root: Node) -> Capabilities {
    let version = root.attribute("version").unwrap_or("1.3.0").to_owned();
    let v13 = !version.starts_with("1.1") && !version.starts_with("1.0");
    let get =
        wms_get(root, "GetMap").unwrap_or_else(|| url.split('?').next().unwrap_or(url).to_owned());
    let formats: Vec<String> = path(root, &["Capability", "Request", "GetMap"])
        .map(|o| {
            kids(o, "Format")
                .filter_map(|f| f.text().map(|t| t.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default();
    let format = ["image/png", "image/png8", "image/jpeg"]
        .into_iter()
        .find(|f| formats.iter().any(|x| x.eq_ignore_ascii_case(f)))
        .map(str::to_owned);
    let mut layers = Vec::new();
    if let Some(top) = kid(root, "Capability").and_then(|c| kid(c, "Layer")) {
        wms_layers(
            top,
            &[],
            None,
            &get,
            &version,
            v13,
            format.as_deref(),
            0,
            &mut layers,
        );
    }
    Capabilities {
        kind: Kind::Wms,
        title: service_title(root),
        layers,
        collections: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn wms_layers(
    n: Node,
    crs_up: &[String],
    bbox_up: Option<[f64; 4]>,
    get: &str,
    version: &str,
    v13: bool,
    format: Option<&str>,
    depth: usize,
    out: &mut Vec<Layer>,
) {
    if depth >= MAX_LAYER_DEPTH {
        return;
    }
    let mut crs: Vec<String> = crs_up.to_vec();
    crs.extend(
        kids(n, "CRS")
            .chain(kids(n, "SRS"))
            .filter_map(|c| c.text())
            .flat_map(|t| t.split_whitespace().map(str::to_owned).collect::<Vec<_>>()),
    );
    let bbox = kid(n, "EX_GeographicBoundingBox")
        .and_then(|b| {
            let f = |k: &str| text_of(b, k)?.parse::<f64>().ok();
            Some([
                f("westBoundLongitude")?,
                f("southBoundLatitude")?,
                f("eastBoundLongitude")?,
                f("northBoundLatitude")?,
            ])
        })
        .or_else(|| {
            kid(n, "LatLonBoundingBox").and_then(|b| {
                let f = |k: &str| b.attribute(k)?.parse::<f64>().ok();
                Some([f("minx")?, f("miny")?, f("maxx")?, f("maxy")?])
            })
        })
        .or(bbox_up);
    if let Some(name) = text_of(n, "Name") {
        let merc = crs
            .iter()
            .find(|c| web_mercator(c) && !c.contains("GOOGLE"))
            .cloned();
        let offer = match (merc, format) {
            (_, None) => Offer::Refused(Refusal::NoImageFormat),
            (None, _) => Offer::Refused(Refusal::NoWebMercator),
            (Some(c), Some(f)) => {
                let crs_key = if v13 { "CRS" } else { "SRS" };
                Offer::Tiles(Tiles::Wms(with_query(
                    get,
                    &format!(
                        "SERVICE=WMS&VERSION={version}&REQUEST=GetMap&LAYERS={}&STYLES=&{crs_key}={c}&BBOX={{bbox}}&WIDTH=256&HEIGHT=256&FORMAT={}&TRANSPARENT=TRUE",
                        encode(&name),
                        encode(f)
                    ),
                )))
            }
        };
        let attribution = kid(n, "Attribution").and_then(|a| text_of(a, "Title"));
        out.push(Layer {
            title: text_of(n, "Title").unwrap_or_else(|| name.clone()),
            id: name,
            bbox,
            offer,
            attribution,
        });
    }
    for c in kids(n, "Layer") {
        wms_layers(c, &crs, bbox, get, version, v13, format, depth + 1, out);
    }
}

fn ows_bbox(n: Node) -> Option<[f64; 4]> {
    let b = kid(n, "WGS84BoundingBox")?;
    let pair = |k: &str| -> Option<(f64, f64)> {
        let t = text_of(b, k)?;
        let mut it = t.split_whitespace().map(|v| v.parse::<f64>().ok());
        Some((it.next()??, it.next()??))
    };
    let (w, s) = pair("LowerCorner")?;
    let (e, nn) = pair("UpperCorner")?;
    Some([w, s, e, nn])
}

/// A tile matrix set's identifiers per Web Mercator zoom, when it is one (256-pixel tiles
/// from the world's top-left corner at the standard scales).
fn mercator_matrices(set: Node) -> Option<Vec<Option<String>>> {
    let crs = text_of(set, "SupportedCRS")?;
    if !web_mercator(&crs) {
        return None;
    }
    let mut out: Vec<Option<String>> = Vec::new();
    for m in kids(set, "TileMatrix") {
        let id = text_of(m, "Identifier")?;
        let scale: f64 = text_of(m, "ScaleDenominator")?.parse().ok()?;
        let tw: u32 = text_of(m, "TileWidth")
            .and_then(|t| t.parse().ok())
            .unwrap_or(256);
        let corner: Vec<f64> = text_of(m, "TopLeftCorner")?
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        if tw != 256 || corner.len() != 2 {
            continue;
        }
        let tl_ok =
            (corner[0] + MERCATOR_HALF).abs() < 1.0 && (corner[1] - MERCATOR_HALF).abs() < 1.0;
        let z = (SCALE0 / scale).log2().round();
        if !tl_ok
            || !(0.0..=24.0).contains(&z)
            || ((SCALE0 / 2f64.powf(z)) / scale - 1.0).abs() > 1e-3
        {
            continue;
        }
        let z = z as usize;
        if out.len() <= z {
            out.resize(z + 1, None);
        }
        out[z] = Some(id);
    }
    out.iter().any(Option::is_some).then_some(out)
}

fn wmts(url: &str, root: Node) -> Capabilities {
    let contents = kid(root, "Contents");
    let mut sets: Vec<(String, Vec<Option<String>>)> = Vec::new();
    if let Some(c) = contents {
        for s in kids(c, "TileMatrixSet") {
            if let (Some(id), Some(m)) = (text_of(s, "Identifier"), mercator_matrices(s)) {
                sets.push((id, m));
            }
        }
    }
    // The KVP GetTile address, for servers without REST templates.
    let kvp = kid(root, "OperationsMetadata").and_then(|om| {
        kids(om, "Operation")
            .find(|o| o.attribute("name") == Some("GetTile"))?
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "Get")
            .and_then(href)
    });
    let mut layers = Vec::new();
    for l in contents.into_iter().flat_map(|c| kids(c, "Layer")) {
        let Some(id) = text_of(l, "Identifier") else {
            continue;
        };
        let style = kids(l, "Style")
            .find(|s| s.attribute("isDefault") == Some("true"))
            .or_else(|| kid(l, "Style"))
            .and_then(|s| text_of(s, "Identifier"))
            .unwrap_or_else(|| "default".into());
        let formats: Vec<String> = kids(l, "Format")
            .filter_map(|f| f.text().map(|t| t.trim().to_owned()))
            .collect();
        let templates: Vec<(String, String)> = kids(l, "ResourceURL")
            .filter(|r| r.attribute("resourceType") == Some("tile"))
            .filter_map(|r| {
                Some((
                    r.attribute("format").unwrap_or("").to_owned(),
                    r.attribute("template")?.to_owned(),
                ))
            })
            .collect();
        let image = |f: &str| f.starts_with("image/png") || f.starts_with("image/jpeg");
        let set = kids(l, "TileMatrixSetLink")
            .filter_map(|k| text_of(k, "TileMatrixSet"))
            .find_map(|s| sets.iter().find(|(id, _)| *id == s).cloned());
        let offer = match set {
            None => Offer::Refused(Refusal::NoWebMercator),
            Some((set_id, matrices)) => {
                let rest = templates
                    .iter()
                    .find(|(f, _)| image(f))
                    .map(|(_, t)| t.clone());
                let fmt = formats.iter().find(|f| image(f)).cloned();
                let addr = match (rest, fmt, &kvp) {
                    (Some(t), _, _) => Some(
                        t.replace("{TileMatrixSet}", &set_id)
                            .replace("{Style}", &style)
                            .replace("{style}", &style),
                    ),
                    (None, Some(f), Some(k)) => Some(with_query(
                        k,
                        &format!(
                            "SERVICE=WMTS&REQUEST=GetTile&VERSION=1.0.0&LAYER={}&STYLE={}&TILEMATRIXSET={}&TILEMATRIX={{TileMatrix}}&TILEROW={{TileRow}}&TILECOL={{TileCol}}&FORMAT={}",
                            encode(&id),
                            encode(&style),
                            encode(&set_id),
                            encode(&f)
                        ),
                    )),
                    _ => None,
                };
                match addr {
                    Some(u) => Offer::Tiles(Tiles::Matrix {
                        url: resolve(url, &u),
                        matrices,
                    }),
                    None => Offer::Refused(Refusal::NoImageFormat),
                }
            }
        };
        layers.push(Layer {
            title: text_of(l, "Title").unwrap_or_else(|| id.clone()),
            id,
            bbox: ows_bbox(l),
            offer,
            attribution: None,
        });
    }
    Capabilities {
        kind: Kind::Wmts,
        title: service_title(root),
        layers,
        collections: None,
    }
}

fn wfs(url: &str, root: Node) -> Capabilities {
    let version = root.attribute("version").unwrap_or("2.0.0").to_owned();
    let v2 = version.starts_with('2');
    let om = kid(root, "OperationsMetadata");
    let get_feature =
        om.and_then(|om| kids(om, "Operation").find(|o| o.attribute("name") == Some("GetFeature")));
    let get = get_feature
        .and_then(|o| {
            o.descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "Get")
                .and_then(href)
        })
        .unwrap_or_else(|| url.split('?').next().unwrap_or(url).to_owned());
    // Output formats: on GetFeature, else on the whole service.
    let mut formats: Vec<String> = Vec::new();
    for scope in get_feature.into_iter().chain(om) {
        for p in kids(scope, "Parameter").filter(|p| {
            p.attribute("name")
                .is_some_and(|n| n.eq_ignore_ascii_case("outputFormat"))
        }) {
            formats.extend(
                p.descendants()
                    .filter(|n| n.is_element() && n.tag_name().name() == "Value")
                    .filter_map(|n| n.text().map(|t| t.trim().to_owned())),
            );
        }
    }
    let json = formats
        .iter()
        .find(|f| {
            let f = f.to_ascii_lowercase();
            f == "application/json"
                || f == "json"
                || f.contains("geojson")
                || f == "application/geo+json"
        })
        .cloned();
    let mut layers = Vec::new();
    for ft in kid(root, "FeatureTypeList")
        .into_iter()
        .flat_map(|l| kids(l, "FeatureType"))
    {
        let Some(name) = text_of(ft, "Name") else {
            continue;
        };
        let offer = match &json {
            None => Offer::Refused(Refusal::NoGeoJson),
            Some(f) => {
                let q = if v2 {
                    format!(
                        "SERVICE=WFS&VERSION={version}&REQUEST=GetFeature&TYPENAMES={}&OUTPUTFORMAT={}&SRSNAME=urn:ogc:def:crs:OGC:1.3:CRS84&BBOX={{bbox}},urn:ogc:def:crs:OGC:1.3:CRS84&COUNT={{limit}}",
                        encode(&name),
                        encode(f)
                    )
                } else {
                    format!(
                        "SERVICE=WFS&VERSION={version}&REQUEST=GetFeature&TYPENAME={}&OUTPUTFORMAT={}&SRSNAME=EPSG:4326&BBOX={{bbox}},EPSG:4326&MAXFEATURES={{limit}}",
                        encode(&name),
                        encode(f)
                    )
                };
                Offer::Features(with_query(&get, &q))
            }
        };
        layers.push(Layer {
            title: text_of(ft, "Title").unwrap_or_else(|| name.clone()),
            id: name,
            bbox: ows_bbox(ft),
            offer,
            attribution: None,
        });
    }
    Capabilities {
        kind: Kind::Wfs,
        title: service_title(root),
        layers,
        collections: None,
    }
}

// ----- JSON --------------------------------------------------------------------------

fn link<'a>(j: &'a serde_json::Value, rels: &[&str]) -> Option<&'a serde_json::Value> {
    j.get("links")?.as_array()?.iter().find(|l| {
        l.get("rel").and_then(|r| r.as_str()).is_some_and(|r| {
            rels.iter()
                .any(|w| r == *w || r.ends_with(&format!("/{w}")))
        })
    })
}

fn link_href(base: &str, j: &serde_json::Value, rels: &[&str]) -> Option<String> {
    link(j, rels)
        .and_then(|l| l.get("href")?.as_str())
        .map(|h| resolve(base, h))
}

fn str_of(j: &serde_json::Value, k: &str) -> Option<String> {
    j.get(k)?.as_str().map(str::to_owned)
}

fn parse_json(url: &str, j: &serde_json::Value) -> Result<Capabilities, ParseError> {
    let stac = j.get("stac_version").is_some();
    if let Some(cols) = j.get("collections").and_then(|c| c.as_array()) {
        // A collections list (OGC API or STAC).
        let search = link_href(url, j, &["search"]);
        let kind = if stac || cols.iter().any(|c| c.get("stac_version").is_some()) {
            Kind::Stac
        } else {
            Kind::OgcApi
        };
        let layers = cols
            .iter()
            .filter_map(|c| collection(url, c, kind, search.as_deref()))
            .collect();
        return Ok(Capabilities {
            kind,
            title: str_of(j, "title").unwrap_or_default(),
            layers,
            collections: None,
        });
    }
    if j.get("links").is_some() {
        // A landing page: its collections come next.
        let kind = if stac { Kind::Stac } else { Kind::OgcApi };
        let collections = link_href(url, j, &["data"])
            .or_else(|| link_href(url, j, &["collections"]))
            .or_else(|| (kind == Kind::OgcApi).then(|| join(url, "collections")));
        let mut layers = Vec::new();
        if kind == Kind::Stac
            && let Some(s) = link_href(url, j, &["search"])
        {
            layers.push(Layer {
                id: String::new(),
                title: str_of(j, "title").unwrap_or_default(),
                bbox: None,
                offer: Offer::Search(stac_search(&s, None)),
                attribution: None,
            });
        }
        return Ok(Capabilities {
            kind,
            title: str_of(j, "title").unwrap_or_default(),
            layers,
            collections,
        });
    }
    Err(ParseError::Unknown)
}

/// `base` with a path segment added.
fn join(base: &str, seg: &str) -> String {
    let b = base.split(['?', '#']).next().unwrap_or(base);
    format!("{}/{seg}", b.trim_end_matches('/'))
}

fn stac_search(search: &str, collection: Option<&str>) -> String {
    let mut q = "bbox={bbox}&limit={limit}".to_owned();
    if let Some(c) = collection {
        q.push_str(&format!("&collections={}", encode(c)));
    }
    q.push_str("{datetime}");
    with_query(search, &q)
}

fn collection(
    base: &str,
    c: &serde_json::Value,
    kind: Kind,
    search: Option<&str>,
) -> Option<Layer> {
    let id = str_of(c, "id")?;
    let title = str_of(c, "title").unwrap_or_else(|| id.clone());
    let bbox = c
        .pointer("/extent/spatial/bbox/0")
        .and_then(|b| b.as_array())
        .and_then(|b| {
            let v: Vec<f64> = b.iter().filter_map(|x| x.as_f64()).collect();
            match v.len() {
                4 => Some([v[0], v[1], v[2], v[3]]),
                6 => Some([v[0], v[1], v[3], v[4]]),
                _ => None,
            }
        });
    let attribution = str_of(c, "attribution");
    let offer = if kind == Kind::Stac {
        let s = search.map(str::to_owned).unwrap_or_else(|| {
            // Without a search link: the collection's items.
            link_href(base, c, &["items"]).unwrap_or_else(|| join(&join(base, &id), "items"))
        });
        Offer::Search(stac_search(&s, Some(&id)))
    } else if let Some(t) = link_href(base, c, &["tilesets-map", "tiles"]) {
        Offer::Tiles(Tiles::Matrix {
            url: format!(
                "{}/WebMercatorQuad/{{TileMatrix}}/{{TileRow}}/{{TileCol}}?f=png",
                t.split('?').next().unwrap_or(&t).trim_end_matches('/')
            ),
            matrices: (0..=19).map(|z: u8| Some(z.to_string())).collect(),
        })
    } else {
        let items = link_href(base, c, &["items"])
            .unwrap_or_else(|| join(&join(base, "collections"), &id) + "/items");
        let items = items.split('?').next().unwrap_or(&items).to_owned();
        Offer::Features(format!("{items}?bbox={{bbox}}&limit={{limit}}&f=json"))
    };
    Some(Layer {
        id,
        title,
        bbox,
        offer,
        attribution,
    })
}

/// A features or search address for an area (west, south, east, north in degrees), at
/// most `limit` features, and for STAC a time range (`from`/`to` as "YYYY-MM-DD").
pub fn fill(template: &str, bbox: [f64; 4], limit: usize, time: Option<(&str, &str)>) -> String {
    let b = format!(
        "{:.6},{:.6},{:.6},{:.6}",
        bbox[0], bbox[1], bbox[2], bbox[3]
    );
    let dt = time
        .map(|(a, b)| format!("&datetime={a}T00:00:00Z/{b}T23:59:59Z"))
        .unwrap_or_default();
    template
        .replace("{bbox}", &b)
        .replace("{limit}", &limit.to_string())
        .replace("{datetime}", &dt)
}

/// A STAC item with its Cloud-Optimized GeoTIFFs.
#[derive(Clone, Debug, PartialEq)]
pub struct StacItem {
    pub id: String,
    pub datetime: Option<String>,
    /// Cloud cover, per cent.
    pub cloud: Option<f64>,
    pub bbox: Option<[f64; 4]>,
    /// (asset key, title, address), the visual one first.
    pub cogs: Vec<(String, String, String)>,
}

/// The items of a STAC search answer (a GeoJSON feature collection), and the next page.
pub fn stac_items(url: &str, body: &[u8]) -> Result<(Vec<StacItem>, Option<String>), ParseError> {
    let j: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| ParseError::Malformed(e.to_string()))?;
    let feats = j
        .get("features")
        .and_then(|f| f.as_array())
        .ok_or(ParseError::Unknown)?;
    let mut out = Vec::new();
    for f in feats {
        let Some(id) = str_of(f, "id") else { continue };
        let props = f.get("properties");
        let datetime =
            props.and_then(|p| str_of(p, "datetime").or_else(|| str_of(p, "start_datetime")));
        let cloud = props
            .and_then(|p| p.get("eo:cloud_cover"))
            .and_then(|c| c.as_f64());
        let bbox = f.get("bbox").and_then(|b| b.as_array()).and_then(|b| {
            let v: Vec<f64> = b.iter().filter_map(|x| x.as_f64()).collect();
            (v.len() >= 4).then(|| {
                if v.len() == 6 {
                    [v[0], v[1], v[3], v[4]]
                } else {
                    [v[0], v[1], v[2], v[3]]
                }
            })
        });
        let mut cogs: Vec<(u8, String, String, String)> = Vec::new();
        if let Some(assets) = f.get("assets").and_then(|a| a.as_object()) {
            for (key, a) in assets {
                let ty = str_of(a, "type").unwrap_or_default().to_ascii_lowercase();
                let Some(href) = str_of(a, "href") else {
                    continue;
                };
                let roles: Vec<String> = a
                    .get("roles")
                    .and_then(|r| r.as_array())
                    .map(|r| {
                        r.iter()
                            .filter_map(|x| x.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let cog = ty.contains("cloud-optimized")
                    || (ty.starts_with("image/tiff")
                        && roles.iter().any(|r| r == "data" || r == "visual"));
                if !cog {
                    continue;
                }
                let rank = if key == "visual" || roles.iter().any(|r| r == "visual") {
                    0
                } else {
                    1
                };
                let title = str_of(a, "title").unwrap_or_else(|| key.clone());
                cogs.push((rank, key.clone(), title, resolve(url, &href)));
            }
        }
        cogs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        out.push(StacItem {
            id,
            datetime,
            cloud,
            bbox,
            cogs: cogs.into_iter().map(|(_, k, t, h)| (k, t, h)).collect(),
        });
    }
    let next = link(&j, &["next"])
        .filter(|l| {
            l.get("method")
                .and_then(|m| m.as_str())
                .is_none_or(|m| m == "GET")
        })
        .and_then(|l| l.get("href")?.as_str())
        .map(|h| resolve(url, h));
    Ok((out, next))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WMS: &str = r#"<?xml version="1.0"?>
<WMS_Capabilities version="1.3.0" xmlns="http://www.opengis.net/wms" xmlns:xlink="http://www.w3.org/1999/xlink">
 <Service><Name>WMS</Name><Title>Test WMS</Title></Service>
 <Capability>
  <Request><GetMap><Format>image/jpeg</Format><Format>image/png</Format>
   <DCPType><HTTP><Get><OnlineResource xlink:href="http://127.0.0.1/wms?map=x&amp;"/></Get></HTTP></DCPType>
  </GetMap></Request>
  <Layer><Title>Root</Title><CRS>EPSG:4326</CRS><CRS>EPSG:3857</CRS>
   <EX_GeographicBoundingBox><westBoundLongitude>5</westBoundLongitude><eastBoundLongitude>11</eastBoundLongitude><southBoundLatitude>45</southBoundLatitude><northBoundLatitude>48</northBoundLatitude></EX_GeographicBoundingBox>
   <Layer><Name>relief</Name><Title>Relief</Title><Attribution><Title>Swiss Data</Title></Attribution></Layer>
   <Layer><Name>roads</Name><Title>Roads &amp; paths</Title></Layer>
  </Layer>
 </Capability>
</WMS_Capabilities>"#;

    /// Layers nested 200 deep: read down to the cap, the deeper ones left out; a document
    /// nested 100 000 deep is refused before the XML parser (which recurses per level).
    #[test]
    fn deep_wms_layers() {
        let doc = |depth: usize| {
            WMS.replacen(
                "<Layer><Name>roads</Name>",
                &format!(
                    "{}<Layer><Name>roads</Name>",
                    "<Layer><Name>n</Name>".repeat(depth)
                ),
                1,
            )
            .replacen(
                "</Layer>\n  </Layer>",
                &format!("</Layer>{}\n  </Layer>", "</Layer>".repeat(depth)),
                1,
            )
        };
        let url = "http://127.0.0.1/wms?SERVICE=WMS&REQUEST=GetCapabilities";
        let c = specular_thread::on_stack(256 * 1024, || parse(url, doc(200).as_bytes())).unwrap();
        assert_eq!(c.layers.len(), MAX_LAYER_DEPTH);
        let r = specular_thread::on_stack(256 * 1024, || parse(url, doc(100_000).as_bytes()));
        assert!(matches!(r, Err(ParseError::Malformed(_))));
    }

    #[test]
    fn wms_layers_become_tiles() {
        let c = parse(
            "http://127.0.0.1/wms?SERVICE=WMS&REQUEST=GetCapabilities",
            WMS.as_bytes(),
        )
        .unwrap();
        assert_eq!(c.kind, Kind::Wms);
        assert_eq!(c.title, "Test WMS");
        assert_eq!(c.layers.len(), 2);
        let l = &c.layers[0];
        assert_eq!(l.id, "relief");
        assert_eq!(l.bbox, Some([5.0, 45.0, 11.0, 48.0]));
        assert_eq!(l.attribution.as_deref(), Some("Swiss Data"));
        let Offer::Tiles(t) = &l.offer else {
            panic!("{:?}", l.offer)
        };
        let u = t.url((1, 1, 0)).unwrap();
        assert!(
            u.starts_with(
                "http://127.0.0.1/wms?map=x&SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=relief"
            ),
            "{u}"
        );
        assert!(u.contains("CRS=EPSG:3857"), "{u}");
        assert!(u.contains("FORMAT=image%2Fpng"), "{u}");
        assert!(
            u.contains("BBOX=0.000,0.000,20037508.343,20037508.343"),
            "{u}"
        );
        assert_eq!(c.layers[1].title, "Roads & paths");
    }

    #[test]
    fn wms_without_web_mercator_is_refused() {
        let x = WMS.replace("<CRS>EPSG:3857</CRS>", "");
        let c = parse("http://h/wms", x.as_bytes()).unwrap();
        assert_eq!(c.layers[0].offer, Offer::Refused(Refusal::NoWebMercator));
    }

    #[test]
    fn exceptions_are_reported() {
        let x = r#"<ServiceExceptionReport><ServiceException>Unknown layer</ServiceException></ServiceExceptionReport>"#;
        assert_eq!(
            parse("http://h/", x.as_bytes()),
            Err(ParseError::Exception("Unknown layer".into()))
        );
        assert_eq!(parse("http://h/", b"hello"), Err(ParseError::Unknown));
    }

    fn wmts_doc() -> String {
        let mut m = String::new();
        for z in 0..=3 {
            let scale = SCALE0 / f64::from(1u32 << z);
            m.push_str(&format!(
                "<TileMatrix><ows:Identifier>L{z}</ows:Identifier><ScaleDenominator>{scale}</ScaleDenominator><TopLeftCorner>-20037508.3427892 20037508.3427892</TopLeftCorner><TileWidth>256</TileWidth><TileHeight>256</TileHeight><MatrixWidth>{n}</MatrixWidth><MatrixHeight>{n}</MatrixHeight></TileMatrix>",
                n = 1u32 << z
            ));
        }
        format!(
            r#"<Capabilities xmlns="http://www.opengis.net/wmts/1.0" xmlns:ows="http://www.opengis.net/ows/1.1" version="1.0.0">
<ows:ServiceIdentification><ows:Title>Tiles</ows:Title></ows:ServiceIdentification>
<Contents>
 <Layer><ows:Title>Ortho</ows:Title><ows:Identifier>ortho</ows:Identifier>
  <ows:WGS84BoundingBox><ows:LowerCorner>5.9 45.8</ows:LowerCorner><ows:UpperCorner>10.5 47.8</ows:UpperCorner></ows:WGS84BoundingBox>
  <Style isDefault="true"><ows:Identifier>default</ows:Identifier></Style>
  <Format>image/jpeg</Format>
  <TileMatrixSetLink><TileMatrixSet>merc</TileMatrixSet></TileMatrixSetLink>
  <ResourceURL format="image/jpeg" resourceType="tile" template="https://t.example/{{Style}}/{{TileMatrixSet}}/{{TileMatrix}}/{{TileRow}}/{{TileCol}}.jpg"/>
 </Layer>
 <Layer><ows:Title>Swiss grid only</ows:Title><ows:Identifier>lv95</ows:Identifier>
  <Format>image/png</Format><TileMatrixSetLink><TileMatrixSet>2056</TileMatrixSet></TileMatrixSetLink></Layer>
 <TileMatrixSet><ows:Identifier>merc</ows:Identifier><ows:SupportedCRS>urn:ogc:def:crs:EPSG::3857</ows:SupportedCRS>{m}</TileMatrixSet>
 <TileMatrixSet><ows:Identifier>2056</ows:Identifier><ows:SupportedCRS>urn:ogc:def:crs:EPSG::2056</ows:SupportedCRS></TileMatrixSet>
</Contents></Capabilities>"#
        )
    }

    #[test]
    fn wmts_matrices_map_to_zooms() {
        let c = parse(
            "https://t.example/WMTSCapabilities.xml",
            wmts_doc().as_bytes(),
        )
        .unwrap();
        assert_eq!(c.kind, Kind::Wmts);
        assert_eq!(c.layers.len(), 2);
        let l = &c.layers[0];
        assert_eq!(l.bbox, Some([5.9, 45.8, 10.5, 47.8]));
        let Offer::Tiles(t) = &l.offer else { panic!() };
        assert_eq!(t.max_zoom(), 3);
        assert_eq!(
            t.url((2, 3, 1)).unwrap(),
            "https://t.example/default/merc/L2/1/3.jpg"
        );
        assert_eq!(t.url((5, 0, 0)), None);
        assert_eq!(c.layers[1].offer, Offer::Refused(Refusal::NoWebMercator));
    }

    #[test]
    fn wfs_feature_types_ask_for_geojson() {
        let x = r#"<wfs:WFS_Capabilities version="2.0.0" xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:ows="http://www.opengis.net/ows/1.1" xmlns:xlink="http://www.w3.org/1999/xlink">
<ows:ServiceIdentification><ows:Title>Features</ows:Title></ows:ServiceIdentification>
<ows:OperationsMetadata><ows:Operation name="GetFeature"><ows:DCP><ows:HTTP><ows:Get xlink:href="http://127.0.0.1:9/wfs"/></ows:HTTP></ows:DCP>
<ows:Parameter name="outputFormat"><ows:AllowedValues><ows:Value>application/gml+xml; version=3.2</ows:Value><ows:Value>application/json</ows:Value></ows:AllowedValues></ows:Parameter></ows:Operation></ows:OperationsMetadata>
<wfs:FeatureTypeList><wfs:FeatureType><wfs:Name>topp:states</wfs:Name><wfs:Title>States</wfs:Title>
<ows:WGS84BoundingBox><ows:LowerCorner>-124 24</ows:LowerCorner><ows:UpperCorner>-66 49</ows:UpperCorner></ows:WGS84BoundingBox></wfs:FeatureType></wfs:FeatureTypeList>
</wfs:WFS_Capabilities>"#;
        let c = parse(
            "http://127.0.0.1:9/wfs?SERVICE=WFS&REQUEST=GetCapabilities",
            x.as_bytes(),
        )
        .unwrap();
        assert_eq!(c.kind, Kind::Wfs);
        let l = &c.layers[0];
        assert_eq!(l.id, "topp:states");
        let Offer::Features(t) = &l.offer else {
            panic!()
        };
        let u = fill(t, [-80.0, 30.0, -70.0, 40.0], 500, None);
        assert!(u.starts_with("http://127.0.0.1:9/wfs?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature&TYPENAMES=topp:states&OUTPUTFORMAT=application%2Fjson"), "{u}");
        assert!(
            u.contains(
                "BBOX=-80.000000,30.000000,-70.000000,40.000000,urn:ogc:def:crs:OGC:1.3:CRS84"
            ),
            "{u}"
        );
        assert!(u.ends_with("COUNT=500"), "{u}");
        let gml_only = x.replace("<ows:Value>application/json</ows:Value>", "");
        let c = parse("http://h/wfs", gml_only.as_bytes()).unwrap();
        assert_eq!(c.layers[0].offer, Offer::Refused(Refusal::NoGeoJson));
    }

    #[test]
    fn ogc_api_landing_then_collections() {
        let landing = br#"{"title":"Demo","links":[{"rel":"self","href":"https://api.example/"},{"rel":"data","href":"https://api.example/collections"}]}"#;
        let c = parse("https://api.example/", landing).unwrap();
        assert_eq!(c.kind, Kind::OgcApi);
        assert_eq!(
            c.collections.as_deref(),
            Some("https://api.example/collections")
        );
        let cols = br#"{"collections":[
          {"id":"lakes","title":"Lakes","extent":{"spatial":{"bbox":[[-180,-90,180,90]]}},"links":[{"rel":"items","type":"application/geo+json","href":"https://api.example/collections/lakes/items?f=json"}]},
          {"id":"dem","title":"Relief","links":[{"rel":"http://www.opengis.net/def/rel/ogc/1.0/tilesets-map","href":"/collections/dem/map/tiles"}]}]}"#;
        let c = parse("https://api.example/collections", cols).unwrap();
        assert_eq!(c.layers.len(), 2);
        let Offer::Features(t) = &c.layers[0].offer else {
            panic!()
        };
        assert_eq!(
            fill(t, [0.0, 1.0, 2.0, 3.0], 10, None),
            "https://api.example/collections/lakes/items?bbox=0.000000,1.000000,2.000000,3.000000&limit=10&f=json"
        );
        let Offer::Tiles(t) = &c.layers[1].offer else {
            panic!()
        };
        assert_eq!(
            t.url((3, 4, 5)).unwrap(),
            "https://api.example/collections/dem/map/tiles/WebMercatorQuad/3/5/4?f=png"
        );
    }

    #[test]
    fn stac_search_and_items() {
        let landing = br#"{"stac_version":"1.0.0","type":"Catalog","title":"Earth","links":[{"rel":"search","href":"https://stac.example/search","method":"GET"},{"rel":"data","href":"https://stac.example/collections"}]}"#;
        let c = parse("https://stac.example/", landing).unwrap();
        assert_eq!(c.kind, Kind::Stac);
        let Offer::Search(s) = &c.layers[0].offer else {
            panic!()
        };
        let u = fill(
            s,
            [7.0, 46.0, 8.0, 47.0],
            20,
            Some(("2024-06-01", "2024-06-30")),
        );
        assert_eq!(
            u,
            "https://stac.example/search?bbox=7.000000,46.000000,8.000000,47.000000&limit=20&datetime=2024-06-01T00:00:00Z/2024-06-30T23:59:59Z"
        );
        let items = br#"{"type":"FeatureCollection","features":[{"id":"S2A_1","bbox":[7,46,8,47],"properties":{"datetime":"2024-06-03T10:20:31Z","eo:cloud_cover":12.5},
          "assets":{"B04":{"href":"https://cogs.example/B04.tif","type":"image/tiff; application=geotiff; profile=cloud-optimized","roles":["data"]},
                    "visual":{"href":"visual.tif","type":"image/tiff; application=geotiff; profile=cloud-optimized","title":"True colour","roles":["visual"]},
                    "thumbnail":{"href":"t.jpg","type":"image/jpeg","roles":["thumbnail"]}}}],
          "links":[{"rel":"next","href":"https://stac.example/search?page=2"}]}"#;
        let (v, next) = stac_items("https://stac.example/items/", items).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].cloud, Some(12.5));
        assert_eq!(v[0].cogs[0].1, "True colour");
        assert_eq!(v[0].cogs[0].2, "https://stac.example/items/visual.tif");
        assert_eq!(v[0].cogs.len(), 2);
        assert_eq!(next.as_deref(), Some("https://stac.example/search?page=2"));
    }

    #[test]
    fn probing_and_joining() {
        let p = probe_urls("https://h.example/ows");
        assert_eq!(p.len(), 4);
        assert_eq!(
            p[1],
            "https://h.example/ows?SERVICE=WMS&REQUEST=GetCapabilities"
        );
        assert_eq!(
            probe_urls("https://h/x?request=GetCapabilities&service=WMS").len(),
            1
        );
        assert_eq!(
            resolve("https://h/a/b/c.json", "d.json"),
            "https://h/a/b/d.json"
        );
        assert_eq!(resolve("https://h/a/b", "/z"), "https://h/z");
        assert_eq!(encode("a b/c"), "a%20b%2Fc");
        let t = ServiceTiles {
            tiles: Tiles::Matrix {
                url: "https://t/{TileMatrix}/{TileRow}/{TileCol}.png".into(),
                matrices: vec![Some("EPSG:3857:0".into()), None, Some("a,b".into())],
            },
            name: "Ortho\tphoto".into(),
            credit: "© Someone".into(),
        };
        let back = ServiceTiles::decode(&t.encode()).unwrap();
        assert_eq!(back.tiles, t.tiles);
        assert_eq!(back.name, "Ortho photo");
        assert_eq!(back.credit, t.credit);
        assert!(ServiceTiles::decode("xyz\tfile:///etc/passwd").is_none());
        assert_eq!(decode(&encode("a b/c%")), "a b/c%");
        let b = tile_bounds((0, 0, 0));
        assert!((b[0] + MERCATOR_HALF).abs() < 1e-6 && (b[3] - MERCATOR_HALF).abs() < 1e-6);
    }
}
