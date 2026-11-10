//! The readers against an independent reading by Python's standard library (`configparser`
//! for PLS, `xml.etree` for XSPF, a line reader for M3U, `urllib.parse` for addresses) on
//! generated playlists: every entry's location, title and length must agree. Skipped when
//! `python3` is not installed.

use std::path::{Path, PathBuf};
use std::process::Command;

use specular_playlist::{Kind, Location, read};

/// The oracle: reads each `<n>.<ext>` named on stdin and prints one JSON list per file.
const ORACLE: &str = r#"
import sys, json, re, posixpath, configparser, urllib.parse
import xml.etree.ElementTree as ET

def text(b):
    if b.startswith(b'\xef\xbb\xbf'):
        return b[3:].decode('utf-8')
    try:
        return b.decode('utf-8')
    except UnicodeDecodeError:
        return b.decode('cp1252')

def loc(raw, base, uri=False):
    raw = raw.strip()
    if raw.lower().startswith('file:'):
        return ['file', urllib.parse.unquote(urllib.parse.urlparse(raw).path)]
    if re.match(r'^[A-Za-z][A-Za-z0-9+.-]+:', raw):
        return ['url', raw]
    p = urllib.parse.unquote(raw) if uri else raw.replace('\\', '/')
    if not p.startswith('/'):
        p = posixpath.normpath(posixpath.join(base, p))
    return ['file', p]

def num(s):
    try:
        v = float(s)
    except ValueError:
        return None
    return v if v >= 0 else None

def m3u(t, base):
    out, pending = [], (None, None)
    for line in t.split('\n'):
        line = line.strip().lstrip('﻿')
        if not line:
            continue
        if line.startswith('#EXTINF:'):
            head, _, title = line[8:].partition(',')
            d = num(head.split()[0]) if head.split() else None
            pending = (d, title.strip() or None)
        elif line.startswith('#'):
            continue
        else:
            l = loc(line, base)
            out.append([l[0], l[1], pending[1], pending[0]])
            pending = (None, None)
    return out

def pls(t, base):
    cp = configparser.ConfigParser(interpolation=None, strict=False)
    cp.read_string(t)
    s = cp['playlist']
    rows = {}
    for k, v in s.items():
        m = re.match(r'^(file|title|length)(\d+)$', k)
        if m:
            rows.setdefault(int(m.group(2)), {})[m.group(1)] = v.strip()
    out = []
    for n in sorted(rows):
        r = rows[n]
        if 'file' not in r:
            continue
        l = loc(r['file'], base)
        out.append([l[0], l[1], r.get('title') or None, num(r['length']) if 'length' in r else None])
    return out

def xspf(t, base):
    ns = {'x': 'http://xspf.org/ns/0/'}
    root = ET.fromstring(t)
    out = []
    for tr in root.findall('x:trackList/x:track', ns):
        def get(name):
            e = tr.find('x:' + name, ns)
            return e.text.strip() if e is not None and e.text and e.text.strip() else None
        location = get('location')
        if location is None:
            continue
        l = loc(location, base, uri=True)
        title, creator = get('title'), get('creator')
        if title and creator:
            title = creator + ' – ' + title
        elif creator:
            title = None
        d = get('duration')
        out.append([l[0], l[1], title, num(d) / 1000 if d is not None and num(d) is not None else None])
    return out

base = sys.argv[1]
res = []
for name in sys.stdin.read().split():
    b = open(posixpath.join(base, name), 'rb').read()
    t = text(b)
    ext = name.rsplit('.', 1)[1]
    res.append({'m3u': m3u, 'pls': pls, 'xspf': xspf}[ext](t, base))
print(json.dumps(res))
"#;

/// xorshift for the generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[self.below(v.len())]
    }
}

const WORDS: &[&str] = &[
    "Song",
    "a b",
    "Café",
    "naïve",
    "שיר",
    "日本",
    "x%41y",
    "Tom & Jerry",
    "rock,roll",
    "#1",
    "Ünïcode",
    "end.",
    "(live)",
    "it’s",
    "ß",
];
const EXTS: &[&str] = &["mp3", "flac", "ogg", "wav", "m4a"];

fn name(r: &mut Rng) -> String {
    format!("{} {}.{}", r.pick(WORDS), r.below(100), r.pick(EXTS))
}

