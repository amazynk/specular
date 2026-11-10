//! C-GET and C-MOVE SCU (PS3.4 annex C.4.2, C.4.3; PS3.7 9.1.3, 9.1.4): a study or series
//! retrieved, Study Root.
//!
//! - **C-GET** needs no inbound port: the association offers the GET context and the storage
//!   classes with the SCP role (SCP/SCU Role Selection), and the server sends each instance
//!   as a C-STORE sub-operation on the same association. Each storage class gets a context
//!   with Explicit and Implicit VR Little Endian; the common image classes also get one per
//!   common compressed syntax, so that the server can send what it stores as stored.
//! - **C-MOVE** asks the server to send the instances to an AE title it knows: here our own,
//!   received by [`crate::listen`] while the move runs.
//!
//! Instances go to a [`Storage`] as they arrive, a fragment at a time (nothing is held in
//! memory). The pending responses' counts (remaining, completed, failed, warning) are
//! reported as they come. A stop asks the server with C-CANCEL and waits for its final
//! answer (sub-operations still arriving meanwhile are stored).

use std::io;
use std::time::Duration;

use parking_lot::Mutex;
use specular_core::CancelToken;

use crate::assoc::{Association, Got, Link, Options, Part, Peer, SoftStop};
use crate::command::{Command, Counts, field};
use crate::find::{Level, Query};
use crate::pdu::{ContextRq, Role};
use crate::sop::{self, EXPLICIT_LE, IMPLICIT_LE, STUDY_ROOT_GET, STUDY_ROOT_MOVE};
use crate::{Error, Result};

const STUDY_UID: u32 = 0x0020_000D;
const SERIES_UID: u32 = 0x0020_000E;
/// Failed SOP Instance UID List (0008,0058), in a final response's identifier.
const FAILED_UIDS: u32 = 0x0008_0058;

/// How long a stopped retrieval waits for the server's final answer.
const DRAIN: Duration = Duration::from_secs(10);

/// What to retrieve: a study, or one of its series.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub study: String,
    pub series: Option<String>,
}

impl Target {
    /// The identifier: Query/Retrieve Level and the unique keys.
    pub fn identifier(&self) -> Vec<u8> {
        let mut keys = vec![(STUDY_UID, self.study.trim().to_owned())];
        let level = match &self.series {
            Some(s) => {
                keys.push((SERIES_UID, s.trim().to_owned()));
                Level::Series
            }
            None => Level::Study,
        };
        Query { level, keys }.identifier()
    }
}

/// Where retrieved instances go. Several can arrive at once (a C-MOVE's server may open
/// more than one association).
pub trait Storage: Send {
    /// An instance starts arriving (its data set in `transfer_syntax`); `Err` refuses it.
    fn begin(
        &mut self,
        sop_class: &str,
        sop_instance: &str,
        transfer_syntax: &str,
    ) -> io::Result<Box<dyn Incoming>>;
    /// An instance was kept ([`Incoming::finish`] succeeded).
    fn stored(&mut self, _sop_instance: &str) {}
}

/// One instance arriving. Dropped without [`finish`](Self::finish): it did not arrive
/// whole, and what was written goes.
pub trait Incoming: Send {
    /// The next piece of its data set.
    fn write(&mut self, data: &[u8]) -> io::Result<()>;
    /// It arrived whole; `Ok`: kept.
    fn finish(self: Box<Self>) -> io::Result<()>;
}

/// How far a retrieval is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// The server's counts from its last response.
    pub counts: Counts,
    /// Instances stored here.
    pub received: u32,
    /// Instances that arrived but could not be stored.
    pub refused: u32,
    /// Data set bytes received.
    pub bytes: u64,
}

impl Progress {
    /// Sub-operations ended (either way) and in all, as far as the server said.
    pub fn done_total(&self) -> (u32, u32) {
        let c = self.counts;
        let done = u32::from(c.completed) + u32::from(c.failed) + u32::from(c.warning);
        let done = done.max(self.received + self.refused);
        (done, done + u32::from(c.remaining.unwrap_or(0)))
    }
}

/// How a retrieval ended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The final response's status: 0 success, 0xB000 some sub-operations failed, 0xFE00
    /// cancelled, 0xA7xx out of resources, 0xA801 unknown move destination, 0xA900/0xCxxx
    /// refused.
    pub status: u16,
    pub progress: Progress,
    /// Stopped by us (C-CANCEL sent).
    pub cancelled: bool,
    /// The server answered the role selection and gave us the SCP role for no class
    /// (C-GET only): it cannot send anything over this association.
    pub roles_refused: bool,
    /// From the final response, when the server listed them.
    pub failed_uids: Vec<String>,
}

