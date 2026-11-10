use super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[test]
fn check_follows_the_scheme() {
    for ok in ["dec-jpeg-3", "wd-host", "rayon-99", "a", "lib-index"] {
        assert_eq!(check(ok), Ok(()), "{ok}");
    }
    for bad in [
        "",
        "specular-host-watchdog",
        "video export",
        "Dec-jpeg",
        "dec_jpeg",
        "-dec",
        "dec-",
        "dec--jpeg",
        "specular-anim",
        "dec-jpeg-ü",
    ] {
        assert!(check(bad).is_err(), "{bad}");
    }
    assert_eq!(pool_name("dec-jxl")(7), "dec-jxl-7");
}

#[test]
fn helpers_name_their_threads() {
    let name = spawn("t-plain", || {
        std::thread::current().name().map(str::to_owned)
    });
    assert_eq!(name.join().unwrap().as_deref(), Some("t-plain"));
    std::thread::scope(|s| {
        let h = spawn_scoped(s, "t-scoped", || {
            std::thread::current().name().map(str::to_owned)
        });
        assert_eq!(h.join().unwrap().as_deref(), Some("t-scoped"));
    });
}

/// A name off the scheme only warns: the thread still starts (the app must not crash over a
/// name). The workspace scan below is what fails on one.
#[test]
fn long_names_warn_but_still_spawn() {
    let ran = builder("specular-host-watchdog").spawn(|| 7).unwrap();
    assert_eq!(ran.join().unwrap(), 7);
}

/// Calls whose first string literal argument is a thread name (or a pool prefix: `pool`).
/// `.background(` is the app's `Loader::background(name, f)`; `JobPool::new(n, prefix)` names
/// its workers `{prefix}-N`; `self.spawn(name, …)` is the AI pack's.
const CALLS: [(&str, bool); 10] = [
    ("specular_thread::spawn(", false),
    ("specular_thread::builder(", false),
    ("specular_thread::spawn_scoped(", false),
    ("pool_name(", true),
    ("rayon_pool(", true),
    ("JobPool::new(", true),
    ("JobPool::background(", true),
    (".background(", false),
    (".thread_name(", false),
    ("self.spawn(", false),
];

/// Every thread name literal in the workspace's library and binary sources (not tests,
/// examples, benches or fuzz targets), with where it is. The flags: whether the name is a pool
/// prefix, and whether it is outside test code (a file's inline `#[cfg(test)] mod`), which the
/// uniqueness check needs.
fn names() -> Vec<(String, bool, bool, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for top in ["app", "crates", "tools"] {
        walk(&root.join(top), &mut files);
    }
    assert!(
        files.len() > 500,
        "found only {} sources under {}",
        files.len(),
        root.display()
    );
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let path = f.strip_prefix(&root).unwrap_or(&f).display().to_string();
        let lines: Vec<&str> = text.lines().collect();
        // `#[cfg(test)]`, maybe more attributes, then `mod …`.
        let tests_from = (0..lines.len())
            .find(|&i| {
                lines[i].trim() == "#[cfg(test)]"
                    && lines[i + 1..]
                        .iter()
                        .find(|l| !l.trim_start().starts_with("#["))
                        .is_some_and(|l| l.trim_start().starts_with("mod "))
            })
            .unwrap_or(lines.len());
        for (n, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for (call, pool) in CALLS {
                let mut from = 0;
                while let Some(at) = code[from..].find(call) {
                    let start = from + at + call.len();
                    from = start;
                    // The call's arguments, which may continue on the next lines.
                    let mut args = code[start..].to_owned();
                    for more in lines.iter().skip(n + 1).take(3) {
                        args.push(' ');
                        args.push_str(more);
                    }
                    if let Some(name) = first_literal(&args) {
                        out.push((name, pool, n < tests_from, format!("{path}:{}", n + 1)));
                    }
                }
            }
            // `std::thread::Builder::new()` with an allow (vendored code): its `.name(`.
            if code.contains("Builder::new()")
                && !code.contains("ThreadPoolBuilder")
                && let Some(next) = lines[n..].iter().take(4).find(|l| l.contains(".name("))
                && let Some(name) = first_literal(&next[next.find(".name(").unwrap() + 6..])
            {
                out.push((name, false, true, format!("{path}:{}", n + 1)));
            }
        }
    }
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        if p.is_dir() {
            if !matches!(
                &*name,
                "target" | "tests" | "examples" | "benches" | "fuzz" | "tmp" | "corpus" | ".git"
            ) {
                walk(&p, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" {
            out.push(p);
        }
    }
}

/// The first argument if it is a string literal or `format!("…")`, with every `{…}` replaced
/// by `99`; `None` when the argument is not a literal (a variable name or an expression).
fn first_literal(args: &str) -> Option<String> {
    let args = args.trim_start();
    // The first argument up to its comma or the closing paren.
    let arg = if let Some(rest) = args.strip_prefix("format!(") {
        rest
    } else {
        // `JobPool::new(threads, "x")`: the literal may be the second argument.
        let first_end = args.find([',', ')']).unwrap_or(args.len());
        let first = args[..first_end].trim();
        if first.starts_with('"') {
            args
        } else if args[first_end..].starts_with(',') {
            let second = args[first_end + 1..].trim_start();
            if second.starts_with('"') || second.starts_with("format!(") {
                return first_literal(second);
            }
            return None;
        } else {
            return None;
        }
    };
    let arg = arg.trim_start();
    let body = arg.strip_prefix('"')?;
    let end = body.find('"')?;
    let mut s = String::new();
    let mut in_brace = false;
    for c in body[..end].chars() {
        match c {
            '{' => {
                in_brace = true;
                s.push_str("99");
            }
            '}' => in_brace = false,
            _ if !in_brace => s.push(c),
            _ => {}
        }
    }
    Some(s)
}

#[test]
fn every_thread_name_in_the_workspace_follows_the_scheme() {
    let names = names();
    assert!(names.len() > 100, "found only {} thread names", names.len());
    let mut bad = Vec::new();
    let mut prefixes: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, pool, live, at) in &names {
        let full = if *pool {
            format!("{name}-99")
        } else {
            name.clone()
        };
        if let Err(why) = check(&full) {
            bad.push(format!("{at}: {full:?}: {why}"));
        }
        if *pool && *live {
            prefixes.entry(name).or_default().push(at);
        }
    }
    let shared: Vec<_> = prefixes
        .iter()
        .filter(|(_, at)| at.len() > 1)
        .map(|(p, at)| format!("{p}: {at:?}"))
        .collect();
    assert!(
        bad.is_empty(),
        "thread names off the scheme:\n{}",
        bad.join("\n")
    );
    assert!(
        shared.is_empty(),
        "pool prefixes used by more than one pool:\n{}",
        shared.join("\n")
    );
}

/// A job spawned on a pool that panics is dropped; the pool (and the process) run on.
#[cfg(feature = "rayon")]
#[test]
fn a_panicking_pool_job_does_not_abort() {
    let pool = rayon_pool("t-pool").num_threads(1).build().unwrap();
    pool.spawn(|| panic!("test panic in a pool job"));
    let (tx, rx) = std::sync::mpsc::channel();
    pool.spawn(move || tx.send(7).unwrap());
    let got = rx.recv_timeout(std::time::Duration::from_secs(10));
    assert_eq!(got, Ok(7));
}
