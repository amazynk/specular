//! `specular --remote COMMAND` and `specular ipc COMMAND`: controls a running viewer through
//! its socket and exits (`ipc watch` prints its events until stopped).
//!
//! ```text
//! specular [--socket PATH] [-0] --remote COMMAND [ARGUMENTS]
//! specular ipc [--socket PATH] [-0] COMMAND [ARGUMENTS]
//! ```
//!
//! `ipc` is a subcommand only as the first argument: a file named `ipc` opens as `./ipc` or
//! after `--`.
//!
//! The socket is `--socket PATH`, else `$SPECULAR_SOCKET`, else the first viewer's
//! (`$XDG_RUNTIME_DIR/specular/specular.sock`, or `specular/run/` in the cache folder). Exit
//! status: 0 done, 1 the viewer refused (the reason is on standard error), 2 bad usage,
//! 3 no viewer is running.

use std::ffi::OsString;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use specular_platform::ipc::{self, Line, Stream};

use super::protocol::{self, Nav, Request, Window};

pub const USAGE: &str = "Remote control (a running viewer, through its socket):\n  \
     --remote COMMAND       open [--new-window] FILE…|-, url URL, next, prev,\n                         \
     first, last, goto N, state, marked, action ID, activate,\n                         \
     quit, hello, raw (JSON requests from standard input),\n                         \
     select [--add] FILE…, mark [--flag pick|reject|none]\n                         \
     [--stars N] [--label COLOUR|none] FILE…, apply-style STYLE\n                         \
     FILE…, export RECIPE FILE…\n  \
     ipc COMMAND            the same (first argument only), and: watch [EVENT…]\n                         \
     [--json] (events until stopped), events (their names)\n  \
     --socket PATH          the viewer to control (default: $SPECULAR_SOCKET, else\n                         \
     the first viewer started)\n  \
     -0, --null             with --remote marked/open -: NUL-separated lists";

const TIMEOUT: Duration = Duration::from_secs(10);

/// What the command line asked for.
#[derive(Debug, PartialEq)]
pub struct Invocation {
    pub socket: Option<PathBuf>,
    pub null: bool,
    pub cmd: Cmd,
}

#[derive(Debug, PartialEq)]
pub enum Cmd {
    Send(Request),
    /// `open -`: the files come from standard input.
    OpenStdin(Window),
    /// Requests as JSON lines from standard input, responses to standard output.
    Raw,
    /// `ipc watch`: events (no names: all) until stopped, as text or JSON lines.
    Watch {
        events: Vec<String>,
        json: bool,
    },
    /// `ipc events`: the event names (no viewer needed).
    Events,
}

/// A failed request: the protocol's error code and message.
#[derive(Debug)]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    fn local(code: &str, e: impl std::fmt::Display) -> Error {
        Error {
            code: code.into(),
            message: e.to_string(),
        }
    }
}

/// A connection to a viewer.
pub struct Client {
    r: BufReader<Stream>,
    w: Stream,
    next_id: u64,
}