impl Outcome {
    /// Nothing matched: success with no sub-operation at all.
    pub fn no_matches(&self) -> bool {
        let c = self.progress.counts;
        matches!(self.status, 0x0000 | 0xB000)
            && c.completed == 0
            && c.failed == 0
            && c.warning == 0
            && self.progress.received == 0
    }
}

/// What both threads of a C-MOVE (and the one of a C-GET) share.
pub(crate) struct Shared<'a> {
    pub storage: &'a mut dyn Storage,
    pub progress: Progress,
    pub report: &'a mut (dyn FnMut(&Progress) + Send),
}

impl Shared<'_> {
    fn tell(&mut self) {
        let p = self.progress;
        (self.report)(&p);
    }
}

/// Receives C-STORE-RQ data sets into the shared storage, a fragment at a time.
pub(crate) struct Receiver<'s, 'a> {
    pub shared: &'s Mutex<Shared<'a>>,
    /// (context id, transfer syntax) accepted for storage.
    pub contexts: Vec<(u8, String)>,
    /// The current instance: `Some(Ok)` while it is being written.
    current: Option<io::Result<Box<dyn Incoming>>>,
    /// The data set of a message that is not a C-STORE (a response's identifier).
    pub other: Vec<u8>,
}

impl<'s, 'a> Receiver<'s, 'a> {
    pub fn new(shared: &'s Mutex<Shared<'a>>, contexts: Vec<(u8, String)>) -> Self {
        Receiver {
            shared,
            contexts,
            current: None,
            other: Vec::new(),
        }
    }

    /// Takes a message part ([`Link::read_parts`]).
    pub fn on(&mut self, p: Part<'_>) -> Result<()> {
        match p {
            Part::Start(ctx, c) if c.field == field::C_STORE_RQ => {
                let ts = self.contexts.iter().find(|(id, _)| *id == ctx);
                let r = match ts {
                    Some((_, ts)) => {
                        self.shared
                            .lock()
                            .storage
                            .begin(&c.sop_class, &c.sop_instance, ts)
                    }
                    None => Err(io::Error::other("no storage context")),
                };
                self.current = Some(r);
            }
            Part::Start(..) => {
                self.current = None;
                self.other.clear();
            }
            Part::Data(d) => match self.current.as_mut() {
                Some(Ok(inc)) => {
                    let r = inc.write(d);
                    self.shared.lock().progress.bytes += d.len() as u64;
                    if let Err(e) = r {
                        self.current = Some(Err(e));
                    }
                }
                Some(Err(_)) => {}
                None => {
                    if self.other.len() + d.len() > 1 << 20 {
                        return Err(Error::Protocol("response identifier too large".into()));
                    }
                    self.other.extend_from_slice(d);
                }
            },
        }
        Ok(())
    }

    /// The C-STORE `rq` ended: keeps the instance and answers the server.
    pub fn answer(&mut self, link: &mut Link, ctx: u8, rq: &Command, peer_max: u32) -> Result<()> {
        let r = match self.current.take() {
            Some(Ok(inc)) => inc.finish(),
            Some(Err(e)) => Err(e),
            // A C-STORE-RQ without a data set.
            None => Err(io::Error::other("no data set")),
        };
        let status = {
            let mut s = self.shared.lock();
            let status = match &r {
                Ok(()) => {
                    s.progress.received += 1;
                    s.storage.stored(&rq.sop_instance);
                    0x0000
                }
                Err(e) => {
                    specular_log::warn!(target: specular_log::IO,
                        "dimse retrieve: instance not stored: {e}");
                    s.progress.refused += 1;
                    // Out of resources: the server lists it as failed.
                    0xA700
                }
            };
            s.tell();
            status
        };
        let rsp = Command::response(rq, status);
        link.send_message(ctx, &rsp.encode(), None, peer_max, &mut |_| {})
    }
}

