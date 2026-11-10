//! A small SAMP hub for tests (loopback only): registration, metadata, subscriptions,
//! callbacks, `notify`/`notifyAll`, `call`/`callAll`/`reply`, and a probe client of its own
//! that sends messages and records what it receives. Writes its lock file where asked.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::http::{self, Server};
use crate::xmlrpc::{self, Value};

#[derive(Clone, Default)]
struct Rec {
    id: String,
    meta: Value,
    subs: Vec<String>,
    callback: Option<String>,
}

#[derive(Default)]
struct State {
    /// By private key.
    clients: HashMap<String, Rec>,
    next: u32,
    /// What the probe received: mtype and params.
    probe: Vec<(String, Value)>,
    /// Replies to the probe's calls: msg-tag and response.
    responses: Vec<Value>,
    calls: u32,
}

/// The hub. Stops answering when dropped.
pub struct Hub {
    secret: String,
    server: Server,
    state: Arc<Mutex<State>>,
}

const PROBE_KEY: &str = "probe-key";
const PROBE_ID: &str = "c-probe";

fn subscribed(subs: &[String], mtype: &str) -> bool {
    subs.iter().any(|s| {
        s == mtype
            || s == "*"
            || s.strip_suffix('*')
                .is_some_and(|p| mtype.starts_with(p) && p.ends_with('.'))
    })
}

impl Hub {
    /// Starts a hub; its lock file goes to `lock`.
    pub fn start(lock: &Path) -> std::io::Result<Hub> {
        let state = Arc::new(Mutex::new(State::default()));
        {
            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
            s.clients.insert(
                PROBE_KEY.into(),
                Rec {
                    id: PROBE_ID.into(),
                    meta: Value::map([("samp.name", Value::str("Probe"))]),
                    subs: vec!["*".into()],
                    callback: None,
                },
            );
        }
        let secret = "test-secret".to_owned();
        let st = state.clone();
        let sec = secret.clone();
        let server = Server::start(
            "test-samp-hub",
            Arc::new(move |body: &[u8]| match xmlrpc::parse_call(body) {
                Ok((m, p)) => match handle(&st, &sec, &m, &p) {
                    Ok(v) => xmlrpc::response(&v).into_bytes(),
                    Err(e) => xmlrpc::fault(1, &e).into_bytes(),
                },
                Err(e) => xmlrpc::fault(1, &e.to_string()).into_bytes(),
            }),
        )?;
        std::fs::write(
            lock,
            format!(
                "# SAMP Standard Profile lockfile (test hub)\nsamp.secret={secret}\n\
                 samp.hub.xmlrpc.url={}\nsamp.profile.version=1.3\n",
                server.url()
            ),
        )?;
        Ok(Hub {
            secret,
            server,
            state,
        })
    }

    pub fn url(&self) -> String {
        self.server.url()
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }

    /// The registered clients (the probe left out): id, `samp.name` and subscriptions.
    pub fn clients(&self) -> Vec<(String, String, Vec<String>)> {
        let s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<_> = s
            .clients
            .values()
            .filter(|r| r.id != PROBE_ID)
            .map(|r| {
                (
                    r.id.clone(),
                    r.meta.get_str("samp.name").unwrap_or("").to_owned(),
                    r.subs.clone(),
                )
            })
            .collect();
        v.sort();
        v
    }

    /// The probe sends a notification to every subscribed client.
    pub fn notify(&self, mtype: &str, params: Value) {
        let msg = Value::map([("samp.mtype", Value::str(mtype)), ("samp.params", params)]);
        let _ = handle(
            &self.state,
            &self.secret,
            "samp.hub.notifyAll",
            &[Value::str(PROBE_KEY), msg],
        );
    }

    /// The probe calls every subscribed client (their replies land in [`Hub::responses`]).
    pub fn call(&self, mtype: &str, params: Value) {
        let msg = Value::map([("samp.mtype", Value::str(mtype)), ("samp.params", params)]);
        let _ = handle(
            &self.state,
            &self.secret,
            "samp.hub.callAll",
            &[Value::str(PROBE_KEY), Value::str("tag"), msg],
        );
    }

    /// What the probe received.
    pub fn received(&self) -> Vec<(String, Value)> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .probe
            .clone()
    }

    /// Replies to the probe's calls.
    pub fn responses(&self) -> Vec<Value> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .responses
            .clone()
    }
}

/// Delivers to `rec` (the probe records; others get an XML-RPC callback, from a thread so
/// the sender's call returns first).
fn deliver(state: &Arc<Mutex<State>>, rec: &Rec, key: &str, method: &str, params: Vec<Value>) {
    if rec.id == PROBE_ID {
        let msg = params.last().cloned().unwrap_or(Value::str(""));
        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
        if method == "samp.client.receiveResponse" {
            s.responses.push(msg);
        } else {
            let mtype = msg.get_str("samp.mtype").unwrap_or("").to_owned();
            let p = msg
                .get("samp.params")
                .cloned()
                .unwrap_or(Value::Map(vec![]));
            s.probe.push((mtype, p));
        }
        return;
    }
    let Some(url) = rec.callback.clone() else {
        return;
    };
    let mut all = vec![Value::str(key)];
    all.extend(params);
    let body = xmlrpc::call(method, &all);
    let _ = specular_thread::builder("test-samp-cb").spawn(move || {
        let _ = http::post(&url, body.as_bytes());
    });
}