impl Client {
    /// Connects; where the viewer keeps a key next to its socket (Windows), or when a hook
    /// started this process (`$SPECULAR_HOOK`), a `hello` carrying them goes first.
    pub fn connect(path: &Path) -> std::io::Result<Client> {
        let s = ipc::connect(path, TIMEOUT)?;
        let mut c = Client {
            w: s.try_clone()?,
            r: BufReader::new(s),
            next_id: 1,
        };
        let (key, hook) = specular_client::protocol::introduction(path);
        if key.is_some() || hook.is_some() {
            let id = c.take_id();
            let line =
                specular_client::protocol::hello_line(1, id, key.as_deref(), hook.as_deref());
            if let Err(e) = c.answer(line.as_bytes(), id) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    e.message,
                ));
            }
        }
        Ok(c)
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Connects, retrying while the viewer is still coming up (or briefly full).
    #[cfg(test)]
    pub fn connect_with_retry(path: &Path, within: Duration) -> std::io::Result<Client> {
        let until = std::time::Instant::now() + within;
        loop {
            match Client::connect(path) {
                Ok(c) => return Ok(c),
                Err(e) if std::time::Instant::now() >= until => return Err(e),
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }

    /// Sends one line and returns the response line (event lines on the way are skipped:
    /// `watch` reads those with [`Client::read`]).
    pub fn exchange(&mut self, line: &[u8]) -> Result<Vec<u8>, Error> {
        ipc::write_line(&mut self.w, line).map_err(|e| Error::local("io", e))?;
        loop {
            let l = self.read()?;
            let event = serde_json::from_slice::<Value>(&l)
                .is_ok_and(|v| specular_client::protocol::is_event(&v));
            if !event {
                return Ok(l);
            }
        }
    }

    /// The next line from the viewer.
    pub fn read(&mut self) -> Result<Vec<u8>, Error> {
        match ipc::read_line(&mut self.r, protocol::MAX_LINE) {
            Ok(Line::Text(t)) => Ok(t),
            Ok(Line::TooLong) => Err(Error::local("too-long", "response too long")),
            Ok(Line::End) => Err(Error::local("io", "the viewer closed the connection")),
            Err(e) => Err(Error::local("io", e)),
        }
    }

    /// How long a read waits (`None`: for ever).
    pub fn set_read_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.r.get_ref().set_read_timeout(t)
    }

    /// Sends `req` and returns its `result`.
    pub fn call(&mut self, req: &Request) -> Result<Value, Error> {
        let id = self.take_id();
        self.answer(protocol::request_line(id, req).as_bytes(), id)
    }

    /// Sends `line` (a request with `id`) and returns its `result`.
    fn answer(&mut self, line: &[u8], id: u64) -> Result<Value, Error> {
        let line = self.exchange(line)?;
        let v: Value = serde_json::from_slice(&line).map_err(|e| Error::local("parse", e))?;
        // A `busy` notice arrives unasked, with no id.
        if v["ok"] == Value::Bool(true) && v["id"] == id {
            return Ok(v["result"].clone());
        }
        Err(Error {
            code: v["error"]["code"].as_str().unwrap_or("parse").to_owned(),
            message: v["error"]["message"]
                .as_str()
                .unwrap_or("malformed response")
                .to_owned(),
        })
    }
}

/// `Some(exit status)` when the arguments are a remote command (run here); `None` otherwise.
pub fn from_args() -> Option<i32> {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if is_ipc(&args) {
        return Some(match parse_ipc(&args[1..]) {
            Ok(None) => {
                println!("Usage: specular ipc [--socket PATH] [-0] COMMAND\n{USAGE}");
                0
            }
            Ok(Some(inv)) => run(inv),
            Err(msg) => {
                eprintln!("specular ipc: {msg} (try specular ipc --help)");
                2
            }
        });
    }
    if !is_remote(&args) {
        return None;
    }
    Some(match parse(&args) {
        Ok(inv) => run(inv),
        Err(msg) => {
            eprintln!("specular: {msg} (try --help)");
            2
        }
    })
}

/// Whether the first argument is the `ipc` subcommand (a file of that name is `./ipc`, or
/// comes after `--`).
fn is_ipc(args: &[OsString]) -> bool {
    args.first().is_some_and(|a| a == "ipc")
}

