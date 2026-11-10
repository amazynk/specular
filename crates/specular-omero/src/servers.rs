//! The user's OMERO servers, saved as `omero.json` in the config folder: name, address and
//! user name only (passwords stay in memory for the session). The Image Data Resource is
//! offered as a preset.

use std::io;
use std::path::Path;

use serde_json::{Value, json};

/// The Image Data Resource (public, anonymous browsing).
pub const IDR_URL: &str = "https://idr.openmicroscopy.org";
pub const IDR_NAME: &str = "IDR";

/// A server the user added.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Server {
    pub id: String,
    pub name: String,
    /// The OMERO.web address (`https://idr.openmicroscopy.org`).
    pub url: String,
    /// Empty: public browsing.
    pub user: String,
}

impl Server {
    /// The IDR preset.
    pub fn idr() -> Server {
        Server {
            id: "idr".into(),
            name: IDR_NAME.into(),
            url: IDR_URL.into(),
            user: String::new(),
        }
    }
    /// The name to show: the given name, else the host.
    pub fn label(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        let rest = self
            .url
            .split_once("://")
            .map_or(self.url.as_str(), |(_, r)| r);
        rest.split('/').next().unwrap_or(rest).to_string()
    }
    pub fn is_public(&self) -> bool {
        self.user.trim().is_empty()
    }
}

/// The saved list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerList {
    pub servers: Vec<Server>,
}

impl ServerList {
    /// Reads `path`; a missing or unreadable file is an empty list.
    pub fn load(path: &Path) -> ServerList {
        let Ok(text) = std::fs::read(path) else {
            return ServerList::default();
        };
        let Ok(v) = serde_json::from_slice::<Value>(&text) else {
            specular_log::warn!(target: specular_log::IO, "omero servers: {} is not valid JSON", path.display());
            return ServerList::default();
        };
        let servers = v
            .get("servers")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .take(256)
                    .filter_map(|s| {
                        let g =
                            |k: &str| s.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                        let url = crate::api::check_url(&g("url"))?;
                        let id = g("id");
                        if id.is_empty() {
                            return None;
                        }
                        Some(Server {
                            id,
                            name: g("name"),
                            url,
                            user: g("user"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        ServerList { servers }
    }

    /// Writes `path` atomically.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let servers: Vec<Value> = self
            .servers
            .iter()
            .map(|s| json!({ "id": s.id, "name": s.name, "url": s.url, "user": s.user }))
            .collect();
        let text = serde_json::to_vec_pretty(&json!({ "version": 1, "servers": servers }))
            .map_err(io::Error::other)?;
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    /// Adds `s` or replaces the server with its id; a new one gets an id.
    pub fn upsert(&mut self, mut s: Server) -> String {
        if let Some(old) = self
            .servers
            .iter_mut()
            .find(|o| !s.id.is_empty() && o.id == s.id)
        {
            *old = s;
            return old.id.clone();
        }
        if s.id.is_empty() {
            s.id = self.new_id();
        }
        let id = s.id.clone();
        self.servers.push(s);
        id
    }

    pub fn remove(&mut self, id: &str) {
        self.servers.retain(|s| s.id != id);
    }

    pub fn get(&self, id: &str) -> Option<&Server> {
        self.servers.iter().find(|s| s.id == id)
    }

    /// Whether the IDR preset (by address) is in the list.
    pub fn has_idr(&self) -> bool {
        self.servers.iter().any(|s| s.url == IDR_URL)
    }

    fn new_id(&self) -> String {
        (1..)
            .map(|n| format!("s{n}"))
            .find(|id| self.get(id).is_none())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_without_secrets() {
        let dir = std::env::temp_dir().join(format!("omero-servers-{}", std::process::id()));
        let path = dir.join("omero.json");
        let mut list = ServerList::default();
        list.upsert(Server::idr());
        let id = list.upsert(Server {
            name: String::new(),
            url: "https://omero.example.org/".into(),
            user: "alice".into(),
            ..Server::default()
        });
        assert_eq!(id, "s1");
        list.servers[1].url = crate::api::check_url(&list.servers[1].url).unwrap();
        list.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("password"));
        let back = ServerList::load(&path);
        assert_eq!(back, list);
        assert!(back.has_idr());
        assert_eq!(back.servers[1].label(), "omero.example.org");
        assert!(!back.servers[1].is_public());
        std::fs::remove_dir_all(&dir).ok();
    }
}
