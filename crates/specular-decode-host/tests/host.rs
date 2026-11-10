//! End-to-end tests of the decode host through the viewer-side client.
//!
//! Fixtures (tests/data), generated with ImageMagick, heif-enc (libheif 1.21) and rar 7.20:
//! - grad.png: `magick -size 64x48 gradient:red-blue -depth 8`; grad.rgb is its raw RGB.
//! - grad.heic: `heif-enc -q 90`; lossless.heic: `heif-enc -L -p chroma=444
//!   --matrix_coefficients=0`; grad10.heic: the 16-bit gradient with `-b 10 -q 80`.
//! - alpha.heic: 40×30, green→yellow, alpha white→black (opaque top, transparent bottom).
//! - rar5.rar: `rar a -ma5 -m3` of a.txt ("hello from rar\n"), sub/grad.png,
//!   sub/numbers.txt (`seq 1 20000`), sub/random.bin (= random.bin); solid.rar the same with
//!   `-s`; encrypted.rar with `-pSecret`; hdrenc.rar (a.txt) with `-hpSecret`.
//!
//! HEIC tests skip (with a message) when libheif is not installed; the corpus tests skip when
//! `tools/corpus/conformance` has not been fetched.
//!
//! The same tests run on every platform; what the sandbox can promise differs, so some
//! expectations are `#[cfg]`-gated (see `specular-sandbox`'s crate docs for the test plan on
//! macOS and Windows, which CI only cross-checks).
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use specular_sandbox::protocol::{ErrorKind, ImageLimits, Response, TestOp};
use specular_sandbox::{Config, Error, Host, SandboxLevel};
use std::time::{Duration, Instant};

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DATA}{name}")).unwrap()
}

fn config() -> Config {
    Config {
        exe: Some(env!("CARGO_BIN_EXE_specular-decode-host").into()),
        timeout: Duration::from_secs(20),
        pool_size: 1,
        test_hooks: true,
        ..Config::default()
    }
}

fn host() -> Host {
    Host::new(config())
}

fn echo(h: &Host, n: usize) {
    let b: Vec<u8> = (0..n).map(|i| (i * 7) as u8).collect();
    assert_eq!(h.test(TestOp::Echo(b.clone())).unwrap(), Response::Echo(b));
}

#[test]
fn hello_and_echo() {
    let h = host();
    let info = h.info().unwrap();
    eprintln!("host: {info:?}");
    assert_eq!(info.version, specular_sandbox::protocol::VERSION);
    assert!(info.test_hooks);
    #[cfg(target_os = "linux")]
    {
        assert!(info.sandbox >= SandboxLevel::Partial, "{info:?}");
        assert!(info.rar);
    }
    #[cfg(target_os = "macos")]
    {
        assert_eq!(info.sandbox, SandboxLevel::Full, "{info:?}");
        assert!(info.sandbox_detail.starts_with("seatbelt"), "{info:?}");
        assert_eq!(info.rar, cfg!(feature = "rar"));
    }
    #[cfg(windows)]
    {
        assert_eq!(info.sandbox, SandboxLevel::Full, "{info:?}");
        assert!(info.sandbox_detail.contains("appcontainer"), "{info:?}");
        assert_eq!(info.rar, cfg!(feature = "rar"));
    }
    for n in [0, 1, 1000, 3 << 20] {
        echo(&h, n);
    }
    assert_eq!(h.pids().len(), 1);
}

/// The host's parent-death signal is tied to the thread that forked it: a host started from a
/// worker thread that then exits must stay alive (it is spawned from a long-lived thread).
#[test]
fn survives_the_thread_that_started_it() {
    let h = std::sync::Arc::new(host());
    let h2 = h.clone();
    std::thread::spawn(move || echo(&h2, 10)).join().unwrap();
    let pid = h.pids()[0];
    std::thread::sleep(Duration::from_millis(100));
    echo(&h, 10);
    assert_eq!(h.pids(), [pid], "the host was restarted");
}

#[test]
fn test_hooks_need_the_flag() {
    let h = Host::new(Config {
        test_hooks: false,
        ..config()
    });
    match h.test(TestOp::Echo(vec![1])) {
        Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::Protocol),
        r => panic!("{r:?}"),
    }
}

#[test]
fn crash_is_an_error_and_the_host_restarts() {
    let h = host();
    echo(&h, 10);
    let pid = h.pids()[0];
    match h.test(TestOp::Crash) {
        Err(Error::Crashed(how)) => {
            // abort(): SIGABRT on Unix, a fail-fast exception on Windows.
            let want = if cfg!(windows) {
                "0xc0000409"
            } else {
                "signal"
            };
            assert!(how.contains(want), "{how}");
        }
        r => panic!("{r:?}"),
    }
    assert!(h.pids().is_empty());
    echo(&h, 10);
    assert_ne!(h.pids()[0], pid);
}

/// A stack overflow (a parser of nested data without a depth limit) and a panic in the host
/// cost the request only: the viewer gets an error, and the next request gets a new host.
#[test]
fn overflow_and_panic_are_errors_and_the_host_restarts() {
    let h = host();
    for op in [TestOp::Overflow, TestOp::Panic] {
        echo(&h, 10);
        let pid = h.pids()[0];
        match h.test(op.clone()) {
            Err(Error::Crashed(how)) => {
                // Overflow: SIGSEGV caught by Rust's guard page handler, which aborts
                // (SIGABRT); Windows: STATUS_STACK_OVERFLOW. Panic: exit status 101.
                assert!(!how.is_empty(), "{op:?}");
            }
            r => panic!("{op:?}: {r:?}"),
        }
        assert!(h.pids().is_empty(), "{op:?}");
        echo(&h, 10);
        assert_ne!(h.pids()[0], pid, "{op:?}");
    }
}