/// Percent-encodes all but unreserved characters and `/`.
fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// A location as a playlist of `kind` writes it.
fn location(r: &mut Rng, xspf: bool) -> String {
    let n = name(r);
    match r.below(6) {
        0 => format!("http://radio.example.org:8000/{}", enc(&n)),
        1 => format!("file://{}", enc(&format!("/music/{n}"))),
        2 if xspf => format!("../{}", enc(&n)),
        2 => format!("../{n}"),
        3 if xspf => format!("sub/{}", enc(&n)),
        3 => format!("sub\\{n}"),
        4 if xspf => enc(&n),
        4 => format!("/abs/{n}"),
        _ if xspf => format!("./{}", enc(&n)),
        _ => format!("./{n}"),
    }
}

fn duration(r: &mut Rng) -> String {
    match r.below(4) {
        0 => "-1".into(),
        1 => format!("{}.5", r.below(600)),
        _ => r.below(4000).to_string(),
    }
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

/// A playlist of each kind in turn, with its bytes.
fn playlist(r: &mut Rng, i: usize) -> (String, Vec<u8>) {
    let n = 1 + r.below(8);
    match i % 3 {
        0 => {
            let mut s = String::new();
            if r.below(2) == 0 {
                s.push_str("#EXTM3U\r\n");
            }
            for _ in 0..n {
                if r.below(3) > 0 {
                    s.push_str(&format!("#EXTINF:{},{}\n", duration(r), name(r)));
                }
                if r.below(5) == 0 {
                    s.push_str("# a comment\n\n");
                }
                s.push_str(&location(r, false));
                s.push('\n');
            }
            // Latin-1 when it can be, sometimes; else UTF-8, sometimes with a mark.
            let latin: Option<Vec<u8>> = s
                .chars()
                .map(|c| {
                    u8::try_from(u32::from(c))
                        .ok()
                        .filter(|b| *b < 0x80 || *b >= 0xA0)
                })
                .collect();
            let bytes = match (latin, r.below(3)) {
                (Some(l), 0) => l,
                (_, 1) => [b"\xEF\xBB\xBF".as_slice(), s.as_bytes()].concat(),
                _ => s.into_bytes(),
            };
            (format!("{i}.m3u"), bytes)
        }
        1 => {
            let mut s = String::from("[playlist]\n");
            let mut nums: Vec<usize> = (1..=n).collect();
            // Out of order, and sometimes with a gap.
            nums.reverse();
            for k in nums {
                let k = if r.below(4) == 0 { k + 10 } else { k };
                s.push_str(&format!("File{k}={}\n", location(r, false)));
                if r.below(2) == 0 {
                    s.push_str(&format!("Title{k}={}\n", name(r)));
                }
                if r.below(2) == 0 {
                    s.push_str(&format!("Length{k}={}\n", duration(r)));
                }
            }
            s.push_str(&format!("NumberOfEntries={n}\nVersion=2\n"));
            (format!("{i}.pls"), s.into_bytes())
        }
        _ => {
            let mut s = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\">\n<trackList>\n",
            );
            for _ in 0..n {
                s.push_str(&format!(
                    "<track><location>{}</location>",
                    xml(&location(r, true))
                ));
                if r.below(2) == 0 {
                    s.push_str(&format!("<title>{}</title>", xml(&name(r))));
                }
                if r.below(3) == 0 {
                    s.push_str(&format!("<creator>{}</creator>", xml(r.pick(WORDS))));
                }
                if r.below(2) == 0 {
                    s.push_str(&format!("<duration>{}</duration>", r.below(900_000)));
                }
                s.push_str("</track>\n");
            }
            s.push_str("</trackList>\n</playlist>\n");
            (format!("{i}.xspf"), s.into_bytes())
        }
    }
}

type Row = (String, String, Option<String>, Option<f64>);

fn ours(data: &[u8], name: &str, base: &Path) -> Vec<Row> {
    let kind = Kind::of_path(Path::new(name)).unwrap();
    read(data, kind, base)
        .unwrap()
        .entries
        .into_iter()
        .map(|e| {
            let (t, v) = match e.location {
                Location::File(p) => ("file".to_owned(), p.to_string_lossy().into_owned()),
                Location::Url(u) => ("url".to_owned(), u),
            };
            (t, v, e.title, e.duration)
        })
        .collect()
}

