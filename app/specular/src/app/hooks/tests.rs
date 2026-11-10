use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use serde_json::json;
use specular_settings::commands::parse_hooks;

use super::*;
use crate::remote::events::{Bus, Event};

fn hooks(text: &str) -> Vec<Hook> {
    let (h, problems) = parse_hooks(&text.parse().unwrap());
    assert!(problems.is_empty(), "{problems:?}");
    h
}

const UPLOAD: &str = r#"
[[hooks]]
name = "Upload picks"
on = ["marked"]
when = { flag = "pick" }
program = "/home/u/bin/upload"
"#;

/// A test folder of this process (the build's temporary folder).
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("specular-hooks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn line(e: &Event) -> Line {
    let bus = Bus::new();
    bus.publish(e, &Source::User.members()).remove(0)
}

fn marked(files: &[&str], flag: &str, stars: u8) -> Line {
    let files = files.iter().map(PathBuf::from).collect();
    line(
        &Event::new("marked", files)
            .with("mark", json!({"flag": flag, "stars": stars, "label": null})),
    )
}

#[test]
fn fingerprints_follow_what_runs() {
    let h = hooks(UPLOAD).remove(0);
    let fp = fingerprint(&h);
    let mut renamed = h.clone();
    renamed.name = "Other name".into();
    renamed.timeout = 5;
    renamed.enabled = false;
    assert_eq!(
        fingerprint(&renamed),
        fp,
        "name, timeout and switch do not ask again"
    );
    for change in [
        |h: &mut Hook| h.program = "/home/u/bin/upload2".into(),
        |h: &mut Hook| h.args.push("--all".into()),
        |h: &mut Hook| h.on.push("exported".into()),
    ] {
        let mut c = h.clone();
        change(&mut c);
        assert_ne!(fingerprint(&c), fp, "{c:?}");
    }
}

#[test]
fn filters_and_jobs() {
    let h = hooks(UPLOAD).remove(0);
    assert!(passes(&h, &marked(&["/a"], "pick", 3).value));
    assert!(!passes(&h, &marked(&["/a"], "reject", 3).value));
    let stars = hooks(
        "[[hooks]]\nname = \"S\"\non = [\"marked\"]\nwhen = { \"mark.stars\" = 5, label = \"none\" }\nprogram = \"x\"\n",
    )
    .remove(0);
    assert!(passes(&stars, &marked(&["/a"], "pick", 5).value));
    assert!(!passes(&stars, &marked(&["/a"], "pick", 4).value));

    let h = hooks(
        "[[hooks]]\nname = \"Log\"\non = [\"*\"]\nprogram = \"/bin/log\"\nargs = [\"{event}\", \"{stars}\", \"{files}\"]\ntimeout = 9\n",
    )
    .remove(0);
    let l = marked(&["/p/a b.cr3", "/p/$(x);y.cr3"], "pick", 4);
    let j = job(&h, &l, Some(Path::new("/run/specular.sock"))).unwrap();
    assert_eq!(j.program, "/bin/log");
    assert_eq!(
        j.args,
        ["marked", "4", "/p/a b.cr3", "/p/$(x);y.cr3"].map(OsString::from)
    );
    assert_eq!(j.timeout, Duration::from_secs(9));
    let input: Value = serde_json::from_slice(&j.input).unwrap();
    assert_eq!(input["event"], "marked");
    assert_eq!(*j.input.last().unwrap(), b'\n');
    let env: HashMap<&str, OsString> = j.env.iter().cloned().collect();
    assert_eq!(env["SPECULAR_EVENT"], "marked");
    assert_eq!(env["SPECULAR_HOOK"], "Log");
    assert_eq!(env["SPECULAR_SOCKET"], "/run/specular.sock");
}

/// The programs a fake runner "runs", with how to end each.
type Live = Arc<Mutex<Vec<(String, OnExit)>>>;

/// A runner whose programs "run" until the test lets them end.
fn fake() -> (Arc<Runner>, Receiver<Note>, Live) {
    let live: Live = Arc::default();
    let l2 = live.clone();
    let (r, notes) = Runner::new(Box::new(move |j: &Job, done: OnExit| {
        l2.lock().push((j.name.clone(), done));
        Ok(())
    }));
    (r, notes, live)
}

fn fake_job(name: &str, key: u64) -> Job {
    Job {
        key,
        name: name.into(),
        program: "x".into(),
        args: Vec::new(),
        input: Vec::new(),
        env: Vec::new(),
        timeout: Duration::from_secs(1),
    }
}

/// Ends the first run of `name` (any when `None`).
fn end(live: &Mutex<Vec<(String, OnExit)>>, name: Option<&str>, code: i32) {
    let done = {
        let mut l = live.lock();
        let i = l
            .iter()
            .position(|(n, _)| name.is_none_or(|w| w == n))
            .unwrap();
        l.remove(i).1
    };
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(code << 8);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(code as u32);
    done(Ok(status));
}

