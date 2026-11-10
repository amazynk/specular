//! The DICOMweb client against the fake server (in-process, loopback, synthetic studies).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use specular_dicomweb::fake::{FakeAuth, FakeConfig, FakeServer};
use specular_dicomweb::multipart::{Multipart, MultipartError};
use specular_dicomweb::{
    DicomWeb, LoadEvent, LoadOptions, SeriesLoad, StudyQuery, json::parse_array, wado::parts_of,
};
use specular_remote::testing::ServerOptions;
use specular_remote::{Auth, CancelToken, Client, ClientConfig, DiskCache, Secret};

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/test-tmp/dicomweb"
    ))
    .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn client() -> Client {
    Client::new(ClientConfig::for_tests()).unwrap()
}

fn fake(cfg: FakeConfig) -> FakeServer {
    FakeServer::start(cfg, ServerOptions::default()).unwrap()
}

fn web(f: &FakeServer, auth: Auth) -> DicomWeb {
    DicomWeb::new(client(), &f.base(), auth)
}

// ---- multipart -------------------------------------------------------------------------

fn feed_all(ct: &str, body: &[u8], step: usize) -> Result<Vec<Vec<u8>>, MultipartError> {
    let mut m = Multipart::new(ct)?;
    let mut out = Vec::new();
    for c in body.chunks(step.max(1)) {
        m.feed(c, |p| out.push(p.body))?;
    }
    m.finish()?;
    Ok(out)
}

#[test]
fn multipart_edge_cases() {
    // Quoted boundary with characters that need quoting, transfer-syntax parameters on the
    // type and the part, a preamble, transport padding and an epilogue.
    let ct = "multipart/related; type=\"application/dicom\"; transfer-syntax=1.2.840.10008.1.2.1; boundary=\"a:b/c=d?\"";
    let body = b"preamble\r\n--a:b/c=d?  \r\nContent-Type: application/dicom; transfer-syntax=1.2.840.10008.1.2.4.80\r\n\r\nFIRST\r\n--a:b/c=d?\r\nContent-Type: application/dicom\r\n\r\n\r\n--a:b/c=d?\r\n\r\nthird with --a:b/c=d no CRLF before\r\n--a:b/c=d?--\r\nepilogue";
    for step in [1, 2, 3, 7, 64, body.len()] {
        let parts = feed_all(ct, body, step).unwrap();
        assert_eq!(
            parts,
            vec![
                b"FIRST".to_vec(),
                Vec::new(),
                b"third with --a:b/c=d no CRLF before".to_vec()
            ],
            "step {step}"
        );
    }
    let parts = parts_of(Some(ct), body.to_vec()).unwrap();
    assert_eq!(
        parts[0].transfer_syntax().as_deref(),
        Some("1.2.840.10008.1.2.4.80")
    );
    assert_eq!(parts[0].content_type(), "application/dicom");
    // No preamble, bare boundary token.
    let parts = feed_all(
        "multipart/related; boundary=XyZ",
        b"--XyZ\r\nContent-Type: application/octet-stream\r\n\r\n\x00\x01\r\n--XyZ--",
        5,
    )
    .unwrap();
    assert_eq!(parts, vec![vec![0u8, 1]]);
    // Truncated: no closing delimiter.
    assert!(matches!(
        feed_all(ct, &body[..body.len() - 30], 9),
        Err(MultipartError::Truncated)
    ));
    assert!(matches!(
        Multipart::new("multipart/related; type=\"application/dicom\""),
        Err(MultipartError::NoBoundary)
    ));
    // A non-multipart body is one part.
    let one = parts_of(Some("application/dicom"), b"DICM".to_vec()).unwrap();
    assert_eq!(one.len(), 1);
}

// ---- QIDO ------------------------------------------------------------------------------