#[test]
fn killed_while_idle_restarts_transparently() {
    let h = host();
    echo(&h, 10);
    let pid = h.pids()[0];
    let killed = if cfg!(windows) {
        std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .status()
    } else {
        std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status()
    };
    assert!(killed.unwrap().success());
    std::thread::sleep(Duration::from_millis(50));
    echo(&h, 10);
    assert_ne!(h.pids()[0], pid);
}

#[test]
fn timeout_kills_and_restarts() {
    let h = Host::new(Config {
        timeout: Duration::from_millis(300),
        ..config()
    });
    echo(&h, 10);
    let t0 = Instant::now();
    match h.test(TestOp::Hang { millis: 10_000 }) {
        Err(Error::Timeout(d)) => assert_eq!(d, Duration::from_millis(300)),
        r => panic!("{r:?}"),
    }
    assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    assert!(h.pids().is_empty());
    echo(&h, 10);
    assert_eq!(h.test(TestOp::Hang { millis: 10 }).unwrap(), Response::Ok);
}

/// macOS accepts `RLIMIT_AS` but does not enforce it: no memory limit there.
#[cfg(not(target_os = "macos"))]
#[test]
fn memory_limit_holds() {
    let h = Host::new(Config {
        memory_limit: 512 << 20,
        ..config()
    });
    assert!(matches!(
        h.test(TestOp::Alloc { bytes: 32 << 20 }),
        Ok(Response::Echo(_))
    ));
    match h.test(TestOp::Alloc { bytes: 2 << 30 }) {
        Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::TooLarge),
        r => panic!("{r:?}"),
    }
    echo(&h, 10);
}

#[cfg(unix)]
#[test]
fn version_mismatch_is_refused() {
    use specular_sandbox::protocol::Request;
    use specular_sandbox::transport::Conn;
    use std::os::unix::net::UnixStream;
    let (ours, theirs) = UnixStream::pair().unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_specular-decode-host"))
        .arg("--ipc")
        .stdin(std::os::fd::OwnedFd::from(theirs))
        .spawn()
        .unwrap();
    let mut c = Conn::new(ours);
    c.set_timeout(Some(Duration::from_secs(10))).unwrap();
    c.send(&Request::Hello { version: 999 }.encode(), &[])
        .unwrap();
    let (body, _) = c.recv().unwrap();
    match Response::decode(&body).unwrap() {
        Response::Hello(i) => assert_eq!(i.version, specular_sandbox::protocol::VERSION),
        r => panic!("{r:?}"),
    }
    assert!(!child.wait().unwrap().success());
    assert!(c.recv().is_err());
}