#[test]
fn the_limits_hold() {
    let (r, notes, live) = fake();
    for _ in 0..5 {
        r.submit(fake_job("a", 1));
    }
    assert_eq!(live.lock().len(), PER_HOOK, "two of one hook at once");
    for _ in 0..5 {
        r.submit(fake_job("b", 2));
    }
    r.submit(fake_job("c", 3));
    assert_eq!(live.lock().len(), TOTAL, "four in all");
    assert_eq!(r.running(), TOTAL);
    // One of a's ends: the next waiting job with room starts (b's, or a's own).
    end(&live, Some("a"), 0);
    assert_eq!(live.lock().len(), TOTAL);
    // Everything drains, in turns, never above the limits.
    let mut seen = 0;
    while !live.lock().is_empty() {
        {
            let l = live.lock();
            assert!(l.len() <= TOTAL);
            for n in ["a", "b", "c"] {
                assert!(l.iter().filter(|(x, _)| x == n).count() <= PER_HOOK);
            }
        }
        end(&live, None, 0);
        seen += 1;
    }
    assert_eq!(seen + 1, 11);
    assert!(notes.try_recv().is_err(), "all ended well");
    assert_eq!(r.running(), 0);
}

#[test]
fn a_full_queue_drops_and_says_so() {
    let (r, notes, live) = fake();
    for _ in 0..(PER_HOOK + QUEUE + 3) {
        r.submit(fake_job("a", 1));
    }
    assert_eq!(live.lock().len(), PER_HOOK);
    let dropped = notes
        .try_iter()
        .filter(|n| matches!(n, Note::Dropped { .. }))
        .count();
    assert_eq!(dropped, 3);
    end(&live, None, 3);
    assert_eq!(
        notes.try_recv().unwrap(),
        Note::Failed {
            name: "a".into(),
            why: Why::Exit(3)
        }
    );
    assert!(
        note_text(&Note::Failed {
            name: "a".into(),
            why: Why::Exit(3)
        })
        .contains('3')
    );
}

#[test]
fn nothing_runs_before_consent_and_a_change_asks_again() {
    let (runner, notes, _live) = fake();
    let mut hs = Hooks {
        runner,
        notes,
        ..Hooks::default()
    };
    let h = hooks(UPLOAD).remove(0);
    let key = fingerprint(&h);
    let l = marked(&["/a"], "pick", 1);
    let j = || vec![job(&h, &l, None).unwrap()];
    // Not allowed yet: nothing to run, the question waits.
    assert!(hs.admit(&h, j(), false).is_empty());
    assert!(hs.admit(&h, j(), false).is_empty());
    assert_eq!(
        hs.next_question().map(|q| q.name.as_str()),
        Some("Upload picks")
    );
    // Allow: both waiting runs go.
    assert_eq!(hs.answered(key, true).len(), 2);
    assert!(hs.next_question().is_none());
    let mut trusted = HashSet::new();
    trusted.insert(key);
    assert_eq!(hs.admit(&h, j(), trusted.contains(&key)).len(), 1);
    // The program changed: a new fingerprint, so it asks again.
    let mut changed = h.clone();
    changed.program = "/tmp/evil".into();
    let k2 = fingerprint(&changed);
    assert!(!trusted.contains(&k2));
    assert!(hs.admit(&changed, j(), trusted.contains(&k2)).is_empty());
    assert_eq!(
        hs.next_question().map(|q| q.program.as_str()),
        Some("/tmp/evil")
    );
    // Not now: dropped, and it neither runs nor asks again this session.
    assert!(hs.answered(k2, false).is_empty());
    assert!(hs.admit(&changed, j(), false).is_empty());
    assert!(hs.next_question().is_none());
    // The question names the program and the events.
    let (title, detail) = consent_text(&h);
    let title = title.replace(['\u{2068}', '\u{2069}'], "");
    assert!(
        title.contains("Upload picks") && title.contains("/home/u/bin/upload"),
        "{title}"
    );
    assert!(title.contains("marked"), "{title}");
    assert!(detail.contains("{files}"), "{detail}");
}