#[test]
fn qido_pages_filters_and_empty_results() {
    let f = fake(FakeConfig::synthetic(25, 3, 16));
    let w = web(&f, Auth::None);
    let cancel = CancelToken::new();
    let mut q = StudyQuery {
        limit: 10,
        ..StudyQuery::default()
    };
    let mut seen = Vec::new();
    loop {
        let p = w.search_studies(&q, &cancel).unwrap();
        seen.extend(p.rows.iter().map(|r| r.study_uid.clone()));
        if !p.more {
            assert_eq!(p.rows.len(), 5);
            break;
        }
        q.offset = p.next_offset();
    }
    assert_eq!(seen.len(), 25);
    seen.dedup();
    assert_eq!(seen.len(), 25);

    // Without the "more results" warning a full page still means "maybe more".
    let f2 = fake(FakeConfig {
        no_more_warning: true,
        ..FakeConfig::synthetic(20, 1, 8)
    });
    let p = web(&f2, Auth::None)
        .search_studies(
            &StudyQuery {
                limit: 10,
                ..StudyQuery::default()
            },
            &cancel,
        )
        .unwrap();
    assert!(p.more);

    // Filters: name prefix (case-insensitive), date range, modality.
    let p = w
        .search_studies(
            &StudyQuery {
                patient_name: "fake^al".into(),
                ..StudyQuery::default()
            },
            &cancel,
        )
        .unwrap();
    assert!(!p.rows.is_empty());
    assert!(p.rows.iter().all(|r| r.patient.name == "FAKE^ALPHA"));
    let r = &p.rows[0];
    assert_eq!(r.series_count, Some(2));
    assert_eq!(r.instance_count, Some(4));
    assert!(format!("{r:?}").contains("<hidden>"));
    assert!(!format!("{r:?}").contains("ALPHA"));
    let p = w
        .search_studies(
            &StudyQuery {
                date_from: "2025-02-01".into(),
                date_to: "2025-02-28".into(),
                modality: "MR".into(),
                ..StudyQuery::default()
            },
            &cancel,
        )
        .unwrap();
    assert!(!p.rows.is_empty());
    for r in &p.rows {
        assert!(r.date.as_str() >= "20250201" && r.date.as_str() <= "20250228");
        assert!(r.modalities.contains(&"MR".to_string()));
    }
    let last = f.server.log().pop().unwrap();
    assert_eq!(
        last.param("StudyDate").as_deref(),
        Some("20250201-20250228")
    );
    // Nothing found: 204.
    let p = w
        .search_studies(
            &StudyQuery {
                patient_id: "nobody".into(),
                ..StudyQuery::default()
            },
            &cancel,
        )
        .unwrap();
    assert!(p.rows.is_empty() && !p.more);

    // Series and instances of a study.
    let study = &seen[0];
    let s = w.search_series(study, 0, 0, &cancel).unwrap();
    assert_eq!(s.rows.len(), 2);
    assert_eq!(s.rows[0].number, Some(1));
    assert_eq!(s.rows[0].instance_count, Some(3));
    let i = w
        .search_instances(study, &s.rows[0].series_uid, 2, 0, &cancel)
        .unwrap();
    assert_eq!(i.rows.len(), 2);
    assert!(i.more);
    assert_eq!(i.rows[0].rows, Some(16));
    // Not a UID: refused before anything is sent.
    let before = f.server.requests();
    assert!(w.search_series("1.2/../x", 0, 0, &cancel).is_err());
    assert_eq!(f.server.requests(), before);
}

// ---- WADO ------------------------------------------------------------------------------

