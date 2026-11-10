//! Remote Zarr: a small `.zarrurl` link (the store's address and the image's path in it;
//! never a secret) names an OME-Zarr image or a plain Zarr store on an HTTP(S) server or in
//! an S3 bucket. This crate does no networking: the application registers the opener
//! ([`set_zarr_store_opener`]) that turns an address into a [`KeyStore`], and the local
//! readers (the slide reader for OME-Zarr, the arrays reader otherwise) read through it.
//! A view's decode asks for only the chunks of its level and region, which the store
//! fetches several at a time ([`KeyStore::prefetch`]); the viewer's usual progression (a
//! coarse whole image first, then the region at the zoom shown) makes opening progressive.
//!
//! [`images`] lists the images of a store (one image, a bioformats2raw series, the fields
//! of a plate's wells) for the application to write one link per image.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use parking_lot::{Mutex, RwLock};
use serde_json::Value;
use specular_core::Source;
use specular_core::keys::{KeyError, KeyStore, Want};

use crate::{DecodeError, Decoder, Format, Result, Session};

/// Turns a store's address into a store.
pub type ZarrStoreOpener =
    Arc<dyn Fn(&str) -> std::result::Result<Arc<dyn KeyStore>, KeyError> + Send + Sync>;

static OPENER: RwLock<Option<ZarrStoreOpener>> = RwLock::new(None);

/// The first bytes of every link (after optional white space).
pub const ZARR_LINK_MAGIC: &[u8] = b"{\"specular-zarr\":";

/// The most bytes read of a link.
pub const MAX_LINK: usize = 64 << 10;

/// Images listed at most.
pub const MAX_IMAGES: usize = 4096;

/// Registers (or with `None` removes) the opener of remote stores.
pub fn set_zarr_store_opener(opener: Option<ZarrStoreOpener>) {
    *OPENER.write() = opener;
}

/// What a `.zarrurl` file holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZarrLink {
    /// The store's root, without a query string or credentials.
    pub url: String,
    /// The image's group in the store ("" for the root, else ending in '/').
    pub path: String,
    /// What the image is called ("B3, field 2"), empty when it has no name of its own.
    pub name: String,
}

impl ZarrLink {
    /// The link's file: the magic first, then a field a line.
    pub fn to_json(&self) -> Vec<u8> {
        let q = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
        format!(
            "{{\"specular-zarr\":1,\n \"url\":{},\n \"path\":{},\n \"name\":{}}}\n",
            q(&self.url),
            q(&self.path),
            q(&self.name)
        )
        .into_bytes()
    }

    pub fn parse(bytes: &[u8]) -> Option<ZarrLink> {
        if bytes.len() > MAX_LINK {
            return None;
        }
        let v: Value = serde_json::from_slice(bytes).ok()?;
        v.get("specular-zarr")?.as_u64().filter(|&n| n == 1)?;
        let url = v.get("url")?.as_str()?.to_owned();
        let path = v.get("path").and_then(Value::as_str).unwrap_or("");
        if path.split('/').any(|c| c == "..") || path.starts_with('/') {
            return None;
        }
        Some(ZarrLink {
            url,
            path: dir(path),
            name: v
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        })
    }
}

/// "" or "a/b/".
fn dir(p: &str) -> String {
    let p = p.trim_matches('/');
    if p.is_empty() {
        String::new()
    } else {
        format!("{p}/")
    }
}

pub struct ZarrLinkDecoder;

