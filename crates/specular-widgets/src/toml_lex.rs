//! A TOML lexer for syntax colouring, one line at a time.
//!
//! Lines are lexed independently given the [`State`] at their start (inside a multi-line
//! string, inside an array or inline table), so an editor re-lexes only from the first
//! changed line and stops as soon as a line ends in the state it ended in before. It never
//! fails: text that is not TOML is coloured as plain text and left to the linter.

/// What a piece of text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tok {
    /// Whitespace and anything not recognised.
    Text,
    /// A `[table]` or `[[array.of.tables]]` header, brackets included.
    Table,
    Key,
    String,
    /// An escape sequence inside a basic string.
    Escape,
    Number,
    Boolean,
    Datetime,
    Comment,
    /// `=`, `.`, `,`, brackets and braces.
    Punct,
}

/// A coloured byte range of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub tok: Tok,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Multiline {
    #[default]
    None,
    /// Inside `"""…"""`.
    Basic,
    /// Inside `'''…'''`.
    Literal,
}

/// The lexer's state between lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct State {
    pub multiline: Multiline,
    /// Open arrays and inline tables, innermost in the lowest bit: 1 = inline table.
    stack: u64,
    depth: u8,
}

impl State {
    fn top_is_table(&self) -> bool {
        self.depth > 0 && self.stack & 1 == 1
    }
    fn push(&mut self, table: bool) {
        if self.depth < 63 {
            self.stack = (self.stack << 1) | table as u64;
            self.depth += 1;
        }
    }
    fn pop(&mut self) {
        if self.depth > 0 {
            self.stack >>= 1;
            self.depth -= 1;
        }
    }
    /// Inside an array or inline table.
    pub fn nested(&self) -> bool {
        self.depth > 0
    }
}

fn is_bare(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
    out: Vec<Span>,
}