#[test]
fn metadata_maps_into_the_shared_model() {
    let cfg = FakeConfig::synthetic(1, 5, 16);
    let st = cfg.studies[0].clone();
    let f = fake(cfg);
    let w = web(&f, Auth::None);
    let cancel = CancelToken::new();
    let se = &st.series[0];
    let (sets, raw) = w.series_metadata(&st.uid, &se.uid, &cancel).unwrap();
    assert_eq!(sets.len(), 5);
    assert_eq!(parse_array(&raw).unwrap().len(), 5);
    let inst = sets[2].to_instance();
    assert_eq!(inst.study_uid, st.uid);
    assert_eq!(inst.series_uid, se.uid);
    assert_eq!(inst.instance_number, Some(3));
    assert_eq!(inst.frames, 1);
    assert_eq!(inst.orientation, Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]));
    assert_eq!(inst.position.map(|p| p[2]), Some(-5.0));
    assert!(sets[0].get(0x7FE0_0010).unwrap().bulk_data_uri.is_some());

    // The instance itself: a Part 10 file whose header matches the metadata, and whose
    // pixels decode.
    let sop = &inst.sop_uid;
    let part = w.instance(&st.uid, &se.uid, sop, &cancel, |_| {}).unwrap();
    assert_eq!(
        part.transfer_syntax().as_deref(),
        Some(specular_dicomweb::fake::TS)
    );
    let local = specular_dicom::Instance::read(&part.body, true)
        .unwrap()
        .unwrap();
    assert_eq!(local.sop_uid, inst.sop_uid);
    assert_eq!(local.position, inst.position);
    let d = specular_dicom::Dicom::parse(&part.body).unwrap();
    assert!(d.decode_frame(&part.body, 0, None).is_ok());

    // Frames, bulk data, rendered and the whole series in one response.
    let frames = w.frames(&st.uid, &se.uid, sop, &[1], &cancel).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].body.len(), 16 * 16 * 2);
    let uri = sets[0]
        .get(0x7FE0_0010)
        .unwrap()
        .bulk_data_uri
        .clone()
        .unwrap();
    assert_eq!(w.bulk_data(&uri, &cancel).unwrap().body.len(), 512);
    assert!(w.bulk_data("https://elsewhere.example/x", &cancel).is_err());
    let (mt, png) = w
        .rendered(&st.uid, &se.uid, sop, None, Some((64, 64)), &cancel)
        .unwrap();
    assert_eq!(mt, "image/png");
    assert!(png.starts_with(b"\x89PNG"));
    let mut n = 0;
    w.series_instances(&st.uid, &se.uid, &cancel, |p| {
        assert!(specular_dicom::sniff(&p.body));
        n += 1;
        std::ops::ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!(n, 5);
    // The Accept header offers the decodable syntaxes.
    let accept = f
        .server
        .log()
        .iter()
        .find(|r| r.path().ends_with(sop.as_str()))
        .and_then(|r| r.header("accept").map(str::to_string))
        .unwrap();
    assert!(accept.contains("transfer-syntax=1.2.840.10008.1.2.4.201"));
    assert!(!accept.contains("transfer-syntax=*"));
}

#[test]
fn not_acceptable_falls_back_and_chunked_bodies_parse() {
    let cfg = FakeConfig {
        only_any_syntax: true,
        chunked: Some(997),
        ..FakeConfig::synthetic(1, 2, 32)
    };
    let st = cfg.studies[0].clone();
    let f = fake(cfg);
    let w = web(&f, Auth::None);
    let se = &st.series[0];
    let sop = format!("{}.1", se.uid);
    let p = w
        .instance(&st.uid, &se.uid, &sop, &CancelToken::new(), |_| {})
        .unwrap();
    assert!(specular_dicom::sniff(&p.body));
    let log = f.server.log();
    let tries: Vec<_> = log.iter().filter(|r| r.path().ends_with(&sop)).collect();
    assert_eq!(tries.len(), 2);
    assert!(
        tries[1]
            .header("accept")
            .unwrap()
            .contains("transfer-syntax=*")
    );
}

#[test]
fn auth_and_errors() {
    let cfg = FakeConfig {
        auth: Some(FakeAuth::Basic {
            user: "reader".into(),
            password: "s3cret".into(),
        }),
        ..FakeConfig::synthetic(2, 2, 8)
    };
    let st = cfg.studies[0].clone();
    let f = fake(cfg);
    let cancel = CancelToken::new();
    let e = web(&f, Auth::None).check(&cancel).unwrap_err();
    assert!(e.is_auth());
    let wrong = Auth::Basic {
        user: "reader".into(),
        password: Secret::new("nope"),
    };
    assert!(web(&f, wrong).check(&cancel).unwrap_err().is_auth());
    let good = Auth::Basic {
        user: "reader".into(),
        password: Secret::new("s3cret"),
    };
    let w = web(&f, good);
    w.check(&cancel).unwrap();

    let bearer = fake(FakeConfig {
        auth: Some(FakeAuth::Bearer("tok".into())),
        ..FakeConfig::synthetic(1, 1, 8)
    });
    web(&bearer, Auth::Bearer(Secret::new("tok")))
        .check(&cancel)
        .unwrap();
    assert!(
        web(&bearer, Auth::Bearer(Secret::new("other")))
            .check(&cancel)
            .unwrap_err()
            .is_auth()
    );

    // 404, retried 503, persistent 503.
    let se = &st.series[0];
    assert_eq!(
        w.search_series("1.2.3.4", 0, 0, &cancel)
            .unwrap_err()
            .status(),
        Some(404)
    );
    let sop = format!("{}.1", se.uid);
    f.fail_next.store(1, Ordering::SeqCst);
    w.instance(&st.uid, &se.uid, &sop, &cancel, |_| {}).unwrap();
    f.fail_next.store(10, Ordering::SeqCst);
    let e = w
        .instance(&st.uid, &se.uid, &sop, &cancel, |_| {})
        .unwrap_err();
    assert_eq!(e.status(), Some(503));
    f.fail_next.store(0, Ordering::SeqCst);

    // Nobody listening: offline.
    let dead = {
        let tmp = fake(FakeConfig::default());
        tmp.base()
    };
    let e = DicomWeb::new(client(), &dead, Auth::None)
        .check(&cancel)
        .unwrap_err();
    assert!(e.is_offline(), "{e}");
}

// ---- progressive loading ---------------------------------------------------------------

type Events = Arc<Mutex<Vec<LoadEvent>>>;

fn sink(ev: &Events) -> specular_dicomweb::progressive::Sink {
    let ev = ev.clone();
    Arc::new(move |e| ev.lock().push(e))
}

fn load(
    w: DicomWeb,
    cache: &Arc<DiskCache>,
    study: &str,
    series: &str,
    parallel: usize,
) -> (SeriesLoad, Events) {
    let ev: Events = Arc::default();
    let l = SeriesLoad::start(
        w,
        cache.clone(),
        "dicomweb/test",
        study,
        series,
        LoadOptions { parallel },
        sink(&ev),
    );
    (l, ev)
}

fn instances(ev: &[LoadEvent]) -> Vec<usize> {
    ev.iter()
        .filter_map(|e| match e {
            LoadEvent::Instance { index, .. } => Some(*index),
            _ => None,
        })
        .collect()
}

#[test]
fn progressive_middle_first_then_nearest_then_cached_and_offline() {
    let cfg = FakeConfig::synthetic(1, 21, 16);
    let st = cfg.studies[0].clone();
    let se = st.series[0].clone();
    let f = fake(cfg);
    let dir = scratch("progressive");
    let cache = DiskCache::open(&dir, 64 << 20).unwrap();

    let (l, ev) = load(web(&f, Auth::None), &cache, &st.uid, &se.uid, 1);
    l.join();
    let ev = std::mem::take(&mut *ev.lock());
    let LoadEvent::Metadata { series, offline } = &ev[0] else {
        panic!("{:?}", ev[0]);
    };
    assert!(!offline);
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].items.len(), 21);
    // Ordered by position: instance numbers follow.
    let nums: Vec<_> = series[0]
        .items
        .iter()
        .map(|i| i.instance.instance_number.unwrap())
        .collect();
    let mut sorted = nums.clone();
    sorted.sort();
    assert!(nums == sorted || nums.iter().rev().eq(sorted.iter()));
    let order = instances(&ev);
    assert_eq!(order.len(), 21);
    assert_eq!(order[0], 10, "middle slice first");
    // Then alternating outwards, ahead first.
    assert_eq!(&order[1..5], &[11, 9, 12, 8]);
    assert!(matches!(
        ev.last(),
        Some(LoadEvent::Done {
            failed: 0,
            offline: false,
            auth: false,
            cancelled: false
        })
    ));
    for e in &ev {
        if let LoadEvent::Instance { path, .. } = e {
            assert!(specular_dicom::sniff(&std::fs::read(path).unwrap()));
        }
    }
    let fetched = f.instance_requests.load(Ordering::SeqCst);
    assert_eq!(fetched, 21);

    // Again: everything from the cache, no instance requests.
    let (l, ev) = load(web(&f, Auth::None), &cache, &st.uid, &se.uid, 4);
    l.join();
    assert_eq!(instances(&ev.lock()).len(), 21);
    assert_eq!(f.instance_requests.load(Ordering::SeqCst), fetched);
    assert!(cache.stats().hits >= 21);

    // Server gone: metadata and slices from the cache, marked offline.
    let base = f.base();
    drop(f);
    let w = DicomWeb::new(client(), &base, Auth::None);
    let (l, ev) = load(w, &cache, &st.uid, &se.uid, 4);
    l.join();
    let ev = ev.lock();
    assert!(matches!(ev[0], LoadEvent::Metadata { offline: true, .. }));
    assert_eq!(instances(&ev).len(), 21);
    assert!(matches!(
        ev.last(),
        Some(LoadEvent::Done {
            failed: 0,
            offline: false,
            ..
        })
    ));
}

