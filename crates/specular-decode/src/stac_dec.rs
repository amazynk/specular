//! STAC items, item collections, collections and catalogs (JSON) as their GeoTIFF assets:
//! one page per asset of each item (visual ones first), each opened with the TIFF reader.
//!
//! Assets and linked items are found from the document's address (its `self` link, else
//! where the file is). Local ones are read directly; ones on a server go through the
//! application's fetcher ([`set_stac_remote`]), which honours the no-network switch, shows
//! the download and keeps a Cloud-Optimized GeoTIFF's fitting level in the cache. This crate
//! does no networking.

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;
use specular_core::Source;
use specular_gis::stac::{self, Doc, Item};

use crate::georaster_dec::{GeoWrap, Paged};
use crate::{DecodeError, Decoder, Format, PageLabel, Registry, Result, Session};

/// Fetches one address, or says why not in plain words.
pub type Fetch<T> = Box<dyn Fn(&str) -> std::result::Result<T, String> + Send + Sync>;

/// Fetches what a STAC document names on a server.
pub struct StacRemote {
    /// A linked JSON document's bytes.
    pub json: Fetch<Vec<u8>>,
    /// A GeoTIFF asset as a local file (a COG's level that fits the budgets).
    pub geotiff: Fetch<PathBuf>,
}

static REMOTE: RwLock<Option<Arc<StacRemote>>> = RwLock::new(None);

/// Registers (or with `None` removes) the fetcher of STAC documents and assets on servers.
pub fn set_stac_remote(r: Option<Arc<StacRemote>>) {
    *REMOTE.write() = r;
}

/// Linked documents read at most (a collection's items, a catalog's children).
const MAX_LINKED: usize = 64;
/// Bytes of one JSON document at most.
const MAX_JSON: usize = 16 << 20;

pub struct StacDecoder;

fn remote(href: &str) -> bool {
    let l = href.to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://") || l.starts_with("s3://")
}

fn local_path(href: &str) -> PathBuf {
    PathBuf::from(href.strip_prefix("file://").unwrap_or(href))
}

fn fetch_json(href: &str) -> Result<Vec<u8>> {
    if remote(href) {
        let r = REMOTE.read().clone().ok_or_else(no_fetcher)?;
        let b = (r.json)(href).map_err(DecodeError::Unsupported)?;
        if b.len() > MAX_JSON {
            return Err(DecodeError::TooLarge);
        }
        Ok(b)
    } else {
        let s = Source::open(local_path(href)).map_err(DecodeError::Io)?;
        if s.len() > MAX_JSON {
            return Err(DecodeError::TooLarge);
        }
        Ok(s.bytes().to_vec())
    }
}

fn no_fetcher() -> DecodeError {
    DecodeError::Unsupported("the STAC assets are on a server, which cannot be reached here".into())
}

/// One page: an item's asset, by its resolved address.
#[derive(Clone, Debug)]
struct Page {
    item: String,
    title: String,
    href: String,
}

/// The pages of the items, in order.
fn item_pages(items: &[(Item, String)]) -> Vec<Page> {
    let mut out = Vec::new();
    for (it, base) in items {
        let base = it.self_href.clone().unwrap_or_else(|| base.clone());
        for a in stac::raster_assets(it) {
            out.push(Page {
                item: it.id.clone(),
                title: if a.title.is_empty() {
                    a.key.clone()
                } else {
                    a.title.clone()
                },
                href: stac::resolve(&base, &a.href),
            });
        }
    }
    out
}

/// The items a document stands for, with each one's address (for relative links): itself,
/// its features, or the items its links name (and its children's, one level down).
fn items_of(doc: Doc, base: &str) -> Result<Vec<(Item, String)>> {
    Ok(match doc {
        Doc::Item(it) => vec![(it, base.to_owned())],
        Doc::Items(v) => v.into_iter().map(|it| (it, base.to_owned())).collect(),
        Doc::Catalog {
            self_href,
            items,
            children,
            ..
        } => {
            let base = self_href.unwrap_or_else(|| base.to_owned());
            let mut out = Vec::new();
            let mut links: Vec<String> = items.iter().map(|h| stac::resolve(&base, h)).collect();
            for c in children.iter().take(MAX_LINKED) {
                let url = stac::resolve(&base, c);
                if let Ok(b) = fetch_json(&url)
                    && let Ok(Doc::Catalog {
                        self_href, items, ..
                    }) = stac::parse(&b)
                {
                    let cb = self_href.unwrap_or(url);
                    links.extend(items.iter().map(|h| stac::resolve(&cb, h)));
                }
            }
            for url in links.into_iter().take(MAX_LINKED) {
                let b = fetch_json(&url)?;
                if let Ok(Doc::Item(it)) = stac::parse(&b) {
                    out.push((it, url));
                }
            }
            out
        }
    })
}