/// Every entry of 300 generated playlists reads as Python's standard library reads it.
#[test]
fn readers_agree_with_python() {
    let python = Command::new("python3").arg("--version").output();
    if python.is_err() {
        eprintln!("python3 not installed: skipped");
        return;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("playlist-oracle");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("oracle_script");
    std::fs::create_dir_all(&script).unwrap();
    std::fs::write(script.join("oracle.py"), ORACLE).unwrap();
    let lists = dir.join("lists");
    std::fs::create_dir_all(&lists).unwrap();
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut names = Vec::new();
    let mut want: Vec<Vec<Row>> = Vec::new();
    for i in 0..300 {
        let (name, bytes) = playlist(&mut r, i);
        std::fs::write(lists.join(&name), &bytes).unwrap();
        want.push(ours(&bytes, &name, &lists));
        names.push(name);
    }
    let mut child = Command::new("python3")
        .arg("-I")
        .arg(script.join("oracle.py"))
        .arg(&lists)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(names.join("\n").as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let theirs: Vec<Vec<Row>> = parse_json(&String::from_utf8(out.stdout).unwrap());
    assert_eq!(theirs.len(), want.len());
    let mut entries = 0;
    let mut mismatches = Vec::new();
    for ((name, a), b) in names.iter().zip(&want).zip(&theirs) {
        entries += a.len();
        if a.len() != b.len() {
            mismatches.push(format!("{name}: {} vs {} entries", a.len(), b.len()));
            continue;
        }
        for (x, y) in a.iter().zip(b) {
            let same_dur = match (x.3, y.3) {
                (Some(p), Some(q)) => (p - q).abs() < 1e-9,
                (p, q) => p == q,
            };
            if x.0 != y.0 || x.1 != y.1 || x.2 != y.2 || !same_dur {
                mismatches.push(format!("{name}: {x:?} vs {y:?}"));
            }
        }
    }
    eprintln!(
        "playlist oracle: {} playlists, {entries} entries, {} mismatches",
        names.len(),
        mismatches.len()
    );
    assert!(entries > 1000, "{entries}");
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The oracle's JSON (lists of `[type, value, title|null, duration|null]`), read without a
/// JSON crate.
fn parse_json(s: &str) -> Vec<Vec<Row>> {
    let mut p = J {
        s: s.trim().as_bytes(),
        i: 0,
    };
    let mut out = Vec::new();
    p.eat(b'[');
    while !p.peek(b']') {
        let mut list = Vec::new();
        p.eat(b'[');
        while !p.peek(b']') {
            p.eat(b'[');
            let t = p.string().unwrap();
            p.eat(b',');
            let v = p.string().unwrap();
            p.eat(b',');
            let title = p.string();
            p.eat(b',');
            let d = p.number();
            p.eat(b']');
            list.push((t, v, title, d));
            p.comma();
        }
        p.eat(b']');
        out.push(list);
        p.comma();
    }
    out
}

struct J<'a> {
    s: &'a [u8],
    i: usize,
}

impl J<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self, c: u8) -> bool {
        self.ws();
        self.s.get(self.i) == Some(&c)
    }
    fn eat(&mut self, c: u8) {
        self.ws();
        assert_eq!(self.s[self.i], c, "at {}", self.i);
        self.i += 1;
    }
    fn comma(&mut self) {
        if self.peek(b',') {
            self.i += 1;
        }
    }
    fn null(&mut self) -> bool {
        self.ws();
        if self.s[self.i..].starts_with(b"null") {
            self.i += 4;
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Option<String> {
        if self.null() {
            return None;
        }
        self.eat(b'"');
        let mut units: Vec<u16> = Vec::new();
        loop {
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.s[self.i];
                    self.i += 1;
                    match e {
                        b'u' => {
                            let h = std::str::from_utf8(&self.s[self.i..self.i + 4]).unwrap();
                            units.push(u16::from_str_radix(h, 16).unwrap());
                            self.i += 4;
                        }
                        b'n' => units.push(u16::from(b'\n')),
                        b't' => units.push(u16::from(b'\t')),
                        b'r' => units.push(u16::from(b'\r')),
                        other => units.push(u16::from(other)),
                    }
                }
                c => units.push(u16::from(c)),
            }
        }
        Some(String::from_utf16(&units).unwrap())
    }
    fn number(&mut self) -> Option<f64> {
        if self.null() {
            return None;
        }
        self.ws();
        let start = self.i;
        while self.i < self.s.len() && b"0123456789.-+eE".contains(&self.s[self.i]) {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .unwrap()
            .parse()
            .ok()
    }
}