#[test]
fn progressive_follows_focus_and_cancels() {
    let cfg = FakeConfig::synthetic(1, 40, 64);
    let st = cfg.studies[0].clone();
    let se = st.series[0].clone();
    // ~8 KB per slice at 400 KB/s: ~20 ms each.
    let f = FakeServer::start(
        cfg,
        ServerOptions {
            latency: Duration::from_millis(5),
            bandwidth: Some(400_000),
        },
    )
    .unwrap();
    let dir = scratch("focus");
    let cache = DiskCache::open(&dir, 64 << 20).unwrap();
    let (l, ev) = load(web(&f, Auth::None), &cache, &st.uid, &se.uid, 1);
    let t = Instant::now();
    while instances(&ev.lock()).is_empty() {
        assert!(t.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
    l.set_focus(0);
    while instances(&ev.lock()).len() < 6 {
        assert!(t.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
    let order = instances(&ev.lock());
    // After the focus moved, the slices near 0 come next (one request may have been in
    // flight already).
    assert!(order[2..6].iter().all(|i| *i < 8), "{order:?}");
    l.cancel();
    let t = Instant::now();
    l.join();
    assert!(t.elapsed() < Duration::from_secs(2));
    let ev = ev.lock();
    assert!(instances(&ev).len() < 40);
    assert!(matches!(
        ev.last(),
        Some(LoadEvent::Done {
            cancelled: true,
            ..
        })
    ));
}

#[test]
fn progressive_reports_metadata_errors_and_auth() {
    let cfg = FakeConfig {
        auth: Some(FakeAuth::Bearer("t".into())),
        ..FakeConfig::synthetic(1, 3, 8)
    };
    let st = cfg.studies[0].clone();
    let f = fake(cfg);
    let cache = DiskCache::open(scratch("errors"), 1 << 20).unwrap();
    let (l, ev) = load(web(&f, Auth::None), &cache, &st.uid, &st.series[0].uid, 2);
    l.join();
    let ev = ev.lock();
    assert_eq!(ev.len(), 1);
    assert!(matches!(&ev[0], LoadEvent::Error(e) if e.is_auth()));
}

/// Time to first image and to the whole series: a 300-slice 512×512 CT over a link with
/// 50 ms latency and 100 Mbit/s. `cargo test -p specular-dicomweb --release -- --ignored
/// --nocapture measure_300_slice_ct`.
#[test]
#[ignore = "measurement (about 15 s)"]
fn measure_300_slice_ct() {
    for (label, bandwidth) in [("100 Mbit/s", 12_500_000u64), ("20 Mbit/s", 2_500_000)] {
        if label.starts_with("20") && std::env::var_os("DICOMWEB_SLOW").is_none() {
            continue;
        }
        let cfg = FakeConfig::synthetic(1, 300, 512);
        let st = cfg.studies[0].clone();
        let se = st.series[0].clone();
        let f = FakeServer::start(
            cfg,
            ServerOptions {
                latency: Duration::from_millis(50),
                bandwidth: Some(bandwidth),
            },
        )
        .unwrap();
        for parallel in [1, 6] {
            let cache = DiskCache::open(scratch(&format!("measure-{parallel}")), 1 << 30).unwrap();
            let t = Instant::now();
            let first: Arc<Mutex<(Option<Duration>, Option<Duration>)>> = Arc::default();
            let fr = first.clone();
            let l = SeriesLoad::start(
                web(&f, Auth::None),
                cache.clone(),
                "dicomweb/m",
                &st.uid,
                &se.uid,
                LoadOptions { parallel },
                Arc::new(move |e| {
                    let mut g = fr.lock();
                    match e {
                        LoadEvent::Metadata { .. } => g.0 = Some(t.elapsed()),
                        LoadEvent::Instance { .. } if g.1.is_none() => g.1 = Some(t.elapsed()),
                        _ => {}
                    }
                }),
            );
            l.join();
            let all = t.elapsed();
            let (meta, img) = *first.lock();
            println!(
                "{label}, {parallel} parallel: metadata {:?}, first image {:?}, all 300 {:?} ({:.1} MB)",
                meta.unwrap(),
                img.unwrap(),
                all,
                cache.stats().bytes as f64 / 1e6
            );
        }
    }
}

/// Against a real server, only when asked: `SPECULAR_ORTHANC_URL=http://localhost:8042/dicom-web
/// cargo test -p specular-dicomweb --test dicomweb orthanc -- --ignored --nocapture` (an Orthanc
/// with the DICOMweb plugin and at least one study, e.g. the `orthancteam/orthanc` image with
/// a demo study uploaded). Without the variable it does nothing.
#[test]
#[ignore = "needs a DICOMweb server (SPECULAR_ORTHANC_URL)"]
fn orthanc_demo() {
    let Ok(base) = std::env::var("SPECULAR_ORTHANC_URL") else {
        eprintln!("SPECULAR_ORTHANC_URL not set: skipped");
        return;
    };
    let auth = match (
        std::env::var("SPECULAR_ORTHANC_USER"),
        std::env::var("SPECULAR_ORTHANC_PASSWORD"),
    ) {
        (Ok(user), Ok(p)) => Auth::Basic {
            user,
            password: Secret::new(p),
        },
        _ => Auth::None,
    };
    let w = DicomWeb::new(Client::new(ClientConfig::default()).unwrap(), &base, auth);
    let cancel = CancelToken::new();
    let q = StudyQuery {
        limit: 5,
        ..StudyQuery::default()
    };
    let studies = w.search_studies(&q, &cancel).unwrap();
    eprintln!("{} studies (more: {})", studies.rows.len(), studies.more);
    let study = studies.rows.first().expect("the server has a study");
    let series = w.search_series(&study.study_uid, 10, 0, &cancel).unwrap();
    let s = series.rows.first().expect("the study has a series");
    eprintln!("series {} ({:?} images)", s.series_uid, s.instance_count);
    let cache = DiskCache::open(scratch("orthanc"), 1 << 30).unwrap();
    let t = Instant::now();
    let (l, ev) = load(w, &cache, &study.study_uid, &s.series_uid, 6);
    l.join();
    let ev = ev.lock();
    let done = ev
        .iter()
        .find(|e| matches!(e, LoadEvent::Done { .. }))
        .expect("the load ends");
    eprintln!(
        "{} images in {:?}: {done:?}",
        instances(&ev).len(),
        t.elapsed()
    );
    assert!(!instances(&ev).is_empty());
}
