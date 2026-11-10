//! C-MOVE with a temporary storage SCP: while one move runs, a listener on the address and
//! port the user chose takes the server's C-STORE sub-operations, then closes.
//!
//! The listener exists only for the duration of [`move_into`]: bound after the no-network
//! switch is checked, it accepts associations one at a time and only for our AE title
//! (others are rejected with "called AE title not recognised"), offers only storage classes
//! (and Verification) in syntaxes the in-tree decoders read, and is closed once the server
//! gave its final answer and its last association ended. The server must know our AE title
//! with this computer's address and port.

use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use specular_core::CancelToken;

use crate::assoc::{Got, Link, Options, Peer, SoftStop, pdu_name};
use crate::command::{Command, field};
use crate::pdu::{self, Associate, ContextAc, Pdu, result};
use crate::retrieve::{Outcome, Progress, Receiver, Shared, Storage, Target};
use crate::sop;
use crate::{Error, Result};

/// Where the storage SCP listens, and the AE title it answers to (ours, the move's
/// destination).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listen {
    pub address: IpAddr,
    pub port: u16,
    pub ae_title: String,
}

/// How long the listener waits for the server's last association to end after the move.
const LINGER: Duration = Duration::from_secs(10);
/// Our Maximum Length Received as the SCP.
const MAX_PDU: u32 = 1 << 16;
/// Associations served at once.
const PARALLEL: usize = 4;

/// The answer to each presentation context offered: storage classes and Verification in
/// the first syntax offered that the decoders read.
fn answer(rq: &Associate) -> Vec<ContextAc> {
    rq.requested
        .iter()
        .map(|c| {
            let wanted =
                c.abstract_syntax == sop::VERIFICATION || sop::is_storage(&c.abstract_syntax);
            let ts = c
                .transfer_syntaxes
                .iter()
                .find(|t| sop::DECODABLE.contains(&t.as_str()));
            match (wanted, ts) {
                (false, _) => ContextAc {
                    id: c.id,
                    result: result::ABSTRACT_SYNTAX_NOT_SUPPORTED,
                    transfer_syntax: sop::IMPLICIT_LE.into(),
                },
                (true, None) => ContextAc {
                    id: c.id,
                    result: result::TRANSFER_SYNTAXES_NOT_SUPPORTED,
                    transfer_syntax: sop::IMPLICIT_LE.into(),
                },
                (true, Some(t)) => ContextAc {
                    id: c.id,
                    result: result::ACCEPTANCE,
                    transfer_syntax: t.clone(),
                },
            }
        })
        .collect()
}