impl Decoder for ZarrLinkDecoder {
    fn format(&self) -> Format {
        Format::OmeZarr
    }
    fn probe(&self, head: &[u8]) -> bool {
        let start = head
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(head.len());
        head[start..].starts_with(ZARR_LINK_MAGIC)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["zarrurl"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let link = ZarrLink::parse(source.bytes())
            .ok_or_else(|| DecodeError::Malformed("remote Zarr link".into()))?;
        let opener = OPENER.read().clone();
        let Some(open) = opener else {
            return Err(DecodeError::Unsupported("remote Zarr".into()));
        };
        let store = open(&link.url).map_err(|e| DecodeError::Io(e.into_io()))?;
        open_store(source, store, &link.path)
    }
}

/// Opens the image at `path` of `store` (the link's), as a slide when it is OME-Zarr, else
/// as arrays.
pub fn open_store(
    source: Arc<Source>,
    store: Arc<dyn KeyStore>,
    path: &str,
) -> Result<Arc<dyn Session>> {
    let keys = Arc::new(Keys {
        store,
        prefix: dir(path),
        error: Mutex::new(None),
    });
    keys.check()?;
    let location = format!("{}{}", keys.store.location(), keys.prefix);
    let slide: Arc<dyn specular_wsi::zarr::Store> = keys.clone();
    match crate::wsi_dec::open_zarr_store(source.clone(), slide, &location) {
        Ok(s) => Ok(s),
        Err(e @ (DecodeError::UnsupportedFormat | DecodeError::Unsupported(_))) => {
            let arrays: Arc<dyn specular_arrays::zarr::Store> = keys.clone();
            crate::array_dec::open_zarr_store(source, arrays, Some(&location)).map_err(|plain| {
                if let Some(k) = keys.error.lock().take() {
                    return DecodeError::Io(k.into_io());
                }
                if !keys.lists() && keys.unconsolidated_group() {
                    return DecodeError::Unsupported(
                        "a Zarr group read over plain HTTP needs consolidated metadata".into(),
                    );
                }
                match (&e, plain) {
                    (DecodeError::Unsupported(_), _) => e,
                    (_, p) => p,
                }
            })
        }
        Err(e) => Err(keys
            .error
            .lock()
            .take()
            .map_or(e, |k| DecodeError::Io(k.into_io()))),
    }
}

/// A remote store under an image's prefix, for both Zarr readers; remembers the first
/// failure that a reader's `Option` would hide.
struct Keys {
    store: Arc<dyn KeyStore>,
    prefix: String,
    error: Mutex<Option<KeyError>>,
}

impl Keys {
    fn key(&self, k: &str) -> String {
        format!("{}{k}", self.prefix)
    }

    fn note<T>(&self, r: std::result::Result<Option<T>, KeyError>) -> Option<T> {
        match r {
            Ok(v) => v,
            Err(e) => {
                self.error.lock().get_or_insert(e);
                None
            }
        }
    }

    fn wsi<T>(&self, r: std::result::Result<T, KeyError>) -> specular_wsi::Result<T> {
        r.map_err(|e| match e {
            KeyError::Cancelled => specular_wsi::Error::Cancelled,
            e => {
                let m = e.to_string();
                self.error.lock().get_or_insert(e);
                specular_wsi::Error::Io(m)
            }
        })
    }

    fn arr<T>(&self, r: std::result::Result<T, KeyError>) -> specular_arrays::Result<T> {
        r.map_err(|e| {
            let m = e.to_string();
            self.error.lock().get_or_insert(e);
            specular_arrays::Error::Io(m)
        })
    }

    /// Whether the store lists folders (S3), which plain Zarr groups need without
    /// consolidated metadata.
    fn lists(&self) -> bool {
        self.store.location().starts_with("s3://")
    }

    /// Whether the image is a group whose arrays only a listing would find (no
    /// consolidated metadata).
    fn unconsolidated_group(&self) -> bool {
        let get = |k: &str| self.store.get(&self.key(k), None).ok().flatten();
        if get(".zmetadata").is_some() {
            return false;
        }
        if get(".zgroup").is_some() {
            return true;
        }
        get("zarr.json")
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|v| {
                v.get("node_type").and_then(Value::as_str) == Some("group")
                    && v.get("consolidated_metadata").is_none_or(Value::is_null)
            })
    }