/// The contexts and roles a C-GET association offers: GET first (id 1), then each storage
/// class with the little-endian syntaxes, then the common classes with each common
/// compressed syntax, as far as the 128 contexts go.
pub fn get_contexts() -> (Vec<ContextRq>, Vec<Role>) {
    let mut out = vec![ContextRq {
        id: 1,
        abstract_syntax: STUDY_ROOT_GET.into(),
        transfer_syntaxes: vec![IMPLICIT_LE.into()],
    }];
    let mut roles = Vec::new();
    let push = |out: &mut Vec<ContextRq>, class: &str, ts: Vec<String>| {
        if out.len() < crate::store::MAX_CONTEXTS {
            let id = (out.len() * 2 + 1) as u8;
            out.push(ContextRq {
                id,
                abstract_syntax: class.to_owned(),
                transfer_syntaxes: ts,
            });
        }
    };
    for c in sop::storage_classes() {
        push(&mut out, c, vec![EXPLICIT_LE.into(), IMPLICIT_LE.into()]);
        roles.push(Role {
            sop_class: c.to_owned(),
            scu: false,
            scp: true,
        });
    }
    for c in sop::COMMON_IMAGES {
        for ts in sop::COMMON_COMPRESSED {
            push(&mut out, c, vec![(*ts).to_owned()]);
        }
    }
    (out, roles)
}

fn pending(s: u16) -> bool {
    matches!(s, 0xFF00 | 0xFF01)
}

/// The failed UIDs of a final response's identifier.
fn failed_uids(ident: &[u8]) -> Vec<String> {
    crate::find::Match::parse(ident)
        .map(|m| m.texts(FAILED_UIDS))
        .unwrap_or_default()
}

/// Sends `rq` with `ident` on `ctx` and reads the responses until the final one, storing
/// C-STORE sub-operations when `store` is given (C-GET). `stop` asks for a C-CANCEL.
fn run(
    a: &mut Association,
    ctx: u8,
    rq: &Command,
    ident: &[u8],
    shared: &Mutex<Shared<'_>>,
    store: Option<Vec<(u8, String)>>,
    stop: SoftStop,
) -> Result<Outcome> {
    let peer_max = a.peer_max_pdu();
    a.send(ctx, rq, Some(ident))?;
    let accepts_stores = store.is_some();
    let mut rx = Receiver::new(shared, store.unwrap_or_default());
    let mut out = Outcome::default();
    let link = a.link();
    link.soft = Some(stop);
    let id = rq.message_id;
    loop {
        let got = link.read_parts(&mut |p| rx.on(p));
        let (context, command) = match got {
            Ok(Got::Message { context, command }) => (context, command),
            Ok(Got::Release) => {
                return Err(Error::Protocol("release asked during a retrieval".into()));
            }
            Err(Error::Cancelled) if !out.cancelled && link.soft.is_some() => {
                // Stopped between messages: ask the server to stop and wait for its answer.
                link.soft = None;
                link.timeout = link.timeout.min(DRAIN);
                out.cancelled = true;
                let c = Command::cancel_rq(id.unwrap_or(0));
                link.send_message(ctx, &c.encode(), None, peer_max, &mut |_| {})?;
                continue;
            }
            Err(e) => return Err(e),
        };
        match command.field {
            field::C_STORE_RQ if accepts_stores => {
                rx.answer(link, context, &command, peer_max)?;
            }
            field::C_STORE_RQ => {
                return Err(Error::Protocol("C-STORE on a C-MOVE association".into()));
            }
            f if f == rq.field | 0x8000 && command.responded_to == id => {
                let status = command
                    .status
                    .ok_or_else(|| Error::Protocol("retrieve response without a status".into()))?;
                {
                    let mut s = shared.lock();
                    let keep = s.progress.counts.remaining;
                    s.progress.counts = command.counts;
                    if command.counts.remaining.is_none() && pending(status) {
                        s.progress.counts.remaining = keep;
                    }
                    s.tell();
                }
                if !pending(status) {
                    out.status = status;
                    out.progress = shared.lock().progress;
                    out.failed_uids = failed_uids(&rx.other);
                    if status == 0xFE00 {
                        out.cancelled = true;
                    }
                    return Ok(out);
                }
            }
            f => {
                return Err(Error::Protocol(format!(
                    "unexpected message {f:#06x} during a retrieval"
                )));
            }
        }
    }
}

/// Releases `a` after a retrieval (a release that is not answered changes nothing).
fn finish(a: Association, r: Result<Outcome>) -> Result<Outcome> {
    match r {
        Ok(o) => {
            if let Err(e) = a.release() {
                specular_log::debug!(target: specular_log::IO, "dimse release after retrieve: {e}");
            }
            Ok(o)
        }
        // Dropping the association aborts it.
        Err(e) => Err(e),
    }
}

