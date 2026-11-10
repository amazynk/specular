//! A minimal C-ECHO and C-STORE SCP on 127.0.0.1 for tests and screenshots: one
//! association at a time, what it stores kept in memory. Its behaviour is configurable to
//! test the SCU's failure paths (rejection, refused syntaxes, failure statuses, a small
//! maximum PDU, a server that stops answering, an abort).

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use specular_core::CancelToken;

use crate::assoc::Link;
use crate::command::{Command, field};
use crate::pdu::{self, Associate, ContextAc, Pdu, result};
use crate::{Error, Result};

/// How the SCP behaves.
#[derive(Clone, Debug)]
pub struct ScpConfig {
    /// The AE title it answers to (others are rejected with reason 7).
    pub ae_title: String,
    /// Calling AE titles it knows (empty: any); others are rejected with reason 3.
    pub known_callers: Vec<String>,
    /// Transfer syntaxes it accepts, best first.
    pub transfer_syntaxes: Vec<String>,
    /// SOP classes it accepts (empty: any).
    pub sop_classes: Vec<String>,
    /// Its Maximum Length Received.
    pub max_pdu: u32,
    /// Instances answered with this status instead of success.
    pub fail: Vec<(String, u16)>,
    /// Never answer a C-STORE (time-out tests).
    pub silent: bool,
    /// Abort after this many stores.
    pub abort_after: Option<u32>,
    /// What C-FIND searches: one record per study or series, each with its Query/Retrieve
    /// Level (0008,0052) and the attribute values (text).
    pub records: Vec<Vec<(u32, String)>>,
    /// Speak DICOM over TLS with this certificate.
    pub tls: Option<crate::tls::ServerIdentity>,
    /// What C-GET and C-MOVE retrieve: Part 10 files, matched by their study and series.
    pub instances: Vec<Vec<u8>>,
    /// C-MOVE destinations it knows: AE title and loopback port.
    pub destinations: Vec<(String, u16)>,
    /// Answer the role selection without the SCP role (C-GET can then send nothing).
    pub refuse_roles: bool,
    /// Wait this long before each sub-operation (cancel tests).
    pub sub_delay: Duration,
}

impl Default for ScpConfig {
    fn default() -> Self {
        ScpConfig {
            ae_title: "TESTSCP".into(),
            known_callers: Vec::new(),
            transfer_syntaxes: vec![
                crate::sop::EXPLICIT_LE.into(),
                crate::sop::IMPLICIT_LE.into(),
            ],
            sop_classes: Vec::new(),
            max_pdu: 16384,
            fail: Vec::new(),
            silent: false,
            abort_after: None,
            records: Vec::new(),
            tls: None,
            instances: Vec::new(),
            destinations: Vec::new(),
            refuse_roles: false,
            sub_delay: Duration::ZERO,
        }
    }
}

/// One instance stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Received {
    pub calling_ae: String,
    pub sop_class: String,
    pub sop_instance: String,
    pub transfer_syntax: String,
    pub data: Vec<u8>,
    /// P-DATA-TF fragments the data set came in.
    pub fragments: u32,
}