/// The same over the pipes the Windows host talks on (without the AppContainer: the test only
/// checks the protocol).
#[cfg(windows)]
#[test]
fn version_mismatch_is_refused() {
    use specular_sandbox::pipe::Conn;
    use specular_sandbox::protocol::Request;
    use std::process::Stdio;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_specular-decode-host"))
        .args(["--ipc", "--test-hooks", "--no-sandbox"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let (w, r) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
    let mut c = Conn::new(r, w);
    c.send(&Request::Hello { version: 999 }.encode(), &[])
        .unwrap();
    let (body, _) = c.recv().unwrap();
    match Response::decode(&body).unwrap() {
        Response::Hello(i) => assert_eq!(i.version, specular_sandbox::protocol::VERSION),
        r => panic!("{r:?}"),
    }
    assert!(!child.wait().unwrap().success());
    assert!(c.recv().is_err());
}

fn probe(h: &Host) -> Vec<specular_sandbox::protocol::ProbeResult> {
    let read_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml").to_string();
    let write_dir = env!("CARGO_TARGET_TMPDIR").to_string();
    match h.test(TestOp::Probe {
        read_path,
        write_dir,
    }) {
        Ok(Response::Probe(r)) => r,
        r => panic!("{r:?}"),
    }
}

/// Probes every platform must deny under a full sandbox. On Windows the AppContainer may
/// create and bind sockets (the firewall, not the token, stops traffic) and may list the
/// drive root, so only files and processes are certain there. On macOS Seatbelt has no
/// operation for creating an unbound Unix socket, only for binding, connecting and sending,
/// which the profile denies: creating one there is allowed and reaches nothing.
const MUST_DENY: &[&str] = if cfg!(windows) {
    &["read file", "create file", "run program"]
} else if cfg!(target_os = "macos") {
    &[
        "read file",
        "create file",
        "list /",
        "udp socket",
        "tcp connect",
        "run program",
        "signal parent",
    ]
} else {
    &[
        "read file",
        "create file",
        "list /",
        "udp socket",
        "tcp connect",
        "unix socket",
        "run program",
        "signal parent",
    ]
};

#[test]
fn sandbox_denies_files_network_processes() {
    // Control: without the sandbox every probe goes through, so the probes do test something.
    let open = Host::new(Config {
        disable_sandbox: true,
        require_sandbox: false,
        ..config()
    });
    assert_eq!(open.info().unwrap().sandbox, SandboxLevel::None);
    for r in probe(&open) {
        // On Windows the job (always applied) allows one process: no programs even then.
        if !(cfg!(windows) && r.name == "run program") {
            assert!(r.allowed, "unsandboxed control failed: {r:?}");
        }
    }
    let h = host();
    let info = h.info().unwrap();
    let results = probe(&h);
    eprintln!("sandbox: {:?} ({})", info.sandbox, info.sandbox_detail);
    for r in &results {
        eprintln!(
            "  {}: {} ({})",
            r.name,
            if r.allowed { "ALLOWED" } else { "denied" },
            r.detail
        );
    }
    let names: Vec<_> = results.iter().map(|r| r.name.as_str()).collect();
    for want in MUST_DENY {
        assert!(names.contains(want), "{want} missing");
    }
    if info.sandbox == SandboxLevel::Full {
        for r in &results {
            if MUST_DENY.contains(&r.name.as_str()) {
                assert!(!r.allowed, "sandbox allowed {}: {}", r.name, r.detail);
            }
        }
    } else {
        // Only Linux has a partial sandbox (seccomp without Landlock).
        assert!(
            cfg!(target_os = "linux") || info.sandbox == SandboxLevel::Full,
            "{info:?}"
        );
        // seccomp alone (no Landlock) still denies everything probed: there is no `openat`
        // at all (every input arrives as a descriptor), so not even reading.
        for r in &results {
            assert!(!r.allowed, "sandbox allowed {}: {}", r.name, r.detail);
        }
    }
    // The host still works after the probes.
    echo(&h, 100);
}

// --- HEIC ----------------------------------------------------------------------------------

fn heif_host() -> Option<Host> {
    let h = host();
    let info = h.info().unwrap();
    match info.heif {
        Some(v) => {
            eprintln!("libheif {v}");
            Some(h)
        }
        None => {
            eprintln!("SKIP: libheif not installed ({})", info.heif_error);
            None
        }
    }
}

fn close(a: &[u8], b: &[u8], tol: u8) -> bool {
    a.iter().zip(b).all(|(&x, &y)| x.abs_diff(y) <= tol)
}

#[test]
fn heic_decodes() {
    let Some(h) = heif_host() else { return };
    let rgb = data("grad.rgb");
    for (name, tol) in [("grad.heic", 24), ("lossless.heic", 2)] {
        let img = h.heif_decode(&data(name), ImageLimits::default()).unwrap();
        assert_eq!((img.width, img.height), (64, 48), "{name}");
        assert!(!img.has_alpha);
        assert_eq!(img.bit_depth, 8);
        assert_eq!(img.rgba.len(), 64 * 48 * 4);
        let got: Vec<u8> = img
            .rgba
            .chunks(4)
            .flat_map(|p| {
                assert_eq!(p[3], 255);
                [p[0], p[1], p[2]]
            })
            .collect();
        assert!(close(&got, &rgb, tol), "{name} differs from the source");
    }
    let img = h
        .heif_decode(&data("grad10.heic"), ImageLimits::default())
        .unwrap();
    assert_eq!((img.width, img.height, img.bit_depth), (64, 48, 10));
    let img = h
        .heif_decode(&data("alpha.heic"), ImageLimits::default())
        .unwrap();
    assert_eq!((img.width, img.height), (40, 30));
    assert!(img.has_alpha);
    let row = 40 * 4;
    assert!(img.rgba[3] > 240, "top opaque");
    assert!(img.rgba[29 * row + 3] < 15, "bottom transparent");
}

#[test]
fn heic_limits_and_garbage() {
    let Some(h) = heif_host() else { return };
    let small = ImageLimits {
        max_pixels: 1000,
        max_side: 65535,
    };
    match h.heif_decode(&data("grad.heic"), small) {
        Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::TooLarge),
        r => panic!("{:?}", r.map(|i| (i.width, i.height))),
    }
    let side = ImageLimits {
        max_pixels: 1 << 30,
        max_side: 32,
    };
    assert!(matches!(
        h.heif_decode(&data("grad.heic"), side),
        Err(Error::Remote { .. })
    ));
    let bytes = data("alpha.heic");
    let mut crashes = 0;
    for n in (0..bytes.len()).step_by(37) {
        match h.heif_decode(&bytes[..n], ImageLimits::default()) {
            Ok(_) | Err(Error::Remote { .. }) => {}
            Err(Error::Crashed(_)) => crashes += 1,
            Err(e) => panic!("{n}: {e:?}"),
        }
    }
    let mut flipped = bytes.clone();
    for i in (40..flipped.len()).step_by(13) {
        flipped[i] ^= 0x5A;
    }
    match h.heif_decode(&flipped, ImageLimits::default()) {
        Ok(_) | Err(Error::Remote { .. }) => {}
        Err(Error::Crashed(_)) => crashes += 1,
        Err(e) => panic!("{e:?}"),
    }
    eprintln!("{crashes} host crashes on damaged input (handled)");
    h.heif_decode(&data("grad.heic"), ImageLimits::default())
        .unwrap();
}

#[test]
fn heic_without_libheif_says_not_installed() {
    let h = Host::new(Config {
        env: vec![(
            "SPECULAR_LIBHEIF".into(),
            "/nonexistent/libheif.so.1".into(),
        )],
        ..config()
    });
    assert_eq!(h.info().unwrap().heif, None);
    match h.heif_decode(&data("grad.heic"), ImageLimits::default()) {
        Err(Error::Remote { kind, message }) => {
            assert_eq!(kind, ErrorKind::NotInstalled);
            assert!(message.contains("HEIC support not installed"), "{message}");
        }
        r => panic!("{:?}", r.map(|i| i.width)),
    }
}

// --- RAR -----------------------------------------------------------------------------------

/// RAR through the host: UnRAR reads the descriptors (Unix) or duplicated handles (Windows)
/// the viewer passes, through Specular's handle-I/O patch.
#[cfg(all(any(unix, windows), feature = "rar"))]
mod rar {
    use super::*;
    use specular_sandbox::{RarSource, Secret};

    fn numbers() -> Vec<u8> {
        (1..=20000)
            .map(|i| format!("{i}\n"))
            .collect::<String>()
            .into_bytes()
    }

    fn check_archive(h: &Host, a: &RarSource, name: &str, solid: bool) {
        let l = h.rar_list(a, 100, None).unwrap();
        assert_eq!(l.solid, solid, "{name}");
        let mut names: Vec<_> = l.entries.iter().map(|e| e.name.as_str()).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "a.txt",
                "sub",
                "sub/grad.png",
                "sub/numbers.txt",
                "sub/random.bin"
            ],
            "{name}"
        );
        let at = |n: &str| l.entries.iter().position(|e| e.name == n).unwrap();
        assert!(l.entries[at("sub")].directory);
        let numbers = numbers();
        let random = data("random.bin");
        let want: [(&str, &[u8]); 3] = [
            ("a.txt", b"hello from rar\n"),
            ("sub/numbers.txt", &numbers),
            ("sub/random.bin", &random),
        ];
        // Last entry first (solid archives decompress everything before the entry).
        let mut order: Vec<_> = (0..l.entries.len()).rev().collect();
        order.push(l.entries.len() - 1);
        for i in order {
            let e = &l.entries[i];
            if e.directory {
                continue;
            }
            let got = h.rar_read(a, i as u32, e, 1 << 20, None).unwrap();
            assert_eq!(got.len() as u64, e.size, "{name} {}", e.name);
            match want.iter().find(|w| w.0 == e.name) {
                Some(w) => assert_eq!(got, w.1, "{name} {}", e.name),
                None => assert_eq!(&got[..8], b"\x89PNG\r\n\x1a\n"),
            }
        }
        let (dir, txt, num) = (at("sub"), at("a.txt"), at("sub/numbers.txt"));
        // The directory, a wrong name, a limit.
        assert!(matches!(
            h.rar_read(a, dir as u32, &l.entries[dir], 1 << 20, None),
            Err(Error::Remote {
                kind: ErrorKind::Unsupported,
                ..
            })
        ));
        let mut wrong = l.entries[txt].clone();
        wrong.name = "other.txt".into();
        assert!(matches!(
            h.rar_read(a, txt as u32, &wrong, 1 << 20, None),
            Err(Error::Remote {
                kind: ErrorKind::Malformed,
                ..
            })
        ));
        assert!(matches!(
            h.rar_read(a, num as u32, &l.entries[num], 1000, None),
            Err(Error::Remote {
                kind: ErrorKind::TooLarge,
                ..
            })
        ));
        assert!(matches!(
            h.rar_list(a, 2, None),
            Err(Error::Remote {
                kind: ErrorKind::TooLarge,
                ..
            })
        ));
    }

    #[test]
    fn lists_and_reads() {
        let h = host();
        for (name, solid) in [("rar5.rar", false), ("solid.rar", true)] {
            // A private copy (an archive from memory) and the file itself, opened by the
            // viewer and passed as a descriptor or duplicated handle.
            check_archive(
                &h,
                &RarSource::from_bytes(&data(name)).unwrap(),
                name,
                solid,
            );
            let p = std::path::PathBuf::from(format!("{DATA}{name}"));
            check_archive(&h, &RarSource::open(&p).unwrap(), name, solid);
        }
    }

    /// Deterministic incompressible bytes.
    fn noise(n: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 24) as u8
            })
            .collect()
    }

    /// A fresh directory under the test target directory.
    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Runs `rar` in `dir`; false (with a SKIP message) when it is not installed.
    fn rar(dir: &std::path::Path, args: &[&str]) -> bool {
        match std::process::Command::new("rar")
            .current_dir(dir)
            .args(["a", "-idq", "-ep1"])
            .args(args)
            .status()
        {
            Ok(s) if s.success() => true,
            r => {
                eprintln!("SKIP: cannot run `rar` ({r:?})");
                false
            }
        }
    }

    /// Two files, `big.bin` (300 kB, stored, so it spans volumes) and `small.txt`, as volumes
    /// of 100 kB named `archive` (`rar` adds `.partN.rar`).
    fn volumes(dir: &std::path::Path, archive: &str, extra: &[&str]) -> Option<Vec<u8>> {
        let big = noise(300_000, 7);
        std::fs::write(dir.join("big.bin"), &big).unwrap();
        std::fs::write(dir.join("small.txt"), b"after the big one\n").unwrap();
        let mut args = vec!["-m0", "-v100k"];
        args.extend_from_slice(extra);
        args.extend_from_slice(&[archive, "big.bin", "small.txt"]);
        rar(dir, &args).then_some(big)
    }

    fn read_all(h: &Host, a: &RarSource, big: &[u8]) {
        let l = h.rar_list(a, 100, None).unwrap();
        let mut names: Vec<_> = l.entries.iter().map(|e| e.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["big.bin", "small.txt"]);
        // Last first: in a solid archive that decompresses everything before it.
        for (i, e) in l.entries.iter().enumerate().rev() {
            let want: &[u8] = if e.name == "big.bin" {
                big
            } else {
                b"after the big one\n"
            };
            assert_eq!(h.rar_read(a, i as u32, e, 1 << 20, None).unwrap(), want);
        }
    }

    /// Multi-volume archives: the viewer opens the first volume, the host asks for the others
    /// by synthetic name, and the viewer maps those back to files next to the first.
    #[test]
    fn multi_volume() {
        let d = scratch("rar-volumes");
        // Non-ASCII and spaces in the name (the synthetic name is `My_comic_.cbr.part1.rar`).
        let Some(big) = volumes(&d, "My comic é.cbr", &[]) else {
            return;
        };
        let first = d.join("My comic é.cbr.part1.rar");
        assert!(d.join("My comic é.cbr.part3.rar").is_file());
        let h = host();
        read_all(&h, &RarSource::open(&first).unwrap(), &big);
        // Renamed to `.cbr` volumes (UnRAR keeps the extension when numbering).
        for n in 1..=3 {
            std::fs::rename(
                d.join(format!("My comic é.cbr.part{n}.rar")),
                d.join(format!("comic.part{n}.cbr")),
            )
            .unwrap();
        }
        read_all(
            &h,
            &RarSource::open(&d.join("comic.part1.cbr")).unwrap(),
            &big,
        );
        // More volumes than the host may hold descriptors (64): it keeps only a few open.
        let m = scratch("rar-many-volumes");
        let many = noise(300_000, 9);
        std::fs::write(m.join("big.bin"), &many).unwrap();
        std::fs::write(m.join("small.txt"), b"after the big one\n").unwrap();
        assert!(rar(
            &m,
            &["-m0", "-v4k", "many.rar", "big.bin", "small.txt"]
        ));
        assert!(m.join("many.part70.rar").is_file());
        read_all(
            &h,
            &RarSource::open(&m.join("many.part01.rar")).unwrap(),
            &many,
        );
        // Solid and compressed across volumes: reading the second entry decompresses the
        // first through every volume.
        assert!(rar(
            &m,
            &["-s", "-m3", "-v100k", "solid.rar", "big.bin", "small.txt"]
        ));
        let a = RarSource::open(&m.join("solid.part1.rar")).unwrap();
        assert!(h.rar_list(&a, 100, None).unwrap().solid);
        read_all(&h, &a, &many);
        // Without a way to find volumes (an archive from memory) the spanning file is an
        // error, not a hang or a crash.
        let a = RarSource::from_bytes(&std::fs::read(d.join("comic.part1.cbr")).unwrap()).unwrap();
        if let Ok(l) = h.rar_list(&a, 100, None) {
            assert!(h.rar_read(&a, 0, &l.entries[0], 1 << 20, None).is_err());
        }
        echo(&h, 10);
    }

    /// A missing middle volume and a provider that refuses: errors, and the host survives.
    #[test]
    fn missing_volume() {
        let d = scratch("rar-missing-volume");
        let Some(big) = volumes(&d, "gap.rar", &[]) else {
            return;
        };
        let h = host();
        read_all(
            &h,
            &RarSource::open(&d.join("gap.part1.rar")).unwrap(),
            &big,
        );
        let pid = h.pids()[0];
        std::fs::remove_file(d.join("gap.part2.rar")).unwrap();
        let a = RarSource::open(&d.join("gap.part1.rar")).unwrap();
        match h.rar_list(&a, 100, None) {
            Ok(l) => {
                assert!(h.rar_read(&a, 0, &l.entries[0], 1 << 20, None).is_err());
            }
            Err(Error::Remote { .. }) => {}
            Err(e) => panic!("{e:?}"),
        }
        // A provider that only ever offers the first volume again.
        let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let n = asked.clone();
        let p1 = d.join("gap.part1.rar");
        let liar = move |_: &str| {
            n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            std::fs::File::open(&p1).ok()
        };
        let a = RarSource::with_volumes(
            std::fs::File::open(d.join("gap.part1.rar")).unwrap(),
            "gap.part1.rar",
            Some(Box::new(liar)),
        )
        .unwrap();
        // The first volume over and over: its entries again and again, up to the limit.
        assert!(matches!(
            h.rar_list(&a, 100, None),
            Err(Error::Remote {
                kind: ErrorKind::TooLarge,
                ..
            })
        ));
        let l = h.rar_list(
            &RarSource::open(&d.join("gap.part1.rar")).unwrap(),
            100,
            None,
        );
        assert!(l.is_err());
        assert!(asked.load(std::sync::atomic::Ordering::Relaxed) > 0);
        echo(&h, 10);
        assert_eq!(h.pids(), [pid], "the host crashed");
    }

    /// Archives cut short (on disk, so UnRAR seeks past the end of the file to the next
    /// header) are errors or short listings, never crashes.
    #[test]
    fn truncated_and_seek_past_end() {
        let d = scratch("rar-truncated");
        let big = noise(300_000, 11);
        std::fs::write(d.join("big.bin"), &big).unwrap();
        std::fs::write(d.join("small.txt"), b"tail\n").unwrap();
        if !rar(&d, &["-m0", "whole.rar", "big.bin", "small.txt"]) {
            return;
        }
        let whole = std::fs::read(d.join("whole.rar")).unwrap();
        let h = host();
        echo(&h, 1);
        let pid = h.pids()[0];
        for n in [
            0,
            10,
            100,
            1000,
            150_000,
            299_000,
            whole.len() - 20,
            whole.len() - 1,
        ] {
            let p = d.join(format!("cut{n}.rar"));
            std::fs::write(&p, &whole[..n]).unwrap();
            let a = RarSource::open(&p).unwrap();
            match h.rar_list(&a, 100, None) {
                Ok(l) => {
                    for (i, e) in l.entries.iter().enumerate() {
                        let r = h.rar_read(&a, i as u32, e, 1 << 20, None);
                        if e.name == "big.bin" && n < 300_000 {
                            assert!(r.is_err(), "{n}");
                        }
                        if let Err(e) = r {
                            assert!(matches!(e, Error::Remote { .. }), "{n}: {e:?}");
                        }
                    }
                }
                Err(Error::Remote { .. }) => {}
                Err(e) => panic!("{n}: {e:?}"),
            }
        }
        echo(&h, 10);
        assert_eq!(h.pids(), [pid], "the host crashed");
    }

    /// The host dies while it waits for the next volume (killed from inside the viewer's
    /// volume provider): the read fails as a crash and the next request gets a fresh host.
    #[test]
    fn host_killed_mid_read() {
        let d = scratch("rar-killed");
        let Some(big) = volumes(&d, "kill.rar", &[]) else {
            return;
        };
        let h = host();
        echo(&h, 1);
        let pid = h.pids()[0];
        let first = d.join("kill.part1.rar");
        let dir = d.clone();
        let kill = move |want: &str| {
            let killed = if cfg!(windows) {
                std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .status()
            } else {
                std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .status()
            };
            assert!(killed.unwrap().success());
            std::fs::File::open(dir.join(want)).ok()
        };
        let a = RarSource::with_volumes(
            std::fs::File::open(&first).unwrap(),
            "kill.part1.rar",
            Some(Box::new(kill)),
        )
        .unwrap();
        match h.rar_list(&a, 100, None) {
            Err(Error::Crashed(_) | Error::Io(_)) => {}
            r => panic!("{r:?}"),
        }
        assert!(!h.pids().contains(&pid));
        read_all(&h, &RarSource::open(&first).unwrap(), &big);
        assert_ne!(h.pids()[0], pid);
    }

    /// An archive larger than 4 GiB (sparse input, stored): the entry after the big one is
    /// found and read with 64-bit offsets. Slow and needs ~5 GB of disk: run with
    /// `--ignored`.
    #[test]
    #[ignore]
    fn over_4_gib() {
        let d = scratch("rar-4g");
        let big = std::fs::File::create(d.join("big.bin")).unwrap();
        big.set_len((4u64 << 30) + 12345).unwrap();
        drop(big);
        std::fs::write(d.join("small.txt"), b"past four gigabytes\n").unwrap();
        if !rar(&d, &["-m0", "huge.rar", "big.bin", "small.txt"]) {
            return;
        }
        std::fs::remove_file(d.join("big.bin")).unwrap();
        let a = RarSource::open(&d.join("huge.rar")).unwrap();
        assert!(a.len() > 4 << 30);
        let h = host();
        let l = h.rar_list(&a, 100, None).unwrap();
        assert_eq!(l.entries[0].size, (4u64 << 30) + 12345);
        assert_eq!(
            h.rar_read(&a, 1, &l.entries[1], 1 << 20, None).unwrap(),
            b"past four gigabytes\n"
        );
        assert!(matches!(
            h.rar_read(&a, 0, &l.entries[0], 1 << 20, None),
            Err(Error::Remote {
                kind: ErrorKind::TooLarge,
                ..
            })
        ));
        drop(a);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn encrypted() {
        let h = host();
        let a = RarSource::from_bytes(&data("encrypted.rar")).unwrap();
        let l = h.rar_list(&a, 100, None).unwrap();
        let e = l.entries.iter().find(|e| e.name == "a.txt").unwrap();
        assert!(e.encrypted);
        match h.rar_read(&a, 0, e, 1 << 20, None) {
            Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::Encrypted),
            r => panic!("{r:?}"),
        }
        let a = RarSource::from_bytes(&data("hdrenc.rar")).unwrap();
        match h.rar_list(&a, 100, None) {
            Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::Encrypted),
            r => panic!("{r:?}"),
        }
    }

    /// Lists `a` and reads every file with `pw`, or returns the error.
    fn read_with(h: &Host, a: &RarSource, pw: &str) -> Result<Vec<(String, Vec<u8>)>, Error> {
        let pw = Secret::new(pw.to_owned());
        let l = h.rar_list(a, 100, Some(&pw))?;
        let mut out = Vec::new();
        let mut many = Vec::new();
        for (i, e) in l.entries.iter().enumerate() {
            if e.directory {
                continue;
            }
            out.push((
                e.name.clone(),
                h.rar_read(a, i as u32, e, 1 << 20, Some(&pw))?,
            ));
            many.push((i as u32, e));
        }
        let batch = h.rar_read_many(a, &many, 1 << 20, 1 << 24, Some(&pw))?;
        assert_eq!(batch, out.iter().map(|o| o.1.clone()).collect::<Vec<_>>());
        Ok(out)
    }

    fn assert_bad_password(r: Result<Vec<(String, Vec<u8>)>, Error>, what: &str) {
        match r {
            Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::BadPassword, "{what}"),
            r => panic!("{what}: {r:?}"),
        }
    }

    /// RAR5 encrypted data (`-p`) and encrypted headers (`-hp`): the right password reads
    /// everything, a wrong, empty or NUL-carrying one is a wrong password.
    #[test]
    fn rar5_passwords() {
        let h = host();
        for name in ["encrypted.rar", "hdrenc.rar"] {
            let a = RarSource::from_bytes(&data(name)).unwrap();
            let got = read_with(&h, &a, "Secret").unwrap();
            let hello = got.iter().find(|(n, _)| n == "a.txt").unwrap();
            assert_eq!(hello.1, b"hello from rar\n", "{name}");
            for pw in ["secret", "", "Secret\0", "S\u{e9}cret"] {
                assert_bad_password(read_with(&h, &a, pw), &format!("{name} {pw:?}"));
            }
        }
    }

    /// RAR 4 (which RAR 7 can no longer write): libarchive's `rar_encryption_data.rar` and
    /// `rar_encryption_header.rar` (password `12345678`; foo.txt and bar.txt, 16 bytes each),
    /// when fetched by `tools/corpus/fetch-conformance.sh`. RAR 4 has no password check: a
    /// wrong password shows as damaged data (or, for headers, a failed header decryption).
    #[test]
    fn rar4_passwords() {
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/corpus/conformance/libarchive");
        let h = host();
        for name in ["rar_encryption_data.rar", "rar_encryption_header.rar"] {
            let p = d.join(name);
            if !p.is_file() {
                eprintln!("SKIP: {name} not fetched");
                continue;
            }
            let a = RarSource::open(&p).unwrap();
            let got = read_with(&h, &a, "12345678").unwrap();
            assert_eq!(got.len(), 2, "{name}");
            assert!(got.iter().all(|(_, d)| d.len() == 16), "{name}");
            for pw in ["1234567", "", "\u{e9}"] {
                assert_bad_password(read_with(&h, &a, pw), &format!("{name} {pw:?}"));
            }
        }
    }

    #[test]
    fn damaged_archives_are_errors() {
        let h = host();
        let bytes = data("rar5.rar");
        for n in [0, 7, 20, 100, 1000, bytes.len() / 2, bytes.len() - 10] {
            let a = RarSource::from_bytes(&bytes[..n]).unwrap();
            match h.rar_list(&a, 100, None) {
                Ok(l) => {
                    for (i, e) in l.entries.iter().enumerate() {
                        let _ = h.rar_read(&a, i as u32, e, 1 << 20, None);
                    }
                }
                Err(Error::Remote { .. } | Error::Crashed(_)) => {}
                Err(e) => panic!("{n}: {e:?}"),
            }
        }
        let mut flipped = bytes.clone();
        for i in (60..flipped.len()).step_by(101) {
            flipped[i] ^= 0xA5;
        }
        let a = RarSource::from_bytes(&flipped).unwrap();
        if let Ok(l) = h.rar_list(&a, 100, None) {
            for (i, e) in l.entries.iter().enumerate() {
                if !e.directory {
                    assert!(
                        h.rar_read(&a, i as u32, e, 1 << 20, None).is_err(),
                        "{}",
                        e.name
                    );
                }
            }
        }
        echo(&h, 10);
    }
}