impl Lexer<'_> {
    fn push(&mut self, start: usize, end: usize, tok: Tok) {
        if end <= start {
            return;
        }
        // Merge with the previous span of the same kind.
        if let Some(last) = self.out.last_mut()
            && last.tok == tok
            && last.end as usize == start
        {
            last.end = end as u32;
            return;
        }
        self.out.push(Span {
            start: start as u32,
            end: end as u32,
            tok,
        });
    }

    fn peek(&self, n: usize) -> u8 {
        self.s.get(self.i + n).copied().unwrap_or(0)
    }

    fn starts(&self, pat: &[u8]) -> bool {
        self.s[self.i..].starts_with(pat)
    }

    fn whitespace(&mut self) {
        let start = self.i;
        while matches!(self.peek(0), b' ' | b'\t') {
            self.i += 1;
        }
        self.push(start, self.i, Tok::Text);
    }

    /// A basic string body up to and including `close` (`"` or `"""`), escapes marked.
    /// Returns false if the line ended first.
    fn basic_body(&mut self, close: &[u8]) -> bool {
        let mut start = self.i;
        while self.i < self.s.len() {
            if self.starts(close) {
                let mut end = self.i + close.len();
                // `""""` and `"""""`: up to two quotes before the closing three are content.
                if close.len() == 3 {
                    while end < self.s.len() && self.s[end] == b'"' && end - self.i < 5 {
                        end += 1;
                    }
                }
                self.push(start, end, Tok::String);
                self.i = end;
                return true;
            }
            if self.peek(0) == b'\\' {
                self.push(start, self.i, Tok::String);
                let e = self.i;
                let n = match self.peek(1) {
                    b'u' => 6,
                    b'U' => 10,
                    b'x' => 4,
                    0 => 1,
                    _ => 2,
                };
                let mut end = (e + n).min(self.s.len());
                while !self.s.is_char_boundary_at(end) {
                    end += 1;
                }
                self.push(e, end, Tok::Escape);
                self.i = end;
                start = self.i;
                continue;
            }
            self.i += 1;
        }
        self.push(start, self.i, Tok::String);
        false
    }

    /// A literal string body up to and including `close`. False if the line ended first.
    fn literal_body(&mut self, close: &[u8]) -> bool {
        let start = self.i;
        while self.i < self.s.len() {
            if self.starts(close) {
                let mut end = self.i + close.len();
                if close.len() == 3 {
                    while end < self.s.len() && self.s[end] == b'\'' && end - self.i < 5 {
                        end += 1;
                    }
                }
                self.push(start, end, Tok::String);
                self.i = end;
                return true;
            }
            self.i += 1;
        }
        self.push(start, self.i, Tok::String);
        false
    }

    fn comment(&mut self) {
        self.push(self.i, self.s.len(), Tok::Comment);
        self.i = self.s.len();
    }

    /// A key: bare, quoted and dotted parts. Stops before `=` or anything else.
    fn key(&mut self, tok: Tok) {
        loop {
            self.whitespace();
            match self.peek(0) {
                b'"' => {
                    let s = self.i;
                    self.i += 1;
                    self.push(s, self.i, tok);
                    let n = self.out.len();
                    self.basic_body(b"\"");
                    // A quoted key is one colour (escapes included).
                    for sp in &mut self.out[n..] {
                        sp.tok = tok;
                    }
                    let merged = merge(std::mem::take(&mut self.out));
                    self.out = merged;
                }
                b'\'' => {
                    let s = self.i;
                    self.i += 1;
                    while self.i < self.s.len() && self.peek(0) != b'\'' {
                        self.i += 1;
                    }
                    self.i = (self.i + 1).min(self.s.len());
                    self.push(s, self.i, tok);
                }
                c if is_bare(c) => {
                    let s = self.i;
                    while is_bare(self.peek(0)) {
                        self.i += 1;
                    }
                    self.push(s, self.i, tok);
                }
                _ => return,
            }
            self.whitespace();
            if self.peek(0) == b'.' {
                let p = if tok == Tok::Table {
                    Tok::Table
                } else {
                    Tok::Punct
                };
                self.push(self.i, self.i + 1, p);
                self.i += 1;
            } else {
                return;
            }
        }
    }

    /// A number, date or time starting here; the length of the token.
    fn scalar(&mut self) {
        let s = self.i;
        let bytes = self.s;
        let mut e = s;
        while e < bytes.len()
            && (bytes[e].is_ascii_alphanumeric()
                || matches!(bytes[e], b'_' | b'.' | b':' | b'+' | b'-'))
        {
            e += 1;
        }
        let word = &bytes[s..e];
        let digits = |r: std::ops::Range<usize>| {
            word.get(r)
                .is_some_and(|w| !w.is_empty() && w.iter().all(u8::is_ascii_digit))
        };
        let date =
            word.len() >= 10 && digits(0..4) && word[4] == b'-' && digits(5..7) && word[7] == b'-';
        let time = word.len() >= 5 && digits(0..2) && word[2] == b':' && digits(3..5);
        let tok = if date || time {
            // `1979-05-27 07:32:00`: a space may separate the date and the time.
            if date
                && word.len() == 10
                && e + 3 < bytes.len()
                && bytes[e] == b' '
                && bytes[e + 1].is_ascii_digit()
                && bytes[e + 2].is_ascii_digit()
                && bytes[e + 3] == b':'
            {
                e += 1;
                while e < bytes.len()
                    && (bytes[e].is_ascii_alphanumeric()
                        || matches!(bytes[e], b'.' | b':' | b'+' | b'-'))
                {
                    e += 1;
                }
            }
            Tok::Datetime
        } else {
            Tok::Number
        };
        self.i = e;
        self.push(s, e, tok);
    }

    /// A value, and what follows it up to the next value or key.
    fn value(&mut self, st: &mut State) {
        self.whitespace();
        let c = self.peek(0);
        match c {
            0 => {}
            b'#' => self.comment(),
            b'"' if self.starts(b"\"\"\"") => {
                let s = self.i;
                self.i += 3;
                self.push(s, self.i, Tok::String);
                if !self.basic_body(b"\"\"\"") {
                    st.multiline = Multiline::Basic;
                }
            }
            b'\'' if self.starts(b"'''") => {
                let s = self.i;
                self.i += 3;
                self.push(s, self.i, Tok::String);
                if !self.literal_body(b"'''") {
                    st.multiline = Multiline::Literal;
                }
            }
            b'"' => {
                let s = self.i;
                self.i += 1;
                self.push(s, self.i, Tok::String);
                self.basic_body(b"\"");
            }
            b'\'' => {
                let s = self.i;
                self.i += 1;
                self.push(s, self.i, Tok::String);
                self.literal_body(b"'");
            }
            b'[' => {
                self.push(self.i, self.i + 1, Tok::Punct);
                self.i += 1;
                st.push(false);
            }
            b'{' => {
                self.push(self.i, self.i + 1, Tok::Punct);
                self.i += 1;
                st.push(true);
                self.key(Tok::Key);
                self.after_key(st);
            }
            b']' | b'}' => {
                self.push(self.i, self.i + 1, Tok::Punct);
                self.i += 1;
                st.pop();
            }
            b',' => {
                self.push(self.i, self.i + 1, Tok::Punct);
                self.i += 1;
                if st.top_is_table() {
                    self.key(Tok::Key);
                    self.after_key(st);
                }
            }
            _ if self.word(b"true") || self.word(b"false") => {
                let n = if c == b't' { 4 } else { 5 };
                self.push(self.i, self.i + n, Tok::Boolean);
                self.i += n;
            }
            _ if self.word(b"inf") || self.word(b"nan") => {
                self.push(self.i, self.i + 3, Tok::Number);
                self.i += 3;
            }
            b'+' | b'-' if self.peek(1).is_ascii_digit() => self.scalar(),
            b'+' | b'-' if self.word_at(1, b"inf") || self.word_at(1, b"nan") => {
                self.push(self.i, self.i + 4, Tok::Number);
                self.i += 4;
            }
            _ if c.is_ascii_digit() => self.scalar(),
            _ => {
                // Not a value: plain text up to the next space or delimiter.
                let s = self.i;
                self.i += 1;
                while self.i < self.s.len()
                    && !matches!(self.peek(0), b' ' | b'\t' | b',' | b']' | b'}' | b'#')
                {
                    self.i += 1;
                }
                while !self.s.is_char_boundary_at(self.i) {
                    self.i += 1;
                }
                self.push(s, self.i, Tok::Text);
            }
        }
    }

    /// After a key: `=` and its value.
    fn after_key(&mut self, _st: &mut State) {
        self.whitespace();
        if self.peek(0) == b'=' {
            self.push(self.i, self.i + 1, Tok::Punct);
            self.i += 1;
        }
    }

    fn word(&self, w: &[u8]) -> bool {
        self.word_at(0, w)
    }

    fn word_at(&self, off: usize, w: &[u8]) -> bool {
        self.s[self.i + off..].starts_with(w) && !is_bare(self.peek(off + w.len()))
    }

    /// A `[table]` or `[[array]]` header at the start of a line.
    fn header(&mut self) {
        let s = self.i;
        let double = self.starts(b"[[");
        self.i += if double { 2 } else { 1 };
        self.push(s, self.i, Tok::Table);
        self.key(Tok::Table);
        self.whitespace();
        let close: &[u8] = if double { b"]]" } else { b"]" };
        if self.starts(close) {
            self.push(self.i, self.i + close.len(), Tok::Table);
            self.i += close.len();
        }
    }
}