/// Parses what follows `ipc`: `[--socket PATH] [-0] COMMAND [ARGUMENTS]`. `None`: help.
pub fn parse_ipc(args: &[OsString]) -> Result<Option<Invocation>, String> {
    let mut socket = None;
    let mut null = false;
    let mut it = args.iter();
    let name = loop {
        let Some(a) = it.next() else {
            return Err("a command is needed".into());
        };
        let s = a.to_string_lossy();
        match s.as_ref() {
            "-h" | "--help" | "help" => return Ok(None),
            "--socket" => {
                socket = Some(PathBuf::from(it.next().ok_or("--socket needs a path")?));
            }
            _ if s.starts_with("--socket=") => socket = Some(PathBuf::from(&s[9..])),
            "-0" | "--null" => null = true,
            _ if s.starts_with('-') => return Err(format!("unknown option {s}")),
            _ => break s.into_owned(),
        }
    };
    let rest: Vec<&OsString> = it.collect();
    let cmd = match name.as_str() {
        "watch" => {
            let mut events = Vec::new();
            let mut json = false;
            for a in &rest {
                let s = a.to_string_lossy();
                match s.as_ref() {
                    "--json" => json = true,
                    e if protocol::EVENTS.contains(&e) => events.push(e.to_owned()),
                    e => {
                        return Err(format!(
                            "no event {e:?} (events: {})",
                            protocol::EVENTS.join(", ")
                        ));
                    }
                }
            }
            Cmd::Watch { events, json }
        }
        "events" if rest.is_empty() => Cmd::Events,
        "events" => return Err("ipc events takes no arguments".into()),
        _ => command(&name, &rest, "ipc")?,
    };
    Ok(Some(Invocation { socket, null, cmd }))
}

/// Whether `--remote` comes before any `--` (after which everything is a file name).
fn is_remote(args: &[OsString]) -> bool {
    args.iter()
        .map(|a| a.to_string_lossy())
        .take_while(|a| a != "--")
        .any(|a| a == "--remote" || a.starts_with("--remote="))
}

/// Parses `[--socket PATH] [-0] --remote COMMAND [ARGUMENTS]` (the options in any order
/// before the command).
pub fn parse(args: &[OsString]) -> Result<Invocation, String> {
    let mut socket = None;
    let mut null = false;
    let mut it = args.iter();
    let mut cmd_name = None;
    while let Some(a) = it.next() {
        let s = a.to_string_lossy();
        match s.as_ref() {
            "--socket" => {
                socket = Some(PathBuf::from(it.next().ok_or("--socket needs a path")?));
            }
            _ if s.starts_with("--socket=") => socket = Some(PathBuf::from(&s[9..])),
            "-0" | "--null" => null = true,
            "--remote" => {
                cmd_name = Some(
                    it.next()
                        .ok_or("--remote needs a command")?
                        .to_string_lossy()
                        .into_owned(),
                );
                break;
            }
            _ if s.starts_with("--remote=") => {
                cmd_name = Some(s[9..].to_owned());
                break;
            }
            _ => return Err(format!("{s} cannot be combined with --remote")),
        }
    }
    let name = cmd_name.ok_or("--remote needs a command")?;
    let rest: Vec<&OsString> = it.collect();
    let cmd = command(&name, &rest, "--remote")?;
    Ok(Invocation { socket, null, cmd })
}

/// The command `name` with its arguments `rest` (`how`: `--remote` or `ipc`, for messages).
fn command(name: &str, rest: &[&OsString], how: &str) -> Result<Cmd, String> {
    let none = |req: Request| -> Result<Cmd, String> {
        if rest.is_empty() {
            Ok(Cmd::Send(req))
        } else {
            Err(format!("{how} {name} takes no arguments"))
        }
    };
    let one = || -> Result<String, String> {
        match rest {
            [v] => Ok(v.to_string_lossy().into_owned()),
            _ => Err(format!("{how} {name} takes one argument")),
        }
    };
    // A name, then files.
    let named = |what: &str| -> Result<(String, Vec<PathBuf>), String> {
        let (first, files) = rest
            .split_first()
            .ok_or_else(|| format!("{how} {name} needs a {what} and files"))?;
        let files = paths(files);
        if files.is_empty() {
            return Err(format!("{how} {name} needs files after the {what}"));
        }
        Ok((first.to_string_lossy().into_owned(), files))
    };
    Ok(match name {
        "open" => open(rest)?,
        "url" | "open-url" => Cmd::Send(Request::OpenUrl { url: one()? }),
        "next" => none(Request::Navigate(Nav::Next))?,
        "prev" | "previous" => none(Request::Navigate(Nav::Prev))?,
        "first" => none(Request::Navigate(Nav::First))?,
        "last" => none(Request::Navigate(Nav::Last))?,
        "goto" => {
            let n: usize = one()?
                .parse()
                .ok()
                .filter(|n| *n >= 1)
                .ok_or("--remote goto takes a position from 1")?;
            Cmd::Send(Request::Navigate(Nav::Index(n - 1)))
        }
        "state" => none(Request::State)?,
        "marked" => none(Request::Marked)?,
        "action" => Cmd::Send(Request::Action { name: one()? }),
        "activate" => none(Request::Activate)?,
        "quit" => none(Request::Quit)?,
        "hello" => none(Request::Hello)?,
        "raw" if rest.is_empty() => Cmd::Raw,
        "raw" => return Err(format!("{how} raw takes no arguments")),
        "select" => {
            let add = rest.first().is_some_and(|a| *a == "--add");
            let files = paths(&rest[usize::from(add)..]);
            if files.is_empty() {
                return Err(format!("{how} select needs files"));
            }
            Cmd::Send(Request::Select { files, add })
        }
        "mark" => mark(rest, how)?,
        "apply-style" => {
            let (style, files) = named("style")?;
            Cmd::Send(Request::ApplyStyle { files, style })
        }
        "export" => {
            let (recipe, files) = named("recipe")?;
            Cmd::Send(Request::Export { files, recipe })
        }
        _ => return Err(format!("unknown remote command {name:?}")),
    })
}

