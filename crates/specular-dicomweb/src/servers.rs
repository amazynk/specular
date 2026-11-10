//! The user's servers, saved as `dicomweb.json` in the config folder: DICOMweb services
//! (searched, opened and sent to with STOW-RS) and DICOM (DIMSE) nodes, which studies can be
//! sent to with C-STORE.
//!
//! Only the name, service root URL or host, port and AE titles, kind of credentials and user
//! name are saved; passwords and tokens never reach the disk (they live in a
//! [`specular_remote::CredentialStore`]).

use std::io;
use std::path::Path;

use serde_json::{Value, json};
use specular_remote::AuthKind;

/// How a server is reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// DICOMweb (QIDO-RS, WADO-RS, STOW-RS) over HTTP(S).
    #[default]
    DicomWeb,
    /// The DICOM upper layer (C-ECHO, C-STORE) over TCP.
    Dimse,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::DicomWeb => "dicomweb",
            Kind::Dimse => "dimse",
        }
    }
}

/// How a DIMSE server's series are retrieved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Retrieve {
    /// C-GET: the server sends on our own association (no inbound port).
    #[default]
    Get,
    /// C-MOVE: the server sends to our AE title at [`Server::receive_address`] and
    /// [`Server::receive_port`], where a storage SCP listens while the move runs.
    Move,
}

impl Retrieve {
    pub fn id(self) -> &'static str {
        match self {
            Retrieve::Get => "get",
            Retrieve::Move => "move",
        }
    }
}

/// The calling AE title used when none is given.
pub const DEFAULT_OUR_AE: &str = "SPECULAR";
/// The registered DICOM port (PS3.15 annex B; 104 needs privileges on most systems).
pub const DEFAULT_PORT: u16 = 104;

/// A server the user added.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Server {
    /// Stable id: credential key and disk-cache folder.
    pub id: String,
    pub name: String,
    pub kind: Kind,
    /// The service root (`https://pacs.example/dicom-web`); DICOMweb only.
    pub base_url: String,
    pub auth: AuthKind,
    /// User name for Basic authentication.
    pub user: String,
    /// DIMSE: host name or address, port, the server's AE title and ours.
    pub host: String,
    pub port: u16,
    pub ae_title: String,
    pub our_ae: String,
    /// DIMSE: DICOM over TLS (the server's certificate checked against the system's trust
    /// store).
    pub tls: bool,
    /// DIMSE: C-GET or C-MOVE.
    pub retrieve: Retrieve,
    /// DIMSE, C-MOVE: the address and port our storage SCP listens on while a move runs
    /// (port 0: not set, so no move).
    pub receive_address: String,
    pub receive_port: u16,
}

impl Server {
    pub fn is_https(&self) -> bool {
        self.kind == Kind::DicomWeb
            && self
                .base_url
                .get(..8)
                .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
    }
    /// Our AE title for a DIMSE server (the default when none was given).
    pub fn calling_ae(&self) -> &str {
        let t = self.our_ae.trim();
        if t.is_empty() { DEFAULT_OUR_AE } else { t }
    }
    /// Where it is: the service root, or `AE@host:port`.
    pub fn address(&self) -> String {
        match self.kind {
            Kind::DicomWeb => self.base_url.clone(),
            Kind::Dimse => format!(
                "{}@{}:{}",
                self.ae_title.trim(),
                self.host.trim(),
                self.port
            ),
        }
    }
    /// The name to show: the given name, else the host.
    pub fn label(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        if self.kind == Kind::Dimse {
            return self.host.trim().to_string();
        }
        let rest = self
            .base_url
            .split_once("://")
            .map_or(self.base_url.as_str(), |(_, r)| r);
        let host = rest.split('/').next().unwrap_or(rest);
        host.rsplit_once('@').map_or(host, |(_, h)| h).to_string()
    }
    /// The disk-cache prefix for this server.
    pub fn cache_prefix(&self) -> String {
        specular_remote::DiskCache::key(&["dicomweb", &self.id])
    }
}

/// Why a URL is not a usable service root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlProblem {
    Empty,
    /// Not `http://` or `https://`.
    Scheme,
    NoHost,
    /// Credentials in the URL (`https://user:pw@host`): use the fields instead.
    UserInfo,
}

