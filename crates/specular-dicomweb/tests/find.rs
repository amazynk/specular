//! Study and series search by C-FIND against the loopback SCP: the QIDO search form, paging
//! by skip and cancel, and the rows the list shows.

use specular_dicomweb::StudyQuery;
use specular_dicomweb::find::{search_series, search_studies};
use specular_dimse::scp::{ScpConfig, TestScp};
use specular_dimse::{Options, Peer};
use specular_remote::CancelToken;

fn study(uid: &str, name: &str, date: &str, mods: &str) -> Vec<(u32, String)> {
    vec![
        (0x0008_0052, "STUDY".into()),
        (0x0020_000D, uid.into()),
        (0x0010_0010, name.into()),
        (0x0010_0020, format!("ID{uid}")),
        (0x0008_0020, date.into()),
        (0x0008_0030, "101500".into()),
        (0x0008_1030, format!("Study {uid}")),
        (0x0008_0061, mods.into()),
        (0x0020_1206, "2".into()),
        (0x0020_1208, "40".into()),
    ]
}

fn series(study: &str, uid: &str, n: u32, modality: &str) -> Vec<(u32, String)> {
    vec![
        (0x0008_0052, "SERIES".into()),
        (0x0020_000D, study.into()),
        (0x0020_000E, uid.into()),
        (0x0020_0011, n.to_string()),
        (0x0008_0060, modality.into()),
        (0x0008_103E, format!("Series {n}")),
        (0x0020_1209, "20".into()),
    ]
}

fn scp() -> TestScp {
    let mut records = vec![
        study("1.2.1", "FAKE^ALPHA", "20240105", "CT"),
        study("1.2.2", "FAKE^BRAVO", "20240210", "MR"),
        study("1.2.3", "TEST^ECHO", "20240301", "CT\\SR"),
        study("1.2.4", "Müller^Jörg", "20240302", "US"),
    ];
    for i in 1..=4 {
        records.push(series("1.2.1", &format!("1.2.1.{i}"), 5 - i, "CT"));
    }
    records.push(series("1.2.2", "1.2.2.1", 1, "MR"));
    TestScp::start(ScpConfig {
        records,
        ..ScpConfig::default()
    })
    .unwrap()
}

fn peer(port: u16) -> Peer {
    Peer {
        host: "127.0.0.1".into(),
        port,
        called_ae: "TESTSCP".into(),
        calling_ae: "SPECULAR".into(),
    }
}

fn uids(rows: &[specular_dicomweb::StudyRow]) -> Vec<&str> {
    rows.iter().map(|r| r.study_uid.as_str()).collect()
}

#[test]
fn studies_by_the_search_form() {
    let s = scp();
    let (p, o, c) = (peer(s.port), Options::default(), CancelToken::new());
    let all = search_studies(&p, &o, &StudyQuery::default(), &c).unwrap();
    assert_eq!(uids(&all.rows), ["1.2.1", "1.2.2", "1.2.3", "1.2.4"]);
    assert!(!all.more);
    let r = &all.rows[0];
    assert_eq!(r.date, "20240105");
    assert_eq!(r.time, "101500");
    assert_eq!(r.description, "Study 1.2.1");
    assert_eq!(r.modalities, ["CT"]);
    assert_eq!(r.series_count, Some(2));
    assert_eq!(r.instance_count, Some(40));
    assert_eq!(r.patient.name, "FAKE^ALPHA");
    assert_eq!(r.patient.id, "ID1.2.1");
    assert_eq!(all.rows[2].modalities, ["CT", "SR"]);
    // UTF-8 names both ways.
    assert_eq!(all.rows[3].patient.name, "Müller^Jörg");
    let q = |f: &dyn Fn(&mut StudyQuery)| {
        let mut q = StudyQuery::default();
        f(&mut q);
        search_studies(&p, &o, &q, &c).unwrap().rows
    };
    // A prefix of the name, without regard to case.
    assert_eq!(
        uids(&q(&|q| q.patient_name = "fake".into())),
        ["1.2.1", "1.2.2"]
    );
    assert_eq!(uids(&q(&|q| q.patient_name = "Mül".into())), ["1.2.4"]);
    assert_eq!(uids(&q(&|q| q.modality = "CT".into())), ["1.2.1", "1.2.3"]);
    assert_eq!(
        uids(&q(&|q| {
            q.date_from = "2024-02-01".into();
            q.date_to = "2024-03-01".into();
        })),
        ["1.2.2", "1.2.3"]
    );
    assert_eq!(uids(&q(&|q| q.date_from = "20240302".into())), ["1.2.4"]);
}

#[test]
fn pages_skip_and_cancel() {
    let s = scp();
    let (p, o, c) = (peer(s.port), Options::default(), CancelToken::new());
    let first = search_studies(
        &p,
        &o,
        &StudyQuery {
            limit: 3,
            ..StudyQuery::default()
        },
        &c,
    )
    .unwrap();
    assert_eq!(uids(&first.rows), ["1.2.1", "1.2.2", "1.2.3"]);
    assert!(first.more);
    let next = search_studies(
        &p,
        &o,
        &StudyQuery {
            limit: 3,
            offset: first.next_offset(),
            ..StudyQuery::default()
        },
        &c,
    )
    .unwrap();
    assert_eq!(uids(&next.rows), ["1.2.4"]);
    assert!(!next.more);
    assert_eq!(next.offset, 3);
}

#[test]
fn series_of_a_study_by_number() {
    let s = scp();
    let (p, o, c) = (peer(s.port), Options::default(), CancelToken::new());
    let page = search_series(&p, &o, "1.2.1", 0, 0, &c).unwrap();
    let got: Vec<(&str, Option<i64>)> = page
        .rows
        .iter()
        .map(|r| (r.series_uid.as_str(), r.number))
        .collect();
    assert_eq!(
        got,
        [
            ("1.2.1.4", Some(1)),
            ("1.2.1.3", Some(2)),
            ("1.2.1.2", Some(3)),
            ("1.2.1.1", Some(4))
        ]
    );
    assert_eq!(page.rows[0].modality, "CT");
    assert_eq!(page.rows[0].instance_count, Some(20));
    assert_eq!(page.rows[0].study_uid, "1.2.1");
    assert_eq!(
        search_series(&p, &o, "1.2.9", 0, 0, &c).unwrap().rows.len(),
        0
    );
}

#[test]
fn a_server_without_query_says_so() {
    let s = TestScp::start(ScpConfig {
        sop_classes: vec!["1.2.840.10008.5.1.4.1.1.2".into()],
        ..ScpConfig::default()
    })
    .unwrap();
    let e = search_studies(
        &peer(s.port),
        &Options::default(),
        &StudyQuery::default(),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(e, specular_dimse::Error::NothingAccepted), "{e}");
}
