//! SpatioTemporal Asset Catalog (STAC) JSON: an Item (a GeoJSON Feature with assets), an
//! ItemCollection (a FeatureCollection of Items), a Collection or a Catalog (links to
//! items and children). What is read: each item's id, time, bounding box, a few common
//! properties, and its assets (address, media type, title, roles), with the GeoTIFF ones
//! (Cloud-Optimized or not) picked out for viewing, visual ones first.

use serde_json::Value;

/// One asset of an item.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// The key in the item's `assets` ("visual", "B04", "data").
    pub key: String,
    pub href: String,
    pub media_type: String,
    pub title: String,
    pub roles: Vec<String>,
}

/// One item.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub id: String,
    pub datetime: String,
    pub bbox: Vec<f64>,
    /// The item's own address (`links` rel `self`), for relative asset addresses.
    pub self_href: Option<String>,
    pub assets: Vec<Asset>,
    /// Properties worth listing (platform, instruments, cloud cover, EPSG, GSD…), as text.
    pub properties: Vec<(String, String)>,
}

/// What a STAC document is.
#[derive(Clone, Debug, PartialEq)]
pub enum Doc {
    Item(Item),
    /// An ItemCollection or a search result.
    Items(Vec<Item>),
    /// A Collection or a Catalog: its id, own address, item links and assets.
    Catalog {
        id: String,
        self_href: Option<String>,
        items: Vec<String>,
        children: Vec<String>,
        assets: Vec<Asset>,
    },
}

/// Items read from one document at most.
pub const MAX_ITEMS: usize = 256;
/// Assets read from one item at most.
const MAX_ASSETS: usize = 256;

/// Whether the start of a JSON file is a STAC document.
pub fn sniff(head: &[u8]) -> bool {
    let h = &head[..head.len().min(64 << 10)];
    let t = h.iter().position(|b| !b.is_ascii_whitespace());
    t.is_some_and(|i| h[i] == b'{') && h.windows(14).any(|w| w == b"\"stac_version\"")
}

fn s(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn links(v: &Value, rel: &str) -> Vec<String> {
    v.get("links")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|l| l.get("rel").and_then(Value::as_str) == Some(rel))
                .filter_map(|l| l.get("href").and_then(Value::as_str))
                .map(str::to_owned)
                .take(MAX_ITEMS)
                .collect()
        })
        .unwrap_or_default()
}