/// Files as absolute paths; after `--` names that look like options are files too.
fn paths(rest: &[&OsString]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut only = false;
    for a in rest {
        if !only && *a == "--" {
            only = true;
            continue;
        }
        out.push(absolute(a));
    }
    out
}

/// `mark [--flag pick|reject|none] [--stars N] [--label COLOUR|none] FILE…`.
fn mark(rest: &[&OsString], how: &str) -> Result<Cmd, String> {
    let mut m = protocol::MarkSet::default();
    let mut i = 0;
    while i < rest.len() {
        let a = rest[i].to_string_lossy();
        let value = || {
            rest.get(i + 1)
                .map(|v| v.to_string_lossy().into_owned())
                .ok_or_else(|| format!("{a} needs a value"))
        };
        match a.as_ref() {
            "--flag" => {
                let v = value()?;
                m.flag =
                    Some(protocol::Flag::from_key(&v).ok_or("--flag is pick, reject or none")?);
            }
            "--stars" => {
                let v = value()?;
                m.stars = Some(
                    v.parse::<u8>()
                        .ok()
                        .filter(|s| *s <= 5)
                        .ok_or("--stars is 0 to 5")?,
                );
            }
            "--label" => {
                let v = value()?.to_ascii_lowercase();
                m.label = Some(if v == "none" {
                    0
                } else {
                    protocol::LABELS
                        .iter()
                        .position(|l| *l == v)
                        .ok_or("--label is red, yellow, green, blue or none")?
                        as u8
                        + 1
                });
            }
            _ => break,
        }
        i += 2;
    }
    if m.is_empty() {
        return Err(format!("{how} mark needs --flag, --stars or --label"));
    }
    let files = paths(&rest[i..]);
    if files.is_empty() {
        return Err(format!("{how} mark needs files"));
    }
    Ok(Cmd::Send(Request::Mark { files, mark: m }))
}

fn open(rest: &[&OsString]) -> Result<Cmd, String> {
    let mut window = Window::Current;
    let mut files = Vec::new();
    let mut only_paths = false;
    let mut stdin = false;
    for a in rest {
        let s = a.to_string_lossy();
        if !only_paths && s == "--new-window" {
            window = Window::New;
        } else if !only_paths && s == "--" {
            only_paths = true;
        } else if !only_paths && s == "-" {
            stdin = true;
        } else {
            files.push(absolute(a));
        }
    }
    match (stdin, files.is_empty()) {
        (true, true) => Ok(Cmd::OpenStdin(window)),
        (true, false) => Err("--remote open takes files or -, not both".into()),
        (false, true) => Err("--remote open needs files (or - to read them)".into()),
        (false, false) => Ok(Cmd::Send(Request::Open {
            files,
            window,
            view: None,
            compare: None,
        })),
    }
}