#[test]
fn the_panel_lists_hooks_with_switches() {
    let text = format!(
        "{UPLOAD}\n[[hooks]]\nname = \"Off\"\non = [\"exported\"]\nprogram = \"p\"\nenabled = false\n\n[[hooks]]\nname = \"Bad\"\non = [\"clicked\"]\nprogram = \"x\"\n"
    );
    let (list, problems) = parse_hooks(&text.parse().unwrap());
    let mut trusted = HashSet::new();
    trusted.insert(fingerprint(&list[0]));
    let p = panel(&list, &problems, &trusted);
    assert_eq!(p.rows.len(), 3);
    assert_eq!(p.rows[0].id, "hook:1");
    assert_eq!(p.rows[0].check, Some(CheckState::On));
    assert_eq!(p.rows[0].lines, ["/home/u/bin/upload {files}"]);
    assert_eq!(p.rows[1].check, Some(CheckState::Off));
    assert!(p.rows[2].attention && p.rows[2].check.is_none());
    assert_eq!(p.rows[2].id, "hook:3");
    assert!(
        p.rows.iter().all(|r| r.button.is_some()),
        "each can be removed"
    );
    assert!(panel(&[], &[], &trusted).rows.is_empty());
}

/// The fake program of the recipes' tests, for hooks: it writes its arguments, its input
/// and `$SPECULAR_EVENT` to a file.
#[cfg(unix)]
fn program(d: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = d.join("hook.sh");
    std::fs::write(
        &p,
        "#!/bin/sh\nout=\"$(dirname \"$0\")/out.txt\"\nfor a in \"$@\"; do printf '[%s]\\n' \"$a\" >> \"$out\"; done\nprintf 'event=%s\\n' \"$SPECULAR_EVENT\" >> \"$out\"\ncat >> \"$out\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[cfg(unix)]
fn wait_for(cond: impl Fn() -> bool) {
    let t = Instant::now();
    while !cond() {
        assert!(t.elapsed() < Duration::from_secs(30), "timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn a_real_program_gets_its_arguments_and_the_event() {
    let d = dir("real");
    let prog = program(&d);
    let text = format!(
        "[[hooks]]\nname = \"Rec\"\non = [\"marked\"]\nprogram = \"{}\"\nargs = [\"{{event}}\", \"{{files}}\"]\n",
        prog.display()
    );
    let h = hooks(&text).remove(0);
    // Shell syntax in names stays literal: no shell is involved.
    let evil = [
        "/p/a b.jpg",
        "/p/$(touch pwned).jpg",
        "/p/;rm -rf ~;.jpg",
        "/p/`x`'\".jpg",
    ];
    let l = marked(&evil, "pick", 2);
    let (runner, notes) = Runner::real();
    runner.submit(job(&h, &l, None).unwrap());
    let out = d.join("out.txt");
    wait_for(|| std::fs::read_to_string(&out).is_ok_and(|t| t.contains("\"event\":\"marked\"")));
    wait_for(|| runner.running() == 0);
    let text = std::fs::read_to_string(&out).unwrap();
    let mut want = String::from("[marked]\n");
    for e in evil {
        want.push_str(&format!("[{e}]\n"));
    }
    want.push_str("event=marked\n");
    assert!(text.starts_with(&want), "{text}");
    let json_line = text.lines().last().unwrap();
    let v: Value = serde_json::from_str(json_line).unwrap();
    assert_eq!(v["files"].as_array().unwrap().len(), evil.len());
    assert!(!d.join("pwned").exists() && !Path::new("pwned").exists());
    assert!(notes.try_recv().is_err(), "it worked");
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(unix)]
#[test]
fn a_hook_that_runs_too_long_is_stopped() {
    let d = dir("slow");
    let h = hooks(
        "[[hooks]]\nname = \"Slow\"\non = [\"opened\"]\nprogram = \"/bin/sleep\"\nargs = [\"30\"]\ntimeout = 1\n",
    )
    .remove(0);
    let l = line(&Event::new("opened", vec!["/a".into()]));
    let (runner, notes) = Runner::real();
    let t = Instant::now();
    runner.submit(job(&h, &l, None).unwrap());
    let n = notes.recv_timeout(Duration::from_secs(20)).unwrap();
    assert_eq!(
        n,
        Note::Failed {
            name: "Slow".into(),
            why: Why::Timeout(1)
        }
    );
    assert!(t.elapsed() < Duration::from_secs(10));
    // The note goes out just before the runner drops the job from its count.
    let until = Instant::now() + Duration::from_secs(5);
    while runner.running() > 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(runner.running(), 0);
    // One that cannot start says so.
    let mut missing = h.clone();
    missing.program = d.join("no-such-program").display().to_string();
    runner.submit(job(&missing, &l, None).unwrap());
    let n = notes.recv_timeout(Duration::from_secs(20)).unwrap();
    assert!(
        matches!(
            n,
            Note::Failed {
                why: Why::Start(_),
                ..
            }
        ),
        "{n:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn counted_starts() {
    // The real runner never starts more than the limits either.
    let started = Arc::new(AtomicUsize::new(0));
    let s2 = started.clone();
    let (r, _notes) = Runner::new(Box::new(move |_: &Job, _done: OnExit| {
        s2.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }));
    for k in 0..10 {
        r.submit(fake_job("x", k));
    }
    assert_eq!(started.load(Ordering::Relaxed), TOTAL);
}
