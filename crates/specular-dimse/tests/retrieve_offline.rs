//! The no-network switch refuses a C-MOVE before its storage SCP listens, and a C-GET
//! before it connects (its own test binary: the switch is process-wide).

use std::time::Duration;

use specular_core::CancelToken;
use specular_core::net::off::{self, By};
use specular_dimse::listen::{Listen, move_into};
use specular_dimse::retrieve::{self, Incoming, Storage, Target};
use specular_dimse::{Error, Options, Peer};

struct Nothing;

impl Storage for Nothing {
    fn begin(&mut self, _: &str, _: &str, _: &str) -> std::io::Result<Box<dyn Incoming>> {
        Err(std::io::Error::other("unused"))
    }
}

#[test]
fn no_network_refuses_before_listening() {
    off::set(By::Flag, true);
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let listen = Listen {
        address: "127.0.0.1".parse().unwrap(),
        port,
        ae_title: "SPECULAR".into(),
    };
    let peer = Peer {
        host: "127.0.0.1".into(),
        port: 1,
        called_ae: "PACS".into(),
        calling_ae: "SPECULAR".into(),
    };
    let target = Target {
        study: "1.2".into(),
        series: None,
    };
    let opts = Options {
        timeout: Duration::from_secs(2),
        ..Options::default()
    };
    let e = move_into(
        &peer,
        &opts,
        &target,
        &listen,
        &mut Nothing,
        &mut |_| {},
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::NetworkOff), "{e}");
    // Nothing listened: the port is still free.
    assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_ok());
    let e = retrieve::get(
        &peer,
        &opts,
        &target,
        &mut Nothing,
        &mut |_| {},
        Box::new(|| false),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::NetworkOff), "{e}");
    off::set(By::Flag, false);
}