trait CharBoundary {
    fn is_char_boundary_at(&self, i: usize) -> bool;
}

impl CharBoundary for [u8] {
    fn is_char_boundary_at(&self, i: usize) -> bool {
        i >= self.len() || (self[i] as i8) >= -0x40
    }
}

fn merge(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    for s in spans {
        match out.last_mut() {
            Some(l) if l.tok == s.tok && l.end == s.start => l.end = s.end,
            _ => out.push(s),
        }
    }
    out
}

/// Lexes one line (without its line break) that starts in `state`. Returns the spans,
/// which cover the line without gaps, and the state at its end.
pub fn lex_line(line: &str, state: State) -> (Vec<Span>, State) {
    let mut lx = Lexer {
        s: line.as_bytes(),
        i: 0,
        out: Vec::new(),
    };
    let mut st = state;
    // The rest of a multi-line string.
    match st.multiline {
        Multiline::Basic => {
            if lx.basic_body(b"\"\"\"") {
                st.multiline = Multiline::None;
            } else {
                return (lx.out, st);
            }
        }
        Multiline::Literal => {
            if lx.literal_body(b"'''") {
                st.multiline = Multiline::None;
            } else {
                return (lx.out, st);
            }
        }
        Multiline::None => {
            lx.whitespace();
            if !st.nested() {
                match lx.peek(0) {
                    b'[' => lx.header(),
                    b'#' | 0 => {}
                    _ => {
                        lx.key(Tok::Key);
                        lx.after_key(&mut st);
                    }
                }
            } else if st.top_is_table() {
                lx.key(Tok::Key);
                lx.after_key(&mut st);
            }
        }
    }
    let mut guard = 0;
    while lx.i < lx.s.len() && st.multiline == Multiline::None {
        let before = lx.i;
        lx.value(&mut st);
        if lx.i == before {
            // Never stall on something unexpected.
            guard += 1;
            lx.push(lx.i, lx.i + 1, Tok::Text);
            lx.i += 1;
            if guard > line.len() {
                break;
            }
        }
    }
    (lx.out, st)
}

