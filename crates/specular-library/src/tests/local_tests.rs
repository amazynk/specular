//! Local copies (`local.rs`) with a "removable" location that goes away and comes back: the
//! copies stand in while it is offline, changes wait against the originals' identity, and
//! the check on return tells clean changes from conflicts (the original or a sidecar
//! changed meanwhile). The size limit, scopes sharing copies, and removal.

use super::*;
use crate::local::{ChangeKind, Check, CopyKind, KeepReport, LocalCopies, Mode, ProxyMaker, Scope};

fn copies(d: &Dir) -> LocalCopies {
    LocalCopies::open(
        &d.0.join("db/library.sqlite3"),
        d.0.join("data/local-copies"),
    )
    .unwrap()
}

fn keep(lc: &LocalCopies, scope: &Scope, mode: Mode, files: &[PathBuf]) -> KeepReport {
    let proxy: ProxyMaker = Arc::new(|_p: &Path, b: &[u8], edge: u32| {
        assert_eq!(edge, crate::local::PROXY_LONG_EDGE);
        // A stand-in proxy: a tenth of the raw.
        Some(b[..b.len() / 10 + 1].to_vec())
    });
    lc.keep(scope, "card", mode, files, Some(&proxy), &|| false, &|_| {})
        .unwrap()
}

fn offline(lib: &Library, want: bool) -> bool {
    eventually(|| lib.locations().first().is_some_and(|l| l.offline == want))
}

