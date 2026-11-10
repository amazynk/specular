//! Running other programs with the current file: user commands (DESIGN.md §11, "Integration
//! and automation") and "Open with…" handlers.
//!
//! Programs are always started directly with an argument vector. No shell is involved, so a
//! file name can never be interpreted as shell syntax (`; rm -rf ~` is just an odd name). Users
//! who want a pipeline can name the shell as the program (`program = "sh"`,
//! `args = ["-c", "…", "sh", "{file}"]`), which keeps the file a separate argument.
//!
//! Placeholders in arguments ([`expand_args`]):
//!
//! | Placeholder | Becomes |
//! |---|---|
//! | `{file}` | the file's absolute path |
//! | `{files}` | every selected file, one argument each (must be a whole argument) |
//! | `{dir}` | the folder containing the file |
//! | `{name}` | the file name with extension (`IMG_1234.jpg`) |
//! | `{stem}` | the file name without extension (`IMG_1234`) |
//! | `{ext}` | the extension without the dot (`jpg`), empty if none |
//! | `{uri}` | the file as a `file://` URI |
//! | `{page}` | the current page or frame, starting at 1 |
//! | `{{`, `}}` | a literal `{` or `}` |

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

/// What a command runs on.
#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    /// The current file (absolute).
    pub file: &'a Path,
    /// The selected files (the current file alone until marking exists).
    pub files: &'a [PathBuf],
    /// Current page or frame, 1-based.
    pub page: u32,
}

/// A problem with an argument template, found when the command is loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateError {
    /// `{something}` that is not a known placeholder.
    Unknown(String),
    /// A `{` without its `}`, or a lone `}`.
    Unbalanced,
    /// `{files}` inside a longer argument (it expands to several arguments).
    FilesNotAlone,
    /// Another list (`{outputs}`) inside a longer argument.
    ListNotAlone(String),
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateError::Unknown(p) => write!(f, "unknown placeholder {{{p}}}"),
            TemplateError::Unbalanced => {
                f.write_str("unbalanced { or } (write {{ or }} for a literal brace)")
            }
            TemplateError::FilesNotAlone => f.write_str("{files} must be an argument of its own"),
            TemplateError::ListNotAlone(p) => write!(f, "{{{p}}} must be an argument of its own"),
        }
    }
}

impl std::error::Error for TemplateError {}

/// Placeholder names [`expand_args`] knows.
pub const PLACEHOLDERS: &[&str] = &["file", "files", "dir", "name", "stem", "ext", "uri", "page"];

/// One piece of a parsed argument.
#[derive(Debug, PartialEq)]
enum Piece<'a> {
    Text(String),
    Var(&'a str),
}

fn parse(arg: &str) -> Result<Vec<Piece<'_>>, TemplateError> {
    parse_in(arg, PLACEHOLDERS)
}