    /// A readable error when the image has no metadata at all (a wrong address, a private
    /// bucket), before the readers' own guesses.
    fn check(&self) -> Result<()> {
        let docs = ["zarr.json", ".zattrs", ".zgroup", ".zarray"];
        let wants: Vec<Want> = docs.iter().map(|d| Want::whole(self.key(d))).collect();
        let io = |e: KeyError| match e {
            KeyError::Cancelled => DecodeError::Cancelled,
            e => DecodeError::Io(e.into_io()),
        };
        self.store.prefetch(&wants, None).map_err(io)?;
        for d in docs {
            if self.store.get(&self.key(d), None).map_err(io)?.is_some() {
                return Ok(());
            }
        }
        Err(DecodeError::Io(
            if self.store.refused() {
                KeyError::Denied
            } else {
                KeyError::Status(404)
            }
            .into_io(),
        ))
    }
}

impl specular_wsi::zarr::Store for Keys {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.note(self.store.get(&self.key(key), None))
    }
    fn get_range(&self, key: &str, start: u64, len: u64) -> Option<Vec<u8>> {
        self.note(self.store.get_range(&self.key(key), start, len, None))
    }
    fn size(&self, key: &str) -> Option<u64> {
        self.note(self.store.size(&self.key(key), None))
    }
    fn try_get(
        &self,
        key: &str,
        cancel: Option<&AtomicBool>,
    ) -> specular_wsi::Result<Option<Vec<u8>>> {
        self.wsi(self.store.get(&self.key(key), cancel))
    }
    fn try_get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
        cancel: Option<&AtomicBool>,
    ) -> specular_wsi::Result<Option<Vec<u8>>> {
        self.wsi(self.store.get_range(&self.key(key), start, len, cancel))
    }
    fn try_get_suffix(
        &self,
        key: &str,
        len: u64,
        cancel: Option<&AtomicBool>,
    ) -> specular_wsi::Result<Option<(Vec<u8>, u64)>> {
        self.wsi(self.store.get_suffix(&self.key(key), len, cancel))
    }
    fn try_size(
        &self,
        key: &str,
        cancel: Option<&AtomicBool>,
    ) -> specular_wsi::Result<Option<u64>> {
        self.wsi(self.store.size(&self.key(key), cancel))
    }
    fn batched(&self) -> bool {
        true
    }
    fn prefetch(&self, wants: &[Want], cancel: Option<&AtomicBool>) -> specular_wsi::Result<()> {
        let wants: Vec<Want> = wants
            .iter()
            .map(|w| Want {
                key: self.key(&w.key),
                part: w.part,
            })
            .collect();
        self.wsi(self.store.prefetch(&wants, cancel))
    }
}

impl specular_arrays::zarr::Store for Keys {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.note(self.store.get(&self.key(key), None))
    }
    fn get_range(&self, key: &str, start: u64, len: u64) -> Option<Vec<u8>> {
        self.note(self.store.get_range(&self.key(key), start, len, None))
    }
    fn size(&self, key: &str) -> Option<u64> {
        self.note(self.store.size(&self.key(key), None))
    }
    fn subdirs(&self, dir: &str) -> Vec<String> {
        self.note(self.store.subdirs(&self.key(dir), None).map(Some))
            .unwrap_or_default()
    }
    fn try_get(&self, key: &str) -> specular_arrays::Result<Option<Vec<u8>>> {
        self.arr(self.store.get(&self.key(key), None))
    }
    fn try_get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
    ) -> specular_arrays::Result<Option<Vec<u8>>> {
        self.arr(self.store.get_range(&self.key(key), start, len, None))
    }
    fn try_size(&self, key: &str) -> specular_arrays::Result<Option<u64>> {
        self.arr(self.store.size(&self.key(key), None))
    }
    fn batched(&self) -> bool {
        true
    }
    fn prefetch(&self, wants: &[Want]) -> specular_arrays::Result<()> {
        let wants: Vec<Want> = wants
            .iter()
            .map(|w| Want {
                key: self.key(&w.key),
                part: w.part,
            })
            .collect();
        self.arr(self.store.prefetch(&wants, None))
    }
}