/// The kind of token at byte `at` of a lexed line.
pub fn tok_at(spans: &[Span], at: usize) -> Tok {
    let at = at as u32;
    let i = spans.partition_point(|s| s.end <= at);
    spans
        .get(i)
        .filter(|s| s.start <= at)
        .map_or(Tok::Text, |s| s.tok)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line as (text, kind) pairs, whitespace-only plain text left out.
    fn toks(line: &str, state: State) -> (Vec<(&str, Tok)>, State) {
        let (spans, st) = lex_line(line, state);
        // Spans cover the line without gaps or overlaps.
        let mut pos = 0;
        for s in &spans {
            assert_eq!(s.start, pos, "{line:?}: {spans:?}");
            assert!(s.end > s.start);
            pos = s.end;
        }
        assert_eq!(pos as usize, line.len(), "{line:?}: {spans:?}");
        let v = spans
            .iter()
            .map(|s| (&line[s.start as usize..s.end as usize], s.tok))
            .filter(|(t, k)| !(*k == Tok::Text && t.trim().is_empty()))
            .collect();
        (v, st)
    }

    fn one(line: &str) -> Vec<(&str, Tok)> {
        let (v, st) = toks(line, State::default());
        assert_eq!(st, State::default(), "{line:?}");
        v
    }

    use Tok::*;

    #[test]
    fn tables_and_keys() {
        assert_eq!(one("[viewing]"), [("[viewing]", Table)]);
        assert_eq!(
            one("  [[commands]]  # list"),
            [("[[commands]]", Table), ("# list", Comment)]
        );
        assert_eq!(one("[a.\"b c\".d]"), [("[a.\"b c\".d]", Table)]);
        assert_eq!(
            one("a.\"b.c\".'d' = 1"),
            [
                ("a", Key),
                (".", Punct),
                ("\"b.c\"", Key),
                (".", Punct),
                ("'d'", Key),
                ("=", Punct),
                ("1", Number)
            ]
        );
        assert_eq!(
            one("bare-key_1=true"),
            [("bare-key_1", Key), ("=", Punct), ("true", Boolean)]
        );
    }

    #[test]
    fn strings_and_escapes() {
        assert_eq!(
            one(r#"k = "a\tb\u00e9 # not a comment" # comment"#),
            [
                ("k", Key),
                ("=", Punct),
                ("\"a", String),
                ("\\t", Escape),
                ("b", String),
                ("\\u00e9", Escape),
                (" # not a comment\"", String),
                ("# comment", Comment)
            ]
        );
        assert_eq!(
            one(r#"k = 'C:\path\' "#),
            [("k", Key), ("=", Punct), ("'C:\\path\\'", String)]
        );
        assert_eq!(
            one(r#"k = "say \"hi\"""#),
            [
                ("k", Key),
                ("=", Punct),
                ("\"say ", String),
                ("\\\"", Escape),
                ("hi", String),
                ("\\\"", Escape),
                ("\"", String)
            ]
        );
        // Unterminated: string to the end of the line, and the next line is a key again.
        assert_eq!(
            one(r#"k = "oops"#),
            [("k", Key), ("=", Punct), ("\"oops", String)]
        );
    }

    #[test]
    fn multiline_strings() {
        let (v, st) = toks(r#"k = """first"#, State::default());
        assert_eq!(v, [("k", Key), ("=", Punct), ("\"\"\"first", String)]);
        assert_eq!(st.multiline, Multiline::Basic);
        let (v, st2) = toks(r#"a = 1 \n "#, st);
        assert_eq!(v, [("a = 1 ", String), ("\\n", Escape), (" ", String)]);
        assert_eq!(st2, st);
        let (v, st3) = toks(r#"end"""" # c"#, st2);
        assert_eq!(v, [("end\"\"\"\"", String), ("# c", Comment)]);
        assert_eq!(st3, State::default());

        let (_, st) = toks("k = '''", State::default());
        assert_eq!(st.multiline, Multiline::Literal);
        let (v, st) = toks(r#"no \escapes """ here'''"#, st);
        assert_eq!(v, [(r#"no \escapes """ here'''"#, String)]);
        assert_eq!(st, State::default());
        // Opened and closed on one line.
        assert_eq!(
            one(r#"k = """x""" "#),
            [("k", Key), ("=", Punct), ("\"\"\"x\"\"\"", String)]
        );
    }

    #[test]
    fn numbers() {
        for n in [
            "42",
            "+17",
            "-5",
            "1_000",
            "0xDEAD_beef",
            "0o755",
            "0b1101",
            "3.14",
            "-0.01",
            "5e+22",
            "1e06",
            "-2E-2",
            "6.626e-34",
            "224_617.445_991",
            "inf",
            "+inf",
            "-nan",
            "nan",
        ] {
            let line = format!("n = {n}");
            assert_eq!(one(&line), [("n", Key), ("=", Punct), (n, Number)], "{n}");
        }
    }

    #[test]
    fn dates_and_times() {
        for d in [
            "1979-05-27T07:32:00Z",
            "1979-05-27T00:32:00.999999-07:00",
            "1979-05-27 07:32:00Z",
            "1979-05-27T07:32:00",
            "1979-05-27",
            "07:32:00",
            "00:32:00.999999",
        ] {
            let line = format!("d = {d} # c");
            assert_eq!(
                one(&line),
                [("d", Key), ("=", Punct), (d, Datetime), ("# c", Comment)],
                "{d}"
            );
        }
    }

    #[test]
    fn arrays_and_inline_tables() {
        assert_eq!(
            one(r#"a = [1, "x", { k = true, j = 2 }]"#),
            [
                ("a", Key),
                ("=", Punct),
                ("[", Punct),
                ("1", Number),
                (",", Punct),
                ("\"x\"", String),
                (",", Punct),
                ("{", Punct),
                ("k", Key),
                ("=", Punct),
                ("true", Boolean),
                (",", Punct),
                ("j", Key),
                ("=", Punct),
                ("2", Number),
                // Neighbouring tokens of one kind are one span.
                ("}]", Punct)
            ]
        );
        // An array over several lines: its lines hold values, not keys.
        let (_, st) = toks("args = [", State::default());
        assert!(st.nested());
        let (v, st) = toks(r#"    "{file}", # first"#, st);
        assert_eq!(
            v,
            [("\"{file}\"", String), (",", Punct), ("# first", Comment)]
        );
        let (v, st) = toks("    true,", st);
        assert_eq!(v, [("true", Boolean), (",", Punct)]);
        let (v, st) = toks("]", st);
        assert_eq!(v, [("]", Punct)]);
        assert_eq!(st, State::default());
    }

    #[test]
    fn junk_never_stalls() {
        for line in [
            "= = =",
            "]]]",
            "k = @@@",
            "\"",
            "'",
            "k = \"\\",
            "[unclosed",
            "k = 1979-05-27 x",
            "k = é ü",
            "日本 = \"語\"",
            "\u{0}",
            "{",
            "k = { a = ",
        ] {
            toks(line, State::default());
        }
        assert_eq!(tok_at(&lex_line("k = 1", State::default()).0, 4), Number);
    }
}