/// Parses `arg` with the placeholders `known`; `{files}` and `{outputs}` (lists) must stand
/// alone.
fn parse_in<'a>(arg: &'a str, known: &[&str]) -> Result<Vec<Piece<'a>>, TemplateError> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut rest = arg;
    while let Some(c) = rest.chars().next() {
        match c {
            '{' if rest.starts_with("{{") => {
                text.push('{');
                rest = &rest[2..];
            }
            '}' if rest.starts_with("}}") => {
                text.push('}');
                rest = &rest[2..];
            }
            '{' => {
                let end = rest.find('}').ok_or(TemplateError::Unbalanced)?;
                let name = &rest[1..end];
                if name.contains('{') {
                    return Err(TemplateError::Unbalanced);
                }
                if !known.contains(&name) {
                    return Err(TemplateError::Unknown(name.to_owned()));
                }
                if !text.is_empty() {
                    out.push(Piece::Text(std::mem::take(&mut text)));
                }
                out.push(Piece::Var(name));
                rest = &rest[end + 1..];
            }
            '}' => return Err(TemplateError::Unbalanced),
            c => {
                text.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    if !text.is_empty() || out.is_empty() {
        out.push(Piece::Text(text));
    }
    if out.len() > 1 && out.contains(&Piece::Var("files")) {
        return Err(TemplateError::FilesNotAlone);
    }
    if out.len() > 1 && out.contains(&Piece::Var("outputs")) {
        return Err(TemplateError::ListNotAlone("outputs".into()));
    }
    Ok(out)
}

/// Checks argument templates without expanding them.
pub fn validate_args<S: AsRef<str>>(args: &[S]) -> Result<(), TemplateError> {
    args.iter().try_for_each(|a| parse(a.as_ref()).map(drop))
}

/// Whether any argument mentions a file placeholder (otherwise the command runs without the
/// file, which is allowed: "Open folder in terminal" uses only `{dir}`).
pub fn mentions_file<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter()
        .any(|a| parse(a.as_ref()).is_ok_and(|p| p.iter().any(|p| matches!(p, Piece::Var(_)))))
}

/// Expands argument templates for `t`. Paths stay `OsString`s, so names that are not valid
/// UTF-8 reach the program unchanged.
pub fn expand_args<S: AsRef<str>>(args: &[S], t: &Target) -> Result<Vec<OsString>, TemplateError> {
    let mut out = Vec::with_capacity(args.len());
    for a in args {
        let pieces = parse(a.as_ref())?;
        if let [Piece::Var("files")] = pieces.as_slice() {
            if t.files.is_empty() {
                out.push(t.file.as_os_str().to_owned());
            } else {
                out.extend(t.files.iter().map(|f| f.as_os_str().to_owned()));
            }
            continue;
        }
        let mut s = OsString::new();
        for p in pieces {
            match p {
                Piece::Text(x) => s.push(x),
                Piece::Var(v) => s.push(var(v, t)),
            }
        }
        out.push(s);
    }
    Ok(out)
}

fn var(name: &str, t: &Target) -> OsString {
    let f = t.file;
    match name {
        "file" | "files" => f.as_os_str().to_owned(),
        "dir" => f
            .parent()
            .map(|p| p.as_os_str().to_owned())
            .unwrap_or_default(),
        "name" => f.file_name().map(OsStr::to_owned).unwrap_or_default(),
        "stem" => f.file_stem().map(OsStr::to_owned).unwrap_or_default(),
        "ext" => f.extension().map(OsStr::to_owned).unwrap_or_default(),
        "uri" => file_uri(f).into(),
        "page" => t.page.max(1).to_string().into(),
        _ => OsString::new(),
    }
}

/// Placeholder names of event hooks ([`expand_hook_args`]).
pub const HOOK_PLACEHOLDERS: &[&str] = &[
    "file", "files", "dir", "name", "stem", "ext", "uri", "event", "recipe", "outputs", "stars",
];

/// What an event hook runs on, beyond its files.
#[derive(Clone, Copy, Debug, Default)]
pub struct HookVars<'a> {
    /// The event's name (`marked`).
    pub event: &'a str,
    /// The export recipe's name (`exported`), else empty.
    pub recipe: &'a str,
    /// The files written (`exported`): `{outputs}`, one argument each.
    pub outputs: &'a [PathBuf],
    /// The stars set (`marked`), else empty.
    pub stars: Option<u8>,
}

/// Checks a hook's argument templates ([`HOOK_PLACEHOLDERS`]).
pub fn validate_hook_args<S: AsRef<str>>(args: &[S]) -> Result<(), TemplateError> {
    args.iter()
        .try_for_each(|a| parse_in(a.as_ref(), HOOK_PLACEHOLDERS).map(drop))
}

