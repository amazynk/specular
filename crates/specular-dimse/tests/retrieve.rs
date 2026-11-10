//! C-GET and C-MOVE against the in-crate test SCP on loopback (ephemeral ports).

mod common;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use common::*;
use parking_lot::Mutex;
use specular_core::CancelToken;
use specular_dimse::listen::{Listen, move_into};
use specular_dimse::retrieve::{self, Incoming, Outcome, Progress, Storage, Target};
use specular_dimse::scp::{ScpConfig, TestScp};
use specular_dimse::{Error, Options, Peer, Rejection, assoc};

const STUDY: &str = "1.2.826.0.1.3680043.10.9.1";
const SERIES: &str = "1.2.826.0.1.3680043.10.9.2";

/// What arrived: (class, instance, syntax, data set).
type Got = Arc<Mutex<Vec<(String, String, String, Vec<u8>)>>>;

#[derive(Default)]
struct Mem {
    got: Got,
}

struct MemIn {
    head: (String, String, String),
    data: Vec<u8>,
    got: Got,
}

impl Storage for Mem {
    fn begin(&mut self, c: &str, i: &str, ts: &str) -> io::Result<Box<dyn Incoming>> {
        Ok(Box::new(MemIn {
            head: (c.into(), i.into(), ts.into()),
            data: Vec::new(),
            got: self.got.clone(),
        }))
    }
}

impl Incoming for MemIn {
    fn write(&mut self, d: &[u8]) -> io::Result<()> {
        self.data.extend_from_slice(d);
        Ok(())
    }
    fn finish(self: Box<Self>) -> io::Result<()> {
        let (c, i, t) = self.head;
        self.got.lock().push((c, i, t, self.data));
        Ok(())
    }
}

fn opts() -> Options {
    Options {
        connect_timeout: Duration::from_secs(5),
        timeout: Duration::from_secs(10),
        max_pdu: 16384,
        tls: None,
    }
}

fn peer(port: u16) -> Peer {
    Peer {
        host: "127.0.0.1".into(),
        port,
        called_ae: "TESTSCP".into(),
        calling_ae: "SPECULAR".into(),
    }
}

/// Four CT slices of the series (explicit, implicit, RLE), and one MR of another series.
fn files() -> Vec<Vec<u8>> {
    let mut v = vec![
        explicit(CT, "1.2.3.1", 32, 0),
        implicit(CT, "1.2.3.2", 32, 1),
        rle(CT, "1.2.3.3", 32, 2),
        explicit(CT, "1.2.3.4", 32, 3),
    ];
    let mut o = object(MR, "1.2.3.9", 16, 9);
    o.put_text(
        0x0020_000E,
        specular_dicom::dict::Vr::UI,
        "1.2.826.0.1.3680043.10.9.3",
    );
    let px: Vec<u8> = pixels(16, 9).iter().flat_map(|p| p.to_le_bytes()).collect();
    o.put_bytes(0x7FE0_0010, specular_dicom::dict::Vr::OW, px);
    v.push(
        specular_dicom::write::part10(&o, &specular_dicom::write::FileMeta::of(&o, EXPLICIT))
            .unwrap(),
    );
    v
}

fn series() -> Target {
    Target {
        study: STUDY.into(),
        series: Some(SERIES.into()),
    }
}

fn get(
    scp: &TestScp,
    t: &Target,
    stop: Box<dyn Fn() -> bool + Send>,
) -> (Outcome, Got, Vec<Progress>) {
    let mut m = Mem::default();
    let got = m.got.clone();
    let mut seen = Vec::new();
    let o = retrieve::get(
        &peer(scp.port),
        &opts(),
        t,
        &mut m,
        &mut |p| seen.push(*p),
        stop,
        &CancelToken::new(),
    )
    .unwrap();
    (o, got, seen)
}

/// Every pixel of what arrived equals the file served.
fn check_pixels(got: &Got, served: &[Vec<u8>]) -> usize {
    let mut n = 0;
    for (c, i, ts, data) in got.lock().iter() {
        let f = served
            .iter()
            .find(|f| specular_dimse::store::head(f).unwrap().sop_instance == *i)
            .unwrap();
        assert_eq!(decoded(&wrap(data, c, i, ts)), decoded(f), "{i}");
        n += 1;
    }
    n
}

#[test]
fn c_get_series_arrives_whole() {
    let served = files();
    let scp = TestScp::start(ScpConfig {
        instances: served.clone(),
        transfer_syntaxes: vec![EXPLICIT.into(), IMPLICIT.into(), RLE.into()],
        ..ScpConfig::default()
    })
    .unwrap();
    let (o, got, seen) = get(&scp, &series(), Box::new(|| false));
    assert_eq!(o.status, 0, "{o:?}");
    assert_eq!(o.progress.received, 4);
    assert_eq!(o.progress.counts.completed, 4);
    assert!(!o.no_matches());
    assert_eq!(check_pixels(&got, &served), 4);
    // The RLE slice came as stored.
    assert!(got.lock().iter().any(|g| g.1 == "1.2.3.3" && g.2 == RLE));
    // Pending responses counted down.
    assert!(seen.iter().any(|p| p.counts.remaining == Some(3)));
    assert_eq!(seen.last().unwrap().done_total(), (4, 4));

    // The whole study: the MR too.
    let study = Target {
        study: STUDY.into(),
        series: None,
    };
    let (o, got, _) = get(&scp, &study, Box::new(|| false));
    assert_eq!(o.progress.received, 5);
    assert_eq!(check_pixels(&got, &served), 5);
}

