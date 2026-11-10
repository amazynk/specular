//! An import: the plan (what goes where), the free-space check, and the copy loop.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::Key;
use crate::copy::{Control, CopyError, copy_verified, hash_file};
use crate::ledger::{Entry, Ledger};
use crate::pattern::{Facts, FolderPattern, RenamePattern};
use crate::scan::{MediaKind, Shot};

/// Room kept free on the destination beyond the import itself.
pub const MARGIN: u64 = 32 << 20;

/// How an import lays files out.
#[derive(Clone, Debug)]
pub struct Options {
    pub dest_root: PathBuf,
    pub folder: FolderPattern,
    /// `None` keeps the camera's names.
    pub rename: Option<RenamePattern>,
    pub event: String,
    /// A RAW stays in the same folder as its JPEG (otherwise it goes in a `RAW` folder beside
    /// it). Either way the two keep one name.
    pub keep_pairs: bool,
    /// Remove each shot from the source once all its files are verified at the destination.
    pub delete_after: bool,
}

/// How a sidecar's name follows its file's final name.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Follow {
    /// `IMG_0001.CR2.xmp`: the file's whole name, then this (`.xmp`).
    Name(String),
    /// `IMG_0001.xmp`: the file's name without its extension, then this.
    Stem(String),
}

/// One file to copy.
#[derive(Clone, Debug)]
pub struct PlanFile {
    pub src: PathBuf,
    pub size: u64,
    pub key: Key,
    pub dir: PathBuf,
    pub name: String,
    sidecars: Vec<(PathBuf, Follow)>,
    /// Index into [`Plan::shots`].
    pub shot: usize,
}

/// A shot's files on the source, for delete-after.
#[derive(Clone, Debug, Default)]
pub struct PlanShot {
    /// Every member's key and path, imported before or not.
    pub members: Vec<(Key, PathBuf)>,
    pub sidecars: Vec<PathBuf>,
}

/// What an import will do.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub files: Vec<PlanFile>,
    pub shots: Vec<PlanShot>,
    /// Bytes to copy, sidecars included.
    pub bytes: u64,
    pub delete_after: bool,
}

/// The file name of `p`, split at its last dot.
fn split_name(p: &Path) -> (String, String) {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_owned(), name[i + 1..].to_owned()),
        _ => (name, String::new()),
    }
}

fn stem_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

impl Options {
    /// The folder a shot goes to.
    pub fn folder_for(&self, shot: &Shot) -> PathBuf {
        let f = Facts {
            date: shot.date,
            camera: shot.camera.as_deref().unwrap_or(""),
            event: &self.event,
        };
        self.dest_root.join(self.folder.render(&f))
    }

    /// Where a file of a shot would go, for the sheet's preview line.
    pub fn preview(&self, shot: &Shot, index: usize) -> PathBuf {
        let m = &shot.members[0];
        let dir = self.member_dir(shot, m.kind);
        dir.join(self.member_name(shot, &m.path, index))
    }

    fn member_dir(&self, shot: &Shot, kind: MediaKind) -> PathBuf {
        let dir = self.folder_for(shot);
        if !self.keep_pairs && kind == MediaKind::Raw && shot.is_pair() {
            dir.join("RAW")
        } else {
            dir
        }
    }

    fn member_name(&self, shot: &Shot, path: &Path, index: usize) -> String {
        let (stem, ext) = split_name(path);
        match &self.rename {
            Some(r) => r.render(
                &stem,
                &ext,
                index,
                &Facts {
                    date: shot.date,
                    camera: shot.camera.as_deref().unwrap_or(""),
                    event: &self.event,
                },
            ),
            None => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }
}

/// Plans copying the new files of `shots` (in that order: the rename counter follows it).
pub fn plan(shots: &[Shot], ledger: &Ledger, opts: &Options) -> Plan {
    let mut p = Plan {
        delete_after: opts.delete_after,
        ..Plan::default()
    };
    for (i, shot) in shots.iter().enumerate() {
        let mut ps = PlanShot::default();
        for m in &shot.members {
            ps.members.push((m.key, m.path.clone()));
            ps.sidecars.extend(m.sidecars.iter().cloned());
            if ledger.contains(&m.key) {
                continue;
            }
            let orig = m
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let sidecars: Vec<(PathBuf, Follow)> = m
                .sidecars
                .iter()
                .map(|s| {
                    let sn = s
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let follow = if sn.to_lowercase().starts_with(&format!("{orig}.")) {
                        Follow::Name(sn[orig.len()..].to_owned())
                    } else {
                        let (_, e) = split_name(s);
                        Follow::Stem(format!(".{e}"))
                    };
                    (s.clone(), follow)
                })
                .collect();
            p.bytes += m.size
                + m.sidecars
                    .iter()
                    .filter_map(|s| std::fs::metadata(s).ok())
                    .map(|md| md.len())
                    .sum::<u64>();
            p.files.push(PlanFile {
                src: m.path.clone(),
                size: m.size,
                key: m.key,
                dir: opts.member_dir(shot, m.kind),
                name: opts.member_name(shot, &m.path, i + 1),
                sidecars,
                shot: i,
            });
        }
        p.shots.push(ps);
    }
    p
}

/// Where an import stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub files_done: usize,
    pub files_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// The file being copied.
    pub current: String,
    /// Time left at the rate so far (not counting pauses), once there is a rate.
    pub eta: Option<Duration>,
}

