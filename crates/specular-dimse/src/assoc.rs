//! An association as the requestor (PS3.8 section 7 and 9; PS3.7 section 9 and 10).
//!
//! [`Association::request`] connects (bounded by [`Options::connect_timeout`]), sends the
//! A-ASSOCIATE-RQ and waits for the answer; every later wait for the server is bounded by
//! [`Options::timeout`] (the ARTIM and DIMSE timers) and polls the cancel token. Messages
//! are fragmented into P-DATA-TF PDUs no longer than the server's Maximum Length Received.
//! Dropping an association that was not released aborts it.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use specular_core::CancelToken;

use crate::command::{Command, field};
use crate::pdu::{self, Associate, ContextRq, Pdu};
use crate::tls::{Stream, Tls};
use crate::{Error, Result};

/// How often a blocked read wakes to look at the cancel token.
const POLL: Duration = Duration::from_millis(100);
/// Largest fragment sent even to a server without a limit.
const MAX_SEND: u32 = 1 << 20;

/// Which marker ended a read ([`Error::from_read`]).
#[derive(Debug)]
pub(crate) enum Stop {
    Cancelled,
    Timeout,
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Stop::Cancelled => "cancelled",
            Stop::Timeout => "timed out",
        })
    }
}

impl std::error::Error for Stop {}

/// The server to talk to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub host: String,
    pub port: u16,
    /// The server's AE title.
    pub called_ae: String,
    /// Ours.
    pub calling_ae: String,
}

impl Peer {
    /// `host:port` as the download indicator shows it (IPv6 literals in brackets).
    pub fn address(&self) -> String {
        let h = self
            .host
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']');
        if h.contains(':') {
            format!("[{h}]:{}", self.port)
        } else {
            format!("{h}:{}", self.port)
        }
    }
}

/// Timers, limits and transport.
#[derive(Clone, Debug)]
pub struct Options {
    pub connect_timeout: Duration,
    /// Longest wait for any answer (association, response, release).
    pub timeout: Duration,
    /// Our Maximum Length Received.
    pub max_pdu: u32,
    /// DICOM over TLS, verifying the server this way; `None`: plain TCP.
    pub tls: Option<Tls>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            connect_timeout: Duration::from_secs(10),
            timeout: Duration::from_secs(30),
            max_pdu: 65536,
            tls: None,
        }
    }
}

/// A presentation context the server accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accepted {
    pub id: u8,
    pub abstract_syntax: String,
    pub transfer_syntax: String,
}

/// A message received: its context, command and data set (if one followed).
#[derive(Clone, Debug)]
pub struct Message {
    pub context: u8,
    pub command: Command,
    pub data: Option<Vec<u8>>,
}

/// The connection with deadlines and cancellation; shared by the SCU and the test SCP.
pub(crate) struct Link {
    pub stream: Stream,
    pub timeout: Duration,
    pub cancel: CancelToken,
    /// Our Maximum Length Received.
    pub max_pdu: u32,
    /// A stop asked for between PDUs only (the stream stays in step): a read that has not
    /// started a PDU ends with [`Error::Cancelled`] once this says so.
    pub soft: Option<SoftStop>,
    /// Inside a message: a soft stop waits for its end.
    in_message: bool,
}

/// See [`Link::soft`].
pub type SoftStop = Box<dyn Fn() -> bool + Send>;

/// Part of a message as it arrives ([`Link::read_parts`]).
pub(crate) enum Part<'a> {
    /// The command is complete and a data set follows.
    Start(u8, &'a Command),
    /// A fragment of the data set.
    Data(&'a [u8]),
}

/// What [`Link::read_parts`] read.
pub(crate) enum Got {
    Message {
        context: u8,
        command: Command,
    },
    /// The peer asked to release (between messages).
    Release,
}

struct Timed<'a> {
    link: &'a mut Link,
    deadline: Instant,
}

impl Read for Timed<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.link.cancel.is_cancelled() {
                return Err(io::Error::other(Stop::Cancelled));
            }
            if Instant::now() >= self.deadline {
                return Err(io::Error::other(Stop::Timeout));
            }
            match self.link.stream.read(buf) {
                Ok(n) => return Ok(n),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(e),
            }
        }
    }
}

/// Short reads (to look at the cancel token), bounded writes, no Nagle delay.
pub(crate) fn tune(s: &TcpStream, timeout: Duration) {
    let _ = s.set_read_timeout(Some(POLL));
    let _ = s.set_write_timeout(Some(timeout));
    let _ = s.set_nodelay(true);
}

