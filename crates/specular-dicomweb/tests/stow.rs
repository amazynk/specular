//! STOW-RS against the fake server (in-process, loopback).

use specular_dicomweb::fake::{self, FakeAuth, FakeConfig, FakeServer};
use specular_dicomweb::{DicomWeb, StoreOutcome};
use specular_remote::testing::ServerOptions;
use specular_remote::{Auth, CancelToken, Client, ClientConfig, Secret};

fn files() -> (Vec<Vec<u8>>, Vec<String>, String) {
    let cfg = FakeConfig::synthetic(1, 3, 16);
    let st = &cfg.studies[0];
    let se = &st.series[0];
    let files: Vec<Vec<u8>> = (0..3).map(|i| fake::part10(st, se, i)).collect();
    let uids = files
        .iter()
        .map(|f| {
            let d = specular_dicom::Dicom::parse(f).unwrap();
            let v = d.values(f);
            String::from_utf8_lossy(v.bytes(d.dataset.get(0x0008_0018).unwrap()))
                .trim_end_matches(['\0', ' '])
                .to_string()
        })
        .collect();
    (files, uids, st.uid.clone())
}

fn web(f: &FakeServer, auth: Auth) -> DicomWeb {
    DicomWeb::new(
        Client::new(ClientConfig::for_tests()).unwrap(),
        &f.base(),
        auth,
    )
}

#[test]
fn stores_and_names_failures() {
    let (files, uids, study) = files();
    for xml in [false, true] {
        let f = FakeServer::start(
            FakeConfig {
                stow_fail: vec![uids[1].clone()],
                stow_xml: xml,
                ..FakeConfig::default()
            },
            ServerOptions::default(),
        )
        .unwrap();
        let w = web(&f, Auth::None);
        let parts: Vec<&[u8]> = files.iter().map(Vec::as_slice).collect();
        let r = w.store(&parts, None, &CancelToken::new()).unwrap();
        assert_eq!(r.status, 202);
        assert_eq!(r.outcome(&uids[0]), StoreOutcome::Stored);
        assert_eq!(r.outcome(&uids[1]), StoreOutcome::Failed(Some(0xA700)));
        assert_eq!(r.outcome(&uids[2]), StoreOutcome::Stored);
        let kept = f.stored.lock().clone();
        assert_eq!(
            kept,
            vec![files[0].clone(), files[2].clone()],
            "bytes changed in transit"
        );
        // Only the failing one: 409 with its reason, not an error.
        let r = w
            .store(&[&files[1]], Some(&study), &CancelToken::new())
            .unwrap();
        assert_eq!(
            (r.status, r.outcome(&uids[1])),
            (409, StoreOutcome::Failed(Some(0xA700)))
        );
        // Into another study: refused per instance.
        let r = w
            .store(&[&files[0]], Some("1.2.3"), &CancelToken::new())
            .unwrap();
        assert_eq!(r.outcome(&uids[0]), StoreOutcome::Failed(Some(0xA900)));
    }
}

#[test]
fn credentials_and_errors() {
    let (files, uids, _) = files();
    let f = FakeServer::start(
        FakeConfig {
            auth: Some(FakeAuth::Bearer("t0k".into())),
            ..FakeConfig::default()
        },
        ServerOptions::default(),
    )
    .unwrap();
    let e = web(&f, Auth::None)
        .store(&[&files[0]], None, &CancelToken::new())
        .unwrap_err();
    assert!(e.is_auth(), "{e}");
    let r = web(&f, Auth::Bearer(Secret::new(String::from("t0k"))))
        .store(&[&files[0]], None, &CancelToken::new())
        .unwrap();
    assert_eq!((r.status, r.outcome(&uids[0])), (200, StoreOutcome::Stored));
    // A cancelled request never reaches the server.
    let c = CancelToken::new();
    c.cancel();
    let e = web(&f, Auth::Bearer(Secret::new(String::from("t0k"))))
        .store(&[&files[1]], None, &c)
        .unwrap_err();
    assert!(e.is_cancelled(), "{e}");
    assert_eq!(f.stored.lock().len(), 1);
}