/// Expands a hook's argument templates: `{file}` is the first of `files` (empty when there
/// are none), `{files}` and `{outputs}` give one argument per path. Nothing is quoted: the
/// program is started without a shell, so names reach it exactly.
pub fn expand_hook_args<S: AsRef<str>>(
    args: &[S],
    files: &[PathBuf],
    vars: &HookVars,
) -> Result<Vec<OsString>, TemplateError> {
    let first = files.first().map_or(Path::new(""), PathBuf::as_path);
    let t = Target {
        file: first,
        files,
        page: 1,
    };
    let mut out = Vec::with_capacity(args.len());
    for a in args {
        let pieces = parse_in(a.as_ref(), HOOK_PLACEHOLDERS)?;
        match pieces.as_slice() {
            [Piece::Var("files")] => {
                out.extend(files.iter().map(|f| f.as_os_str().to_owned()));
                continue;
            }
            [Piece::Var("outputs")] => {
                out.extend(vars.outputs.iter().map(|f| f.as_os_str().to_owned()));
                continue;
            }
            _ => {}
        }
        let mut s = OsString::new();
        for p in pieces {
            match p {
                Piece::Text(x) => s.push(x),
                Piece::Var("event") => s.push(vars.event),
                Piece::Var("recipe") => s.push(vars.recipe),
                Piece::Var("stars") => {
                    if let Some(n) = vars.stars {
                        s.push(n.to_string());
                    }
                }
                Piece::Var(_) if files.is_empty() => {}
                Piece::Var(v) => s.push(var(v, &t)),
            }
        }
        out.push(s);
    }
    Ok(out)
}

/// `file://` URI of an absolute path, percent-encoding everything but unreserved characters
/// and `/` (RFC 8089; what freedesktop `text/uri-list` and `%u` expect).
pub fn file_uri(p: &Path) -> String {
    let mut s = String::from("file://");
    #[cfg(windows)]
    {
        // file:///C:/dir/name
        s.push('/');
        let text = p.to_string_lossy().replace('\\', "/");
        encode_into(&mut s, text.as_bytes());
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::ffi::OsStrExt;
        encode_into(&mut s, p.as_os_str().as_bytes());
    }
    s
}

fn encode_into(s: &mut String, bytes: &[u8]) {
    for &b in bytes {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) || (cfg!(windows) && b == b':') {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
}

/// Decodes a `file://` URI (`file:///p`, `file://localhost/p`) to a path. Other hosts and
/// schemes give `None`.
pub fn path_from_uri(uri: &str) -> Option<PathBuf> {
    let rest = uri.trim().strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let b = rest.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    #[cfg(windows)]
    {
        // file:///C:/x → C:/x
        let s = String::from_utf8(out).ok()?;
        let s = s.strip_prefix('/').unwrap_or(&s);
        Some(PathBuf::from(s))
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(OsString::from_vec(out)))
    }
}

/// Lists, comma-separated, the environment variables Specular set on itself at start-up (Vulkan
/// loader tuning for this machine), which programs it starts must not inherit.
pub const SCRUB_ENV: &str = "SPECULAR_SCRUB_ENV";

/// Removes the variables named in [`SCRUB_ENV`] (and it) from `cmd`'s environment.
pub fn scrub_env(cmd: &mut Command) {
    if let Some(list) = std::env::var_os(SCRUB_ENV) {
        for name in list.to_string_lossy().split(',').filter(|n| !n.is_empty()) {
            cmd.env_remove(name);
        }
        cmd.env_remove(SCRUB_ENV);
    }
}

/// Called once when a started program exits (or could not be waited for).
pub type OnExit = Box<dyn FnOnce(io::Result<ExitStatus>) + Send + 'static>;

/// Starts `program` with `args`, detached from Specular's terminal input. Returns once the
/// process exists; a background thread waits for it (so it never lingers as a zombie) and
/// calls `on_exit` with how it ended.
pub fn spawn(
    program: &OsStr,
    args: &[OsString],
    cwd: Option<&Path>,
    on_exit: Option<OnExit>,
) -> io::Result<()> {
    let mut cmd = Command::new(program);
    scrub_env(&mut cmd);
    cmd.args(args).stdin(Stdio::null());
    if let Some(d) = cwd.filter(|d| d.is_dir()) {
        cmd.current_dir(d);
    }
    let mut child = cmd.spawn()?;
    specular_log::debug!(target: specular_log::STARTUP, "started {:?} {:?} (pid {})", program, args, child.id());
    specular_thread::builder("plat-child").spawn(move || {
        let r = child.wait();
        if let Some(f) = on_exit {
            f(r);
        }
    })?;
    Ok(())
}