/// One image of a store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZarrImage {
    /// Its group ("" for the root, else ending in '/').
    pub path: String,
    /// "B3, field 2", "Series 2"; empty for a store that is one image.
    pub name: String,
}

/// The attributes of the group at `dir`: `.zattrs` (v2) or `zarr.json`'s, with NGFF 0.5's
/// `ome` key unwrapped.
fn attributes(
    store: &dyn KeyStore,
    dir: &str,
    cancel: Option<&AtomicBool>,
) -> std::result::Result<Option<Value>, KeyError> {
    attributes_of(store, dir, None, cancel)
}

/// [`attributes`] from one Zarr version's document only (`Some(true)`: v2's `.zattrs`,
/// `Some(false)`: v3's `zarr.json`), so a plate's wells cost one request each.
fn attributes_of(
    store: &dyn KeyStore,
    dir: &str,
    v2: Option<bool>,
    cancel: Option<&AtomicBool>,
) -> std::result::Result<Option<Value>, KeyError> {
    let parse = |b: Vec<u8>| serde_json::from_slice::<Value>(&b).ok();
    if v2 != Some(false)
        && let Some(a) = store.get(&format!("{dir}.zattrs"), cancel)?.and_then(parse)
    {
        return Ok(Some(a));
    }
    if v2 == Some(true) {
        return Ok(None);
    }
    let Some(doc) = store
        .get(&format!("{dir}zarr.json"), cancel)?
        .and_then(parse)
    else {
        return Ok(None);
    };
    let a = doc.get("attributes").cloned().unwrap_or(Value::Null);
    Ok(Some(a.get("ome").cloned().unwrap_or(a)))
}

fn safe_path(p: &str) -> Option<String> {
    let p = p.trim_matches('/');
    (!p.is_empty() && !p.split('/').any(|c| c.is_empty() || c == "..")).then(|| format!("{p}/"))
}

/// The images of a store: the root itself when it is an image (or plain Zarr), else the
/// fields of a plate's wells (in the plate's order, named "B3" or "B3, field 2") or a
/// bioformats2raw layout's series ("Series 1"…). At most [`MAX_IMAGES`].
pub fn images(
    store: &dyn KeyStore,
    cancel: Option<&AtomicBool>,
) -> std::result::Result<Vec<ZarrImage>, KeyError> {
    let docs: Vec<Want> = [".zattrs", "zarr.json", ".zgroup", ".zarray"]
        .iter()
        .map(|d| Want::whole(*d))
        .collect();
    store.prefetch(&docs, cancel)?;
    let root = ZarrImage {
        path: String::new(),
        name: String::new(),
    };
    let Some(attrs) = attributes(store, "", cancel)? else {
        return Ok(vec![root]);
    };
    if attrs.get("multiscales").is_some() {
        return Ok(vec![root]);
    }
    if let Some(plate) = attrs.get("plate") {
        // The wells are written as the plate is (Zarr v2 `.zattrs` or v3 `zarr.json`):
        // only that document is fetched for each, all of them in parallel.
        let v2 = store.get(".zattrs", cancel)?.is_some();
        return plate_images(store, plate, v2, cancel);
    }
    if attrs.get("bioformats2raw.layout").is_some() {
        return series(store, cancel);
    }
    Ok(vec![root])
}

