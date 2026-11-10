//! A SAMP client: registers with the hub, says who it is and what it listens to, receives
//! messages through its own XML-RPC callback server (on `127.0.0.1`), and sends.

use std::sync::{Arc, Mutex, mpsc};

use crate::http::{self, Handler, Server};
use crate::lockfile::HubInfo;
use crate::xmlrpc::{self, Value};

/// Messages this client listens to.
pub const SUBSCRIPTIONS: &[&str] = &[
    "image.load.fits",
    "table.load.votable",
    "coord.pointAt.sky",
    "table.highlight.row",
    "table.select.rowList",
    "samp.app.ping",
    "samp.hub.event.shutdown",
    "samp.hub.disconnect",
];

/// Who this client is (`samp.name`, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub doc_url: String,
}

/// A message received.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// `image.load.fits`: open the image.
    LoadFits {
        url: String,
        name: Option<String>,
        image_id: Option<String>,
    },
    /// `table.load.votable`: overlay the catalogue.
    LoadVotable {
        url: String,
        table_id: Option<String>,
        name: Option<String>,
    },
    /// `coord.pointAt.sky`: degrees, ICRS.
    PointAt { ra: f64, dec: f64 },
    /// `table.highlight.row`: a row (0-based) of a table loaded before.
    HighlightRow {
        table_id: Option<String>,
        url: Option<String>,
        row: u64,
    },
    /// `table.select.rowList`.
    SelectRows {
        table_id: Option<String>,
        url: Option<String>,
        rows: Vec<u64>,
    },
    /// The hub went away or asked us to leave.
    Disconnected(String),
}