impl Link {
    pub fn new(stream: Stream, timeout: Duration, cancel: CancelToken, max_pdu: u32) -> Link {
        tune(stream.tcp(), timeout);
        Link {
            stream,
            timeout,
            cancel,
            max_pdu,
            soft: None,
            in_message: false,
        }
    }

    pub fn read_pdu(&mut self) -> Result<Pdu> {
        let deadline = Instant::now() + self.timeout;
        let max = if self.max_pdu == 0 {
            0
        } else {
            self.max_pdu.saturating_add(6)
        };
        if self.soft.is_none() {
            let mut t = Timed {
                link: self,
                deadline,
            };
            return pdu::read(&mut t, max);
        }
        // The header by hand, so that a soft stop is honoured only before its first byte.
        let mut h = [0u8; 6];
        let mut got = 0;
        while got < h.len() {
            let soft = got == 0 && !self.in_message && self.soft.as_ref().is_some_and(|f| f());
            if self.cancel.is_cancelled() || soft {
                return Err(Error::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            match self.stream.read(&mut h[got..]) {
                Ok(0) => {
                    return Err(Error::Io(io::Error::from(io::ErrorKind::UnexpectedEof)));
                }
                Ok(n) => got += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(Error::Io(e)),
            }
        }
        let mut t = Timed {
            link: self,
            deadline,
        };
        pdu::read_body(h, &mut t, max)
    }

    /// Reads one message, handing its data set over in fragments as they come (nothing of
    /// it is kept here). A release request between messages is [`Got::Release`] (not
    /// answered).
    pub(crate) fn read_parts(&mut self, on: &mut dyn FnMut(Part<'_>) -> Result<()>) -> Result<Got> {
        let r = self.read_parts_inner(on);
        self.in_message = false;
        r
    }

    fn read_parts_inner(&mut self, on: &mut dyn FnMut(Part<'_>) -> Result<()>) -> Result<Got> {
        let mut cmd = Vec::new();
        let mut command: Option<Command> = None;
        let mut context = None;
        loop {
            let pdu = self.read_pdu()?;
            self.in_message = true;
            match pdu {
                Pdu::ReleaseRq if context.is_none() => return Ok(Got::Release),
                Pdu::PData(pdvs) => {
                    for p in pdvs {
                        if *context.get_or_insert(p.context) != p.context {
                            return Err(Error::Protocol(
                                "a message's fragments use different contexts".into(),
                            ));
                        }
                        if p.command {
                            if command.is_some() {
                                return Err(Error::Protocol("command after its end".into()));
                            }
                            cmd.extend_from_slice(&p.data);
                            if p.last {
                                let c = Command::decode(&cmd)?;
                                if !c.data_set {
                                    return Ok(Got::Message {
                                        context: p.context,
                                        command: c,
                                    });
                                }
                                on(Part::Start(p.context, &c))?;
                                command = Some(c);
                            }
                        } else {
                            if command.is_none() {
                                return Err(Error::Protocol("data set before its command".into()));
                            }
                            on(Part::Data(&p.data))?;
                            if p.last {
                                return Ok(Got::Message {
                                    context: p.context,
                                    command: command.take().unwrap_or_default(),
                                });
                            }
                        }
                    }
                }
                Pdu::Abort { source, reason } => return Err(Error::Aborted { source, reason }),
                other => {
                    return Err(Error::Protocol(format!(
                        "unexpected {} during a message",
                        pdu_name(&other)
                    )));
                }
            }
        }
    }

    pub fn write(&mut self, b: &[u8]) -> Result<()> {
        // In pieces, so that a cancel is seen within one piece on a slow link.
        for c in b.chunks(256 * 1024) {
            if self.cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            self.stream.write_all(c).map_err(|e| match e.kind() {
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Error::Timeout,
                _ => Error::Io(e),
            })?;
        }
        Ok(())
    }

    pub fn write_pdu(&mut self, p: &Pdu) -> Result<()> {
        self.write(&pdu::encode(p))
    }

    /// Sends a command and its data set as P-DATA-TF fragments of at most `peer_max` bytes
    /// per PDU (0: no limit). `sent` is told the data bytes as they go.
    pub fn send_message(
        &mut self,
        context: u8,
        command: &[u8],
        data: Option<&[u8]>,
        peer_max: u32,
        sent: &mut dyn FnMut(u64),
    ) -> Result<()> {
        let limit = if peer_max == 0 {
            MAX_SEND
        } else {
            peer_max.min(MAX_SEND)
        };
        // A PDV item is 6 bytes of header and the fragment; at least one byte of data.
        let frag = (limit as usize).saturating_sub(6).max(1);
        let mut put = |link: &mut Link, bytes: &[u8], cmd: bool| -> Result<()> {
            let n = bytes.len().div_ceil(frag).max(1);
            for (i, c) in bytes
                .chunks(frag)
                .chain(bytes.is_empty().then_some(&[][..]))
                .enumerate()
            {
                let h = pdu::pdata_header(context, cmd, i + 1 == n, c.len());
                link.write(&h)?;
                link.write(c)?;
                if !cmd {
                    sent(c.len() as u64);
                }
            }
            Ok(())
        };
        put(self, command, true)?;
        if let Some(d) = data {
            put(self, d, false)?;
        }
        Ok(())
    }

    /// Reads one whole message (command, then its data set if it has one). `max_data`
    /// bounds the data set's size.
    pub fn read_message(&mut self, max_data: usize) -> Result<Message> {
        let mut cmd = Vec::new();
        let mut data = Vec::new();
        let mut command: Option<Command> = None;
        let mut context = None;
        loop {
            match self.read_pdu()? {
                Pdu::PData(pdvs) => {
                    for p in pdvs {
                        if *context.get_or_insert(p.context) != p.context {
                            return Err(Error::Protocol(
                                "a message's fragments use different contexts".into(),
                            ));
                        }
                        if p.command {
                            if command.is_some() {
                                return Err(Error::Protocol("command after its end".into()));
                            }
                            cmd.extend_from_slice(&p.data);
                            if p.last {
                                let c = Command::decode(&cmd)?;
                                if !c.data_set {
                                    return Ok(Message {
                                        context: p.context,
                                        command: c,
                                        data: None,
                                    });
                                }
                                command = Some(c);
                            }
                        } else {
                            if command.is_none() {
                                return Err(Error::Protocol("data set before its command".into()));
                            }
                            if data.len() + p.data.len() > max_data {
                                return Err(Error::Protocol("data set too large".into()));
                            }
                            data.extend_from_slice(&p.data);
                            if p.last {
                                return Ok(Message {
                                    context: p.context,
                                    command: command.take().unwrap_or_default(),
                                    data: Some(data),
                                });
                            }
                        }
                    }
                }
                Pdu::Abort { source, reason } => return Err(Error::Aborted { source, reason }),
                other => {
                    return Err(Error::Protocol(format!(
                        "unexpected {} during a message",
                        pdu_name(&other)
                    )));
                }
            }
        }
    }
}

pub(crate) fn pdu_name(p: &Pdu) -> &'static str {
    match p {
        Pdu::AssociateRq(_) => "A-ASSOCIATE-RQ",
        Pdu::AssociateAc(_) => "A-ASSOCIATE-AC",
        Pdu::AssociateRj { .. } => "A-ASSOCIATE-RJ",
        Pdu::PData(_) => "P-DATA-TF",
        Pdu::ReleaseRq => "A-RELEASE-RQ",
        Pdu::ReleaseRp => "A-RELEASE-RP",
        Pdu::Abort { .. } => "A-ABORT",
    }
}

/// An open association.
pub struct Association {
    link: Link,
    accepted: Vec<Accepted>,
    peer_max: u32,
    next_id: u16,
    /// Released or aborted: nothing more is sent.
    closed: bool,
    /// What the server said about itself.
    pub peer_implementation: String,
    /// The roles the server answered (SCP/SCU Role Selection).
    pub roles: Vec<pdu::Role>,
}

impl std::fmt::Debug for Association {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Association")
            .field("accepted", &self.accepted)
            .field("peer_max", &self.peer_max)
            .finish_non_exhaustive()
    }
}

fn connect(peer: &Peer, opts: &Options, cancel: &CancelToken) -> Result<TcpStream> {
    // The no-network switch's choke point: before any name lookup or socket.
    if specular_core::net::off::is_off() {
        specular_core::net::off::note_refused();
        return Err(Error::NetworkOff);
    }
    let host = peer
        .host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    let addrs: Vec<_> = (host, peer.port)
        .to_socket_addrs()
        .map_err(|_| Error::UnknownHost)?
        .collect();
    if addrs.is_empty() {
        return Err(Error::UnknownHost);
    }
    let mut last = None;
    for a in addrs {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        match TcpStream::connect_timeout(&a, opts.connect_timeout) {
            Ok(s) => return Ok(s),
            Err(e) => last = Some(e),
        }
    }
    let e = last.unwrap_or_else(|| io::Error::from(io::ErrorKind::NotFound));
    Err(if e.kind() == io::ErrorKind::TimedOut {
        Error::Timeout
    } else {
        Error::Connect(e)
    })
}

impl Association {
    /// Connects to `peer` and negotiates `contexts`; fails unless at least one is accepted.
    pub fn request(
        peer: &Peer,
        opts: &Options,
        contexts: &[ContextRq],
        cancel: &CancelToken,
    ) -> Result<Association> {
        Self::request_with_roles(peer, opts, contexts, &[], cancel)
    }

    /// [`request`](Self::request), proposing these roles too.
    pub fn request_with_roles(
        peer: &Peer,
        opts: &Options,
        contexts: &[ContextRq],
        roles: &[pdu::Role],
        cancel: &CancelToken,
    ) -> Result<Association> {
        let tcp = connect(peer, opts, cancel)?;
        tune(&tcp, opts.timeout);
        let stream = match &opts.tls {
            Some(t) => {
                let host = peer
                    .host
                    .trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']');
                let deadline = Instant::now() + opts.timeout;
                crate::tls::connect(t, host, tcp, deadline, cancel)?
            }
            None => Stream::Plain(tcp),
        };
        let mut link = Link::new(stream, opts.timeout, cancel.clone(), opts.max_pdu);
        let rq = Associate {
            called_ae: peer.called_ae.trim().to_owned(),
            calling_ae: peer.calling_ae.trim().to_owned(),
            requested: contexts.to_vec(),
            max_pdu: opts.max_pdu,
            roles: roles.to_vec(),
            ..Associate::default()
        };
        link.write_pdu(&Pdu::AssociateRq(rq))?;
        let ac = match link.read_pdu()? {
            Pdu::AssociateAc(a) => a,
            Pdu::AssociateRj {
                result,
                source,
                reason,
            } => {
                return Err(Error::Rejected {
                    result,
                    source,
                    reason,
                });
            }
            Pdu::Abort { source, reason } => return Err(Error::Aborted { source, reason }),
            other => {
                let _ = link.write_pdu(&Pdu::Abort {
                    source: 0,
                    reason: 0,
                });
                return Err(Error::Protocol(format!(
                    "{} in answer to A-ASSOCIATE-RQ",
                    pdu_name(&other)
                )));
            }
        };
        let accepted: Vec<Accepted> = ac
            .answered
            .iter()
            .filter(|c| c.result == pdu::result::ACCEPTANCE)
            .filter_map(|c| {
                let rq = contexts.iter().find(|r| r.id == c.id)?;
                // The server must pick one of the syntaxes offered.
                rq.transfer_syntaxes
                    .contains(&c.transfer_syntax)
                    .then(|| Accepted {
                        id: c.id,
                        abstract_syntax: rq.abstract_syntax.clone(),
                        transfer_syntax: c.transfer_syntax.clone(),
                    })
            })
            .collect();
        let mut a = Association {
            link,
            accepted,
            peer_max: ac.max_pdu,
            next_id: 1,
            closed: false,
            peer_implementation: if ac.implementation_version.is_empty() {
                ac.implementation_class
            } else {
                ac.implementation_version
            },
            roles: ac.roles,
        };
        if a.accepted.is_empty() {
            a.abort();
            return Err(Error::NothingAccepted);
        }
        specular_log::debug!(target: specular_log::IO,
            "dimse: association with {} ({}): {} of {} contexts accepted, max PDU {}",
            peer.address(), a.peer_implementation, a.accepted.len(), contexts.len(), a.peer_max);
        Ok(a)
    }

    pub fn accepted(&self) -> &[Accepted] {
        &self.accepted
    }

    /// The server's Maximum Length Received (0: no limit).
    pub fn peer_max_pdu(&self) -> u32 {
        self.peer_max
    }

    /// An accepted context for `abstract_syntax`, with transfer syntax `ts` when given.
    pub fn find(&self, abstract_syntax: &str, ts: Option<&str>) -> Option<&Accepted> {
        self.accepted.iter().find(|c| {
            c.abstract_syntax == abstract_syntax && ts.is_none_or(|t| c.transfer_syntax == t)
        })
    }

    /// The id of the accepted context for `abstract_syntax`.
    pub(crate) fn find_context(&self, abstract_syntax: &str) -> Option<u8> {
        self.find(abstract_syntax, None).map(|c| c.id)
    }

    pub(crate) fn link(&mut self) -> &mut Link {
        &mut self.link
    }

    pub(crate) fn next_message_id(&mut self) -> u16 {
        self.message_id()
    }

    /// Sends one message (no answer awaited).
    pub(crate) fn send(&mut self, context: u8, rq: &Command, data: Option<&[u8]>) -> Result<()> {
        if self.closed {
            return Err(Error::Protocol("the association is closed".into()));
        }
        let peer_max = self.peer_max;
        self.link
            .send_message(context, &rq.encode(), data, peer_max, &mut |_| {})
    }

    /// Reads one message; its data set at most `max_data` bytes.
    pub(crate) fn receive(&mut self, max_data: usize) -> Result<Message> {
        self.link.read_message(max_data)
    }

    fn message_id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    /// Sends a request and waits for its response.
    fn exchange(
        &mut self,
        context: u8,
        rq: &Command,
        data: Option<&[u8]>,
        sent: &mut dyn FnMut(u64),
    ) -> Result<Command> {
        let r = self.exchange_inner(context, rq, data, sent);
        if let Err(e) = &r
            && e.is_fatal()
        {
            // Cancelled, timed out or broken: the association cannot be trusted.
            self.abort();
        }
        r
    }

    fn exchange_inner(
        &mut self,
        context: u8,
        rq: &Command,
        data: Option<&[u8]>,
        sent: &mut dyn FnMut(u64),
    ) -> Result<Command> {
        if self.closed {
            return Err(Error::Protocol("the association is closed".into()));
        }
        let peer_max = self.peer_max;
        self.link
            .send_message(context, &rq.encode(), data, peer_max, sent)?;
        let m = self.link.read_message(1 << 20)?;
        let c = m.command;
        if c.field != rq.field | 0x8000 || c.responded_to != rq.message_id {
            return Err(Error::Protocol(format!(
                "response {:#06x} to message {:?}, expected {:#06x} to {:?}",
                c.field,
                c.responded_to,
                rq.field | 0x8000,
                rq.message_id
            )));
        }
        Ok(c)
    }

    /// C-ECHO; the Verification context must have been offered. `Ok` on status 0.
    pub fn echo(&mut self) -> Result<()> {
        let ctx = self
            .find(crate::sop::VERIFICATION, None)
            .ok_or_else(|| Error::NoContext(crate::sop::VERIFICATION.into()))?
            .id;
        let id = self.message_id();
        let rsp = self.exchange(ctx, &Command::echo_rq(id), None, &mut |_| {})?;
        match rsp.status {
            Some(0) => Ok(()),
            Some(s) => Err(Error::Status(s)),
            None => Err(Error::Protocol("C-ECHO-RSP without a status".into())),
        }
    }

    /// C-STORE of `data` (a data set in the context's transfer syntax). The response's
    /// status and error comment.
    pub fn store(
        &mut self,
        context: &Accepted,
        sop_class: &str,
        sop_instance: &str,
        data: &[u8],
        sent: &mut dyn FnMut(u64),
    ) -> Result<(u16, String)> {
        let id = self.message_id();
        let rq = Command::store_rq(id, sop_class, sop_instance);
        let rsp = self.exchange(context.id, &rq, Some(data), sent)?;
        debug_assert_eq!(rsp.field, field::C_STORE_RSP);
        let status = rsp
            .status
            .ok_or_else(|| Error::Protocol("C-STORE-RSP without a status".into()))?;
        Ok((status, rsp.error_comment))
    }

    /// A-RELEASE: asks to end and waits for the answer.
    pub fn release(mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.link.write_pdu(&Pdu::ReleaseRq)?;
        let r = match self.link.read_pdu() {
            Ok(Pdu::ReleaseRp) => Ok(()),
            Ok(Pdu::Abort { source, reason }) => Err(Error::Aborted { source, reason }),
            Ok(other) => Err(Error::Protocol(format!(
                "{} in answer to A-RELEASE-RQ",
                pdu_name(&other)
            ))),
            Err(e) => Err(e),
        };
        if r.is_err() {
            self.abort();
        }
        self.closed = true;
        r
    }

    /// A-ABORT (no answer awaited).
    pub fn abort(&mut self) {
        if !std::mem::replace(&mut self.closed, true) {
            let _ = self
                .link
                .stream
                .tcp()
                .set_write_timeout(Some(Duration::from_secs(1)));
            let _ = self.link.stream.write_all(&pdu::encode(&Pdu::Abort {
                source: 0,
                reason: 0,
            }));
            let _ = self.link.stream.flush();
            self.link.stream.shutdown();
        }
    }
}

impl Drop for Association {
    fn drop(&mut self) {
        self.abort();
    }
}

/// Verify: an association offering only Verification, one C-ECHO, then release.
pub fn verify(peer: &Peer, opts: &Options, cancel: &CancelToken) -> Result<()> {
    let ctx = [ContextRq {
        id: 1,
        abstract_syntax: crate::sop::VERIFICATION.into(),
        transfer_syntaxes: vec![crate::sop::IMPLICIT_LE.into()],
    }];
    let mut a = Association::request(peer, opts, &ctx, cancel)?;
    a.echo()?;
    a.release()
}