fn plate_images(
    store: &dyn KeyStore,
    plate: &Value,
    v2: bool,
    cancel: Option<&AtomicBool>,
) -> std::result::Result<Vec<ZarrImage>, KeyError> {
    let names = |k: &str| -> Vec<String> {
        plate
            .get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|v| {
                        v.get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned()
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let (rows, cols) = (names("rows"), names("columns"));
    let wells: Vec<(String, String)> = plate
        .get("wells")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|w| {
                    let path = safe_path(w.get("path")?.as_str()?)?;
                    let at = |k: &str, list: &[String]| {
                        w.get(k)
                            .and_then(Value::as_u64)
                            .and_then(|i| list.get(i as usize).cloned())
                    };
                    let name = match (at("rowIndex", &rows), at("columnIndex", &cols)) {
                        (Some(r), Some(c)) => format!("{r}{c}"),
                        _ => path.trim_end_matches('/').replace('/', ""),
                    };
                    Some((path, name))
                })
                .take(MAX_IMAGES)
                .collect()
        })
        .unwrap_or_default();
    let doc = if v2 { ".zattrs" } else { "zarr.json" };
    let wants: Vec<Want> = wells
        .iter()
        .map(|(p, _)| Want::whole(format!("{p}{doc}")))
        .collect();
    store.prefetch(&wants, cancel)?;
    let mut out = Vec::new();
    for (path, name) in &wells {
        let fields: Vec<String> = attributes_of(store, path, Some(v2), cancel)?
            .as_ref()
            .and_then(|a| a.get("well"))
            .and_then(|w| w.get("images"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|i| safe_path(i.get("path")?.as_str()?))
                    .collect()
            })
            .unwrap_or_default();
        let many = fields.len() > 1;
        for (k, f) in fields.iter().enumerate() {
            if out.len() >= MAX_IMAGES {
                return Ok(out);
            }
            out.push(ZarrImage {
                path: format!("{path}{f}"),
                name: if many {
                    format!("{name}, field {}", k + 1)
                } else {
                    name.clone()
                },
            });
        }
    }
    Ok(out)
}

/// A bioformats2raw layout's series: the `OME` group's `series` list, else `0/`, `1/`, …
/// while they are images.
fn series(
    store: &dyn KeyStore,
    cancel: Option<&AtomicBool>,
) -> std::result::Result<Vec<ZarrImage>, KeyError> {
    let named = |k: usize, p: String| ZarrImage {
        path: p,
        name: format!("Series {}", k + 1),
    };
    if let Some(list) = attributes(store, "OME/", cancel)?
        .as_ref()
        .and_then(|a| a.get("series"))
        .and_then(Value::as_array)
    {
        let out: Vec<ZarrImage> = list
            .iter()
            .filter_map(|v| safe_path(v.as_str()?))
            .take(MAX_IMAGES)
            .enumerate()
            .map(|(k, p)| named(k, p))
            .collect();
        if !out.is_empty() {
            return Ok(out);
        }
    }
    let mut out = Vec::new();
    const BATCH: usize = 16;
    while out.len() < MAX_IMAGES {
        let base = out.len();
        let wants: Vec<Want> = (base..base + BATCH)
            .flat_map(|k| {
                [
                    Want::whole(format!("{k}/.zattrs")),
                    Want::whole(format!("{k}/zarr.json")),
                ]
            })
            .collect();
        store.prefetch(&wants, cancel)?;
        for k in base..base + BATCH {
            let is_image = attributes(store, &format!("{k}/"), cancel)?
                .is_some_and(|a| a.get("multiscales").is_some());
            if !is_image {
                return Ok(out);
            }
            out.push(named(k, format!("{k}/")));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_round_trip_and_probe() {
        let l = ZarrLink {
            url: "https://h.example/x.zarr/".into(),
            path: "A/1/0".into(),
            name: "A1".into(),
        };
        let b = l.to_json();
        assert!(ZarrLinkDecoder.probe(&b));
        let back = ZarrLink::parse(&b).unwrap();
        assert_eq!(back.path, "A/1/0/");
        assert_eq!(back.url, l.url);
        assert!(ZarrLink::parse(br#"{"specular-zarr":1,"url":"x","path":"../a"}"#).is_none());
        assert!(ZarrLink::parse(br#"{"specular-zarr":2,"url":"x"}"#).is_none());
        assert!(!ZarrLinkDecoder.probe(b"{\"specular-omero\":1}"));
    }
}