/// What an import did.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Files copied (sidecars not counted).
    pub copied: usize,
    /// Files whose bytes were already at the destination under their name.
    pub existed: usize,
    pub failed: Vec<(PathBuf, String)>,
    /// Source files removed after verification (sidecars included).
    pub deleted: usize,
    pub cancelled: bool,
    /// Bytes written.
    pub bytes: u64,
    /// The folders files went to.
    pub folders: BTreeSet<PathBuf>,
    /// The first file at the destination (to open in the gallery).
    pub first: Option<PathBuf>,
    /// Every file verified at the destination this run, copied or already there (sidecars
    /// not listed), in plan order: what metadata and a Develop style go to afterwards.
    pub files: Vec<PathBuf>,
    pub elapsed: Duration,
}

/// Why an import did not start.
#[derive(Debug)]
pub enum RunError {
    /// The destination lacks room: `need` bytes (with a margin), `free` available.
    NoSpace { need: u64, free: u64 },
    /// The destination folder cannot be made.
    Destination(PathBuf, std::io::Error),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::NoSpace { need, free } => {
                write!(f, "not enough free space: needs {need} bytes, {free} free")
            }
            RunError::Destination(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for RunError {}

/// Refuses a plan that does not fit in `free` bytes.
pub fn check_space(plan: &Plan, free: Option<u64>) -> Result<(), RunError> {
    let need = plan.bytes.saturating_add(MARGIN);
    match free {
        Some(free) if free < need && plan.bytes > 0 => Err(RunError::NoSpace { need, free }),
        _ => Ok(()),
    }
}

/// A ledger entry still stands for a verified copy: the file it went to is there with that
/// content.
fn still_there(e: &Entry) -> bool {
    std::fs::metadata(&e.dest).is_ok_and(|m| m.len() == e.size)
        && hash_file(&e.dest).is_ok_and(|h| h == e.hash)
}

/// Runs `plan` (after [`check_space`] against `free`), recording each verified file in
/// `ledger` as it goes. `progress` is called a few times a second.
pub fn run(
    plan: &Plan,
    dest_root: &Path,
    ledger: &Mutex<Ledger>,
    ctl: &Control,
    free: Option<u64>,
    progress: &mut dyn FnMut(&Progress),
) -> Result<Report, RunError> {
    check_space(plan, free)?;
    std::fs::create_dir_all(dest_root).map_err(|e| RunError::Destination(dest_root.into(), e))?;
    let start = Instant::now();
    let mut report = Report::default();
    let mut p = Progress {
        files_total: plan.files.len(),
        bytes_total: plan.bytes,
        ..Progress::default()
    };
    let mut verified: HashSet<Key> = HashSet::new();
    let mut failed_shots: HashSet<usize> = HashSet::new();
    let mut active = Duration::ZERO;
    let mut last = Instant::now();
    let mut shown = Instant::now() - Duration::from_secs(1);
    let mut copied_now = 0u64;

    let mut tick = |p: &mut Progress, n: u64, force: bool| {
        p.bytes_done += n;
        copied_now += n;
        let now = Instant::now();
        // A pause blocks inside the copy; its gap is not copying time.
        active += (now - last).min(Duration::from_secs(2));
        last = now;
        if copied_now >= 1 << 20 && active >= Duration::from_millis(500) {
            let rate = copied_now as f64 / active.as_secs_f64();
            let left = p.bytes_total.saturating_sub(p.bytes_done) as f64;
            p.eta = Some(Duration::from_secs_f64(left / rate.max(1.0)));
        }
        if force || now - shown >= Duration::from_millis(100) {
            shown = now;
            progress(p);
        }
    };

    for f in &plan.files {
        if ctl.is_cancelled() {
            report.cancelled = true;
            break;
        }
        p.current = f
            .src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        tick(&mut p, 0, true);
        let main = copy_verified(&f.src, &f.dir, &f.name, ctl, &mut |n| {
            tick(&mut p, n, false)
        });
        let c = match main {
            Ok(c) => c,
            Err(CopyError::Cancelled) => {
                report.cancelled = true;
                break;
            }
            Err(e) => {
                report.failed.push((f.src.clone(), e.to_string()));
                failed_shots.insert(f.shot);
                p.files_done += 1;
                continue;
            }
        };
        if c.existed {
            report.existed += 1;
        } else {
            report.copied += 1;
            report.bytes += f.size;
        }
        report.folders.insert(f.dir.clone());
        if report.first.is_none() {
            report.first = Some(c.dest.clone());
        }
        report.files.push(c.dest.clone());
        let entry = Entry {
            hash: c.hash,
            size: f.size,
            dest: c.dest.clone(),
        };
        let recorded = ledger
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .record(f.key, entry);
        if let Err(e) = recorded {
            report.failed.push((f.src.clone(), e.to_string()));
            failed_shots.insert(f.shot);
        }
        verified.insert(f.key);

        let final_name = c
            .dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dir = c.dest.parent().unwrap_or(&f.dir).to_path_buf();
        for (s, follow) in &f.sidecars {
            let name = match follow {
                Follow::Name(rest) => format!("{final_name}{rest}"),
                Follow::Stem(ext) => format!("{}{ext}", stem_of(&final_name)),
            };
            match copy_verified(s, &dir, &name, ctl, &mut |n| tick(&mut p, n, false)) {
                Ok(_) => {}
                Err(CopyError::Cancelled) => {
                    report.cancelled = true;
                    break;
                }
                Err(e) => {
                    report.failed.push((s.clone(), e.to_string()));
                    failed_shots.insert(f.shot);
                }
            }
        }
        p.files_done += 1;
        if report.cancelled {
            break;
        }
    }
    tick(&mut p, 0, true);

    if plan.delete_after && !report.cancelled {
        let l = ledger.lock().unwrap_or_else(|e| e.into_inner());
        for (i, s) in plan.shots.iter().enumerate() {
            if failed_shots.contains(&i) {
                continue;
            }
            let ok = s
                .members
                .iter()
                .all(|(k, _)| verified.contains(k) || l.get(k).is_some_and(still_there));
            if !ok {
                continue;
            }
            for path in s.members.iter().map(|(_, p)| p).chain(&s.sidecars) {
                if std::fs::remove_file(path).is_ok() {
                    report.deleted += 1;
                }
            }
        }
    }
    report.elapsed = start.elapsed();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::*;
    use crate::copy::partial_path;
    use crate::ledger::FILE_NAME;
    use crate::scan;
    use crate::testutil::{card, fresh};

    fn opts(dest: &Path) -> Options {
        Options {
            dest_root: dest.to_path_buf(),
            folder: FolderPattern::parse("{year}/{year}-{month}-{day}").unwrap(),
            rename: None,
            event: String::new(),
            keep_pairs: true,
            delete_after: false,
        }
    }

    fn found(card: &Path, ledger: &Ledger) -> scan::Found {
        scan::scan(
            &[card.join("DCIM")],
            false,
            ledger,
            &AtomicBool::new(false),
            &mut |_, _| {},
        )
    }

    fn rel(root: &Path, mut v: Vec<PathBuf>) -> Vec<String> {
        v.sort();
        v.iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    fn files_under(root: &Path) -> Vec<String> {
        fn walk(d: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                if e.path().is_dir() {
                    walk(&e.path(), out);
                } else {
                    out.push(e.path());
                }
            }
        }
        let mut v = Vec::new();
        walk(root, &mut v);
        rel(root, v)
    }

    fn run_all(found: &scan::Found, ledger: &Mutex<Ledger>, o: &Options) -> Report {
        let p = plan(&found.new, &ledger.lock().unwrap(), o);
        run(
            &p,
            &o.dest_root,
            ledger,
            &Control::default(),
            None,
            &mut |_| {},
        )
        .unwrap()
    }

    #[test]
    fn imports_lay_out_and_dedupe_across_runs_and_renames() {
        let root = fresh("run");
        let c = card(&root);
        let dest = root.join("Pictures");
        let lpath = root.join("data").join(FILE_NAME);
        let ledger = Mutex::new(Ledger::open(&lpath).unwrap());
        let f = found(&c, &ledger.lock().unwrap());
        assert_eq!(f.new.len(), 4);
        let r = run_all(&f, &ledger, &opts(&dest));
        assert_eq!((r.copied, r.existed, r.failed.len()), (5, 0, 0));
        assert_eq!(r.first, Some(dest.join("2024/2024-03-09/IMG_0001.CR2")));
        let video_day = {
            let d = crate::exif::local(chrono::DateTime::from_timestamp(1_710_061_200, 0).unwrap())
                .unwrap();
            format!("2024/{:04}-{:02}-{:02}", d[0], d[1], d[2])
        };
        let mut want = vec![
            "2024/2024-03-09/IMG_0001.CR2".to_owned(),
            "2024/2024-03-09/IMG_0001.CR2.xmp".to_owned(),
            "2024/2024-03-09/IMG_0001.JPG".to_owned(),
            "2024/2024-03-10/IMG_0002.JPG".to_owned(),
            "2024/2024-03-10/IMG_0101.JPG".to_owned(),
            format!("{video_day}/MVI_0003.MP4"),
            format!("{video_day}/MVI_0003.THM"),
        ];
        want.sort();
        assert_eq!(files_under(&dest), want);
        // The verified files, sidecars left out (they follow their files).
        let mut verified = rel(&dest, r.files.clone());
        verified.sort();
        let mains: Vec<String> = want
            .iter()
            .filter(|w| !w.ends_with(".xmp") && !w.ends_with(".THM"))
            .cloned()
            .collect();
        assert_eq!(verified, mains);
        for (a, b) in [
            ("DCIM/100CANON/IMG_0001.CR2", "2024/2024-03-09/IMG_0001.CR2"),
            (
                "DCIM/100CANON/IMG_0001.CR2.xmp",
                "2024/2024-03-09/IMG_0001.CR2.xmp",
            ),
        ] {
            assert_eq!(
                std::fs::read(c.join(a)).unwrap(),
                std::fs::read(dest.join(b)).unwrap()
            );
        }

        // Again: nothing new.
        let again = found(&c, &ledger.lock().unwrap());
        assert!(again.new.is_empty());
        assert_eq!(again.already, 4);

        // Renamed and moved at the destination, and the ledger read back from disk: still
        // nothing new.
        std::fs::rename(
            dest.join("2024/2024-03-10/IMG_0002.JPG"),
            dest.join("2024/best.jpg"),
        )
        .unwrap();
        drop(ledger);
        let ledger = Ledger::open(&lpath).unwrap();
        assert_eq!(ledger.len(), 5);
        assert!(found(&c, &ledger).new.is_empty());

        // A card with one more picture offers just that one.
        crate::testutil::write(
            &c.join("DCIM/100CANON/IMG_0005.JPG"),
            &crate::testutil::exif_jpeg_seeded(
                "Canon",
                "Canon EOS R5",
                "2024:03:11 08:00:00",
                3,
                77,
            ),
        );
        let more = found(&c, &ledger);
        assert_eq!(more.new.len(), 1);
        assert_eq!(more.new[0].preview().file_name().unwrap(), "IMG_0005.JPG");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn files_already_at_the_destination_are_not_copied_again() {
        let root = fresh("present");
        let c = card(&root);
        let dest = root.join("Pictures");
        // Copied by hand earlier, with no ledger.
        let d = dest.join("2024/2024-03-10");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::copy(c.join("DCIM/100CANON/IMG_0002.JPG"), d.join("IMG_0002.JPG")).unwrap();
        let ledger = Mutex::new(Ledger::in_memory());
        let f = found(&c, &ledger.lock().unwrap());
        let r = run_all(&f, &ledger, &opts(&dest));
        assert_eq!((r.copied, r.existed), (4, 1));
        assert!(!d.join("IMG_0002-1.JPG").exists());
        assert!(
            ledger.lock().unwrap().len() == 5,
            "found copies are recorded too"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_imports_resume() {
        let root = fresh("interrupt");
        let c = card(&root);
        let dest = root.join("Pictures");
        let lpath = root.join("data").join(FILE_NAME);
        let ledger = Mutex::new(Ledger::open(&lpath).unwrap());
        let f = found(&c, &ledger.lock().unwrap());
        let o = opts(&dest);
        let p = plan(&f.new, &ledger.lock().unwrap(), &o);
        // Cancel once the second file has started.
        let ctl = Control::default();
        let r = run(&p, &dest, &ledger, &ctl, None, &mut |pr| {
            if pr.files_done >= 1 && pr.current == "IMG_0001.JPG" {
                ctl.cancel();
            }
        })
        .unwrap();
        assert!(r.cancelled);
        assert_eq!(r.copied, 1);
        // A half-written next file, as a crash would leave it.
        let next = dest.join("2024/2024-03-10/IMG_0002.JPG");
        let src = std::fs::read(c.join("DCIM/100CANON/IMG_0002.JPG")).unwrap();
        crate::testutil::write(&partial_path(&next), &src[..src.len() / 2]);
        drop(ledger);

        // Next session: the ledger from disk, a fresh scan, and a run that finishes the rest.
        let ledger = Mutex::new(Ledger::open(&lpath).unwrap());
        let f = found(&c, &ledger.lock().unwrap());
        assert_eq!(
            f.new.len(),
            4,
            "the pair is still new: its JPEG was not copied"
        );
        let p = plan(&f.new, &ledger.lock().unwrap(), &o);
        assert_eq!(p.files.len(), 4);
        let r = run(&p, &dest, &ledger, &Control::default(), None, &mut |_| {}).unwrap();
        assert_eq!(
            (r.copied, r.existed, r.failed.len(), r.cancelled),
            (4, 0, 0, false)
        );
        assert_eq!(std::fs::read(&next).unwrap(), src);
        assert!(
            !files_under(&dest)
                .iter()
                .any(|f| f.contains(".specular-import")),
            "no temporary files left"
        );
        assert_eq!(files_under(&dest).len(), 7);
        assert!(found(&c, &ledger.lock().unwrap()).new.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_without_free_space() {
        let root = fresh("space");
        let c = card(&root);
        let dest = root.join("Pictures");
        let ledger = Mutex::new(Ledger::in_memory());
        let f = found(&c, &ledger.lock().unwrap());
        let p = plan(&f.new, &ledger.lock().unwrap(), &opts(&dest));
        assert!(p.bytes > 200_000);
        let r = run(
            &p,
            &dest,
            &ledger,
            &Control::default(),
            Some(p.bytes),
            &mut |_| {},
        );
        match r {
            Err(RunError::NoSpace { need, free }) => {
                assert_eq!(free, p.bytes);
                assert_eq!(need, p.bytes + MARGIN);
            }
            other => panic!("{other:?}"),
        }
        assert!(!dest.exists(), "nothing written");
        assert!(check_space(&p, Some(p.bytes + MARGIN)).is_ok());
        assert!(
            check_space(&p, None).is_ok(),
            "unknown free space does not block"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn renames_split_pairs_and_delete_after() {
        let root = fresh("rename");
        let c = card(&root);
        let dest = root.join("Pictures");
        let ledger = Mutex::new(Ledger::in_memory());
        let f = found(&c, &ledger.lock().unwrap());
        let o = Options {
            folder: FolderPattern::parse("{camera}/{event}").unwrap(),
            rename: RenamePattern::parse("{date:%Y%m%d}-{n:03}").unwrap(),
            event: "Trip".into(),
            keep_pairs: false,
            delete_after: true,
            ..opts(&dest)
        };
        // Only the pair and the video are selected.
        let chosen: Vec<Shot> = f
            .new
            .iter()
            .filter(|s| s.is_pair() || s.has(MediaKind::Video))
            .cloned()
            .collect();
        let p = plan(&chosen, &ledger.lock().unwrap(), &o);
        let r = run(&p, &dest, &ledger, &Control::default(), None, &mut |_| {}).unwrap();
        assert_eq!(r.failed, []);
        let vd = &chosen[1].date;
        let vname = format!("{:04}{:02}{:02}-002", vd[0], vd[1], vd[2]);
        let mut want = vec![
            "Canon EOS R5/Trip/RAW/20240309-001.CR2".to_owned(),
            "Canon EOS R5/Trip/RAW/20240309-001.CR2.xmp".to_owned(),
            "Canon EOS R5/Trip/20240309-001.JPG".to_owned(),
            format!("Trip/{vname}.MP4"),
            format!("Trip/{vname}.THM"),
        ];
        want.sort();
        assert_eq!(files_under(&dest), want);
        // The selected shots left the card (sidecars too); the rest stayed.
        let left = files_under(&c);
        assert_eq!(
            left,
            [
                "DCIM/100CANON/IMG_0002.JPG",
                "DCIM/100CANON/MVI_0003.LRV",
                "DCIM/100CANON/NOTES.TXT",
                "DCIM/101CANON/IMG_0101.JPG",
                "MISC/AUTPRINT.MRK"
            ]
        );
        assert_eq!(r.deleted, 5);
        let _ = std::fs::remove_dir_all(root);
    }
}