fn absolute(a: &OsString) -> PathBuf {
    let p = match a.to_str() {
        Some(s) if s.len() >= 7 && s[..7].eq_ignore_ascii_case("file://") => {
            specular_platform::launch::path_from_uri(&format!("file://{}", &s[7..]))
                .unwrap_or_else(|| PathBuf::from(a))
        }
        _ => PathBuf::from(a),
    };
    std::path::absolute(&p).unwrap_or(p)
}

/// The socket to use: `--socket`, `$SPECULAR_SOCKET`, else the first viewer's.
pub fn socket_path(given: Option<PathBuf>) -> Option<PathBuf> {
    given
        .or_else(|| std::env::var_os("SPECULAR_SOCKET").map(PathBuf::from))
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| ipc::existing_socket(&fallback_dir()))
}

/// Where the socket goes when the platform has no private runtime folder.
pub fn fallback_dir() -> PathBuf {
    specular_settings::Locations::detect().cache_dir.join("run")
}

fn run(inv: Invocation) -> i32 {
    if inv.cmd == Cmd::Events {
        for e in protocol::EVENTS {
            println!("{e}");
        }
        return 0;
    }
    let Some(path) = socket_path(inv.socket.clone()) else {
        eprintln!("specular: no running viewer to control");
        return 3;
    };
    let mut c = match Client::connect(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("specular: no running viewer at {} ({e})", path.display());
            return 3;
        }
    };
    let req = match inv.cmd {
        Cmd::Raw => return raw(&mut c),
        Cmd::Watch { events, json } => {
            return watch(&mut c, events, json, &mut std::io::stdout().lock());
        }
        Cmd::Events => return 0,
        Cmd::Send(r) => r,
        Cmd::OpenStdin(window) => match crate::file_list::read(Path::new("-"), inv.null) {
            Ok((files, _)) if files.is_empty() => {
                eprintln!("specular: no existing files on standard input");
                return 1;
            }
            Ok((files, _)) if files.len() > protocol::MAX_FILES => {
                eprintln!("specular: more than {} files", protocol::MAX_FILES);
                return 2;
            }
            Ok((files, _)) => Request::Open {
                files,
                window,
                view: None,
                compare: None,
            },
            Err(e) => {
                eprintln!("specular: standard input: {e}");
                return 1;
            }
        },
    };
    match c.call(&req) {
        Ok(v) => print_result(&req, &v, inv.null),
        Err(e) => {
            eprintln!("specular: {} ({})", e.message, e.code);
            1
        }
    }
}

