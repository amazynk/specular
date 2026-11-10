//! A small strict JSON parser for weight-file headers: bounded nesting, no recursion past
//! [`MAX_DEPTH`], numbers kept as their text (converted exactly when asked), duplicate keys
//! kept in order for the caller to reject.

/// Deepest nesting accepted (a safetensors header needs 3).
pub const MAX_DEPTH: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// The number's text, validated as JSON.
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    /// Members in file order (duplicates included).
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// The number as an exact non-negative integer (no fraction, no exponent).
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Num(s) if s.bytes().all(|b| b.is_ascii_digit()) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(o) => Some(o),
            _ => None,
        }
    }

    /// The number as an `f64` (any JSON number).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The first member called `key`.
    pub fn get(&self, key: &str) -> Option<&Json> {
        self.as_object()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

/// Parses one JSON value; trailing whitespace is allowed, anything else after it is not.
pub fn parse(text: &[u8]) -> Result<Json, String> {
    let mut p = Parser { s: text, i: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(format!("trailing bytes at {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at byte {}", self.i))
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.s.get(self.i) {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, b: u8) -> Result<(), String> {
        if self.peek() == Some(b) {
            self.i += 1;
            Ok(())
        } else {
            self.err(&format!("expected '{}'", b as char))
        }
    }

    fn lit(&mut self, word: &[u8], v: Json) -> Result<Json, String> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            self.err("bad literal")
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return self.err("nested too deeply");
        }
        self.ws();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit(b"true", Json::Bool(true)),
            Some(b'f') => self.lit(b"false", Json::Bool(false)),
            Some(b'n') => self.lit(b"null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => self.err("expected a value"),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.eat(b'{')?;
        let mut members = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Obj(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return self.err("expected a key");
            }
            let k = self.string()?;
            self.ws();
            self.eat(b':')?;
            let v = self.value(depth + 1)?;
            members.push((k, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(members));
                }
                _ => return self.err("expected ',' or '}'"),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.eat(b'[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return self.err("expected ',' or ']'"),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let Some(h) = self.s.get(self.i..self.i + 4) else {
            return self.err("short \\u escape");
        };
        let mut v = 0;
        for &b in h {
            let d = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return self.err("bad \\u escape"),
            };
            v = v * 16 + u32::from(d);
        }
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(b) = self.peek() else {
                return self.err("unterminated string");
            };
            self.i += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return self.err("unterminated escape");
                    };
                    self.i += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xd800..0xdc00).contains(&hi) {
                                if self.s.get(self.i..self.i + 2) != Some(b"\\u") {
                                    return self.err("lone surrogate");
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&lo) {
                                    return self.err("bad surrogate pair");
                                }
                                0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                            } else {
                                hi
                            };
                            match char::from_u32(cp) {
                                Some(c) => c,
                                None => return self.err("lone surrogate"),
                            }
                        }
                        _ => return self.err("bad escape"),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                0..=0x1f => return self.err("control character in string"),
                _ => out.push(b),
            }
        }
        String::from_utf8(out).or_else(|_| self.err("string is not UTF-8"))
    }

    fn digits(&mut self) -> usize {
        let start = self.i;
        while let Some(b'0'..=b'9') = self.peek() {
            self.i += 1;
        }
        self.i - start
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let int_start = self.i;
        let n = self.digits();
        if n == 0 || (n > 1 && self.s[int_start] == b'0') {
            return self.err("bad number");
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if self.digits() == 0 {
                return self.err("bad fraction");
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.i += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.i += 1;
            }
            if self.digits() == 0 {
                return self.err("bad exponent");
            }
        }
        // ASCII by construction.
        Ok(Json::Num(
            String::from_utf8_lossy(&self.s[start..self.i]).into_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_values() {
        let v = parse(br#" {"a": [1, 2.5e3, -0, true, null], "b\u00e9": "x\ny\ud83d\ude00"} "#)
            .unwrap();
        assert_eq!(v.get("a").unwrap().as_array().unwrap().len(), 5);
        assert_eq!(v.get("a").unwrap().as_array().unwrap()[0].as_u64(), Some(1));
        assert_eq!(v.get("bé").unwrap().as_str(), Some("x\ny😀"));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            &b"{"[..],
            b"[1,]",
            b"01",
            b"\"\\ud800\"",
            b"\"a\x01\"",
            b"{} x",
            b"-",
            b"1.",
            b"\"\xff\"",
        ] {
            assert!(parse(bad).is_err(), "{:?}", String::from_utf8_lossy(bad));
        }
        let deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
        assert!(parse(deep.as_bytes()).is_err());
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(ok.as_bytes()).is_ok());
    }

    #[test]
    fn u64_is_exact() {
        assert_eq!(
            parse(b"18446744073709551615").unwrap().as_u64(),
            Some(u64::MAX)
        );
        assert_eq!(parse(b"18446744073709551616").unwrap().as_u64(), None);
        assert_eq!(parse(b"1e3").unwrap().as_u64(), None);
        assert_eq!(parse(b"-1").unwrap().as_u64(), None);
    }
}
