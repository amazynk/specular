//! The process's OMERO connections, by server address, and the disk cache tiles go to; the
//! decoder's opener ([`install`]) finds a descriptor's connection here (a public one is made
//! for a server not connected yet).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use specular_remote::{Client, ClientConfig, DiskCache};

use crate::api::{Omero, OmeroError, Result};

#[derive(Default)]
struct Registry {
    client: Mutex<Option<Client>>,
    connections: Mutex<HashMap<String, Arc<Omero>>>,
    cache: Mutex<Option<Arc<DiskCache>>>,
    /// Where the cache is opened on first use (when none was set).
    cache_dir: Mutex<Option<(PathBuf, u64)>>,
    /// A server whose session ended while an image was being shown.
    sign_in: Mutex<Option<String>>,
}

fn reg() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(Registry::default)
}

/// Registers the `.omero` opener with the decoder registry.
pub fn install() {
    specular_decode::set_omero_opener(Some(Arc::new(crate::session::open)));
}

/// The client new connections use (the default settings unless set).
pub fn set_client(client: Client) {
    *reg().client.lock() = Some(client);
}

pub fn client() -> Result<Client> {
    let mut c = reg().client.lock();
    if let Some(c) = &*c {
        return Ok(c.clone());
    }
    let made = Client::new(ClientConfig::default())
        .map_err(|e| OmeroError::Offline(e.to_string()))?
        .tracked(specular_core::net::What::Omero);
    *c = Some(made.clone());
    Ok(made)
}

/// Where tiles, thumbnails and descriptors are kept.
pub fn set_cache(cache: Option<Arc<DiskCache>>) {
    *reg().cache.lock() = cache;
}

/// Opens the cache at `dir` (at most `budget` bytes) when first needed, so that starting
/// the application reads nothing there.
pub fn set_cache_dir(dir: PathBuf, budget: u64) {
    *reg().cache_dir.lock() = Some((dir, budget));
}

pub fn cache() -> Option<Arc<DiskCache>> {
    let mut c = reg().cache.lock();
    if c.is_none()
        && let Some((dir, budget)) = reg().cache_dir.lock().take()
    {
        match DiskCache::open(&dir, budget) {
            Ok(d) => *c = Some(d),
            Err(e) => {
                specular_log::warn!(target: specular_log::CACHE, "omero cache {}: {e}", dir.display())
            }
        }
    }
    c.clone()
}

/// Makes `omero` the connection for its server.
pub fn register(omero: Arc<Omero>) {
    reg()
        .connections
        .lock()
        .insert(omero.base().to_string(), omero);
}

/// Drops the connection to `base` (signing out).
pub fn remove(base: &str) {
    if let Some(o) = reg().connections.lock().remove(base) {
        o.sign_out();
    }
}

/// The connection to `base`, if one was made.
pub fn get(base: &str) -> Option<Arc<Omero>> {
    reg().connections.lock().get(base).cloned()
}

/// The connection to `base`, a public one when none was made.
pub fn connection(base: &str) -> Result<Arc<Omero>> {
    if let Some(o) = get(base) {
        return Ok(o);
    }
    let o = Omero::new(client()?, base)?;
    let mut map = reg().connections.lock();
    Ok(map.entry(o.base().to_string()).or_insert(o).clone())
}

/// Notes that `base` needs signing in again (shown by the application).
pub fn request_sign_in(base: &str) {
    *reg().sign_in.lock() = Some(base.to_string());
}

/// The server that asked for signing in again since the last call.
pub fn take_sign_in_request() -> Option<String> {
    reg().sign_in.lock().take()
}