/// The card (`usb/DCIM`) is indexed and kept, unplugged (renamed away), edited from the
/// copies, changed elsewhere meanwhile, and plugged back in.
#[test]
fn a_removable_location_goes_away_and_comes_back() {
    let d = Dir::new("local-removable");
    let raw_body = format!("camera=X\n{}", "r".repeat(4000));
    let a = d.put("usb/DCIM/IMG_1.cr2", &raw_body);
    let b = d.put("usb/DCIM/IMG_2.jpg", "camera=Y\nsome jpeg");
    let c = d.put("usb/DCIM/IMG_3.jpg", "camera=Z\nanother jpeg");
    let e = d.put("usb/DCIM/IMG_4.jpg", "camera=W\nfourth");
    d.put("usb/DCIM/IMG_2.xmp", &SIDECAR.replace("RATING", "2"));
    let lib = open(&d, false);
    lib.add_location(&d.0.join("usb/DCIM"));
    idle(&lib);
    let loc = lib.locations()[0].id;
    let lc = copies(&d);
    let files = lc.files_of(&Scope::Location(loc)).unwrap();
    assert_eq!(files.len(), 4);
    let rep = keep(&lc, &Scope::Location(loc), Mode::Proxies, &files);
    assert_eq!(
        (rep.copied, rep.proxies, rep.unreachable),
        (4, 1, 0),
        "{rep:?}"
    );
    // The raw became a proxy, the JPEGs copies of themselves; all under the store.
    assert_eq!(lc.copy_of(&a).unwrap().kind, CopyKind::Proxy);
    assert_eq!(lc.copy_of(&b).unwrap().kind, CopyKind::Original);
    let cb = lc.resolve(&b).unwrap();
    assert!(cb.starts_with(lc.root()));
    assert_eq!(std::fs::read(&cb).unwrap(), std::fs::read(&b).unwrap());
    assert_eq!(lc.used(), rep.bytes);
    assert_eq!(lc.kept().unwrap()[0].files, 4);

    // Unplugged.
    std::fs::rename(d.0.join("usb"), d.0.join("usb-away")).unwrap();
    lib.rescan(None);
    assert!(offline(&lib, true), "the location never went offline");
    assert!(!b.exists());
    // The copies stand in, and a keep while offline keeps what is there.
    assert!(lc.resolve(&a).is_some() && lc.resolve(&b).is_some());
    let again = keep(&lc, &Scope::Location(loc), Mode::Proxies, &files);
    assert_eq!((again.copied, again.shared), (0, 4));
    // Changes made on the copies wait against the originals.
    let mark = |r| {
        crate::local::encode_marks(Mark {
            rating: r,
            label: 0,
            flag: 1,
        })
    };
    assert!(lc.enqueue(&a, ChangeKind::Marks, 0, &mark(3)).unwrap());
    assert!(lc.enqueue(&a, ChangeKind::Edit, 0, &[]).unwrap());
    assert!(lc.enqueue(&b, ChangeKind::Marks, 0, &mark(1)).unwrap());
    assert!(
        lc.enqueue(&b, ChangeKind::Marks, 0, &mark(5)).unwrap(),
        "replaces"
    );
    assert!(
        lc.enqueue(&c, ChangeKind::Keywords, 0, b"sun\nsea")
            .unwrap()
    );
    assert!(lc.enqueue(&e, ChangeKind::Marks, 0, &mark(4)).unwrap());
    assert!(
        !lc.enqueue(&d.0.join("elsewhere.jpg"), ChangeKind::Marks, 0, &mark(1))
            .unwrap()
    );
    assert_eq!(lc.waiting(), 5);
    let waiting = lc.changes_under(&d.0.join("usb")).unwrap();
    assert_eq!(waiting.len(), 5);
    let mb = waiting.iter().find(|w| w.path == b).unwrap();
    assert_eq!(crate::local::decode_marks(&mb.payload).unwrap().rating, 5);
    // Still offline: nothing can be checked yet.
    assert!(waiting.iter().all(|w| lc.check(w) == Check::Missing));

    // Meanwhile, on another computer: IMG_2's sidecar is rated again, IMG_3 is rewritten,
    // IMG_1 gets a Develop sidecar (its marks stay clean), IMG_4 is untouched.
    let away = d.0.join("usb-away/DCIM");
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(away.join("IMG_2.xmp"), SIDECAR.replace("RATING", "4")).unwrap();
    std::fs::write(away.join("IMG_3.jpg"), "camera=Z\nanother jpeg, edited").unwrap();
    std::fs::write(away.join("IMG_1.cr2.specular.xmp"), "<x:xmpmeta/>").unwrap();

    // Plugged back in.
    std::fs::rename(d.0.join("usb-away"), d.0.join("usb")).unwrap();
    lib.rescan(None);
    assert!(offline(&lib, false), "the location never came back");
    let by = |p: &Path, k: ChangeKind| {
        let w = waiting.iter().find(|w| w.path == p && w.kind == k).unwrap();
        lc.check(w)
    };
    assert_eq!(by(&a, ChangeKind::Marks), Check::Clean);
    assert_eq!(
        by(&a, ChangeKind::Edit),
        Check::Conflict {
            original: false,
            sidecar: true
        }
    );
    assert_eq!(
        by(&b, ChangeKind::Marks),
        Check::Conflict {
            original: false,
            sidecar: true
        }
    );
    assert_eq!(
        by(&c, ChangeKind::Keywords),
        Check::Conflict {
            original: true,
            sidecar: false
        }
    );
    assert_eq!(by(&e, ChangeKind::Marks), Check::Clean);
    // Written (or answered): they leave the queue, and the copy's base follows the disk.
    let ids: Vec<i64> = waiting.iter().map(|w| w.id).collect();
    lc.done(&ids).unwrap();
    assert_eq!(lc.waiting(), 0);
    lc.rebase(&b).unwrap();
    std::fs::rename(d.0.join("usb"), d.0.join("usb-away")).unwrap();
    assert!(lc.enqueue(&b, ChangeKind::Marks, 0, &mark(2)).unwrap());
    std::fs::rename(d.0.join("usb-away"), d.0.join("usb")).unwrap();
    let w = lc.changes().unwrap();
    assert_eq!(
        lc.check(&w[0]),
        Check::Clean,
        "checked against the rebased sidecar"
    );
    lc.done(&[w[0].id]).unwrap();

    // Back online, a keep makes again the copy whose original changed.
    let fresh = keep(&lc, &Scope::Location(loc), Mode::Proxies, &files);
    assert_eq!((fresh.copied, fresh.shared), (1, 3), "{fresh:?}");
    assert_eq!(
        std::fs::read(lc.resolve(&c).unwrap()).unwrap(),
        std::fs::read(&c).unwrap()
    );
    // Survives a restart.
    drop(lc);
    let lc = copies(&d);
    assert_eq!(lc.count(), 4);
    assert!(lc.resolve(&a).is_some());
}

