//! SFZ instruments (`.sfz`): plain text of headers (`<control>`, `<global>`, `<master>`,
//! `<group>`, `<region>`) and `opcode=value` pairs, with `//` and `/* */` comments and
//! `#define $NAME value`. A region inherits the opcodes of the headers above it. Sample paths
//! (which may hold spaces) are relative to the file, under `<control>`'s `default_path`.
//! `#include "file.sfz"` is followed through a loader ([`parse_with`]): eight levels
//! deep and 1 MiB of text in all at most; what it can't load is counted, not listed.

/// One region's opcodes, after inheritance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    pub opcodes: Vec<(String, String)>,
}

impl Region {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.opcodes
            .iter()
            .rev()
            .find(|(o, _)| o == k)
            .map(|(_, v)| v.as_str())
    }

    /// The sample's path relative to the instrument's folder ('\' as '/').
    pub fn sample(&self) -> Option<String> {
        let s = self.get("sample")?;
        let base = self.get("default_path").unwrap_or("");
        let p = format!("{base}{s}").replace('\\', "/");
        (!p.is_empty() && !s.starts_with('*')).then_some(p)
    }

    /// A key opcode as a MIDI key: a number or a note name (`c4` is 60, `c#4`/`db4`).
    pub fn key(&self, k: &str) -> Option<u8> {
        self.get(k).and_then(parse_key)
    }

    /// The key range: `key` sets both ends.
    pub fn keys(&self) -> (u8, u8) {
        let one = self.key("key");
        let lo = self.key("lokey").or(one).unwrap_or(0);
        let hi = self.key("hikey").or(one).unwrap_or(127);
        (lo, hi)
    }

    pub fn number(&self, k: &str) -> Option<f32> {
        self.get(k)?.trim().parse().ok()
    }
}

/// A note name or number as a MIDI key (SFZ's middle C, `c4`, is 60).
pub fn parse_key(v: &str) -> Option<u8> {
    let v = v.trim();
    if let Ok(n) = v.parse::<i32>() {
        return u8::try_from(n).ok().filter(|&k| k <= 127);
    }
    let b = v.to_ascii_lowercase();
    let b = b.as_bytes();
    let base = match b.first()? {
        b'c' => 0,
        b'd' => 2,
        b'e' => 4,
        b'f' => 5,
        b'g' => 7,
        b'a' => 9,
        b'b' => 11,
        _ => return None,
    };
    let mut i = 1;
    let mut acc: i32 = base;
    match b.get(1) {
        Some(b'#') => {
            acc += 1;
            i = 2;
        }
        Some(b'b') if b.len() > 2 => {
            acc -= 1;
            i = 2;
        }
        _ => {}
    }
    let oct: i32 = std::str::from_utf8(&b[i..]).ok()?.parse().ok()?;
    u8::try_from((oct + 1) * 12 + acc)
        .ok()
        .filter(|&k| k <= 127)
}

/// Whether `d` looks like SFZ text: text with a `<region>` or `<group>` header near the
/// start.
pub fn is_sfz(d: &[u8]) -> bool {
    let head = &d[..d.len().min(16 * 1024)];
    if head.contains(&0) {
        return false;
    }
    let s = String::from_utf8_lossy(head);
    let has = |h: &str| s.contains(h);
    (has("<region>") || has("<group>") || has("<global>") || has("<control>")) && s.contains('=')
}

