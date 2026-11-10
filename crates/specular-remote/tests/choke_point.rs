//! The no-network switch works because there is one way out: `specular_remote::Client`.
//! This test reads the workspace's sources and manifests and fails when a new path to a
//! socket appears anywhere else (a raw `TcpStream`, a `UdpSocket`, a name lookup, `ureq`
//! outside this crate's client, or a networking crate in a Cargo.toml). A new one must go
//! through the client, or be added here with the reason it cannot reach the network.

use std::path::{Path, PathBuf};

/// Patterns that open or resolve sockets.
const SOCKET: &[&str] = &[
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "to_socket_addrs",
    "lookup_host",
    "ureq::",
];

/// Files allowed to use them, and why.
const ALLOWED: &[(&str, &str)] = &[
    // The client itself: checks the switch before every request.
    ("crates/specular-remote/src/http.rs", "the choke point"),
    (
        "crates/specular-remote/src/resume.rs",
        "reads bodies the client opened",
    ),
    // Loopback-only test servers.
    (
        "crates/specular-remote/src/testing.rs",
        "loopback test server",
    ),
    ("crates/specular-remote/tests/", "loopback tests"),
    // The decode host's sandbox self-test: proves sockets are blocked inside it.
    (
        "crates/specular-decode-host/src/testops.rs",
        "sandbox network-block check",
    ),
    // The DICOM upper layer: `assoc::connect` checks the no-network switch before any name
    // lookup or socket, as the client does.
    (
        "crates/specular-dimse/src/assoc.rs",
        "DICOM associations, behind the switch",
    ),
    ("crates/specular-dimse/src/scp.rs", "loopback test SCP"),
    // TLS around the connections `assoc` (behind the switch) and `listen` opened.
    (
        "crates/specular-dimse/src/tls.rs",
        "wraps connections opened behind the switch",
    ),
    (
        "crates/specular-dicomweb/tests/dimse_load.rs",
        "loopback tests (a free port for C-MOVE)",
    ),
    // C-MOVE's temporary storage SCP: bound only after the no-network switch is checked,
    // only while one move runs, on the address and port the user set, accepting only
    // associations for our AE title; closed when the move ends.
    (
        "crates/specular-dimse/src/listen.rs",
        "C-MOVE storage SCP, behind the switch, only during a move",
    ),
    ("crates/specular-dimse/tests/", "loopback tests"),
    // SAMP (off by default): `http::local` accepts only this computer's own addresses and
    // name before any socket, and the callback server binds 127.0.0.1.
    (
        "crates/specular-samp/src/http.rs",
        "SAMP, this computer only",
    ),
    // Casting to a TV or speaker on the local network (DLNA): `off::check` comes before every
    // socket (discovery, the renderer's control requests, the file server), and the file
    // server binds only the interface that reaches the renderer and answers only it.
    (
        "crates/specular-cast/src/",
        "casting on the local network, behind the switch",
    ),
    ("crates/specular-cast/tests/", "loopback tests"),
    // This file names the patterns.
    ("crates/specular-remote/tests/choke_point.rs", "this test"),
];

/// Crates that open sockets (only specular-remote may depend on one).
const NET_CRATES: &[&str] = &[
    "ureq",
    "reqwest",
    "hyper",
    "tokio",
    "socket2",
    "mio",
    "curl",
    "isahc",
    "attohttpc",
    "async-std",
    "surf",
    "hickory-resolver",
    "trust-dns-resolver",
    "tungstenite",
    "quinn",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn rel(p: &Path) -> String {
    let root = root().canonicalize().unwrap();
    p.canonicalize()
        .unwrap()
        .strip_prefix(&root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn only_the_client_opens_sockets() {
    let mut files = Vec::new();
    for d in ["crates", "app", "clients", "tools"] {
        walk(&root().join(d), &mut files);
    }
    let mut bad = Vec::new();
    let mut seen = 0;
    for f in &files {
        if f.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let r = rel(f);
        if ALLOWED.iter().any(|(a, _)| r.starts_with(a)) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        seen += 1;
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if let Some(p) = SOCKET.iter().find(|p| code.contains(**p)) {
                bad.push(format!("{r}:{}: {p}", i + 1));
            }
        }
    }
    assert!(seen > 500, "only {seen} sources read: wrong root?");
    assert!(
        bad.is_empty(),
        "sockets outside specular_remote::Client (go through the client, or allow with a reason):\n{}",
        bad.join("\n")
    );
}

#[test]
fn only_specular_remote_depends_on_a_networking_crate() {
    let mut files = Vec::new();
    for d in ["crates", "app", "clients", "tools"] {
        walk(&root().join(d), &mut files);
    }
    files.push(root().join("Cargo.toml"));
    let mut bad = Vec::new();
    for f in files
        .iter()
        .filter(|f| f.file_name().is_some_and(|n| n == "Cargo.toml"))
    {
        let r = rel(f);
        let text = std::fs::read_to_string(f).unwrap();
        for line in text.lines() {
            let name = line.split(['=', '.']).next().unwrap_or("").trim();
            if NET_CRATES.contains(&name) && r != "crates/specular-remote/Cargo.toml" {
                bad.push(format!("{r}: {name}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "networking crates outside specular-remote:\n{}",
        bad.join("\n")
    );
}