/// One association from the server: C-ECHO and C-STORE until it releases.
fn serve(
    stream: TcpStream,
    ae: &str,
    shared: &Mutex<Shared<'_>>,
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<()> {
    let _ = stream.set_nonblocking(false);
    let mut link = Link::new(
        crate::tls::Stream::Plain(stream),
        timeout,
        cancel.clone(),
        MAX_PDU,
    );
    let rq = match link.read_pdu()? {
        Pdu::AssociateRq(rq) => rq,
        other => {
            return Err(Error::Protocol(format!(
                "{} instead of A-ASSOCIATE-RQ",
                pdu_name(&other)
            )));
        }
    };
    if !rq.called_ae.trim().eq_ignore_ascii_case(ae.trim()) {
        specular_log::warn!(target: specular_log::IO,
            "dimse move: association for another AE title refused");
        return link.write_pdu(&Pdu::AssociateRj {
            result: 1,
            source: 1,
            reason: 7,
        });
    }
    let answered = answer(&rq);
    let ac = Associate {
        called_ae: rq.called_ae.clone(),
        calling_ae: rq.calling_ae.clone(),
        answered: answered.clone(),
        max_pdu: MAX_PDU,
        ..Associate::default()
    };
    link.write_pdu(&Pdu::AssociateAc(ac))?;
    let accepted: Vec<(u8, String)> = answered
        .iter()
        .filter(|c| c.result == result::ACCEPTANCE)
        .map(|c| (c.id, c.transfer_syntax.clone()))
        .collect();
    let mut rx = Receiver::new(shared, accepted.clone());
    loop {
        match link.read_parts(&mut |p| rx.on(p))? {
            Got::Release => return link.write_pdu(&Pdu::ReleaseRp),
            Got::Message { context, command } => {
                if !accepted.iter().any(|(id, _)| *id == context) {
                    return link.write_pdu(&Pdu::Abort {
                        source: 2,
                        reason: 0,
                    });
                }
                match command.field {
                    field::C_STORE_RQ => rx.answer(&mut link, context, &command, rq.max_pdu)?,
                    field::C_ECHO_RQ => {
                        let rsp = Command::response(&command, 0);
                        link.send_message(context, &rsp.encode(), None, rq.max_pdu, &mut |_| {})?;
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
    }
}

/// Accepts associations until `done` (at most [`PARALLEL`] at once), then waits for those
/// still open.
fn accept_loop(
    listener: &TcpListener,
    ae: &str,
    shared: &Mutex<Shared<'_>>,
    done: &AtomicBool,
    timeout: Duration,
    cancel: &CancelToken,
) {
    let active = AtomicUsize::new(0);
    std::thread::scope(|s| {
        while !done.load(Ordering::Acquire) && !cancel.is_cancelled() {
            if active.load(Ordering::Acquire) >= PARALLEL {
                std::thread::sleep(Duration::from_millis(15));
                continue;
            }
            match listener.accept() {
                Ok((stream, from)) => {
                    specular_log::debug!(target: specular_log::IO,
                        "dimse move: association from {from}");
                    active.fetch_add(1, Ordering::AcqRel);
                    let active = &active;
                    specular_thread::spawn_scoped(s, "dimse-mv-assoc", move || {
                        if let Err(e) = serve(stream, ae, shared, timeout, cancel) {
                            specular_log::warn!(target: specular_log::IO, "dimse move: {e}");
                        }
                        active.fetch_sub(1, Ordering::AcqRel);
                    });
                }
                Err(_) => std::thread::sleep(Duration::from_millis(15)),
            }
        }
    });
}

/// Binds the listener (after the no-network switch is checked).
fn bind(l: &Listen) -> Result<TcpListener> {
    if specular_core::net::off::is_off() {
        specular_core::net::off::note_refused();
        return Err(Error::NetworkOff);
    }
    let listener = TcpListener::bind(SocketAddr::new(l.address, l.port)).map_err(Error::Listen)?;
    listener.set_nonblocking(true).map_err(Error::Listen)?;
    Ok(listener)
}

/// C-MOVE of `target` from `peer` to us: listens on `listen` while the move runs, storing
/// what arrives into `storage`. As [`crate::retrieve::get`] otherwise.
#[allow(clippy::too_many_arguments)]
pub fn move_into(
    peer: &Peer,
    opts: &Options,
    target: &Target,
    listen: &Listen,
    storage: &mut dyn Storage,
    report: &mut (dyn FnMut(&Progress) + Send),
    stop: SoftStop,
    cancel: &CancelToken,
) -> Result<Outcome> {
    if !pdu::valid_ae(&listen.ae_title) {
        return Err(Error::Protocol("invalid AE title".into()));
    }
    let listener = bind(listen)?;
    let shared = Mutex::new(Shared {
        storage,
        progress: Progress::default(),
        report,
    });
    let done = AtomicBool::new(false);
    let serving = CancelToken::new();
    let r = std::thread::scope(|s| {
        let h = specular_thread::spawn_scoped(s, "dimse-move-scp", || {
            accept_loop(
                &listener,
                &listen.ae_title,
                &shared,
                &done,
                opts.timeout,
                &serving,
            )
        });
        let r =
            crate::retrieve::move_scu(peer, opts, target, &listen.ae_title, &shared, stop, cancel);
        done.store(true, Ordering::Release);
        let t0 = Instant::now();
        while !h.is_finished() && t0.elapsed() < LINGER {
            std::thread::sleep(Duration::from_millis(15));
        }
        serving.cancel();
        let _ = h.join();
        r
    });
    drop(listener);
    r.map(|mut o| {
        o.progress = shared.lock().progress;
        o
    })
}