/// The set of pages and how to open one.
struct Set {
    pages: Vec<Page>,
}

impl Set {
    fn open(self: &Arc<Self>, k: u32) -> Result<Arc<dyn Session>> {
        let p = self
            .pages
            .get(k as usize)
            .ok_or_else(|| DecodeError::Unsupported("no such page".into()))?;
        let path = if remote(&p.href) {
            let r = REMOTE.read().clone().ok_or_else(no_fetcher)?;
            (r.geotiff)(&p.href).map_err(DecodeError::Unsupported)?
        } else {
            local_path(&p.href)
        };
        let inner = Registry::default().open_path(&path)?;
        let set = self.clone();
        Ok(Arc::new(GeoWrap {
            inner,
            geo: None,
            paged: Some(Paged {
                count: self.pages.len() as u32,
                index: k,
                label: Some(PageLabel::Object {
                    name: p.title.as_str().into(),
                    class: p.item.as_str().into(),
                }),
                open: Arc::new(move |k| set.open(k)),
            }),
        }))
    }
}

impl Decoder for StacDecoder {
    fn format(&self) -> Format {
        Format::Stac
    }
    fn probe(&self, head: &[u8]) -> bool {
        stac::sniff(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        stac::sniff(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        if source.len() > MAX_JSON {
            return Err(DecodeError::TooLarge);
        }
        let doc = stac::parse(source.bytes()).map_err(DecodeError::Malformed)?;
        let base = source.path().to_string_lossy().into_owned();
        let items = items_of(doc, &base)?;
        let pages = item_pages(&items);
        if pages.is_empty() {
            return Err(DecodeError::Unsupported(
                "the STAC document lists no GeoTIFF images".into(),
            ));
        }
        Arc::new(Set { pages }).open(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A local item whose assets are GeoTIFFs beside it: one page each, visual first, each
    /// the GeoTIFF's own size and georeference; a collection linking the item gives the
    /// same pages.
    #[test]
    fn local_item_and_collection() {
        let dir = std::env::temp_dir().join(format!("specular-stac-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let grid = specular_gis::fixtures::GeoGrid {
            epsg: 32633,
            origin: (500_000.0, 4_200_000.0),
            pixel: (10.0, 10.0),
        };
        let values = specular_gis::fixtures::dem_values(40, 30, false);
        let tif = specular_gis::fixtures::geotiff_f32(&values, 40, 30, grid, None, 16);
        std::fs::write(dir.join("a.tif"), &tif).unwrap();
        std::fs::write(dir.join("b.tif"), &tif).unwrap();
        let item = r#"{"type":"Feature","stac_version":"1.0.0","id":"it1","properties":{"datetime":"2024-01-01T00:00:00Z"},
          "assets":{"data":{"href":"./a.tif","type":"image/tiff; application=geotiff","roles":["data"],"title":"Band A"},
                    "visual":{"href":"b.tif","type":"image/tiff; application=geotiff; profile=cloud-optimized","roles":["visual"]}}}"#;
        std::fs::write(dir.join("item.json"), item).unwrap();
        let col = r#"{"type":"Collection","stac_version":"1.0.0","id":"c","links":[{"rel":"item","href":"item.json"}]}"#;
        std::fs::write(dir.join("collection.json"), col).unwrap();
        for f in ["item.json", "collection.json"] {
            let s = Registry::default().open_path(&dir.join(f)).unwrap();
            assert_eq!(s.page_count(), 2, "{f}");
            assert!(matches!(
                s.page_label(),
                Some(PageLabel::Object { ref name, ref class }) if &**name == "visual" && &**class == "it1"
            ));
            assert_eq!((s.info().width, s.info().height), (40, 30));
            assert!(s.geo().is_some());
            let p1 = s.open_page(1).unwrap();
            assert!(matches!(
                p1.page_label(),
                Some(PageLabel::Object { ref name, .. }) if &**name == "Band A"
            ));
            assert_eq!(p1.page_index(), 1);
        }
        // Remote assets without a fetcher are refused in plain words.
        let remote = item
            .replace("./a.tif", "https://example.invalid/a.tif")
            .replace("\"b.tif\"", "\"https://example.invalid/b.tif\"");
        std::fs::write(dir.join("remote.json"), remote).unwrap();
        set_stac_remote(None);
        match Registry::default().open_path(&dir.join("remote.json")) {
            Err(DecodeError::Unsupported(m)) => assert!(m.contains("server")),
            Err(e) => panic!("{e}"),
            Ok(_) => panic!("opened"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