// --- Conformance corpus (tools/corpus/conformance, fetched by script, not in git) ----------

mod corpus {
    use super::*;
    use std::path::{Path, PathBuf};

    fn dir(sub: &str) -> Option<PathBuf> {
        let d = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/corpus/conformance")
            .join(sub);
        if d.is_dir() {
            Some(d)
        } else {
            eprintln!("SKIP: {} not fetched", d.display());
            None
        }
    }

    /// Real HEIC files (libheif's example, Nokia's conformance stills) decode to the size
    /// their container declares.
    #[test]
    fn heic_files_decode() {
        let Some(h) = heif_host() else { return };
        let Some(lh) = dir("libheif/examples") else {
            return;
        };
        let img = h
            .heif_decode(
                &std::fs::read(lh.join("example.heic")).unwrap(),
                ImageLimits::default(),
            )
            .unwrap();
        eprintln!("example.heic: {}x{}", img.width, img.height);
        assert!(img.width >= 64 && img.height >= 64);
        assert_eq!(img.rgba.len(), img.width as usize * img.height as usize * 4);
        // Not one flat colour.
        let first = &img.rgba[..4];
        assert!(img.rgba.chunks(4).any(|p| p != first));
        if let Some(nokia) = dir("nokia-heif/content/images") {
            for name in ["autumn_1440x960.heic", "old_bridge_1440x960.heic"] {
                let p = nokia.join(name);
                if !p.is_file() {
                    continue;
                }
                let img = h
                    .heif_decode(&std::fs::read(&p).unwrap(), ImageLimits::default())
                    .unwrap();
                assert_eq!((img.width, img.height), (1440, 960), "{name}");
            }
        }
    }

