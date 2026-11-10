//! The SCU against the in-crate test SCP on loopback (ephemeral ports, nothing external).

mod common;

use std::time::{Duration, Instant};

use common::*;
use specular_core::CancelToken;
use specular_dimse::assoc::{self, Association};
use specular_dimse::scp::{ScpConfig, TestScp};
use specular_dimse::{Error, Options, Peer, Rejection, Status, store};

fn peer(scp: &TestScp) -> Peer {
    Peer {
        host: "127.0.0.1".into(),
        port: scp.port,
        called_ae: "TESTSCP".into(),
        calling_ae: "SPECULAR".into(),
    }
}

fn quick() -> Options {
    Options {
        connect_timeout: Duration::from_secs(5),
        timeout: Duration::from_secs(5),
        max_pdu: 16384,
        tls: None,
    }
}

fn open(scp: &TestScp, files: &[&[u8]]) -> Association {
    let heads: Vec<_> = files.iter().map(|f| store::head(f).unwrap()).collect();
    let ctx = store::contexts(
        heads
            .iter()
            .map(|h| (h.sop_class.as_str(), h.transfer_syntax.as_str())),
    );
    Association::request(&peer(scp), &quick(), &ctx, &CancelToken::new()).unwrap()
}

#[test]
fn echo_and_rejections() {
    let scp = TestScp::start(ScpConfig {
        known_callers: vec!["SPECULAR".into()],
        ..ScpConfig::default()
    })
    .unwrap();
    assoc::verify(&peer(&scp), &quick(), &CancelToken::new()).unwrap();
    assert_eq!(scp.echoes.load(std::sync::atomic::Ordering::Relaxed), 1);

    let mut p = peer(&scp);
    p.called_ae = "WRONG".into();
    let e = assoc::verify(&p, &quick(), &CancelToken::new()).unwrap_err();
    assert_eq!(e.rejection(), Some(Rejection::CalledAe), "{e}");

    let mut p = peer(&scp);
    p.calling_ae = "STRANGER".into();
    let e = assoc::verify(&p, &quick(), &CancelToken::new()).unwrap_err();
    assert_eq!(e.rejection(), Some(Rejection::CallingAe), "{e}");
}

#[test]
fn nobody_listening() {
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let p = Peer {
        host: "127.0.0.1".into(),
        port,
        called_ae: "X".into(),
        calling_ae: "Y".into(),
    };
    let e = assoc::verify(&p, &quick(), &CancelToken::new()).unwrap_err();
    assert!(matches!(e, Error::Connect(_)), "{e}");
    let mut p = p;
    p.host = "no-such-host.invalid".into();
    let e = assoc::verify(&p, &quick(), &CancelToken::new()).unwrap_err();
    assert!(matches!(e, Error::UnknownHost | Error::Connect(_)), "{e}");
}

#[test]
fn stored_syntax_goes_byte_for_byte_in_fragments() {
    let scp = TestScp::start(ScpConfig {
        max_pdu: 4096,
        ..ScpConfig::default()
    })
    .unwrap();
    let files: Vec<Vec<u8>> = (0..3)
        .map(|i| explicit(CT, &format!("1.2.826.0.1.3680043.10.9.3.{i}"), 64, i))
        .collect();
    let refs: Vec<&[u8]> = files.iter().map(Vec::as_slice).collect();
    let mut a = open(&scp, &refs);
    assert_eq!(a.peer_max_pdu(), 4096);
    let mut bytes = 0;
    for f in &files {
        let s = a.send_file(f, None, &mut |n| bytes += n).unwrap();
        assert_eq!(Status::of(s.status), Status::Success);
        assert!(!s.converted);
        assert_eq!(s.transfer_syntax, EXPLICIT);
    }
    a.release().unwrap();
    let got = scp.received.lock().clone();
    assert_eq!(got.len(), 3);
    let mut total = 0;
    for (r, f) in got.iter().zip(&files) {
        let h = store::head(f).unwrap();
        assert_eq!(r.sop_instance, h.sop_instance);
        assert_eq!(r.calling_ae, "SPECULAR");
        assert_eq!(r.data, &f[h.data_set..], "data set changed in transit");
        // 64×64×2 bytes and more, in PDUs of at most 4096 bytes.
        assert!(
            r.fragments >= (r.data.len() / 4090) as u32,
            "{}",
            r.fragments
        );
        total += r.data.len() as u64;
    }
    assert_eq!(bytes, total);
}