/// C-GET of `target` from `peer` into `storage`. `report` hears every response and stored
/// instance; `stop` (or `cancel`) asks the server to stop with C-CANCEL. `cancel` also
/// cuts a connection that is still being made.
pub fn get(
    peer: &Peer,
    opts: &Options,
    target: &Target,
    storage: &mut dyn Storage,
    report: &mut (dyn FnMut(&Progress) + Send),
    stop: SoftStop,
    cancel: &CancelToken,
) -> Result<Outcome> {
    let (contexts, roles) = get_contexts();
    let mut a = Association::request_with_roles(peer, opts, &contexts, &roles, cancel)?;
    let ctx = a
        .find_context(STUDY_ROOT_GET)
        .ok_or_else(|| Error::NoContext(STUDY_ROOT_GET.into()))?;
    let roles_refused = !a.roles.is_empty() && !a.roles.iter().any(|r| r.scp);
    let store: Vec<(u8, String)> = a
        .accepted()
        .iter()
        .filter(|c| sop::is_storage(&c.abstract_syntax))
        .map(|c| (c.id, c.transfer_syntax.clone()))
        .collect();
    // From here only a stop between messages ends a wait (the stream stays in step).
    a.link().cancel = CancelToken::new();
    let user = cancel.clone();
    let stop: SoftStop = Box::new(move || user.is_cancelled() || stop());
    let shared = Mutex::new(Shared {
        storage,
        progress: Progress::default(),
        report,
    });
    let id = a.next_message_id();
    let rq = Command::get_rq(id, STUDY_ROOT_GET);
    let r = run(
        &mut a,
        ctx,
        &rq,
        &target.identifier(),
        &shared,
        Some(store),
        stop,
    );
    let r = r.map(|mut o| {
        o.roles_refused = roles_refused;
        o
    });
    finish(a, r)
}

/// The C-MOVE SCU half: asks `peer` to send `target` to `destination` and follows its
/// responses. The instances arrive elsewhere ([`crate::listen`]), sharing `shared`.
pub(crate) fn move_scu(
    peer: &Peer,
    opts: &Options,
    target: &Target,
    destination: &str,
    shared: &Mutex<Shared<'_>>,
    stop: SoftStop,
    cancel: &CancelToken,
) -> Result<Outcome> {
    let contexts = [ContextRq {
        id: 1,
        abstract_syntax: STUDY_ROOT_MOVE.into(),
        transfer_syntaxes: vec![IMPLICIT_LE.into()],
    }];
    let mut a = Association::request(peer, opts, &contexts, cancel)?;
    let ctx = a
        .find_context(STUDY_ROOT_MOVE)
        .ok_or_else(|| Error::NoContext(STUDY_ROOT_MOVE.into()))?;
    a.link().cancel = CancelToken::new();
    let user = cancel.clone();
    let stop: SoftStop = Box::new(move || user.is_cancelled() || stop());
    let id = a.next_message_id();
    let rq = Command::move_rq(id, STUDY_ROOT_MOVE, destination);
    let r = run(&mut a, ctx, &rq, &target.identifier(), shared, None, stop);
    finish(a, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contexts_fit_and_offer_the_scp_role() {
        let (c, roles) = get_contexts();
        assert!(c.len() <= crate::store::MAX_CONTEXTS);
        assert_eq!(c[0].abstract_syntax, STUDY_ROOT_GET);
        let mut ids: Vec<u8> = c.iter().map(|c| c.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), c.len());
        assert!(ids.iter().all(|i| i % 2 == 1));
        // Every class proposed has a role item asking for the SCP role only.
        for ctx in &c[1..] {
            assert!(
                roles
                    .iter()
                    .any(|r| r.sop_class == ctx.abstract_syntax && r.scp && !r.scu)
            );
        }
        // CT also as JPEG Lossless and RLE.
        let ct = "1.2.840.10008.5.1.4.1.1.2";
        for ts in ["1.2.840.10008.1.2.4.70", "1.2.840.10008.1.2.5"] {
            assert!(
                c.iter()
                    .any(|x| x.abstract_syntax == ct && x.transfer_syntaxes == [ts])
            );
        }
    }

    #[test]
    fn identifiers() {
        let t = Target {
            study: "1.2.3".into(),
            series: Some("1.2.3.4".into()),
        };
        let m = crate::find::Match::parse(&t.identifier()).unwrap();
        assert_eq!(m.text(crate::find::QUERY_LEVEL), "SERIES");
        assert_eq!(m.text(SERIES_UID), "1.2.3.4");
        let t = Target {
            study: "1.2.3".into(),
            series: None,
        };
        let m = crate::find::Match::parse(&t.identifier()).unwrap();
        assert_eq!(m.text(crate::find::QUERY_LEVEL), "STUDY");
        assert!(!m.has(SERIES_UID));
    }
}