/// Changes to files without a copy wait too, checked against the catalogue's time stamp of
/// their sidecar.
#[test]
fn changes_without_a_copy_are_checked_against_the_catalogue() {
    let d = Dir::new("local-nocopy");
    let a = d.put("usb/IMG_1.jpg", "camera=X");
    let b = d.put("usb/IMG_2.jpg", "camera=Y");
    d.put("usb/IMG_2.xmp", &SIDECAR.replace("RATING", "1"));
    let lib = open(&d, false);
    lib.add_location(&d.0.join("usb"));
    idle(&lib);
    let lc = copies(&d);
    let m = crate::local::encode_marks(Mark {
        rating: 5,
        ..Mark::default()
    });
    assert!(lc.enqueue(&a, ChangeKind::Marks, 0, &m).unwrap());
    assert!(lc.enqueue(&b, ChangeKind::Marks, 0, &m).unwrap());
    let w = lc.changes().unwrap();
    assert!(w.iter().all(|c| c.sidecars.is_none()));
    assert!(w.iter().all(|c| lc.check(c) == Check::Clean), "{w:?}");
    // A sidecar appears for one, the other's is rewritten.
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(d.0.join("usb/IMG_1.xmp"), SIDECAR.replace("RATING", "2")).unwrap();
    std::fs::write(d.0.join("usb/IMG_2.xmp"), SIDECAR.replace("RATING", "3")).unwrap();
    for c in &w {
        assert_eq!(
            lc.check(c),
            Check::Conflict {
                original: false,
                sidecar: true
            }
        );
    }
}

/// Scopes share copies; the size limit stops a keep; removing a scope deletes only the
/// copies no other scope holds, never the originals; originals replace proxies.
#[test]
fn scopes_share_copies_within_the_limit() {
    let d = Dir::new("local-scopes");
    let raw = d.put(
        "share/IMG_1.cr2",
        &format!("camera=X\n{}", "r".repeat(2000)),
    );
    let small = d.put("share/IMG_2.jpg", &"j".repeat(300));
    let big = d.put("share/IMG_3.jpg", &"J".repeat(5000));
    let lib = open(&d, false);
    lib.add_location(&d.0.join("share"));
    idle(&lib);
    let loc = lib.locations()[0].id;
    let lc = copies(&d);
    lc.set_limit(1000);
    let r = keep(
        &lc,
        &Scope::Selection,
        Mode::Proxies,
        &[raw.clone(), small.clone(), big.clone()],
    );
    assert_eq!((r.copied, r.over_limit), (2, 1), "{r:?}");
    assert!(lc.resolve(&big).is_none());
    assert!(lc.used() <= 1000);
    // The location wants originals: the proxy is made again whole, within a new limit.
    lc.set_limit(1 << 20);
    let all = lc.files_of(&Scope::Location(loc)).unwrap();
    let r2 = keep(&lc, &Scope::Location(loc), Mode::Originals, &all);
    assert_eq!((r2.copied, r2.shared), (2, 1), "{r2:?}");
    assert_eq!(lc.copy_of(&raw).unwrap().kind, CopyKind::Original);
    assert_eq!(lc.kept().unwrap().len(), 2);
    // The selection goes: its files stay, the location holds them.
    let sel = lc
        .kept()
        .unwrap()
        .into_iter()
        .find(|k| k.scope == Scope::Selection)
        .unwrap();
    assert_eq!(lc.remove(sel.id).unwrap(), 0);
    assert_eq!(lc.count(), 3);
    // The location goes: the store empties, the originals stay.
    let k = lc.kept_for(&Scope::Location(loc)).unwrap().unwrap();
    let copy = lc.resolve(&raw).unwrap();
    assert!(lc.remove(k.id).unwrap() > 0);
    assert_eq!((lc.count(), lc.used()), (0, 0));
    assert!(!copy.exists());
    assert!(raw.is_file() && small.is_file() && big.is_file());
    // A store file no copy names is tidied away at the next open.
    let stray = lc.root().join("00/stray.jpg");
    std::fs::create_dir_all(stray.parent().unwrap()).unwrap();
    std::fs::write(&stray, b"x").unwrap();
    drop(lc);
    let _lc = copies(&d);
    assert!(!stray.exists());
}