/// The running SCP; stops when dropped.
pub struct TestScp {
    pub port: u16,
    pub received: Arc<Mutex<Vec<Received>>>,
    pub echoes: Arc<AtomicU32>,
    pub associations: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl TestScp {
    pub fn start(config: ScpConfig) -> std::io::Result<TestScp> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let received = Arc::new(Mutex::new(Vec::new()));
        let echoes = Arc::new(AtomicU32::new(0));
        let associations = Arc::new(AtomicU32::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (r, e, a, s) = (
            received.clone(),
            echoes.clone(),
            associations.clone(),
            stop.clone(),
        );
        let handle = specular_thread::spawn("dimse-test-scp", move || {
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        a.fetch_add(1, Ordering::Relaxed);
                        if let Err(err) = serve(stream, &config, &r, &e, &s) {
                            specular_log::debug!(target: specular_log::IO, "test scp: {err}");
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Ok(TestScp {
            port,
            received,
            echoes,
            associations,
            stop,
            handle: Some(handle),
        })
    }
}

impl Drop for TestScp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn serve(
    stream: TcpStream,
    cfg: &ScpConfig,
    received: &Mutex<Vec<Received>>,
    echoes: &AtomicU32,
    stop: &Arc<AtomicBool>,
) -> Result<()> {
    // Cancelled when the SCP stops, so a silent SCP does not hold the test up.
    let cancel = CancelToken::new();
    let watch = stop.clone();
    let c2 = cancel.clone();
    let _watcher = specular_thread::spawn("dimse-test-stop", move || {
        while !watch.load(Ordering::Relaxed) && !c2.is_cancelled() {
            std::thread::sleep(Duration::from_millis(10));
        }
        c2.cancel();
    });
    let timeout = Duration::from_secs(20);
    crate::assoc::tune(&stream, timeout);
    let stream = match &cfg.tls {
        Some(id) => {
            let deadline = std::time::Instant::now() + timeout;
            match crate::tls::accept(id, stream, deadline, &cancel) {
                Ok(s) => s,
                Err(e) => {
                    cancel.cancel();
                    return Err(e);
                }
            }
        }
        None => crate::tls::Stream::Plain(stream),
    };
    let mut link = Link::new(stream, timeout, cancel.clone(), cfg.max_pdu);
    let r = serve_link(&mut link, cfg, received, echoes);
    cancel.cancel();
    r
}

fn serve_link(
    link: &mut Link,
    cfg: &ScpConfig,
    received: &Mutex<Vec<Received>>,
    echoes: &AtomicU32,
) -> Result<()> {
    let Pdu::AssociateRq(rq) = link.read_pdu()? else {
        return Err(Error::Protocol("expected A-ASSOCIATE-RQ".into()));
    };
    if rq.called_ae != cfg.ae_title {
        return link.write_pdu(&Pdu::AssociateRj {
            result: 1,
            source: 1,
            reason: 7,
        });
    }
    if !cfg.known_callers.is_empty() && !cfg.known_callers.contains(&rq.calling_ae) {
        return link.write_pdu(&Pdu::AssociateRj {
            result: 1,
            source: 1,
            reason: 3,
        });
    }
    let answered: Vec<ContextAc> = rq
        .requested
        .iter()
        .map(|c| {
            let class_ok = c.abstract_syntax == crate::sop::VERIFICATION
                || cfg.sop_classes.is_empty()
                || cfg.sop_classes.contains(&c.abstract_syntax);
            let ts = cfg
                .transfer_syntaxes
                .iter()
                .find(|t| c.transfer_syntaxes.contains(t))
                .or_else(|| {
                    (c.abstract_syntax == crate::sop::VERIFICATION)
                        .then(|| c.transfer_syntaxes.first())
                        .flatten()
                });
            match (class_ok, ts) {
                (false, _) => ContextAc {
                    id: c.id,
                    result: result::ABSTRACT_SYNTAX_NOT_SUPPORTED,
                    transfer_syntax: crate::sop::IMPLICIT_LE.into(),
                },
                (true, None) => ContextAc {
                    id: c.id,
                    result: result::TRANSFER_SYNTAXES_NOT_SUPPORTED,
                    transfer_syntax: crate::sop::IMPLICIT_LE.into(),
                },
                (true, Some(t)) => ContextAc {
                    id: c.id,
                    result: result::ACCEPTANCE,
                    transfer_syntax: t.clone(),
                },
            }
        })
        .collect();
    let roles = rq
        .roles
        .iter()
        .map(|r| crate::pdu::Role {
            sop_class: r.sop_class.clone(),
            scu: false,
            scp: r.scp && !cfg.refuse_roles,
        })
        .collect();
    let ac = Associate {
        called_ae: rq.called_ae.clone(),
        calling_ae: rq.calling_ae.clone(),
        answered: answered.clone(),
        max_pdu: cfg.max_pdu,
        roles,
        ..Associate::default()
    };
    link.write_pdu(&Pdu::AssociateAc(ac))?;
    let mut stores = 0u32;
    loop {
        let m = match read_or_release(link)? {
            Some(m) => m,
            None => return Ok(()),
        };
        let ctx = answered.iter().find(|c| c.id == m.context && c.result == 0);
        let Some(ctx) = ctx else {
            return link.write_pdu(&Pdu::Abort {
                source: 2,
                reason: 0,
            });
        };
        match m.command.field {
            field::C_ECHO_RQ => {
                echoes.fetch_add(1, Ordering::Relaxed);
                let rsp = Command::response(&m.command, 0);
                link.send_message(m.context, &rsp.encode(), None, rq.max_pdu, &mut |_| {})?;
            }
            field::C_FIND_RQ => {
                let Some((data, _)) = m.data else {
                    return Err(Error::Protocol("C-FIND-RQ without an identifier".into()));
                };
                let q = crate::find::Match::parse(&data)?;
                for rec in cfg.records.iter().filter(|r| find_matches(r, &q)) {
                    let mut rsp = Command::response(&m.command, 0xFF00);
                    rsp.data_set = true;
                    let level = if rec
                        .iter()
                        .any(|(t, v)| *t == crate::find::QUERY_LEVEL && v == "SERIES")
                    {
                        crate::find::Level::Series
                    } else {
                        crate::find::Level::Study
                    };
                    // The keys asked for, with the record's values.
                    let ident = crate::find::Query {
                        level,
                        keys: q
                            .tags()
                            .map(|t| {
                                let v = rec.iter().find(|(rt, _)| *rt == t);
                                (t, v.map_or_else(String::new, |(_, v)| v.clone()))
                            })
                            .collect(),
                    }
                    .identifier();
                    link.send_message(
                        m.context,
                        &rsp.encode(),
                        Some(&ident),
                        rq.max_pdu,
                        &mut |_| {},
                    )?;
                }
                let rsp = Command::response(&m.command, 0);
                link.send_message(m.context, &rsp.encode(), None, rq.max_pdu, &mut |_| {})?;
            }
            // Matches are sent in one go; a cancel that arrives after them changes nothing.
            field::C_CANCEL_RQ => {}
            field::C_GET_RQ | field::C_MOVE_RQ => {
                let Some((data, _)) = m.data else {
                    return Err(Error::Protocol("retrieve without an identifier".into()));
                };
                let q = crate::find::Match::parse(&data)?;
                let ctx_list: Vec<ContextAc> = answered.clone();
                let requested = rq.requested.clone();
                retrieve(
                    link, cfg, &q, m.context, &m.command, &ctx_list, &requested, rq.max_pdu,
                )?;
            }
            field::C_STORE_RQ => {
                stores += 1;
                if cfg.abort_after.is_some_and(|n| stores > n) {
                    return link.write_pdu(&Pdu::Abort {
                        source: 2,
                        reason: 0,
                    });
                }
                if cfg.silent {
                    // Holds the association open without answering until the SCU
                    // aborts or closes (or the SCP stops).
                    while link.read_pdu().is_ok() {}
                    return Ok(());
                }
                let (data, fragments) = m.data.unwrap_or_default();
                let status = cfg
                    .fail
                    .iter()
                    .find(|(u, _)| *u == m.command.sop_instance)
                    .map_or(0, |(_, s)| *s);
                if status == 0 {
                    received.lock().push(Received {
                        calling_ae: rq.calling_ae.clone(),
                        sop_class: m.command.sop_class.clone(),
                        sop_instance: m.command.sop_instance.clone(),
                        transfer_syntax: ctx.transfer_syntax.clone(),
                        data,
                        fragments,
                    });
                }
                let mut rsp = Command::response(&m.command, status);
                if status != 0 {
                    rsp.error_comment = "Test failure".into();
                }
                link.send_message(m.context, &rsp.encode(), None, rq.max_pdu, &mut |_| {})?;
            }
            _ => {
                return link.write_pdu(&Pdu::Abort {
                    source: 2,
                    reason: 0,
                });
            }
        }
    }
}

/// The files of `cfg.instances` the identifier `q` names (its study, and series if given).
fn matching(cfg: &ScpConfig, q: &crate::find::Match) -> Vec<(Vec<u8>, crate::store::Head)> {
    let study = q.text(0x0020_000D);
    let series = q.text(0x0020_000E);
    cfg.instances
        .iter()
        .filter_map(|f| {
            let i = specular_dicom::series::Instance::read(f, true).ok()??;
            let ok = i.study_uid == study && (series.is_empty() || i.series_uid == series);
            ok.then(|| crate::store::head(f).map(|h| (f.clone(), h)))
                .flatten()
        })
        .collect()
}

/// A C-GET (sub-operations on this association) or C-MOVE (on a new one to the
/// destination's loopback port), with pending responses and C-CANCEL between instances.
#[allow(clippy::too_many_arguments)]
fn retrieve(
    link: &mut Link,
    cfg: &ScpConfig,
    q: &crate::find::Match,
    ctx: u8,
    rq: &Command,
    answered: &[ContextAc],
    requested: &[crate::pdu::ContextRq],
    peer_max: u32,
) -> Result<()> {
    use crate::command::Counts;
    let files = matching(cfg, q);
    let mut counts = Counts {
        remaining: Some(files.len() as u16),
        ..Counts::default()
    };
    let respond = |link: &mut Link, status: u16, counts: Counts| -> Result<()> {
        let mut rsp = Command::response(rq, status);
        rsp.counts = counts;
        if status != 0xFF00 {
            rsp.counts.remaining = None;
        }
        link.send_message(ctx, &rsp.encode(), None, peer_max, &mut |_| {})
    };
    let mut dest = None;
    if rq.field == field::C_MOVE_RQ {
        let Some((_, port)) = cfg
            .destinations
            .iter()
            .find(|(ae, _)| *ae == rq.move_destination)
        else {
            return respond(link, 0xA801, Counts::default());
        };
        let peer = crate::assoc::Peer {
            host: "127.0.0.1".into(),
            port: *port,
            called_ae: rq.move_destination.clone(),
            calling_ae: cfg.ae_title.clone(),
        };
        let heads = files
            .iter()
            .map(|(_, h)| (h.sop_class.as_str(), h.transfer_syntax.as_str()));
        let contexts = crate::store::contexts(heads);
        match crate::assoc::Association::request(
            &peer,
            &crate::assoc::Options::default(),
            &contexts,
            &CancelToken::new(),
        ) {
            Ok(a) => dest = Some(a),
            Err(e) => {
                specular_log::debug!(target: specular_log::IO, "test scp: move destination: {e}");
                counts.failed = files.len() as u16;
                return respond(link, if files.is_empty() { 0 } else { 0xA702 }, counts);
            }
        }
    }
    let mut cancelled = false;
    let mut sub_id = 0x4000u16;
    for (file, h) in &files {
        std::thread::sleep(cfg.sub_delay);
        counts.remaining = counts.remaining.map(|r| r.saturating_sub(1));
        let ok = if let Some(a) = dest.as_mut() {
            a.send_file(file, None, &mut |_| {})
                .map(|s| crate::command::Status::of(s.status).stored())
                .unwrap_or(false)
        } else {
            // On this association: a context for the class with the file's syntax (as
            // stored), else one in a little-endian syntax (converted).
            let pick = |want: Option<&str>| {
                answered.iter().find(|a| {
                    a.result == 0
                        && requested
                            .iter()
                            .any(|r| r.id == a.id && r.abstract_syntax == h.sop_class)
                        && want.is_none_or(|t| a.transfer_syntax == t)
                })
            };
            let roles_ok = !cfg.refuse_roles;
            let chosen = pick(Some(&h.transfer_syntax)).or_else(|| {
                pick(None).filter(|a| {
                    a.transfer_syntax == crate::sop::EXPLICIT_LE
                        || a.transfer_syntax == crate::sop::IMPLICIT_LE
                })
            });
            match chosen.filter(|_| roles_ok) {
                None => false,
                Some(c) => {
                    let data = if c.transfer_syntax == h.transfer_syntax {
                        file[h.data_set..].to_vec()
                    } else {
                        crate::store::transcode(file, &c.transfer_syntax, None)?
                    };
                    sub_id = sub_id.wrapping_add(1);
                    let srq = Command::store_rq(sub_id, &h.sop_class, &h.sop_instance);
                    link.send_message(c.id, &srq.encode(), Some(&data), peer_max, &mut |_| {})?;
                    // The answer; a C-CANCEL may come before it.
                    let stored;
                    loop {
                        let Some(r) = read_or_release(link)? else {
                            return Ok(());
                        };
                        match r.command.field {
                            field::C_CANCEL_RQ => cancelled = true,
                            field::C_STORE_RSP => {
                                stored = r.command.status.is_some_and(|s| s == 0);
                                break;
                            }
                            f => {
                                return Err(Error::Protocol(format!(
                                    "test scp: {f:#06x} during C-GET"
                                )));
                            }
                        }
                    }
                    stored
                }
            }
        };
        if ok {
            counts.completed += 1;
        } else {
            counts.failed += 1;
        }
        if cancelled {
            break;
        }
        respond(link, 0xFF00, counts)?;
        // A cancel sent while the SCU waited for this response.
        let t0 = std::time::Instant::now();
        while t0.elapsed() < cfg.sub_delay {
            if link.stream.tcp().peek(&mut [0u8; 1]).is_ok_and(|n| n > 0) {
                match read_or_release(link)? {
                    Some(r) if r.command.field == field::C_CANCEL_RQ => cancelled = true,
                    Some(_) => return Err(Error::Protocol("test scp: unexpected message".into())),
                    None => return Ok(()),
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if cancelled {
            break;
        }
    }
    if let Some(a) = dest {
        let _ = a.release();
    }
    let status = if cancelled {
        0xFE00
    } else if counts.failed > 0 || counts.warning > 0 {
        0xB000
    } else {
        0x0000
    };
    respond(link, status, counts)
}

/// Whether `rec` matches every key of `q` with a value (PS3.4 C.2.2.2): `*`/`?` wildcards,
/// date ranges, UID lists; person names without regard to case.
fn find_matches(rec: &[(u32, String)], q: &crate::find::Match) -> bool {
    use crate::find::{QUERY_LEVEL, SPECIFIC_CHARACTER_SET};
    let level = q.text(QUERY_LEVEL);
    let val = |t: u32| {
        rec.iter()
            .find(|(rt, _)| *rt == t)
            .map_or("", |(_, v)| v.as_str())
    };
    if val(QUERY_LEVEL) != level {
        return false;
    }
    q.tags().all(|t| {
        let key = q.text(t);
        if key.is_empty() || t == QUERY_LEVEL || t == SPECIFIC_CHARACTER_SET {
            return true;
        }
        let v = val(t);
        let vr = crate::dict_vr(t);
        match vr {
            specular_dicom::dict::Vr::DA | specular_dicom::dict::Vr::TM => {
                match key.split_once('-') {
                    Some((a, b)) => (a.is_empty() || v >= a) && (b.is_empty() || v <= b),
                    None => v == key,
                }
            }
            specular_dicom::dict::Vr::UI => key.split('\\').any(|k| k == v),
            specular_dicom::dict::Vr::PN => wildcard(&key.to_lowercase(), &v.to_lowercase()),
            // Multi-valued (Modalities in Study): any value.
            _ => v.split('\\').any(|v| wildcard(&key, v)),
        }
    })
}

/// `*` any run, `?` one character.
fn wildcard(p: &str, s: &str) -> bool {
    let (p, s): (Vec<char>, Vec<char>) = (p.chars().collect(), s.chars().collect());
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < s.len() {
        if i < p.len() && (p[i] == '?' || p[i] == s[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(st) = star {
            i = st + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|&c| c == '*')
}

struct In {
    context: u8,
    command: Command,
    /// The data set and how many fragments it came in.
    data: Option<(Vec<u8>, u32)>,
}

/// The next message, or `None` once the SCU released (answered).
fn read_or_release(link: &mut Link) -> Result<Option<In>> {
    let mut cmd = Vec::new();
    let mut data = Vec::new();
    let mut fragments = 0;
    let mut command: Option<Command> = None;
    loop {
        match link.read_pdu()? {
            Pdu::ReleaseRq if command.is_none() && cmd.is_empty() => {
                link.write_pdu(&Pdu::ReleaseRp)?;
                return Ok(None);
            }
            Pdu::PData(pdvs) => {
                for p in pdvs {
                    if p.command {
                        cmd.extend_from_slice(&p.data);
                        if p.last {
                            let c = Command::decode(&cmd)?;
                            if !c.data_set {
                                return Ok(Some(In {
                                    context: p.context,
                                    command: c,
                                    data: None,
                                }));
                            }
                            command = Some(c);
                        }
                    } else {
                        fragments += 1;
                        data.extend_from_slice(&p.data);
                        if p.last {
                            return Ok(Some(In {
                                context: p.context,
                                command: command.take().unwrap_or_default(),
                                data: Some((std::mem::take(&mut data), fragments)),
                            }));
                        }
                    }
                }
            }
            Pdu::Abort { source, reason } => return Err(Error::Aborted { source, reason }),
            other => {
                return Err(Error::Protocol(format!(
                    "unexpected {}",
                    crate::assoc::pdu_name(&other)
                )));
            }
        }
    }
}

/// Encodes a PDU (for tests that speak the protocol by hand).
pub fn encode(p: &Pdu) -> Vec<u8> {
    pdu::encode(p)
}
