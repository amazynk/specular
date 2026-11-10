//! A DIMSE series retrieved by C-GET and C-MOVE into the disk cache and opened as series,
//! against the in-crate test SCP on loopback; counts in the download registry; offline from
//! the cache; failures in their own words.

use std::sync::Arc;
use std::time::Duration;

use specular_core::net::{Registry, State};
use specular_dicomweb::LoadEvent;
use specular_dicomweb::fake::{self, FakeConfig};
use specular_dicomweb::retrieve::{DimseLoad, Failure, How, Source};
use specular_dimse::listen::Listen;
use specular_dimse::scp::{ScpConfig, TestScp};
use specular_dimse::{Options, Peer};
use specular_remote::DiskCache;
use std::sync::mpsc::Receiver;

fn cache(name: &str) -> Arc<DiskCache> {
    let d = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp"
    ))
    .join(name);
    let _ = std::fs::remove_dir_all(&d);
    DiskCache::open(d, 1 << 30).unwrap()
}

fn source(port: u16, how: How, reg: &Registry) -> Source {
    Source {
        peer: Peer {
            host: "127.0.0.1".into(),
            port,
            called_ae: "TESTSCP".into(),
            calling_ae: "SPECULAR".into(),
        },
        options: Options {
            timeout: Duration::from_secs(10),
            ..Options::default()
        },
        how,
        label: "Axial phantom".into(),
        registry: Some(reg.clone()),
    }
}

fn start(
    src: Source,
    cache: &Arc<DiskCache>,
    study: &str,
    series: &str,
) -> (DimseLoad, Receiver<LoadEvent>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let load = DimseLoad::start(
        src,
        cache.clone(),
        "dicomweb/s1",
        study,
        series,
        Arc::new(move |e| {
            let _ = tx.send(e);
        }),
    );
    (load, rx)
}

fn events(load: DimseLoad, rx: &Receiver<LoadEvent>) -> Vec<LoadEvent> {
    load.join();
    rx.try_iter().collect()
}

fn files(cfg: &FakeConfig) -> Vec<Vec<u8>> {
    let st = &cfg.studies[0];
    st.series
        .iter()
        .flat_map(|se| (0..se.slices).map(move |i| fake::part10(st, se, i)))
        .collect()
}

#[test]
fn c_get_opens_the_series_from_the_cache() {
    let cfg = FakeConfig::synthetic(1, 7, 32);
    let served = files(&cfg);
    let scp = TestScp::start(ScpConfig {
        instances: served.clone(),
        ..ScpConfig::default()
    })
    .unwrap();
    let st = &cfg.studies[0];
    let c = cache("dimse-load-get");
    let reg = Registry::new();
    let (load, rx) = start(
        source(scp.port, How::Get, &reg),
        &c,
        &st.uid,
        &st.series[0].uid,
    );
    let ev = events(load, &rx);
    let Some(LoadEvent::Metadata { series, offline }) =
        ev.iter().find(|e| matches!(e, LoadEvent::Metadata { .. }))
    else {
        panic!("no metadata: {ev:?}");
    };
    assert!(!offline);
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].items.len(), 7);
    // In slice order, each a cached Part 10 file equal in pixels to what was served.
    let paths: Vec<_> = ev
        .iter()
        .filter_map(|e| match e {
            LoadEvent::Instance { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(paths.len(), 7);
    for p in &paths {
        assert!(p.starts_with(c.root()));
        let got = std::fs::read(p).unwrap();
        let h = specular_dimse::store::head(&got).unwrap();
        let want = served
            .iter()
            .find(|f| specular_dimse::store::head(f).unwrap().sop_instance == h.sop_instance)
            .unwrap();
        let dec = |f: &[u8]| {
            let d = specular_dicom::Dicom::parse(f).unwrap();
            format!("{:?}", d.decode_stored(f, 0, None).unwrap().pixels)
        };
        assert_eq!(dec(&got), dec(want));
    }
    assert!(matches!(
        ev.last(),
        Some(LoadEvent::Done {
            failed: 0,
            cancelled: false,
            ..
        })
    ));
    let a = &reg.snapshot()[0];
    assert_eq!(a.label.host, "127.0.0.1");
    assert_eq!(a.state, State::Done);
    assert_eq!(a.items.map(|i| (i.completed, i.total())), Some((7, 7)));
    assert!(a.bytes > 7 * 32 * 32 * 2);

    // The server gone: the series opens from the cache.
    let port = scp.port;
    drop(scp);
    let (load, rx) = start(source(port, How::Get, &reg), &c, &st.uid, &st.series[0].uid);
    let ev = events(load, &rx);
    assert!(
        ev.iter()
            .any(|e| matches!(e, LoadEvent::Metadata { offline: true, series } if series[0].items.len() == 7)),
        "{ev:?}"
    );
}

#[test]
fn c_move_opens_the_series() {
    let cfg = FakeConfig::synthetic(1, 5, 16);
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let scp = TestScp::start(ScpConfig {
        instances: files(&cfg),
        destinations: vec![("SPECULAR".into(), port)],
        ..ScpConfig::default()
    })
    .unwrap();
    let st = &cfg.studies[0];
    let how = How::Move(Listen {
        address: "127.0.0.1".parse().unwrap(),
        port,
        ae_title: "SPECULAR".into(),
    });
    let reg = Registry::new();
    let (load, rx) = start(
        source(scp.port, how, &reg),
        &cache("dimse-load-move"),
        &st.uid,
        &st.series[0].uid,
    );
    let ev = events(load, &rx);
    let n = ev
        .iter()
        .filter(|e| matches!(e, LoadEvent::Instance { .. }))
        .count();
    assert_eq!(n, 5, "{ev:?}");
}

#[test]
fn failures_are_told_apart() {
    let cfg = FakeConfig::synthetic(1, 3, 16);
    let st = &cfg.studies[0];
    let reg = Registry::new();
    let error = |ev: Vec<LoadEvent>| -> Failure {
        for e in ev {
            if let LoadEvent::Error(specular_dicomweb::Error::Retrieve(f)) = e {
                return f;
            }
        }
        panic!("no retrieve error");
    };
    // No such series.
    let scp = TestScp::start(ScpConfig {
        instances: files(&cfg),
        ..ScpConfig::default()
    })
    .unwrap();
    let (load, rx) = start(
        source(scp.port, How::Get, &reg),
        &cache("dimse-load-f1"),
        &st.uid,
        "9.9",
    );
    assert!(matches!(error(events(load, &rx)), Failure::NoMatches));
    // Roles refused.
    let scp = TestScp::start(ScpConfig {
        instances: files(&cfg),
        refuse_roles: true,
        ..ScpConfig::default()
    })
    .unwrap();
    let (load, rx) = start(
        source(scp.port, How::Get, &reg),
        &cache("dimse-load-f2"),
        &st.uid,
        &st.series[0].uid,
    );
    assert!(matches!(
        error(events(load, &rx)),
        Failure::NothingArrived {
            failed: 3,
            roles_refused: true,
            moved: false
        }
    ));
    // C-MOVE to an AE title the server does not know.
    let how = How::Move(Listen {
        address: "127.0.0.1".parse().unwrap(),
        port: 0,
        ae_title: "SPECULAR".into(),
    });
    let (load, rx) = start(
        source(scp.port, how, &reg),
        &cache("dimse-load-f3"),
        &st.uid,
        &st.series[0].uid,
    );
    assert!(matches!(error(events(load, &rx)), Failure::Status(0xA801)));
}