#[test]
fn compressed_without_its_context_is_converted() {
    let served = files();
    // The server takes only little-endian: the RLE slice is decoded for us.
    let scp = TestScp::start(ScpConfig {
        instances: served.clone(),
        ..ScpConfig::default()
    })
    .unwrap();
    let (o, got, _) = get(&scp, &series(), Box::new(|| false));
    assert_eq!(o.progress.received, 4, "{o:?}");
    assert!(got.lock().iter().all(|g| g.2 != RLE));
    assert_eq!(check_pixels(&got, &served), 4);
}

#[test]
fn c_get_failures_in_words() {
    let scp = TestScp::start(ScpConfig {
        instances: files(),
        refuse_roles: true,
        ..ScpConfig::default()
    })
    .unwrap();
    let (o, got, _) = get(&scp, &series(), Box::new(|| false));
    assert!(o.roles_refused);
    assert_eq!(o.status, 0xB000);
    assert_eq!(o.progress.counts.failed, 4);
    assert!(got.lock().is_empty());

    // Nothing matches.
    let scp = TestScp::start(ScpConfig {
        instances: files(),
        ..ScpConfig::default()
    })
    .unwrap();
    let none = Target {
        study: "9.9.9".into(),
        series: None,
    };
    let (o, _, _) = get(&scp, &none, Box::new(|| false));
    assert!(o.no_matches(), "{o:?}");

    // A server without C-GET.
    let scp = TestScp::start(ScpConfig {
        instances: files(),
        sop_classes: vec![CT.into()],
        ..ScpConfig::default()
    })
    .unwrap();
    let e = retrieve::get(
        &peer(scp.port),
        &opts(),
        &series(),
        &mut Mem::default(),
        &mut |_| {},
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::NoContext(_)), "{e}");

    // Rejected association: the reason kept.
    let mut p = peer(scp.port);
    p.called_ae = "WRONG".into();
    let e = retrieve::get(
        &p,
        &opts(),
        &series(),
        &mut Mem::default(),
        &mut |_| {},
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(e.rejection(), Some(Rejection::CalledAe));
}

#[test]
fn c_get_stops_with_c_cancel() {
    let scp = TestScp::start(ScpConfig {
        instances: files(),
        sub_delay: Duration::from_millis(300),
        ..ScpConfig::default()
    })
    .unwrap();
    let mut m = Mem::default();
    let got = m.got.clone();
    let g2 = got.clone();
    let t0 = std::time::Instant::now();
    let o = retrieve::get(
        &peer(scp.port),
        &opts(),
        &series(),
        &mut m,
        &mut |_| {},
        Box::new(move || !g2.lock().is_empty()),
        &CancelToken::new(),
    )
    .unwrap();
    assert!(o.cancelled, "{o:?}");
    assert_eq!(o.status, 0xFE00);
    let n = got.lock().len();
    assert!((1..4).contains(&n), "{n}");
    assert!(t0.elapsed() < Duration::from_secs(3));
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn c_move_to_a_temporary_scp() {
    let served = files();
    let port = free_port();
    let scp = TestScp::start(ScpConfig {
        instances: served.clone(),
        destinations: vec![("SPECULAR".into(), port)],
        sub_delay: Duration::from_millis(100),
        ..ScpConfig::default()
    })
    .unwrap();
    let listen = Listen {
        address: "127.0.0.1".parse().unwrap(),
        port,
        ae_title: "SPECULAR".into(),
    };
    let mut m = Mem::default();
    let got = m.got.clone();
    let probed = Arc::new(Mutex::new(None));
    let pr = probed.clone();
    let mut first = true;
    let o = move_into(
        &peer(scp.port),
        &opts(),
        &series(),
        &listen,
        &mut m,
        &mut |_| {
            // While the move runs: an association for another AE title is refused.
            if std::mem::take(&mut first) {
                let mut p = peer(port);
                p.called_ae = "OTHER".into();
                *pr.lock() = Some(
                    assoc::verify(&p, &opts(), &CancelToken::new())
                        .unwrap_err()
                        .rejection(),
                );
            }
        },
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(o.status, 0, "{o:?}");
    assert_eq!(o.progress.received, 4);
    assert_eq!(check_pixels(&got, &served), 4);
    assert_eq!(*probed.lock(), Some(Some(Rejection::CalledAe)));
    // Closed afterwards.
    let e = assoc::verify(&peer(port), &opts(), &CancelToken::new()).unwrap_err();
    assert!(matches!(e, Error::Connect(_)), "{e}");

    // A destination the server does not know.
    let listen = Listen {
        ae_title: "STRANGER".into(),
        ..listen
    };
    let o = move_into(
        &peer(scp.port),
        &opts(),
        &series(),
        &listen,
        &mut Mem::default(),
        &mut |_| {},
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(o.status, 0xA801);
}