/// As [`spawn`], but stops the program once `timeout` has passed: `on_exit` then gets an
/// error of kind [`io::ErrorKind::TimedOut`]. Output goes where Specular's goes; nothing is
/// read from the program.
pub fn spawn_with_timeout(
    program: &OsStr,
    args: &[OsString],
    cwd: Option<&Path>,
    timeout: std::time::Duration,
    on_exit: OnExit,
) -> io::Result<()> {
    let mut cmd = Command::new(program);
    scrub_env(&mut cmd);
    cmd.args(args).stdin(Stdio::null());
    if let Some(d) = cwd.filter(|d| d.is_dir()) {
        cmd.current_dir(d);
    }
    let mut child = cmd.spawn()?;
    specular_log::debug!(target: specular_log::STARTUP, "started {:?} {:?} (pid {}, at most {timeout:?})", program, args, child.id());
    let until = std::time::Instant::now() + timeout;
    specular_thread::builder("plat-child").spawn(move || {
        let r = loop {
            match child.try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) if std::time::Instant::now() >= until => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("stopped after {} s", timeout.as_secs()),
                    ));
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(25)),
                Err(e) => break Err(e),
            }
        };
        on_exit(r);
    })?;
    Ok(())
}

/// How [`spawn_piped`] starts a program.
pub struct Piped<'a> {
    pub program: &'a OsStr,
    pub args: &'a [OsString],
    pub cwd: Option<&'a Path>,
    /// Variables set for the program (after [`scrub_env`]).
    pub env: &'a [(&'a str, OsString)],
    /// Written to the program's standard input, which is then closed.
    pub input: Vec<u8>,
    /// The program is stopped once this has passed.
    pub timeout: std::time::Duration,
}

/// As [`spawn_with_timeout`], with `p.input` on the program's standard input (written from a
/// thread of its own, so a program that never reads cannot hold anything up) and `p.env` set.
pub fn spawn_piped(p: Piped, on_exit: OnExit) -> io::Result<u32> {
    let mut cmd = Command::new(p.program);
    scrub_env(&mut cmd);
    cmd.args(p.args).stdin(Stdio::piped());
    for (k, v) in p.env {
        cmd.env(k, v);
    }
    if let Some(d) = p.cwd.filter(|d| d.is_dir()) {
        cmd.current_dir(d);
    }
    let mut child = cmd.spawn()?;
    let pid = child.id();
    specular_log::debug!(target: specular_log::STARTUP, "started {:?} {:?} (pid {pid}, at most {:?})", p.program, p.args, p.timeout);
    if let Some(mut stdin) = child.stdin.take() {
        let input = p.input;
        // A program that exits without reading closes the pipe: that is not an error.
        specular_thread::builder("plat-stdin").spawn(move || {
            let _ = io::Write::write_all(&mut stdin, &input);
        })?;
    }
    let (timeout, until) = (p.timeout, std::time::Instant::now() + p.timeout);
    specular_thread::builder("plat-child").spawn(move || {
        let r = loop {
            match child.try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) if std::time::Instant::now() >= until => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("stopped after {} s", timeout.as_secs()),
                    ));
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(25)),
                Err(e) => break Err(e),
            }
        };
        on_exit(r);
    })?;
    Ok(pid)
}