    /// libarchive's RAR samples, opened the way the viewer opens a CBR (a CBR is a RAR).
    #[cfg(all(any(unix, windows), feature = "rar"))]
    #[test]
    fn rar_files_list_and_read() {
        use specular_sandbox::RarSource;
        let Some(d) = dir("libarchive") else { return };
        let h = host();
        let mut read = 0;
        for name in [
            "rar.rar",
            "rar5_stored.rar",
            "rar5_compressed.rar",
            "rar5_solid.rar",
        ] {
            let p = d.join(name);
            if !p.is_file() {
                continue;
            }
            let a = RarSource::open(&p).unwrap();
            let l = h.rar_list(&a, 1000, None).unwrap();
            assert!(!l.entries.is_empty(), "{name}");
            for (i, e) in l.entries.iter().enumerate() {
                // `testlink` in rar.rar is a symbolic link: its size is the target's length,
                // and UnRAR extracts no data for it.
                if e.directory || e.name == "testlink" {
                    continue;
                }
                let got = h.rar_read(&a, i as u32, e, 64 << 20, None).unwrap();
                assert_eq!(got.len() as u64, e.size, "{name} {}", e.name);
                read += 1;
            }
        }
        let enc = d.join("rar5_solid_encrypted.rar");
        if enc.is_file() {
            let a = RarSource::open(&enc).unwrap();
            match h.rar_list(&a, 1000, None) {
                Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::Encrypted),
                Ok(l) => {
                    let (i, e) = l
                        .entries
                        .iter()
                        .enumerate()
                        .find(|(_, e)| !e.directory)
                        .unwrap();
                    assert!(matches!(
                        h.rar_read(&a, i as u32, e, 64 << 20, None),
                        Err(Error::Remote {
                            kind: ErrorKind::Encrypted,
                            ..
                        })
                    ));
                }
                Err(e) => panic!("{e:?}"),
            }
        }
        eprintln!("{read} entries read");
    }

    /// A CBR as people make them (`rar a` over a folder of pages), built from the Nokia HEIC
    /// stills when the `rar` program is installed: listed, every page read and then decoded,
    /// all in the host — the path the viewer takes for a HEIC comic.
    #[cfg(all(any(unix, windows), feature = "rar"))]
    #[test]
    fn cbr_of_heic_pages() {
        use specular_sandbox::RarSource;
        let Some(h) = heif_host() else { return };
        let Some(nokia) = dir("nokia-heif/content/images") else {
            return;
        };
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cbr");
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(out.join("pages")).unwrap();
        let pages = ["spring", "summer", "autumn", "winter"];
        for (i, p) in pages.iter().enumerate() {
            std::fs::copy(
                nokia.join(format!("{p}_1440x960.heic")),
                out.join(format!("pages/{:02} {p}.heic", i + 1)),
            )
            .unwrap();
        }
        let cbr = out.join("seasons.cbr");
        match std::process::Command::new("rar")
            .args(["a", "-idq", "-ep1", "-m3"])
            .arg(&cbr)
            .arg(out.join("pages"))
            .status()
        {
            Ok(s) if s.success() => {}
            r => {
                eprintln!("SKIP: cannot run `rar` to build a CBR ({r:?})");
                return;
            }
        }
        let a = RarSource::open(&cbr).unwrap();
        let l = h.rar_list(&a, 1000, None).unwrap();
        let files: Vec<_> = l
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.directory)
            .collect();
        assert_eq!(files.len(), pages.len(), "{:?}", l.entries);
        for (i, e) in files {
            assert!(e.name.ends_with(".heic"), "{}", e.name);
            let data = h.rar_read(&a, i as u32, e, 64 << 20, None).unwrap();
            assert_eq!(data.len() as u64, e.size);
            let img = h.heif_decode(&data, ImageLimits::default()).unwrap();
            assert_eq!((img.width, img.height), (1440, 960), "{}", e.name);
        }
    }
}