#[test]
fn implicit_and_explicit_convert_both_ways() {
    // A server that takes only Implicit VR Little Endian gets an explicit file converted,
    // and the reverse.
    for (only, make) in [
        (IMPLICIT, explicit as fn(&str, &str, u16, u32) -> Vec<u8>),
        (EXPLICIT, implicit),
    ] {
        let scp = TestScp::start(ScpConfig {
            transfer_syntaxes: vec![only.into()],
            ..ScpConfig::default()
        })
        .unwrap();
        let f = make(CT, "1.2.826.0.1.3680043.10.9.4", 32, 1);
        let mut a = open(&scp, &[&f]);
        let s = a.send_file(&f, None, &mut |_| {}).unwrap();
        assert_eq!(s.transfer_syntax, only);
        a.release().unwrap();
        let r = scp.received.lock()[0].clone();
        let back = wrap(&r.data, CT, &r.sop_instance, only);
        assert_eq!(decoded(&back), pixels(32, 1));
        let d = specular_dicom::Dicom::parse(&back).unwrap();
        let v = d.values(&back);
        // The sequence item survived.
        let seq = d.dataset.item(0x0008_2218).expect("sequence");
        assert_eq!(v.text_n(seq.get(0x0008_0100).unwrap(), 0), Some("T-D0050"));
        assert!(s.converted);
    }
}

#[test]
fn compressed_as_stored_or_decoded() {
    let f = rle(CT, "1.2.826.0.1.3680043.10.9.5", 48, 2);
    assert_eq!(decoded(&f), pixels(48, 2), "the fixture's RLE decodes");
    // Accepted: sent as stored.
    let scp = TestScp::start(ScpConfig {
        transfer_syntaxes: vec![RLE.into(), EXPLICIT.into()],
        ..ScpConfig::default()
    })
    .unwrap();
    let mut a = open(&scp, &[&f]);
    let s = a.send_file(&f, None, &mut |_| {}).unwrap();
    assert_eq!((s.transfer_syntax.as_str(), s.converted), (RLE, false));
    a.release().unwrap();
    let h = store::head(&f).unwrap();
    assert_eq!(scp.received.lock()[0].data, &f[h.data_set..]);
    // Not accepted: decoded into Explicit VR Little Endian, the same pixels.
    let scp = TestScp::start(ScpConfig::default()).unwrap();
    let mut a = open(&scp, &[&f]);
    let s = a.send_file(&f, None, &mut |_| {}).unwrap();
    assert_eq!((s.transfer_syntax.as_str(), s.converted), (EXPLICIT, true));
    a.release().unwrap();
    let r = scp.received.lock()[0].clone();
    assert_eq!(
        decoded(&wrap(&r.data, CT, &r.sop_instance, EXPLICIT)),
        pixels(48, 2)
    );
}

#[test]
fn failures_are_per_instance() {
    let bad = "1.2.826.0.1.3680043.10.9.6.2";
    let scp = TestScp::start(ScpConfig {
        sop_classes: vec![CT.into()],
        fail: vec![(bad.into(), 0xA700)],
        ..ScpConfig::default()
    })
    .unwrap();
    let ok = explicit(CT, "1.2.826.0.1.3680043.10.9.6.1", 16, 0);
    let refused = explicit(CT, bad, 16, 1);
    let mr = explicit(MR, "1.2.826.0.1.3680043.10.9.6.3", 16, 2);
    let mut a = open(&scp, &[&ok, &refused, &mr]);
    let s = a.send_file(&refused, None, &mut |_| {}).unwrap();
    assert_eq!(Status::of(s.status), Status::OutOfResources);
    assert_eq!(s.comment, "Test failure");
    let e = a.send_file(&mr, None, &mut |_| {}).unwrap_err();
    assert!(matches!(&e, Error::NoContext(c) if c == MR), "{e}");
    assert!(!e.is_fatal());
    let e = a.send_file(b"not dicom", None, &mut |_| {}).unwrap_err();
    assert!(!e.is_fatal(), "{e}");
    // The association goes on.
    let s = a.send_file(&ok, None, &mut |_| {}).unwrap();
    assert!(Status::of(s.status).stored());
    a.release().unwrap();
    assert_eq!(scp.received.lock().len(), 1);
}