fn print_result(req: &Request, v: &Value, null: bool) -> i32 {
    match req {
        Request::State | Request::Hello => println!("{v}"),
        Request::Marked => {
            let files: Vec<PathBuf> = v["files"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect();
            if crate::file_list::print(&files, null).is_err() {
                return 1;
            }
        }
        Request::Open { .. } => {
            for m in v["missing"].as_array().into_iter().flatten() {
                eprintln!("specular: not found: {}", m.as_str().unwrap_or("?"));
            }
            if v["opened"].as_u64() == Some(0) {
                return 1;
            }
        }
        Request::OpenUrl { .. } if v["status"] == "confirm" => {
            eprintln!("specular: the viewer asks for confirmation");
        }
        Request::Select { .. } | Request::Mark { .. } => {
            for m in v["missing"].as_array().into_iter().flatten() {
                eprintln!("specular: not found: {}", m.as_str().unwrap_or("?"));
            }
        }
        Request::ApplyStyle { .. } | Request::Export { .. } => println!("{v}"),
        _ => {}
    }
    0
}

/// `ipc watch`: subscribes and prints each event until stopped (or the viewer goes). Text
/// lines are `EVENT<TAB>FILE` per file (the event alone when it names none); `--json` prints
/// the lines as they come. Lost events are noted on standard error.
fn watch(c: &mut Client, events: Vec<String>, json: bool, out: &mut impl Write) -> i32 {
    let events = if events.is_empty() {
        vec!["*".to_owned()]
    } else {
        events
    };
    if let Err(e) = c.call(&Request::Subscribe { events }) {
        eprintln!("specular: {} ({})", e.message, e.code);
        return 1;
    }
    // The viewer pings every minute; longer silence means it is gone.
    let _ = c.set_read_timeout(Some(Duration::from_secs(150)));
    watch_lines(c, out, json)
}

/// The event lines of a subscribed connection, printed to `out` until it ends.
fn watch_lines(c: &mut Client, out: &mut impl Write, json: bool) -> i32 {
    loop {
        let line = match c.read() {
            Ok(l) => l,
            Err(e) if e.code == "io" && e.message.contains("closed") => {
                eprintln!("specular: the viewer closed the connection");
                return 0;
            }
            Err(e) => {
                eprintln!("specular: {} ({})", e.message, e.code);
                return 1;
            }
        };
        let Ok(v) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if !specular_client::protocol::is_event(&v) {
            continue;
        }
        let r = match v["event"].as_str().unwrap_or("") {
            "ping" => Ok(()),
            "lost" => {
                eprintln!(
                    "specular: {} events were lost (read too slowly)",
                    v["count"].as_u64().unwrap_or(0)
                );
                Ok(())
            }
            _ if json => ipc::write_line(out, &line),
            name => watch_text(out, name, &v),
        };
        if r.is_err() {
            // Standard output closed (`| head`): done.
            return 0;
        }
    }
}

fn watch_text(out: &mut impl Write, name: &str, v: &Value) -> std::io::Result<()> {
    let files = v["files"].as_array().map_or(&[][..], Vec::as_slice);
    if files.is_empty() {
        writeln!(out, "{name}")?;
    }
    for f in files {
        writeln!(out, "{name}\t{}", f.as_str().unwrap_or(""))?;
    }
    out.flush()
}

/// `--remote raw`: each line of standard input as a request, each response line out.
fn raw(c: &mut Client) -> i32 {
    let mut input = BufReader::new(std::io::stdin().lock());
    let mut out = std::io::stdout().lock();
    let mut code = 0;
    loop {
        let line = match ipc::read_line(&mut input, protocol::MAX_LINE) {
            Ok(Line::Text(l)) => l,
            Ok(Line::End) => return code,
            Ok(Line::TooLong) => {
                eprintln!("specular: request longer than {} bytes", protocol::MAX_LINE);
                return 2;
            }
            Err(e) => {
                eprintln!("specular: standard input: {e}");
                return 1;
            }
        };
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match c.exchange(&line) {
            Ok(resp) => {
                let ok = serde_json::from_slice::<Value>(&resp).is_ok_and(|v| v["ok"] == true);
                if !ok {
                    code = 1;
                }
                if ipc::write_line(&mut out, &resp).is_err() {
                    return 1;
                }
            }
            Err(e) => {
                eprintln!("specular: {} ({})", e.message, e.code);
                return 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Invocation, String> {
        let v: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse(&v)
    }

    fn cmd(args: &[&str]) -> Cmd {
        p(args).unwrap().cmd
    }

    #[test]
    fn detects_remote() {
        let v = |a: &[&str]| is_remote(&a.iter().map(OsString::from).collect::<Vec<_>>());
        assert!(v(&["--remote", "next"]));
        assert!(v(&["-0", "--remote=marked"]));
        assert!(!v(&["photo.jpg"]));
        assert!(!v(&["--", "--remote"]));
    }

    #[test]
    fn commands() {
        assert_eq!(
            cmd(&["--remote", "next"]),
            Cmd::Send(Request::Navigate(Nav::Next))
        );
        assert_eq!(
            cmd(&["--remote=prev"]),
            Cmd::Send(Request::Navigate(Nav::Prev))
        );
        assert_eq!(
            cmd(&["--remote", "goto", "3"]),
            Cmd::Send(Request::Navigate(Nav::Index(2)))
        );
        assert_eq!(
            cmd(&["--remote", "action", "zoom-to:200"]),
            Cmd::Send(Request::Action {
                name: "zoom-to:200".into()
            })
        );
        assert_eq!(cmd(&["--remote", "raw"]), Cmd::Raw);
        let inv = p(&["--socket", "/s/x.sock", "-0", "--remote", "marked"]).unwrap();
        assert_eq!(inv.socket, Some(PathBuf::from("/s/x.sock")));
        assert!(inv.null);
        assert_eq!(inv.cmd, Cmd::Send(Request::Marked));
        assert_eq!(
            p(&["--socket=/a", "--remote", "state"]).unwrap().socket,
            Some(PathBuf::from("/a"))
        );
        assert_eq!(
            cmd(&["--remote", "open", "-"]),
            Cmd::OpenStdin(Window::Current)
        );
        assert_eq!(
            cmd(&["--remote", "open", "--new-window", "-"]),
            Cmd::OpenStdin(Window::New)
        );
        let Cmd::Send(Request::Open { files, window, .. }) =
            cmd(&["--remote", "open", "a.jpg", "/b.jpg", "--", "-"])
        else {
            panic!()
        };
        assert_eq!(window, Window::Current);
        assert!(files.iter().all(|f| f.is_absolute()));
        assert_eq!(files[0], std::env::current_dir().unwrap().join("a.jpg"));
        assert_eq!(files[2], std::env::current_dir().unwrap().join("-"));
    }

    #[test]
    fn usage_errors() {
        for bad in [
            &["--remote"][..],
            &["--remote", "fly"],
            &["--remote", "next", "extra"],
            &["--remote", "goto", "0"],
            &["--remote", "goto", "x"],
            &["--remote", "action"],
            &["--remote", "open"],
            &["--remote", "open", "-", "a.jpg"],
            &["--remote", "raw", "x"],
            &["--lang", "de", "--remote", "next"],
            &["--socket"],
        ] {
            assert!(p(bad).is_err(), "{bad:?}");
        }
    }

    fn os(a: &[&str]) -> Vec<OsString> {
        a.iter().map(OsString::from).collect()
    }

    fn ipc(a: &[&str]) -> Result<Option<Invocation>, String> {
        parse_ipc(&os(a))
    }

    fn ipc_cmd(a: &[&str]) -> Cmd {
        ipc(a).unwrap().unwrap().cmd
    }

    /// `ipc` is the subcommand only as the first argument; a file named `ipc` opens as
    /// `./ipc` or after `--`.
    #[test]
    fn ipc_is_a_subcommand_only_first() {
        assert!(is_ipc(&os(&["ipc", "state"])));
        assert!(is_ipc(&os(&["ipc"])));
        for not in [
            &["./ipc"][..],
            &["--", "ipc"],
            &["photo.jpg", "ipc"],
            &["ipc.jpg"],
        ] {
            assert!(!is_ipc(&os(not)), "{not:?}");
            assert!(!is_remote(&os(not)), "{not:?}");
        }
        assert_eq!(ipc(&["--help"]), Ok(None));
        assert!(ipc(&[]).is_err());
    }

    #[test]
    fn ipc_commands() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(ipc_cmd(&["state"]), Cmd::Send(Request::State));
        assert_eq!(
            ipc_cmd(&["action", "zoom-in"]),
            Cmd::Send(Request::Action {
                name: "zoom-in".into()
            })
        );
        let inv = ipc(&["--socket", "/s/x.sock", "-0", "marked"])
            .unwrap()
            .unwrap();
        assert_eq!(
            (inv.socket, inv.null),
            (Some(PathBuf::from("/s/x.sock")), true)
        );
        assert_eq!(
            ipc_cmd(&["select", "--add", "a.jpg", "/b.jpg"]),
            Cmd::Send(Request::Select {
                files: vec![cwd.join("a.jpg"), std::path::absolute("/b.jpg").unwrap()],
                add: true
            })
        );
        assert_eq!(
            ipc_cmd(&[
                "mark", "--flag", "pick", "--stars", "4", "--label", "Red", "--", "--x.jpg"
            ]),
            Cmd::Send(Request::Mark {
                files: vec![cwd.join("--x.jpg")],
                mark: protocol::MarkSet {
                    flag: Some(protocol::Flag::Pick),
                    stars: Some(4),
                    label: Some(1),
                }
            })
        );
        assert_eq!(
            ipc_cmd(&["apply-style", "Warm film", "/a.cr3"]),
            Cmd::Send(Request::ApplyStyle {
                files: vec![PathBuf::from("/a.cr3")],
                style: "Warm film".into()
            })
        );
        assert_eq!(
            ipc_cmd(&["export", "Web", "/a.cr3", "/b.cr3"]),
            Cmd::Send(Request::Export {
                files: vec![PathBuf::from("/a.cr3"), PathBuf::from("/b.cr3")],
                recipe: "Web".into()
            })
        );
        assert_eq!(
            ipc_cmd(&["watch", "marked", "exported", "--json"]),
            Cmd::Watch {
                events: vec!["marked".into(), "exported".into()],
                json: true
            }
        );
        assert_eq!(
            ipc_cmd(&["watch"]),
            Cmd::Watch {
                events: vec![],
                json: false
            }
        );
        assert_eq!(ipc_cmd(&["events"]), Cmd::Events);
        // `--remote` takes the new commands too.
        assert_eq!(
            cmd(&["--remote", "mark", "--stars", "0", "/a.jpg"]),
            Cmd::Send(Request::Mark {
                files: vec![PathBuf::from("/a.jpg")],
                mark: protocol::MarkSet {
                    stars: Some(0),
                    ..Default::default()
                }
            })
        );
        for bad in [
            &["watch", "clicked"][..],
            &["events", "x"],
            &["mark", "/a.jpg"],
            &["mark", "--stars", "6", "/a.jpg"],
            &["mark", "--flag", "maybe", "/a.jpg"],
            &["mark", "--label", "purple", "/a.jpg"],
            &["mark", "--stars", "1"],
            &["select"],
            &["select", "--add"],
            &["export", "Web"],
            &["apply-style"],
            &["--bogus", "state"],
            &["fly"],
        ] {
            assert!(ipc(bad).is_err(), "{bad:?}");
        }
    }

    /// `ipc watch` against a stand-in viewer: the events it publishes come out as text
    /// lines (one per file) or as JSON, pings and others' answers left out; it ends when
    /// the viewer goes.
    #[test]
    #[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
    fn ipc_watch_prints_events() {
        use crate::remote::events::Event;
        use crate::remote::server::{Options, Server};
        for json in [false, true] {
            let dir = ipc::scratch_dir(&format!("specular-watch-{json}")).unwrap();
            let server = Server::start_opts(
                &dir,
                std::sync::Arc::new(|| {}),
                Options {
                    ping: Duration::from_millis(100),
                    ..Options::default()
                },
            )
            .unwrap();
            let bus = server.bus().clone();
            let mut c = Client::connect(server.path()).unwrap();
            let t = std::thread::spawn(move || {
                let mut out = Vec::new();
                let code = watch(&mut c, vec!["marked".into()], json, &mut out);
                (code, String::from_utf8(out).unwrap())
            });
            // Until the subscription is in, publishing reaches nobody.
            let until = std::time::Instant::now() + Duration::from_secs(20);
            while bus.subscribers() == 0 {
                assert!(std::time::Instant::now() < until);
                std::thread::sleep(Duration::from_millis(10));
            }
            let files = vec![PathBuf::from("/p/a.cr3"), PathBuf::from("/p/b c.cr3")];
            let none = serde_json::Map::new();
            bus.publish(&Event::new("opened", files.clone()), &none);
            bus.publish(&Event::new("marked", files), &none);
            std::thread::sleep(Duration::from_millis(300));
            drop(server);
            let (code, out) = t.join().unwrap();
            assert_eq!(code, 0);
            if json {
                let v: Value = serde_json::from_str(out.trim()).unwrap();
                assert_eq!(v["event"], "marked");
                assert_eq!(out.lines().count(), 1, "{out}");
            } else {
                assert_eq!(out, "marked\t/p/a.cr3\nmarked\t/p/b c.cr3\n");
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
