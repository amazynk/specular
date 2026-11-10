//! Self-test operations (`--test-hooks` only): crash, hang, allocate, and probes that try what
//! the sandbox must deny.

use specular_sandbox::protocol::{ErrorKind, ProbeResult, Response, TestOp};
use std::time::Duration;

pub fn run(op: TestOp) -> Response {
    match op {
        TestOp::Echo(b) => Response::Echo(b),
        TestOp::Crash => std::process::abort(),
        TestOp::Overflow => Response::Echo(deeper(0, &[0; 256]).to_le_bytes().to_vec()),
        TestOp::Panic => panic!("test panic in the decode host"),
        TestOp::Hang { millis } => {
            std::thread::sleep(Duration::from_millis(u64::from(millis)));
            Response::Ok
        }
        TestOp::Alloc { bytes } => alloc(bytes),
        TestOp::Probe {
            read_path,
            write_dir,
        } => Response::Probe(probe(&read_path, &write_dir)),
    }
}

/// Recursion without a limit, as a parser of nested data without a depth cap would do on a
/// crafted file: overflows the stack (`pad` keeps each frame large and the calls unfolded).
fn deeper(n: u64, pad: &[u8; 256]) -> u64 {
    let mine = std::hint::black_box(*pad);
    let below = if std::hint::black_box(n) < u64::MAX {
        deeper(n + 1, &mine)
    } else {
        0
    };
    below.wrapping_add(u64::from(mine[(n % 256) as usize]))
}

fn alloc(bytes: u64) -> Response {
    let Ok(n) = usize::try_from(bytes) else {
        return Response::error(ErrorKind::TooLarge, "size overflows");
    };
    let mut v: Vec<u8> = Vec::new();
    if v.try_reserve_exact(n).is_err() {
        return Response::error(
            ErrorKind::TooLarge,
            format!("cannot allocate {bytes} bytes"),
        );
    }
    // Touch every page so the memory really exists.
    v.resize(n, 1);
    let sum: u64 = v.iter().step_by(4096).map(|&b| u64::from(b)).sum();
    Response::Echo(sum.to_le_bytes().to_vec())
}

fn result(name: &str, r: std::io::Result<String>) -> ProbeResult {
    match r {
        Ok(detail) => ProbeResult {
            name: name.into(),
            allowed: true,
            detail,
        },
        Err(e) => ProbeResult {
            name: name.into(),
            allowed: false,
            detail: e.to_string(),
        },
    }
}

fn probe(read_path: &str, write_dir: &str) -> Vec<ProbeResult> {
    use std::io::Read;
    let mut out = Vec::new();
    out.push(result(
        "read file",
        std::fs::File::open(read_path).and_then(|mut f| {
            let mut b = [0u8; 16];
            let n = f.read(&mut b)?;
            Ok(format!("read {n} bytes"))
        }),
    ));
    out.push(result("create file", {
        let p = std::path::Path::new(write_dir).join(format!("probe-{}", std::process::id()));
        std::fs::File::create(&p).map(|_| {
            let _ = std::fs::remove_file(&p);
            format!("created {}", p.display())
        })
    }));
    out.push(result(
        "list /",
        std::fs::read_dir("/").map(|d| format!("{} entries", d.count())),
    ));
    out.push(result(
        "udp socket",
        std::net::UdpSocket::bind("127.0.0.1:0").map(|s| format!("{:?}", s.local_addr())),
    ));
    out.push(result(
        "tcp connect",
        std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], 9)),
            // Windows retries a connection to a closed port for about a second before it
            // reports it refused; a shorter wait reads as "blocked" even without a sandbox.
            Duration::from_millis(if cfg!(windows) { 3000 } else { 200 }),
        )
        .map(|_| "connected".into())
        .or_else(|e| match e.kind() {
            // A refused connection means the socket existed and the attempt left the process.
            std::io::ErrorKind::ConnectionRefused => Ok(format!("attempted ({e})")),
            _ => Err(e),
        }),
    ));
    #[cfg(unix)]
    out.push(result(
        "unix socket",
        std::os::unix::net::UnixDatagram::unbound().map(|_| "created".into()),
    ));
    out.push(result(
        "run program",
        std::process::Command::new(program())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| format!("{s}")),
    ));
    #[cfg(unix)]
    out.push(result("signal parent", {
        let ppid = rustix::process::getppid();
        match ppid {
            Some(p) => rustix::process::test_kill_process(p)
                .map(|()| format!("may signal {}", p.as_raw_nonzero()))
                .map_err(std::io::Error::from),
            None => Err(std::io::Error::other("no parent")),
        }
    }));
    out
}

/// A harmless program that exists on every system of its kind.
fn program() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        std::path::Path::new(&root).join(r"System32\whoami.exe")
    }
    #[cfg(target_os = "macos")]
    {
        "/usr/bin/true".into()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        "/bin/true".into()
    }
}