/// Out-of-process cost against in process (information only):
/// the host's start (spawn, sandbox, handshake), a round trip carrying 0 B, 1 MiB and 24 MiB
/// (the transport alone), and the same HEIC decoded by the host's libheif and by the in-process
/// decoder. Prints one JSON line; wall clock, so run it on a quiet machine:
/// `cargo test --release -p specular-decode-host --test host -- --ignored --nocapture overhead`.
#[test]
#[ignore]
fn overhead_against_in_process() {
    fn median(mut v: Vec<f64>) -> f64 {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    }
    fn ms(t: Instant) -> f64 {
        t.elapsed().as_secs_f64() * 1e3
    }
    let t = Instant::now();
    let h = host();
    let info = h.info().unwrap();
    let start_ms = ms(t);
    let mut echo_ms = Vec::new();
    for n in [0usize, 1 << 20, 24 << 20] {
        let b: Vec<u8> = (0..n).map(|i| (i * 7) as u8).collect();
        let runs: Vec<f64> = (0..10)
            .map(|_| {
                let t = Instant::now();
                h.test(TestOp::Echo(b.clone())).unwrap();
                ms(t)
            })
            .collect();
        echo_ms.push(median(runs));
    }
    let file = data("grad.heic");
    let host_ms = info.heif.is_some().then(|| {
        median(
            (0..20)
                .map(|_| {
                    let t = Instant::now();
                    h.heif_decode(&file, ImageLimits::default()).unwrap();
                    ms(t)
                })
                .collect(),
        )
    });
    let in_process_ms = median(
        (0..20)
            .map(|_| {
                let t = Instant::now();
                let f = specular_heic::HeicFile::open(file.clone()).unwrap();
                f.decode(&specular_heic::DecodeOptions::default()).unwrap();
                ms(t)
            })
            .collect(),
    );
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    println!(
        "{{\"host_start_ms\": {start_ms:.2}, \"echo_ms\": {{\"0\": {:.3}, \"1MiB\": {:.3}, \"24MiB\": {:.3}}}, \
         \"heic_64x48_host_ms\": {}, \"heic_64x48_in_process_ms\": {in_process_ms:.3}, \"sandbox\": \"{:?}\", \"load\": \"{}\"}}",
        echo_ms[0],
        echo_ms[1],
        echo_ms[2],
        host_ms.map_or("null".into(), |v| format!("{v:.3}")),
        info.sandbox,
        load.split_whitespace().next().unwrap_or("?"),
    );
}