fn handle(
    state: &Arc<Mutex<State>>,
    secret: &str,
    method: &str,
    p: &[Value],
) -> Result<Value, String> {
    let arg = |i: usize| p.get(i).and_then(Value::as_str).unwrap_or("").to_owned();
    if method == "samp.hub.ping" {
        return Ok(Value::str(""));
    }
    if method == "samp.hub.register" {
        if arg(0) != secret {
            return Err("bad secret".into());
        }
        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
        s.next += 1;
        let (key, id) = (format!("key-{}", s.next), format!("c{}", s.next));
        s.clients.insert(
            key.clone(),
            Rec {
                id: id.clone(),
                meta: Value::Map(vec![]),
                ..Rec::default()
            },
        );
        return Ok(Value::map([
            ("samp.private-key", Value::str(key)),
            ("samp.self-id", Value::str(id)),
            ("samp.hub-id", Value::str("hub")),
        ]));
    }
    let key = arg(0);
    let me = {
        let s = state.lock().unwrap_or_else(|e| e.into_inner());
        s.clients.get(&key).cloned().ok_or("unknown private key")?
    };
    let others = |mtype: &str| -> Vec<(String, Rec)> {
        let s = state.lock().unwrap_or_else(|e| e.into_inner());
        s.clients
            .iter()
            .filter(|(k, r)| **k != key && subscribed(&r.subs, mtype))
            .map(|(k, r)| (k.clone(), r.clone()))
            .collect()
    };
    let s_upd = |f: &dyn Fn(&mut Rec)| {
        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = s.clients.get_mut(&key) {
            f(r);
        }
    };
    match method {
        "samp.hub.unregister" => {
            state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clients
                .remove(&key);
        }
        "samp.hub.declareMetadata" => {
            let m = p.get(1).cloned().ok_or("no metadata")?;
            s_upd(&|r| r.meta = m.clone());
        }
        "samp.hub.setXmlrpcCallback" => {
            let u = arg(1);
            http::local(&u).map_err(|e| e.to_string())?;
            s_upd(&|r| r.callback = Some(u.clone()));
        }
        "samp.hub.declareSubscriptions" => {
            let subs: Vec<String> = p
                .get(1)
                .and_then(Value::as_map)
                .ok_or("no subscriptions")?
                .iter()
                .map(|(k, _)| k.clone())
                .collect();
            s_upd(&|r| r.subs = subs.clone());
        }
        "samp.hub.getRegisteredClients" => {
            let s = state.lock().unwrap_or_else(|e| e.into_inner());
            return Ok(Value::List(
                s.clients
                    .iter()
                    .filter(|(k, _)| **k != key)
                    .map(|(_, r)| Value::str(&r.id))
                    .collect(),
            ));
        }
        "samp.hub.getMetadata" => {
            let id = arg(1);
            let s = state.lock().unwrap_or_else(|e| e.into_inner());
            return s
                .clients
                .values()
                .find(|r| r.id == id)
                .map(|r| r.meta.clone())
                .ok_or_else(|| "no such client".into());
        }
        "samp.hub.notifyAll" | "samp.hub.notify" => {
            let msg = p.last().cloned().ok_or("no message")?;
            let mtype = msg.get_str("samp.mtype").ok_or("no mtype")?.to_owned();
            let only = (method == "samp.hub.notify").then(|| arg(1));
            let mut ids = Vec::new();
            for (k, r) in others(&mtype) {
                if only.as_ref().is_some_and(|o| *o != r.id) {
                    continue;
                }
                ids.push(Value::str(&r.id));
                deliver(
                    state,
                    &r,
                    &k,
                    "samp.client.receiveNotification",
                    vec![Value::str(&me.id), msg.clone()],
                );
            }
            return Ok(Value::List(ids));
        }
        "samp.hub.callAll" => {
            let msg = p.get(2).cloned().ok_or("no message")?;
            let mtype = msg.get_str("samp.mtype").ok_or("no mtype")?.to_owned();
            let mut out = Vec::new();
            for (k, r) in others(&mtype) {
                let id = {
                    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                    s.calls += 1;
                    format!("{}:{}:{}", me.id, arg(1), s.calls)
                };
                out.push((r.id.clone(), Value::str(&id)));
                deliver(
                    state,
                    &r,
                    &k,
                    "samp.client.receiveCall",
                    vec![Value::str(&me.id), Value::str(id), msg.clone()],
                );
            }
            return Ok(Value::map(out));
        }
        "samp.hub.reply" => {
            let msg_id = arg(1);
            let resp = p.get(2).cloned().ok_or("no response")?;
            let caller = msg_id.split(':').next().unwrap_or("").to_owned();
            let s = state.lock().unwrap_or_else(|e| e.into_inner());
            let target = s
                .clients
                .iter()
                .find(|(_, r)| r.id == caller)
                .map(|(k, r)| (k.clone(), r.clone()));
            drop(s);
            if let Some((k, r)) = target {
                deliver(
                    state,
                    &r,
                    &k,
                    "samp.client.receiveResponse",
                    vec![Value::str(&me.id), Value::str("tag"), resp],
                );
            }
        }
        other => return Err(format!("{other} is not implemented by the test hub")),
    }
    Ok(Value::str(""))
}