/// Checks and normalises a service root URL (trims, drops a trailing `/`).
pub fn check_url(s: &str) -> Result<String, UrlProblem> {
    let s = s.trim();
    if s.is_empty() {
        return Err(UrlProblem::Empty);
    }
    let (scheme, rest) = s.split_once("://").ok_or(UrlProblem::Scheme)?;
    let rest = rest.trim_end_matches('/');
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(UrlProblem::Scheme);
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return Err(UrlProblem::UserInfo);
    }
    let host = authority.rsplit_once(':').map_or(authority, |(h, _)| h);
    if host.is_empty() || s.contains(char::is_whitespace) {
        return Err(UrlProblem::NoHost);
    }
    Ok(format!("{}://{rest}", scheme.to_ascii_lowercase()))
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
            specular_log::warn!(target: specular_log::IO, "dicomweb servers: {} is not valid JSON", path.display());
            return ServerList::default();
        };
        let servers = v
            .get("servers")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        let g =
                            |k: &str| s.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                        let id = g("id");
                        if id.is_empty() {
                            return None;
                        }
                        if g("kind") == Kind::Dimse.id() {
                            let port = s.get("port").and_then(Value::as_u64)?;
                            let host = g("host");
                            if host.trim().is_empty() || port == 0 || port > 65535 {
                                return None;
                            }
                            return Some(Server {
                                id,
                                name: g("name"),
                                kind: Kind::Dimse,
                                host,
                                port: port as u16,
                                ae_title: g("ae_title"),
                                our_ae: g("our_ae"),
                                tls: s.get("tls").and_then(Value::as_bool).unwrap_or(false),
                                retrieve: if g("retrieve") == Retrieve::Move.id() {
                                    Retrieve::Move
                                } else {
                                    Retrieve::Get
                                },
                                receive_address: g("receive_address"),
                                receive_port: s
                                    .get("receive_port")
                                    .and_then(Value::as_u64)
                                    .and_then(|p| u16::try_from(p).ok())
                                    .unwrap_or(0),
                                ..Server::default()
                            });
                        }
                        let base_url = check_url(&g("url")).ok()?;
                        Some(Server {
                            id,
                            name: g("name"),
                            base_url,
                            auth: AuthKind::from_id(&g("auth")).unwrap_or_default(),
                            user: g("user"),
                            ..Server::default()
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
            .map(|s| match s.kind {
                Kind::DicomWeb => json!({
                    "id": s.id,
                    "name": s.name,
                    "url": s.base_url,
                    "auth": s.auth.id(),
                    "user": s.user,
                }),
                Kind::Dimse => json!({
                    "id": s.id,
                    "name": s.name,
                    "kind": s.kind.id(),
                    "host": s.host,
                    "port": s.port,
                    "ae_title": s.ae_title,
                    "our_ae": s.our_ae,
                    "tls": s.tls,
                    "retrieve": s.retrieve.id(),
                    "receive_address": s.receive_address,
                    "receive_port": s.receive_port,
                }),
            })
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

    pub fn get(&self, id: &str) -> Option<&Server> {
        self.servers.iter().find(|s| s.id == id)
    }

    /// Adds or replaces (by id) a server; a new one gets a fresh id. Returns the id.
    pub fn upsert(&mut self, mut server: Server) -> String {
        if server.id.is_empty() {
            server.id = new_id(&self.servers);
        }
        let id = server.id.clone();
        match self.servers.iter_mut().find(|s| s.id == id) {
            Some(s) => *s = server,
            None => self.servers.push(server),
        }
        id
    }

    pub fn remove(&mut self, id: &str) -> Option<Server> {
        let i = self.servers.iter().position(|s| s.id == id)?;
        Some(self.servers.remove(i))
    }
}

fn new_id(existing: &[Server]) -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let mut n = t;
    loop {
        let id = format!("s{n:x}");
        if !existing.iter().any(|s| s.id == id) {
            return id;
        }
        n = n.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(
            check_url(" HTTPS://pacs.example:8443/dicom-web/ "),
            Ok("https://pacs.example:8443/dicom-web".into())
        );
        assert_eq!(check_url(""), Err(UrlProblem::Empty));
        assert_eq!(check_url("ftp://x"), Err(UrlProblem::Scheme));
        assert_eq!(check_url("pacs.example"), Err(UrlProblem::Scheme));
        assert_eq!(check_url("https://"), Err(UrlProblem::NoHost));
        assert_eq!(check_url("https://u:p@h/x"), Err(UrlProblem::UserInfo));
    }

    #[test]
    fn round_trip_without_secrets() {
        let dir = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/dicomweb-servers"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("dicomweb.json");
        assert_eq!(ServerList::load(&path), ServerList::default());
        let mut l = ServerList::default();
        let id = l.upsert(Server {
            name: "Lab".into(),
            base_url: "https://pacs.example/dicom-web".into(),
            auth: AuthKind::Basic,
            user: "reader".into(),
            ..Server::default()
        });
        l.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("password"));
        let back = ServerList::load(&path);
        assert_eq!(back, l);
        assert_eq!(back.get(&id).unwrap().label(), "Lab");
        assert!(back.get(&id).unwrap().is_https());
        let mut b = back.clone();
        b.remove(&id);
        assert!(b.servers.is_empty());
    }

    #[test]
    fn dimse_servers_round_trip() {
        let dir = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/dicomweb-servers-dimse"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("dicomweb.json");
        let mut l = ServerList::default();
        let id = l.upsert(Server {
            kind: Kind::Dimse,
            host: "pacs.local".into(),
            port: 11112,
            ae_title: "ORTHANC".into(),
            ..Server::default()
        });
        l.upsert(Server {
            kind: Kind::Dimse,
            host: "secure.local".into(),
            port: 2762,
            ae_title: "SECURE".into(),
            tls: true,
            retrieve: Retrieve::Move,
            receive_address: "192.168.1.20".into(),
            receive_port: 11113,
            ..Server::default()
        });
        l.save(&path).unwrap();
        let back = ServerList::load(&path);
        assert_eq!(back, l);
        let s = back.get(&id).unwrap();
        assert_eq!(s.label(), "pacs.local");
        assert_eq!(s.calling_ae(), DEFAULT_OUR_AE);
        assert_eq!(s.address(), "ORTHANC@pacs.local:11112");
        assert!(!s.is_https());
        // A DIMSE entry without a host or port is dropped, the rest kept.
        std::fs::write(
            &path,
            r#"{"servers":[{"id":"a","kind":"dimse","host":"","port":104},
                {"id":"b","kind":"dimse","host":"h","port":70000},
                {"id":"c","kind":"dimse","host":"h","port":104,"ae_title":"X"}]}"#,
        )
        .unwrap();
        let back = ServerList::load(&path);
        assert_eq!(back.servers.len(), 1);
        assert_eq!(back.servers[0].id, "c");
    }
}