/// Why a hub exchange failed.
#[derive(Debug)]
pub enum Error {
    Http(http::Error),
    Xml(xmlrpc::Malformed),
    /// The hub answered with a fault.
    Fault(String),
    /// The hub's answer lacks what SAMP says it has.
    Protocol(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Http(e) => write!(f, "{e}"),
            Error::Xml(e) => write!(f, "{e}"),
            Error::Fault(s) => write!(f, "the hub refused: {s}"),
            Error::Protocol(s) => write!(f, "the hub's answer has no {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// Calls `method` on the hub at `url`.
pub fn hub_call(url: &str, method: &str, params: &[Value]) -> Result<Value, Error> {
    let body = xmlrpc::call(method, params);
    let out = http::post(url, body.as_bytes()).map_err(Error::Http)?;
    xmlrpc::parse_response(&out)
        .map_err(Error::Xml)?
        .map_err(Error::Fault)
}

/// A number given as a SAMP string.
fn number(v: Option<&Value>) -> Option<f64> {
    v?.as_str()?
        .trim()
        .parse()
        .ok()
        .filter(|f: &f64| f.is_finite())
}

/// A message's meaning, or why it is refused (for a call's error reply).
pub fn interpret(message: &Value) -> Result<Option<Incoming>, String> {
    let mtype = message.get_str("samp.mtype").ok_or("no samp.mtype")?;
    let empty = Value::Map(Vec::new());
    let p = message.get("samp.params").unwrap_or(&empty);
    let s = |k: &str| p.get_str(k).map(str::to_owned).filter(|v| !v.is_empty());
    Ok(Some(match mtype {
        "image.load.fits" => Incoming::LoadFits {
            url: s("url").ok_or("no url")?,
            name: s("name"),
            image_id: s("image-id"),
        },
        "table.load.votable" => Incoming::LoadVotable {
            url: s("url").ok_or("no url")?,
            table_id: s("table-id"),
            name: s("name"),
        },
        "coord.pointAt.sky" => {
            let ra = number(p.get("ra")).ok_or("no ra")?;
            let dec = number(p.get("dec")).ok_or("no dec")?;
            if !(-90.0..=90.0).contains(&dec) {
                return Err("dec out of range".into());
            }
            Incoming::PointAt {
                ra: ra.rem_euclid(360.0),
                dec,
            }
        }
        "table.highlight.row" => Incoming::HighlightRow {
            table_id: s("table-id"),
            url: s("url"),
            row: s("row")
                .and_then(|r| r.trim().parse().ok())
                .ok_or("no row")?,
        },
        "table.select.rowList" => Incoming::SelectRows {
            table_id: s("table-id"),
            url: s("url"),
            rows: p
                .get("row-list")
                .and_then(Value::as_list)
                .ok_or("no row-list")?
                .iter()
                .filter_map(|v| v.as_str()?.trim().parse().ok())
                .take(1_000_000)
                .collect(),
        },
        "samp.hub.event.shutdown" => Incoming::Disconnected("the hub stopped".into()),
        "samp.hub.disconnect" => Incoming::Disconnected(
            s("reason").unwrap_or_else(|| "the hub disconnected this application".into()),
        ),
        "samp.app.ping" => return Ok(None),
        other => return Err(format!("{other} is not handled")),
    }))
}

/// A response map.
fn status(ok: Result<(), String>) -> Value {
    match ok {
        Ok(()) => Value::map([
            ("samp.status", Value::str("samp.ok")),
            ("samp.result", Value::Map(Vec::new())),
        ]),
        Err(e) => Value::map([
            ("samp.status", Value::str("samp.error")),
            ("samp.error", Value::map([("samp.errortxt", Value::str(e))])),
        ]),
    }
}

/// A connection to the hub. Unregisters when dropped.
pub struct Client {
    hub: String,
    key: String,
    self_id: String,
    hub_id: String,
    _server: Server,
    /// Replies to calls go out from a thread of their own.
    replies: Mutex<mpsc::Sender<(String, Value)>>,
    registered: bool,
}

impl Client {
    /// Registers with the hub, declares `meta` and the [`SUBSCRIPTIONS`], and starts the
    /// callback server; `on` gets every message, on the server's thread.
    pub fn connect(
        hub: &HubInfo,
        meta: &Metadata,
        on: impl Fn(Incoming) + Send + Sync + 'static,
    ) -> Result<Client, Error> {
        let r = hub_call(&hub.url, "samp.hub.register", &[Value::str(&hub.secret)])?;
        let key = r
            .get_str("samp.private-key")
            .ok_or(Error::Protocol("private key"))?
            .to_owned();
        let self_id = r.get_str("samp.self-id").unwrap_or("").to_owned();
        let hub_id = r.get_str("samp.hub-id").unwrap_or("hub").to_owned();
        let url = hub.url.clone();
        // Replies are sent from their own thread: the hub may wait for this server's answer
        // before it reads ours.
        let (tx, rx) = mpsc::channel::<(String, Value)>();
        {
            let (url, key) = (url.clone(), key.clone());
            specular_thread::builder("int-samp-reply")
                .spawn(move || {
                    while let Ok((msg_id, resp)) = rx.recv() {
                        let _ = hub_call(
                            &url,
                            "samp.hub.reply",
                            &[Value::str(&key), Value::str(msg_id), resp],
                        );
                    }
                })
                .map_err(|e| Error::Http(http::Error::Io(e)))?;
        }
        let on = Arc::new(on);
        let handler: Handler = {
            let key = key.clone();
            let tx = Mutex::new(tx.clone());
            Arc::new(move |body: &[u8]| {
                let Ok((method, params)) = xmlrpc::parse_call(body) else {
                    return xmlrpc::fault(1, "unreadable call").into_bytes();
                };
                if params.first().and_then(Value::as_str) != Some(key.as_str()) {
                    return xmlrpc::fault(2, "wrong private key").into_bytes();
                }
                match method.as_str() {
                    "samp.client.receiveNotification" => {
                        if let Some(Ok(Some(m))) = params.get(2).map(interpret) {
                            on(m);
                        }
                    }
                    "samp.client.receiveCall" => {
                        let (Some(msg_id), Some(msg)) =
                            (params.get(2).and_then(Value::as_str), params.get(3))
                        else {
                            return xmlrpc::fault(1, "incomplete call").into_bytes();
                        };
                        let r = interpret(msg).map(|m| {
                            if let Some(m) = m {
                                on(m);
                            }
                        });
                        if let Ok(tx) = tx.lock() {
                            let _ = tx.send((msg_id.to_owned(), status(r)));
                        }
                    }
                    // Answers to our calls: nothing waits for them.
                    "samp.client.receiveResponse" => {}
                    _ => return xmlrpc::fault(1, "unknown method").into_bytes(),
                }
                xmlrpc::response(&Value::str("")).into_bytes()
            })
        };
        let server =
            Server::start("int-samp-srv", handler).map_err(|e| Error::Http(http::Error::Io(e)))?;
        let client = Client {
            hub: url,
            key,
            self_id,
            hub_id,
            _server: server,
            replies: Mutex::new(tx),
            registered: true,
        };
        client.call(
            "samp.hub.declareMetadata",
            vec![Value::map([
                ("samp.name", Value::str(&meta.name)),
                ("samp.description.text", Value::str(&meta.description)),
                ("samp.documentation.url", Value::str(&meta.doc_url)),
                ("specular.version", Value::str(&meta.version)),
            ])],
        )?;
        client.call(
            "samp.hub.setXmlrpcCallback",
            vec![Value::str(client._server.url())],
        )?;
        client.call(
            "samp.hub.declareSubscriptions",
            vec![Value::map(
                SUBSCRIPTIONS.iter().map(|m| (*m, Value::Map(Vec::new()))),
            )],
        )?;
        Ok(client)
    }

    fn call(&self, method: &str, mut params: Vec<Value>) -> Result<Value, Error> {
        params.insert(0, Value::str(&self.key));
        hub_call(&self.hub, method, &params)
    }

    pub fn self_id(&self) -> &str {
        &self.self_id
    }

    pub fn hub_id(&self) -> &str {
        &self.hub_id
    }

    /// Whether the hub still answers.
    pub fn ping(&self) -> bool {
        hub_call(&self.hub, "samp.hub.ping", &[]).is_ok()
    }

    /// Sends `mtype` with `params` to every subscribed client; their ids.
    pub fn notify_all(&self, mtype: &str, params: Value) -> Result<Vec<String>, Error> {
        let msg = Value::map([("samp.mtype", Value::str(mtype)), ("samp.params", params)]);
        let r = self.call("samp.hub.notifyAll", vec![msg])?;
        Ok(r.as_list()
            .unwrap_or(&[])
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect())
    }

    /// `image.load.fits` for a file; who got it.
    pub fn send_image(&self, url: &str, name: &str) -> Result<Vec<String>, Error> {
        self.notify_all(
            "image.load.fits",
            Value::map([("url", Value::str(url)), ("name", Value::str(name))]),
        )
    }

    /// `coord.pointAt.sky` (ICRS degrees).
    pub fn point_at(&self, ra: f64, dec: f64) -> Result<Vec<String>, Error> {
        self.notify_all(
            "coord.pointAt.sky",
            Value::map([
                ("ra", Value::str(format!("{ra:.8}"))),
                ("dec", Value::str(format!("{dec:.8}"))),
            ]),
        )
    }

    /// `table.highlight.row` for row `row` (0-based) of a table.
    pub fn highlight_row(
        &self,
        table_id: Option<&str>,
        url: &str,
        row: u64,
    ) -> Result<Vec<String>, Error> {
        let mut p = vec![
            ("url", Value::str(url)),
            ("row", Value::str(row.to_string())),
        ];
        if let Some(t) = table_id {
            p.push(("table-id", Value::str(t)));
        }
        self.notify_all("table.highlight.row", Value::map(p))
    }

    /// The other registered clients: id and `samp.name` (or the id).
    pub fn clients(&self) -> Vec<(String, String)> {
        let Ok(ids) = self.call("samp.hub.getRegisteredClients", vec![]) else {
            return Vec::new();
        };
        ids.as_list()
            .unwrap_or(&[])
            .iter()
            .filter_map(Value::as_str)
            .filter(|id| *id != self.hub_id)
            .map(|id| {
                let name = self
                    .call("samp.hub.getMetadata", vec![Value::str(id)])
                    .ok()
                    .and_then(|m| m.get_str("samp.name").map(str::to_owned))
                    .unwrap_or_else(|| id.to_owned());
                (id.to_owned(), name)
            })
            .collect()
    }

    /// Unregisters (also done on drop).
    pub fn disconnect(mut self) {
        self.leave();
    }

    fn leave(&mut self) {
        if std::mem::take(&mut self.registered) {
            let _ = self.call("samp.hub.unregister", vec![]);
        }
        // Ends the reply thread.
        if let Ok(mut tx) = self.replies.lock() {
            *tx = mpsc::channel().0;
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.leave();
    }
}

/// What to do with a load message's `url`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A file on this computer.
    File(std::path::PathBuf),
    /// Served by another application on this computer (fetch with [`http::get`]).
    Local(String),
}

/// Where a message's `url` points, or why it is not followed (anything off this computer).
pub fn source(url: &str) -> Result<Source, String> {
    if let Some(p) = crate::lockfile::file_path(url) {
        return Ok(Source::File(p));
    }
    if http::local(url).is_ok() {
        return Ok(Source::Local(url.to_owned()));
    }
    Err(url.to_owned())
}

#[cfg(test)]
mod tests;
