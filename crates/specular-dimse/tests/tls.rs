//! DICOM over TLS against the loopback SCP with a test-only certificate (tests/tls: a test
//! CA and a server certificate for 127.0.0.1 and localhost, signed by it). The CA is trusted
//! only here; without it the server is refused, whatever the rest of the setup.

use std::time::Duration;

use specular_core::CancelToken;
use specular_dimse::assoc;
use specular_dimse::scp::{ScpConfig, TestScp};
use specular_dimse::tls::ServerIdentity;
use specular_dimse::{Error, Options, Peer, Tls, TlsError};

const CA: &[u8] = include_bytes!("tls/ca.der");
const CERT: &[u8] = include_bytes!("tls/server.der");
const KEY: &[u8] = include_bytes!("tls/server.key.der");

fn tls_scp() -> TestScp {
    TestScp::start(ScpConfig {
        tls: Some(ServerIdentity::new(&[CERT], KEY).unwrap()),
        records: vec![vec![
            (0x0008_0052, "STUDY".into()),
            (0x0020_000D, "1.2.3".into()),
        ]],
        ..ScpConfig::default()
    })
    .unwrap()
}

fn peer(host: &str, port: u16) -> Peer {
    Peer {
        host: host.into(),
        port,
        called_ae: "TESTSCP".into(),
        calling_ae: "SPECULAR".into(),
    }
}

fn opts(tls: Option<Tls>) -> Options {
    Options {
        timeout: Duration::from_secs(5),
        tls,
        ..Options::default()
    }
}

#[test]
fn echo_store_and_find_over_tls() {
    let scp = tls_scp();
    let c = CancelToken::new();
    let trusted = Tls::trusting(&[CA]).unwrap();
    // By IP address and by name: both are in the certificate.
    for host in ["127.0.0.1", "localhost"] {
        assoc::verify(&peer(host, scp.port), &opts(Some(trusted.clone())), &c).unwrap();
    }
    assert_eq!(scp.echoes.load(std::sync::atomic::Ordering::Relaxed), 2);
    let q = specular_dimse::find::Query {
        level: specular_dimse::find::Level::Study,
        keys: vec![(0x0020_000D, String::new())],
    };
    let f = specular_dimse::find::find(
        &peer("127.0.0.1", scp.port),
        &opts(Some(trusted)),
        &q,
        0,
        0,
        &c,
    )
    .unwrap();
    assert_eq!(f.matches.len(), 1);
    assert_eq!(f.matches[0].text(0x0020_000D), "1.2.3");
}

#[test]
fn an_untrusted_server_is_refused() {
    let scp = tls_scp();
    let c = CancelToken::new();
    // Trusting the server's own certificate is not trusting its issuer.
    let wrong = Tls::trusting(&[CERT]).unwrap();
    let e = assoc::verify(&peer("127.0.0.1", scp.port), &opts(Some(wrong)), &c).unwrap_err();
    assert!(matches!(e, Error::Tls(TlsError::Certificate(_))), "{e}");
    // The system trust store does not know the test CA either.
    if let Ok(system) = Tls::system() {
        let e = assoc::verify(&peer("127.0.0.1", scp.port), &opts(Some(system)), &c).unwrap_err();
        assert!(matches!(e, Error::Tls(TlsError::Certificate(_))), "{e}");
    }
    assert_eq!(scp.echoes.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[test]
fn plain_and_tls_do_not_mix() {
    let c = CancelToken::new();
    // TLS to a plain server: the handshake fails, plainly.
    let plain = TestScp::start(ScpConfig::default()).unwrap();
    let e = assoc::verify(
        &peer("127.0.0.1", plain.port),
        &opts(Some(Tls::trusting(&[CA]).unwrap())),
        &c,
    )
    .unwrap_err();
    assert!(
        matches!(e, Error::Tls(_) | Error::Io(_) | Error::Timeout),
        "{e}"
    );
    // Plain to a TLS server: no association.
    let scp = tls_scp();
    let e = assoc::verify(&peer("127.0.0.1", scp.port), &opts(None), &c).unwrap_err();
    assert!(!matches!(e, Error::Status(_)), "{e}");
}