/// The text without comments.
fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            out.push(' ');
        } else {
            let c = s[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// What an SFZ file holds: its regions and the headers' counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Instrument {
    pub regions: Vec<Region>,
    pub groups: usize,
    /// `#include` lines not followed (missing, refused or past the limits).
    pub includes: usize,
}

impl Instrument {
    /// The distinct sample paths, in first use order.
    pub fn samples(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for r in &self.regions {
            if let Some(s) = r.sample()
                && !out.contains(&s)
            {
                out.push(s);
            }
        }
        out
    }
}

/// Levels of `#include` followed.
const MAX_INCLUDE_DEPTH: usize = 8;
/// Text read through `#include` in all.
const MAX_INCLUDE_BYTES: usize = 1 << 20;

/// The path of an `#include "path"` line.
fn include_path(t: &str) -> Option<&str> {
    let rest = t.strip_prefix("#include")?.trim();
    let rest = rest.strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

/// `text` with its `#include` lines replaced by the files `load` gives (comments already
/// stripped); counts the ones not followed.
fn expand(
    text: &str,
    load: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    depth: usize,
    budget: &mut usize,
    missed: &mut usize,
    out: &mut String,
) {
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("#include") {
            let got = include_path(t)
                .filter(|_| depth < MAX_INCLUDE_DEPTH)
                .and_then(&mut *load)
                .filter(|b| b.len() <= *budget);
            match got {
                Some(b) => {
                    *budget -= b.len();
                    let inner = strip_comments(&String::from_utf8_lossy(&b));
                    expand(&inner, load, depth + 1, budget, missed, out);
                }
                None => *missed += 1,
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
}

/// Parses SFZ text; `#include` lines are counted, not followed.
pub fn parse(d: &[u8]) -> Instrument {
    parse_with(d, &mut |_| None)
}

/// Parses SFZ text, reading `#include "path"` files through `load` (the path as written;
/// the caller resolves it and refuses what lies outside the instrument's folder).
pub fn parse_with(d: &[u8], load: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Instrument {
    let text = String::from_utf8_lossy(d);
    let text = strip_comments(&text);
    let mut includes = 0;
    let mut budget = MAX_INCLUDE_BYTES;
    let mut flat = String::with_capacity(text.len());
    expand(&text, load, 0, &mut budget, &mut includes, &mut flat);
    let text = flat;
    let mut defines: Vec<(String, String)> = Vec::new();
    let mut body = String::new();
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("#define") {
            let mut it = rest.split_whitespace();
            if let (Some(k), Some(v)) = (it.next(), it.next()) {
                defines.push((k.to_owned(), v.to_owned()));
            }
            continue;
        }
        let mut l = line.to_owned();
        // Longest names first, so `$A` doesn't eat `$AB`.
        let mut ds = defines.clone();
        ds.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
        for (k, v) in &ds {
            if l.contains(k.as_str()) {
                l = l.replace(k.as_str(), v);
            }
        }
        body.push_str(&l);
        body.push('\n');
    }
    // Levels: control, global, master, group; a region takes all four.
    let mut levels: [Vec<(String, String)>; 4] = Default::default();
    let mut current: Option<usize> = None; // 0..4, or 4 for a region
    let mut region: Vec<(String, String)> = Vec::new();
    let mut out = Instrument {
        includes,
        ..Instrument::default()
    };
    let flush = |region: &mut Vec<(String, String)>,
                 levels: &[Vec<(String, String)>; 4],
                 out: &mut Instrument| {
        let mut ops: Vec<(String, String)> = levels.iter().flatten().cloned().collect();
        ops.append(region);
        out.regions.push(Region { opcodes: ops });
    };
    let mut rest = body.as_str();
    while !rest.is_empty() {
        let t = rest.trim_start();
        if t.is_empty() {
            break;
        }
        if let Some(h) = t.strip_prefix('<') {
            let end = h.find('>').unwrap_or(h.len());
            let name = &h[..end];
            if current == Some(4) {
                flush(&mut region, &levels, &mut out);
            }
            current = match name {
                "control" => Some(0),
                "global" => Some(1),
                "master" => Some(2),
                "group" => Some(3),
                "region" => Some(4),
                _ => None,
            };
            if let Some(l) = current.filter(|&l| l < 4) {
                for k in levels.iter_mut().skip(l) {
                    k.clear();
                }
                if l == 3 {
                    out.groups += 1;
                }
            }
            rest = h.get(end + 1..).unwrap_or("");
            continue;
        }
        // An opcode: the name up to '=', the value up to the next opcode or header.
        let Some(eq) = t.find('=') else {
            break;
        };
        let name = t[..eq].trim();
        if name.contains(char::is_whitespace) || name.contains('<') {
            // Stray text: skip a word.
            let skip = t.find(char::is_whitespace).unwrap_or(t.len());
            rest = &t[skip..];
            continue;
        }
        let after = &t[eq + 1..];
        let end = value_end(after, name == "sample" || name == "default_path");
        let value = after[..end].trim().to_owned();
        rest = &after[end..];
        let pair = (name.to_owned(), value);
        match current {
            Some(4) => region.push(pair),
            Some(l) => levels[l].push(pair),
            None => {}
        }
    }
    if current == Some(4) {
        flush(&mut region, &levels, &mut out);
    }
    out
}

/// Where an opcode's value ends: at the next header, the next `name=` or (for values without
/// spaces) the first white space.
fn value_end(s: &str, spaces: bool) -> usize {
    let line_end = s.find(['\n', '\r']).unwrap_or(s.len());
    let header = s[..line_end].find('<').unwrap_or(line_end);
    let limit = header.min(line_end);
    if !spaces {
        return s[..limit].find(char::is_whitespace).unwrap_or(limit);
    }
    // A path runs to the next " word=" on its line.
    let b = &s[..limit];
    let mut i = 0;
    while let Some(off) = b[i..].find(char::is_whitespace) {
        let ws = i + off;
        let next = b[ws..].trim_start();
        let word_end = next.find(char::is_whitespace).unwrap_or(next.len());
        if next[..word_end].contains('=') {
            return ws;
        }
        i = ws + 1;
        if i >= b.len() {
            break;
        }
    }
    limit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_followed_with_limits() {
        let main = b"<control> default_path=s/\n#define $K 60\n#include \"keys.sfz\"\n#include \"gone.sfz\"\n<region> sample=last.wav\n";
        let mut load = |p: &str| match p {
            "keys.sfz" => {
                Some(b"<region> sample=a.wav key=$K // c\n#include \"more.sfz\"\n".to_vec())
            }
            "more.sfz" => Some(b"<region> sample=b.wav\n".to_vec()),
            _ => None,
        };
        let i = parse_with(main, &mut load);
        let s = i.samples();
        assert_eq!(s, ["s/a.wav", "s/b.wav", "s/last.wav"]);
        assert_eq!(i.regions[0].get("key"), Some("60"));
        assert_eq!(i.includes, 1);
        // without a loader: counted only
        assert_eq!(parse(main).includes, 2);
        // a file including itself stops at the depth limit
        let mut selfish = |_: &str| Some(b"#include \"x.sfz\"\n<region> sample=r.wav\n".to_vec());
        let i = parse_with(b"#include \"x.sfz\"\n", &mut selfish);
        assert_eq!(i.regions.len(), MAX_INCLUDE_DEPTH);
        assert_eq!(i.includes, 1);
    }

    #[test]
    fn regions_inherit_and_paths_keep_spaces() {
        let t = b"// piano\n<control> default_path=Samples/\n#define $V 80\n<global> ampeg_release=0.5\n<group> lovel=0 hivel=$V\n<region> sample=Grand Piano C4.wav lokey=c4 hikey=60 pitch_keycenter=c4\n<region>sample=d4.flac key=d#4 /* inline */ volume=-3\n<group> trigger=release\n<region> sample=rel\\noise.wav\n";
        assert!(is_sfz(t));
        let i = parse(t);
        assert_eq!(i.regions.len(), 3);
        assert_eq!(i.groups, 2);
        let r = &i.regions[0];
        assert_eq!(r.sample().as_deref(), Some("Samples/Grand Piano C4.wav"));
        assert_eq!(r.keys(), (60, 60));
        assert_eq!(r.get("hivel"), Some("80"));
        assert_eq!(r.get("ampeg_release"), Some("0.5"));
        assert_eq!(i.regions[1].keys(), (63, 63));
        assert_eq!(i.regions[1].number("volume"), Some(-3.0));
        assert_eq!(i.regions[2].get("hivel"), None);
        assert_eq!(
            i.regions[2].sample().as_deref(),
            Some("Samples/rel/noise.wav")
        );
        assert_eq!(i.samples().len(), 3);
        assert_eq!(parse_key("a4"), Some(69));
        assert_eq!(parse_key("bb3"), Some(58));
        assert!(!is_sfz(b"<html>"));
    }
}
