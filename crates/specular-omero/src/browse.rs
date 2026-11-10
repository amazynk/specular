//! The containers of an OMERO server through the JSON API: Projects → Datasets → Images and
//! Screens → Plates → Wells → Images, a page at a time (`offset`, `limit`, and the total
//! from `meta.totalCount`).

use serde_json::Value;
use specular_remote::CancelToken;

use crate::api::{Omero, OmeroError, Result};

/// The most rows asked for in one page (OMERO.web's default `maxLimit` is 500).
pub const MAX_PAGE: u64 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Project,
    Dataset,
    Image,
    Screen,
    Plate,
    Well,
}

/// What to list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Listing {
    Projects,
    Screens,
    Datasets { project: u64 },
    Images { dataset: u64 },
    Plates { screen: u64 },
    Wells { plate: u64 },
}

impl Listing {
    /// What the listing holds.
    pub fn kind(self) -> Kind {
        match self {
            Listing::Projects => Kind::Project,
            Listing::Screens => Kind::Screen,
            Listing::Datasets { .. } => Kind::Dataset,
            Listing::Images { .. } => Kind::Image,
            Listing::Plates { .. } => Kind::Plate,
            Listing::Wells { .. } => Kind::Well,
        }
    }
}

/// An image's dimensions as a listing gives them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageBrief {
    /// X, Y, Z, C, T.
    pub size: [u64; 5],
    pub pixels_type: String,
    /// Physical pixel size X and Y (value, unit symbol).
    pub physical: Option<(f64, String)>,
    pub acquired: Option<String>,
}

/// A well of a plate and the images of its fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WellInfo {
    pub row: u32,
    pub column: u32,
    /// (image id, name) per field.
    pub images: Vec<(u64, String)>,
}

impl WellInfo {
    /// "B4": letters for the row (A…Z, AA…), the column from 1.
    pub fn label(&self) -> String {
        format!("{}{}", row_letters(self.row), self.column + 1)
    }
}

/// "A" for 0, "Z" for 25, "AA" for 26.
pub fn row_letters(mut row: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (row % 26) as u8);
        if row < 26 {
            break;
        }
        row = row / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// A row of a listing.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: Kind,
    pub id: u64,
    pub name: String,
    pub description: String,
    /// "First Last" of the owner.
    pub owner: Option<String>,
    pub group: Option<String>,
    /// Children, when the server counted them.
    pub children: Option<u64>,
    pub image: Option<ImageBrief>,
    pub well: Option<WellInfo>,
}

impl Node {
    /// The image to open or show as a thumbnail: the image itself, a well's first field.
    pub fn image_id(&self) -> Option<u64> {
        match self.kind {
            Kind::Image => Some(self.id),
            Kind::Well => self.well.as_ref()?.images.first().map(|i| i.0),
            _ => None,
        }
    }
}

/// A page of a listing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub items: Vec<Node>,
    pub offset: u64,
    /// All rows of the listing (`meta.totalCount`; the rows seen when the server omits it).
    pub total: u64,
}

impl Page {
    /// Rows after this page.
    pub fn more(&self) -> bool {
        self.offset + (self.items.len() as u64) < self.total
    }
}

impl Omero {
    /// One page of `what`, from `offset`, at most `limit` rows (capped to [`MAX_PAGE`]).
    pub fn list(
        &self,
        what: Listing,
        offset: u64,
        limit: u64,
        cancel: &CancelToken,
    ) -> Result<Page> {
        let urls = self.urls(cancel)?;
        let base = match what {
            Listing::Projects => urls.projects.clone(),
            Listing::Screens => urls.screens.clone(),
            Listing::Datasets { project } => format!("{}{project}/datasets/", urls.projects),
            Listing::Images { dataset } => format!("{}{dataset}/images/", urls.datasets),
            Listing::Plates { screen } => format!("{}{screen}/plates/", urls.screens),
            Listing::Wells { plate } => format!("{}{plate}/wells/", urls.plates),
        };
        let limit = limit.clamp(1, MAX_PAGE);
        let mut url = format!("{base}?offset={offset}&limit={limit}");
        if !matches!(what, Listing::Images { .. } | Listing::Wells { .. }) {
            url.push_str("&childCount=true");
        }
        let v = self.get_json(&url, cancel)?;
        parse_page(&v, what.kind(), offset)
    }

    /// Every row of `what`, page after page (stops at `max` rows).
    pub fn list_all(&self, what: Listing, max: u64, cancel: &CancelToken) -> Result<Vec<Node>> {
        let mut out = Vec::new();
        loop {
            let page = self.list(what, out.len() as u64, MAX_PAGE, cancel)?;
            let more = page.more() && !page.items.is_empty();
            out.extend(page.items);
            if !more || out.len() as u64 >= max {
                out.truncate(max as usize);
                return Ok(out);
            }
        }
    }

    /// One image's listing row (dimensions, owner, acquisition date).
    pub fn image(&self, id: u64, cancel: &CancelToken) -> Result<Node> {
        let urls = self.urls(cancel)?;
        let v = self.get_json(&format!("{}{id}/", urls.images), cancel)?;
        let d = v
            .get("data")
            .ok_or_else(|| OmeroError::Malformed("image without data".into()))?;
        parse_node(d, Kind::Image).ok_or_else(|| OmeroError::Malformed("bad image".into()))
    }
}

/// A listing page from the JSON API.
pub fn parse_page(v: &Value, kind: Kind, offset: u64) -> Result<Page> {
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| OmeroError::Malformed("listing without data".into()))?;
    let items: Vec<Node> = data.iter().filter_map(|d| parse_node(d, kind)).collect();
    let meta = v.get("meta");
    let total = meta
        .and_then(|m| m.get("totalCount"))
        .and_then(Value::as_u64)
        .unwrap_or(offset + items.len() as u64);
    let offset = meta
        .and_then(|m| m.get("offset"))
        .and_then(Value::as_u64)
        .unwrap_or(offset);
    Ok(Page {
        items,
        offset,
        total,
    })
}

fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn id_of(v: &Value) -> Option<u64> {
    v.get("@id").and_then(Value::as_u64)
}

/// "First Last" of an experimenter (`omero:details.owner`).
fn person(v: Option<&Value>) -> Option<String> {
    let v = v?;
    let name = format!("{} {}", text(v, "FirstName"), text(v, "LastName"));
    let name = name.trim();
    if name.is_empty() {
        let user = text(v, "UserName");
        (!user.is_empty()).then_some(user)
    } else {
        Some(name.to_string())
    }
}

/// A unit value (`{"Value": 0.65, "Symbol": "µm"}`).
pub fn quantity(v: Option<&Value>) -> Option<(f64, String)> {
    let v = v?;
    let value = v.get("Value").and_then(Value::as_f64)?;
    let symbol = v
        .get("Symbol")
        .and_then(Value::as_str)
        .unwrap_or("µm")
        .to_string();
    (value.is_finite() && value > 0.0).then_some((value, symbol))
}

/// Milliseconds since 1970 (as the API gives `AcquisitionDate`) or text, as
/// `YYYY-MM-DD HH:MM:SS` UTC.
pub fn date_text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::Number(n) => n.as_i64().map(|ms| utc(ms.div_euclid(1000))),
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

/// Seconds since 1970 as `YYYY-MM-DD HH:MM:SS` (UTC).
pub fn utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

fn parse_image_brief(v: &Value) -> ImageBrief {
    let p = v.get("Pixels");
    let n = |k: &str| {
        p.and_then(|p| p.get(k))
            .and_then(Value::as_u64)
            .unwrap_or(1)
    };
    ImageBrief {
        size: [n("SizeX"), n("SizeY"), n("SizeZ"), n("SizeC"), n("SizeT")],
        pixels_type: p
            .and_then(|p| p.get("Type"))
            .and_then(|t| t.get("value"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        physical: quantity(p.and_then(|p| p.get("PhysicalSizeX"))),
        acquired: date_text(v.get("AcquisitionDate")),
    }
}

fn parse_node(v: &Value, kind: Kind) -> Option<Node> {
    let id = id_of(v)?;
    let details = v.get("omero:details");
    let well = (kind == Kind::Well).then(|| WellInfo {
        row: v.get("Row").and_then(Value::as_u64).unwrap_or(0) as u32,
        column: v.get("Column").and_then(Value::as_u64).unwrap_or(0) as u32,
        images: v
            .get("WellSamples")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        let img = s.get("Image")?;
                        Some((id_of(img)?, text(img, "Name")))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    });
    let image = match kind {
        Kind::Image => Some(parse_image_brief(v)),
        Kind::Well => v
            .get("WellSamples")
            .and_then(|s| s.get(0))
            .and_then(|s| s.get("Image"))
            .map(parse_image_brief),
        _ => None,
    };
    let mut name = text(v, "Name");
    if let Some(w) = &well
        && name.is_empty()
    {
        name = w.label();
    }
    Some(Node {
        kind,
        id,
        name,
        description: text(v, "Description"),
        owner: person(details.and_then(|d| d.get("owner"))),
        group: details
            .and_then(|d| d.get("group"))
            .map(|g| text(g, "Name"))
            .filter(|g| !g.is_empty()),
        children: v.get("omero:childCount").and_then(Value::as_u64),
        image,
        well,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rows_are_lettered() {
        assert_eq!(row_letters(0), "A");
        assert_eq!(row_letters(25), "Z");
        assert_eq!(row_letters(26), "AA");
        assert_eq!(row_letters(27), "AB");
        assert_eq!(row_letters(701), "ZZ");
        assert_eq!(row_letters(702), "AAA");
    }

    #[test]
    fn dates() {
        assert_eq!(utc(0), "1970-01-01 00:00:00");
        assert_eq!(utc(1_459_513_800), "2016-04-01 12:30:00");
        assert_eq!(utc(951_825_600), "2000-02-29 12:00:00");
        assert_eq!(
            date_text(Some(&json!(1_459_513_800_000_i64))).as_deref(),
            Some("2016-04-01 12:30:00")
        );
    }

    #[test]
    fn parses_a_wells_page() {
        let v = json!({
            "data": [{
                "@id": 7, "@type": "http://www.openmicroscopy.org/Schemas/OME/2016-06#Well",
                "Row": 1, "Column": 3,
                "WellSamples": [{"@id": 70, "Image": {"@id": 700, "Name": "B4 f0",
                    "Pixels": {"SizeX": 64, "SizeY": 32, "SizeZ": 1, "SizeC": 2, "SizeT": 1,
                               "Type": {"value": "uint16"}}}}],
                "omero:details": {"owner": {"@id": 2, "FirstName": "Ada", "LastName": "Lovelace"},
                                  "group": {"@id": 3, "Name": "public"}}
            }],
            "meta": {"offset": 0, "limit": 200, "maxLimit": 500, "totalCount": 96}
        });
        let p = parse_page(&v, Kind::Well, 0).unwrap();
        assert_eq!(p.total, 96);
        assert!(p.more());
        let w = &p.items[0];
        assert_eq!(w.name, "B4");
        assert_eq!(w.image_id(), Some(700));
        assert_eq!(w.owner.as_deref(), Some("Ada Lovelace"));
        assert_eq!(w.group.as_deref(), Some("public"));
        assert_eq!(w.image.as_ref().unwrap().size, [64, 32, 1, 2, 1]);
    }
}