/// Quotes `s` for a POSIX shell command line (single quotes).
pub fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-+=:,@%".contains(&b))
    {
        return s.to_owned();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Whether `program` can be found: an existing path, or a name on `PATH` (with `PATHEXT` on
/// Windows). Used to grey out commands whose program is missing.
pub fn find_program(program: &str) -> Option<PathBuf> {
    let p = Path::new(program);
    if p.components().count() > 1 || p.is_absolute() {
        return p.is_file().then(|| p.to_path_buf());
    }
    let exts: Vec<String> = if cfg!(windows) {
        let pe = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
        std::iter::once(String::new())
            .chain(pe.split(';').map(str::to_owned))
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| {
        exts.iter().find_map(|e| {
            let c = dir.join(format!("{program}{e}"));
            c.is_file().then_some(c)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(file: &Path) -> Target<'_> {
        Target {
            file,
            files: &[],
            page: 3,
        }
    }

    #[test]
    fn hook_placeholders_expand() {
        let files = vec![
            PathBuf::from("/p/a $(rm -rf ~).jpg"),
            PathBuf::from("/p/b;c.jpg"),
        ];
        let outputs = vec![PathBuf::from("/out/a.jpg")];
        let v = HookVars {
            event: "exported",
            recipe: "Web",
            outputs: &outputs,
            stars: Some(4),
        };
        let args = [
            "{files}",
            "{file}",
            "{dir}",
            "--e={event}",
            "{recipe}",
            "{outputs}",
            "{stars}",
        ];
        let out = expand_hook_args(&args, &files, &v).unwrap();
        let out: Vec<String> = out
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            out,
            [
                "/p/a $(rm -rf ~).jpg",
                "/p/b;c.jpg",
                "/p/a $(rm -rf ~).jpg",
                "/p",
                "--e=exported",
                "Web",
                "/out/a.jpg",
                "4"
            ]
        );
        // No files: the file placeholders are empty, `{files}` gives nothing.
        let out = expand_hook_args(&["{files}", "x{file}"], &[], &HookVars::default()).unwrap();
        assert_eq!(out, [OsString::from("x")]);
        assert!(validate_hook_args(&["{event}", "{outputs}"]).is_ok());
        assert_eq!(
            validate_hook_args(&["a{outputs}"]),
            Err(TemplateError::ListNotAlone("outputs".into()))
        );
        assert!(validate_hook_args(&["{page}"]).is_err());
        // The event placeholders are hooks' only.
        assert!(validate_args(&["{event}"]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn piped_programs_get_their_input() {
        let dir = std::env::temp_dir().join(format!("specular-piped-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out.txt");
        let args: Vec<OsString> = vec![
            "-c".into(),
            "cat > \"$1\"; printf '%s' \"$SPECULAR_EVENT\" >> \"$1\"".into(),
            "sh".into(),
            out.clone().into_os_string(),
        ];
        let (tx, rx) = std::sync::mpsc::channel();
        spawn_piped(
            Piped {
                program: OsStr::new("/bin/sh"),
                args: &args,
                cwd: None,
                env: &[("SPECULAR_EVENT", "marked".into())],
                input: b"{\"event\":\"marked\"}\n".to_vec(),
                timeout: std::time::Duration::from_secs(20),
            },
            Box::new(move |r| {
                let _ = tx.send(r.map(|s| s.success()));
            }),
        )
        .unwrap();
        let r = rx.recv_timeout(std::time::Duration::from_secs(30)).unwrap();
        assert!(r.unwrap());
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "{\"event\":\"marked\"}\nmarked"
        );
        // One that never ends is stopped.
        let (tx, rx) = std::sync::mpsc::channel();
        let t0 = std::time::Instant::now();
        spawn_piped(
            Piped {
                program: OsStr::new("/bin/sh"),
                args: &["-c".into(), "sleep 30".into()],
                cwd: None,
                env: &[],
                input: Vec::new(),
                timeout: std::time::Duration::from_millis(300),
            },
            Box::new(move |r| {
                let _ = tx.send(r.map_err(|e| e.kind()));
            }),
        )
        .unwrap();
        let r = rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap();
        assert_eq!(r.unwrap_err(), io::ErrorKind::TimedOut);
        assert!(t0.elapsed() < std::time::Duration::from_secs(10));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn placeholders_expand() {
        let f = Path::new("/photos/2024/IMG 1234.jpeg");
        let args = [
            "{file}",
            "--out={dir}/{stem}.png",
            "{name}",
            "{ext}",
            "p{page}",
            "{{x}}",
        ];
        let got = expand_args(&args, &t(f)).unwrap();
        let want: Vec<OsString> = [
            "/photos/2024/IMG 1234.jpeg",
            "--out=/photos/2024/IMG 1234.png",
            "IMG 1234.jpeg",
            "jpeg",
            "p3",
            "{x}",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn files_expand_to_several_arguments() {
        let f = PathBuf::from("/a/1.jpg");
        let files = [f.clone(), PathBuf::from("/a/2.jpg")];
        let tg = Target {
            file: &f,
            files: &files,
            page: 1,
        };
        let got = expand_args(&["-x", "{files}"], &tg).unwrap();
        assert_eq!(got, ["-x", "/a/1.jpg", "/a/2.jpg"].map(OsString::from));
        // Nothing selected: the current file.
        let got = expand_args(&["{files}"], &t(&f)).unwrap();
        assert_eq!(got, [OsString::from("/a/1.jpg")]);
    }

    #[test]
    fn shell_syntax_stays_one_argument() {
        let f = Path::new("/tmp/a; rm -rf ~ $(x) `y`.jpg");
        let got = expand_args(&["{file}"], &t(f)).unwrap();
        assert_eq!(got, [OsString::from(f)]);
    }

    #[test]
    fn bad_templates_are_reported() {
        assert_eq!(
            validate_args(&["{fiel}"]),
            Err(TemplateError::Unknown("fiel".into()))
        );
        assert_eq!(validate_args(&["{file"]), Err(TemplateError::Unbalanced));
        assert_eq!(validate_args(&["a}b"]), Err(TemplateError::Unbalanced));
        assert_eq!(
            validate_args(&["x{files}"]),
            Err(TemplateError::FilesNotAlone)
        );
        assert_eq!(validate_args(&["", "{{}}", "plain"]), Ok(()));
        assert!(mentions_file(&["--x", "{dir}"]));
        assert!(!mentions_file(&["--version"]));
    }

    #[cfg(unix)]
    #[test]
    fn uris_round_trip() {
        let p = Path::new("/home/me/a b%c/ü.jpg");
        let u = file_uri(p);
        assert_eq!(u, "file:///home/me/a%20b%25c/%C3%BC.jpg");
        assert_eq!(path_from_uri(&u).as_deref(), Some(p));
        assert_eq!(
            path_from_uri("file://localhost/x.png").as_deref(),
            Some(Path::new("/x.png"))
        );
        assert_eq!(path_from_uri("file://host/x.png"), None);
        assert_eq!(path_from_uri("https://x/y.png"), None);
    }

    #[cfg(unix)]
    #[test]
    fn spawn_runs_without_a_shell() {
        let (tx, rx) = std::sync::mpsc::channel();
        // `true` exists on every Unix; its exit status comes back through `on_exit`.
        spawn(
            OsStr::new("true"),
            &[OsString::from("; false")],
            None,
            Some(Box::new(move |r| {
                let _ = tx.send(r.map(|s| s.success()));
            })),
        )
        .unwrap();
        assert!(
            rx.recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
                .unwrap()
        );
        assert!(spawn(OsStr::new("/nonexistent/program"), &[], None, None).is_err());
        assert!(find_program("sh").is_some());
        assert!(find_program("surely-not-a-program-xyz").is_none());
    }

    #[test]
    fn shell_quoting() {
        assert_eq!(shell_quote("/a/b-c.png"), "/a/b-c.png");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("$(rm -rf ~)"), "'$(rm -rf ~)'");
        assert_eq!(shell_quote(""), "''");
    }

    #[cfg(unix)]
    #[test]
    fn a_program_past_its_time_is_stopped() {
        let (tx, rx) = std::sync::mpsc::channel();
        let t0 = std::time::Instant::now();
        spawn_with_timeout(
            OsStr::new("sleep"),
            &["30".into()],
            None,
            std::time::Duration::from_millis(300),
            Box::new(move |r| {
                let _ = tx.send(r.map_err(|e| e.kind()));
            }),
        )
        .unwrap();
        let r = rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap();
        assert_eq!(r.unwrap_err(), io::ErrorKind::TimedOut);
        assert!(t0.elapsed() < std::time::Duration::from_secs(10));
        let (tx, rx) = std::sync::mpsc::channel();
        spawn_with_timeout(
            OsStr::new("true"),
            &[],
            None,
            std::time::Duration::from_secs(20),
            Box::new(move |r| {
                let _ = tx.send(r.map(|s| s.success()).map_err(|e| e.kind()));
            }),
        )
        .unwrap();
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap(),
            Ok(true)
        );
    }
}