#[test]
fn silence_times_out_and_cancel_is_quick() {
    let scp = TestScp::start(ScpConfig {
        silent: true,
        ..ScpConfig::default()
    })
    .unwrap();
    let f = explicit(CT, "1.2.826.0.1.3680043.10.9.7", 16, 0);
    let heads = [store::head(&f).unwrap()];
    let ctx = store::contexts(
        heads
            .iter()
            .map(|h| (h.sop_class.as_str(), h.transfer_syntax.as_str())),
    );
    let opts = Options {
        timeout: Duration::from_millis(400),
        ..quick()
    };
    let mut a = Association::request(&peer(&scp), &opts, &ctx, &CancelToken::new()).unwrap();
    let t0 = Instant::now();
    let e = a.send_file(&f, None, &mut |_| {}).unwrap_err();
    assert!(matches!(e, Error::Timeout), "{e}");
    assert!(t0.elapsed() < Duration::from_secs(3));
    drop(a);

    let cancel = CancelToken::new();
    let mut a = Association::request(&peer(&scp), &quick(), &ctx, &cancel).unwrap();
    let c = cancel.clone();
    let stopper = specular_thread::spawn("dimse-cancel", move || {
        std::thread::sleep(Duration::from_millis(150));
        c.cancel();
    });
    let t0 = Instant::now();
    let e = a.send_file(&f, None, &mut |_| {}).unwrap_err();
    assert!(matches!(e, Error::Cancelled), "{e}");
    assert!(t0.elapsed() < Duration::from_secs(2));
    stopper.join().unwrap();
}

#[test]
fn abort_from_the_server() {
    let scp = TestScp::start(ScpConfig {
        abort_after: Some(1),
        ..ScpConfig::default()
    })
    .unwrap();
    let f1 = explicit(CT, "1.2.826.0.1.3680043.10.9.8.1", 16, 0);
    let f2 = explicit(CT, "1.2.826.0.1.3680043.10.9.8.2", 16, 1);
    let mut a = open(&scp, &[&f1, &f2]);
    a.send_file(&f1, None, &mut |_| {}).unwrap();
    let e = a.send_file(&f2, None, &mut |_| {}).unwrap_err();
    assert!(matches!(e, Error::Aborted { source: 2, .. }), "{e}");
    assert!(e.is_fatal());
}

#[test]
fn files_without_meta_and_heads_from_disk() {
    let f = explicit(CT, "1.2.826.0.1.3680043.10.9.9", 16, 0);
    let dir = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp/dimse-heads"
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("a.dcm");
    std::fs::write(&p, &f).unwrap();
    let h = store::read_head(&p).unwrap().unwrap();
    assert_eq!(h, store::head(&f).unwrap());
    assert_eq!(h.sop_class, CT);
    // A bare Implicit VR Little Endian data set (no File Meta Information).
    let o = object(CT, "1.2.826.0.1.3680043.10.9.10", 16, 0);
    let mut bare = Vec::new();
    store::implicit(&o, &mut bare).unwrap();
    let q = dir.join("bare");
    std::fs::write(&q, &bare).unwrap();
    let h = store::read_head(&q).unwrap().unwrap();
    assert_eq!(
        (h.sop_instance.as_str(), h.data_set),
        ("1.2.826.0.1.3680043.10.9.10", 0)
    );
    assert_eq!(h.transfer_syntax, IMPLICIT);
    let _ = std::fs::remove_dir_all(&dir);
}