fn assets(v: &Value) -> Vec<Asset> {
    let Some(m) = v.get("assets").and_then(Value::as_object) else {
        return Vec::new();
    };
    m.iter()
        .take(MAX_ASSETS)
        .filter_map(|(k, a)| {
            Some(Asset {
                key: k.clone(),
                href: a.get("href")?.as_str()?.to_owned(),
                media_type: s(a, "type"),
                title: s(a, "title"),
                roles: a
                    .get("roles")
                    .and_then(Value::as_array)
                    .map(|r| {
                        r.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(a) if a.iter().all(|x| !x.is_object() && !x.is_array()) => {
            Some(a.iter().filter_map(text).collect::<Vec<_>>().join(", "))
        }
        _ => None,
    }
}

fn item(v: &Value) -> Option<Item> {
    if v.get("type").and_then(Value::as_str) != Some("Feature") {
        return None;
    }
    let props = v.get("properties");
    const LISTED: [&str; 10] = [
        "datetime",
        "start_datetime",
        "end_datetime",
        "platform",
        "constellation",
        "instruments",
        "eo:cloud_cover",
        "gsd",
        "proj:epsg",
        "proj:code",
    ];
    let properties = props
        .and_then(Value::as_object)
        .map(|p| {
            LISTED
                .iter()
                .filter_map(|k| Some((k.to_string(), text(p.get(*k)?)?)))
                .collect()
        })
        .unwrap_or_default();
    Some(Item {
        id: s(v, "id"),
        datetime: props
            .map(|p| s(p, "datetime"))
            .filter(|d| !d.is_empty())
            .or_else(|| props.map(|p| s(p, "start_datetime")))
            .unwrap_or_default(),
        bbox: v
            .get("bbox")
            .and_then(Value::as_array)
            .map(|b| b.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default(),
        self_href: links(v, "self").into_iter().next(),
        assets: assets(v),
        properties,
    })
}

/// Parses a STAC document.
pub fn parse(bytes: &[u8]) -> Result<Doc, String> {
    // serde_json stops at 128 levels of nesting.
    let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("unreadable JSON: {e}"))?;
    if v.get("stac_version").is_none() && v.get("features").is_none() {
        return Err("not a STAC document".into());
    }
    match v.get("type").and_then(Value::as_str) {
        Some("Feature") => item(&v)
            .map(Doc::Item)
            .ok_or_else(|| "not a STAC item".into()),
        Some("FeatureCollection") => Ok(Doc::Items(
            v.get("features")
                .and_then(Value::as_array)
                .map(|f| f.iter().take(MAX_ITEMS).filter_map(item).collect())
                .unwrap_or_default(),
        )),
        Some("Collection" | "Catalog") => Ok(Doc::Catalog {
            id: s(&v, "id"),
            self_href: links(&v, "self").into_iter().next(),
            items: links(&v, "item"),
            children: links(&v, "child"),
            assets: assets(&v),
        }),
        _ => Err("not a STAC item, item collection, collection or catalog".into()),
    }
}

/// Whether an asset is a GeoTIFF to view: a TIFF media type, or a `.tif`/`.tiff` address
/// without one; thumbnails and overviews (small previews) are left out.
pub fn is_raster(a: &Asset) -> bool {
    let t = a.media_type.to_ascii_lowercase();
    let path = a
        .href
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let tiff = t.starts_with("image/tiff")
        || t.contains("geotiff")
        || (t.is_empty() && (path.ends_with(".tif") || path.ends_with(".tiff")));
    tiff && !a.roles.iter().any(|r| r == "thumbnail" || r == "overview")
}

/// An item's GeoTIFF assets in viewing order: `visual` ones first, then `data`, then the
/// rest, each group in the item's order.
pub fn raster_assets(item: &Item) -> Vec<&Asset> {
    let rank = |a: &Asset| {
        if a.key == "visual" || a.roles.iter().any(|r| r == "visual") {
            0
        } else if a.roles.iter().any(|r| r == "data") {
            1
        } else {
            2
        }
    };
    let mut v: Vec<&Asset> = item.assets.iter().filter(|a| is_raster(a)).collect();
    v.sort_by_key(|a| rank(a));
    v
}

/// `href` resolved against `base` (a URL or a local path of the document holding it).
pub fn resolve(base: &str, href: &str) -> String {
    if href.contains("://") || href.starts_with('/') || base.is_empty() {
        return href.to_owned();
    }
    let dir = match base.rfind('/') {
        Some(i) => &base[..=i],
        None => "",
    };
    let mut parts: Vec<&str> = dir.trim_end_matches('/').split('/').collect();
    let keep = if let Some(i) = dir.find("://") {
        // Scheme and host stay.
        dir[i + 3..].split('/').next().map_or(1, |_| 3)
    } else {
        0
    };
    for seg in href.split('/') {
        match seg {
            "." | "" => {}
            ".." if parts.len() > keep => {
                parts.pop();
            }
            ".." => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEM: &str = r#"{"type":"Feature","stac_version":"1.0.0","id":"S2B_33TUN_20240704",
      "bbox":[13.0,47.0,14.5,48.0],
      "properties":{"datetime":"2024-07-04T10:10:00Z","platform":"sentinel-2b","eo:cloud_cover":3.2,"instruments":["msi"]},
      "links":[{"rel":"self","href":"https://example.org/catalog/items/S2B.json"}],
      "assets":{
        "thumbnail":{"href":"thumb.jpg","type":"image/jpeg","roles":["thumbnail"]},
        "B04":{"href":"./B04.tif","type":"image/tiff; application=geotiff; profile=cloud-optimized","roles":["data"]},
        "visual":{"href":"https://example.org/data/TCI.tif","type":"image/tiff; application=geotiff","roles":["visual"]},
        "preview":{"href":"preview.tif","roles":["overview"]}
      }}"#;

    #[test]
    fn item_and_assets() {
        assert!(sniff(ITEM.as_bytes()));
        let Doc::Item(it) = parse(ITEM.as_bytes()).unwrap() else {
            panic!("not an item")
        };
        assert_eq!(it.id, "S2B_33TUN_20240704");
        assert_eq!(it.bbox.len(), 4);
        let r = raster_assets(&it);
        assert_eq!(
            r.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(),
            ["visual", "B04"]
        );
        assert!(
            it.properties
                .iter()
                .any(|(k, v)| k == "eo:cloud_cover" && v == "3.2")
        );
        assert!(
            it.properties
                .iter()
                .any(|(k, v)| k == "instruments" && v == "msi")
        );
        let base = it.self_href.as_deref().unwrap();
        assert_eq!(
            resolve(base, &r[1].href),
            "https://example.org/catalog/items/B04.tif"
        );
        assert_eq!(resolve(base, "../../x.tif"), "https://example.org/x.tif");
        assert_eq!(
            resolve(base, "../../../../x.tif"),
            "https://example.org/x.tif"
        );
        assert_eq!(
            resolve("/data/stac/item.json", "b/c.tif"),
            "/data/stac/b/c.tif"
        );
        assert_eq!(resolve("/data/stac/item.json", "../c.tif"), "/data/c.tif");
    }

    #[test]
    fn collections() {
        let c = r#"{"type":"Collection","stac_version":"1.0.0","id":"s2",
          "links":[{"rel":"item","href":"./a.json"},{"rel":"child","href":"sub/catalog.json"},{"rel":"self","href":"/x/collection.json"}]}"#;
        match parse(c.as_bytes()).unwrap() {
            Doc::Catalog {
                items, children, ..
            } => {
                assert_eq!(items, ["./a.json"]);
                assert_eq!(children, ["sub/catalog.json"]);
            }
            d => panic!("{d:?}"),
        }
        let fc = format!(r#"{{"type":"FeatureCollection","features":[{ITEM},{ITEM}]}}"#);
        assert!(matches!(parse(fc.as_bytes()).unwrap(), Doc::Items(v) if v.len() == 2));
        assert!(parse(b"{\"type\":\"Feature\"}").is_err());
    }
}
