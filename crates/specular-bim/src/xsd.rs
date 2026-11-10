//! XML Schema regular expressions (XSD 1.0 part 2, appendix F) as IDS `xs:pattern`
//! restrictions use them: always matched against the whole string; literals, `.`,
//! character classes with ranges, negation and subtraction (`[a-z-[aeiou]]`), the escapes
//! `\d \D \s \S \w \W \i \I \c \C` and `\n \r \t` and escaped metacharacters, `\p{…}` for
//! the general categories L, Lu, Ll, N, Nd, P and their negations, groups, alternation and
//! the quantifiers `? * + {n} {n,} {n,m}`. Matching backtracks over characters, with a
//! step budget so a hostile pattern cannot hang the check.

/// A compiled pattern.
#[derive(Clone, Debug)]
pub struct Pattern {
    node: Node,
}

#[derive(Clone, Debug)]
enum Node {
    Empty,
    Char(Class),
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Repeat(Box<Node>, u32, Option<u32>),
}

#[derive(Clone, Debug)]
enum Item {
    Range(char, char),
    Esc(char),
    Cat(String, bool),
}

#[derive(Clone, Debug)]
struct Class {
    items: Vec<Item>,
    negated: bool,
    minus: Option<Box<Class>>,
}

impl Class {
    fn one(c: char) -> Class {
        Class {
            items: vec![Item::Range(c, c)],
            negated: false,
            minus: None,
        }
    }

    fn has(&self, c: char) -> bool {
        let hit = self.items.iter().any(|i| match i {
            Item::Range(a, b) => (*a..=*b).contains(&c),
            Item::Esc(e) => esc(*e, c),
            Item::Cat(name, neg) => category(name, c) != *neg,
        });
        let hit = hit != self.negated;
        hit && !self.minus.as_ref().is_some_and(|m| m.has(c))
    }
}

fn name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == ':'
}

fn name_char(c: char) -> bool {
    name_start(c) || c.is_numeric() || c == '-' || c == '.' || c == '\u{B7}'
}

fn esc(e: char, c: char) -> bool {
    match e {
        'd' => c.is_ascii_digit() || (!c.is_ascii() && c.is_numeric()),
        'D' => !esc('d', c),
        's' => matches!(c, ' ' | '\t' | '\n' | '\r'),
        'S' => !esc('s', c),
        // Not punctuation, separators or "other".
        'w' => !(c.is_whitespace() || c.is_control() || (c.is_ascii_punctuation() && c != '_')),
        'W' => !esc('w', c),
        'i' => name_start(c),
        'I' => !name_start(c),
        'c' => name_char(c),
        'C' => !name_char(c),
        '.' => c != '\n' && c != '\r',
        _ => false,
    }
}

fn category(name: &str, c: char) -> bool {
    match name {
        "L" => c.is_alphabetic(),
        "Lu" => c.is_uppercase(),
        "Ll" => c.is_lowercase(),
        "N" => c.is_numeric(),
        "Nd" => esc('d', c),
        "P" => c.is_ascii_punctuation(),
        "Z" | "Zs" => c.is_whitespace(),
        _ => false,
    }
}

/// Why a pattern could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadPattern(pub String);

struct Parser<'a> {
    c: Vec<char>,
    i: usize,
    _src: &'a str,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn bad<T>(&self, why: &str) -> Result<T, BadPattern> {
        Err(BadPattern(format!("{why} at {}", self.i)))
    }

    fn alt(&mut self, depth: usize) -> Result<Node, BadPattern> {
        if depth > 64 {
            return self.bad("nested too deep");
        }
        let mut alts = vec![self.seq(depth)?];
        while self.peek() == Some('|') {
            self.i += 1;
            alts.push(self.seq(depth)?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().unwrap_or(Node::Empty)
        } else {
            Node::Alt(alts)
        })
    }

    fn seq(&mut self, depth: usize) -> Result<Node, BadPattern> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom(depth)?;
            let atom = self.quant(atom)?;
            items.push(atom);
        }
        Ok(Node::Seq(items))
    }

    fn quant(&mut self, atom: Node) -> Result<Node, BadPattern> {
        let (lo, hi) = match self.peek() {
            Some('?') => (0, Some(1)),
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('{') => {
                self.i += 1;
                let lo = self.number()?;
                let hi = if self.peek() == Some(',') {
                    self.i += 1;
                    if self.peek() == Some('}') {
                        None
                    } else {
                        Some(self.number()?)
                    }
                } else {
                    Some(lo)
                };
                if self.peek() != Some('}') {
                    return self.bad("unclosed {");
                }
                if hi.is_some_and(|h| h < lo) || lo > 10_000 {
                    return self.bad("bad repeat");
                }
                (lo, hi)
            }
            _ => return Ok(atom),
        };
        self.i += 1;
        Ok(Node::Repeat(Box::new(atom), lo, hi))
    }

    fn number(&mut self) -> Result<u32, BadPattern> {
        let s = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        let t: String = self.c[s..self.i].iter().collect();
        t.parse().or_else(|_| self.bad("bad number"))
    }

    fn atom(&mut self, depth: usize) -> Result<Node, BadPattern> {
        let Some(c) = self.peek() else {
            return self.bad("end");
        };
        self.i += 1;
        Ok(match c {
            '(' => {
                let n = self.alt(depth + 1)?;
                if self.peek() != Some(')') {
                    return self.bad("unclosed (");
                }
                self.i += 1;
                n
            }
            '.' => Node::Char(Class {
                items: vec![Item::Esc('.')],
                negated: false,
                minus: None,
            }),
            '[' => Node::Char(self.class()?),
            '\\' => Node::Char(self.escape()?),
            '?' | '*' | '+' | '{' => return self.bad("nothing to repeat"),
            ']' | '}' => Node::Char(Class::one(c)),
            c => Node::Char(Class::one(c)),
        })
    }

    /// After a backslash.
    fn escape(&mut self) -> Result<Class, BadPattern> {
        let Some(e) = self.peek() else {
            return self.bad("lone \\");
        };
        self.i += 1;
        let item = match e {
            'n' => Item::Range('\n', '\n'),
            'r' => Item::Range('\r', '\r'),
            't' => Item::Range('\t', '\t'),
            'd' | 'D' | 's' | 'S' | 'w' | 'W' | 'i' | 'I' | 'c' | 'C' => Item::Esc(e),
            'p' | 'P' => {
                if self.peek() != Some('{') {
                    return self.bad("\\p without {");
                }
                self.i += 1;
                let s = self.i;
                while self.peek().is_some_and(|c| c != '}') {
                    self.i += 1;
                }
                let name: String = self.c[s..self.i].iter().collect();
                if self.peek() != Some('}') {
                    return self.bad("unclosed \\p{");
                }
                self.i += 1;
                Item::Cat(name, e == 'P')
            }
            '\\' | '|' | '.' | '-' | '^' | '?' | '*' | '+' | '{' | '}' | '(' | ')' | '[' | ']' => {
                Item::Range(e, e)
            }
            _ => return self.bad("unknown escape"),
        };
        Ok(Class {
            items: vec![item],
            negated: false,
            minus: None,
        })
    }

    /// After `[`.
    fn class(&mut self) -> Result<Class, BadPattern> {
        let mut cl = Class {
            items: Vec::new(),
            negated: false,
            minus: None,
        };
        if self.peek() == Some('^') {
            cl.negated = true;
            self.i += 1;
        }
        let mut first = true;
        loop {
            let Some(c) = self.peek() else {
                return self.bad("unclosed [");
            };
            if c == ']' && !first {
                self.i += 1;
                break;
            }
            first = false;
            if c == '-' && self.c.get(self.i + 1) == Some(&'[') {
                self.i += 2;
                cl.minus = Some(Box::new(self.class()?));
                if self.peek() != Some(']') {
                    return self.bad("subtraction not last");
                }
                self.i += 1;
                break;
            }
            self.i += 1;
            let lo = if c == '\\' {
                let e = self.escape()?;
                match e.items.first() {
                    Some(Item::Range(a, _)) => *a,
                    Some(other) => {
                        cl.items.push(other.clone());
                        continue;
                    }
                    None => continue,
                }
            } else {
                c
            };
            if self.peek() == Some('-')
                && self
                    .c
                    .get(self.i + 1)
                    .is_some_and(|&n| n != ']' && n != '[')
            {
                self.i += 1;
                let Some(mut hi) = self.peek() else {
                    return self.bad("unclosed range");
                };
                self.i += 1;
                if hi == '\\' {
                    let e = self.escape()?;
                    match e.items.first() {
                        Some(Item::Range(a, _)) => hi = *a,
                        _ => return self.bad("bad range"),
                    }
                }
                if hi < lo {
                    return self.bad("bad range");
                }
                cl.items.push(Item::Range(lo, hi));
            } else {
                cl.items.push(Item::Range(lo, lo));
            }
        }
        Ok(cl)
    }
}

impl Pattern {
    pub fn new(src: &str) -> Result<Pattern, BadPattern> {
        let mut p = Parser {
            c: src.chars().collect(),
            i: 0,
            _src: src,
        };
        let node = p.alt(0)?;
        if p.i != p.c.len() {
            return p.bad("unmatched )");
        }
        Ok(Pattern { node })
    }

    /// Whether the whole of `s` matches.
    pub fn matches(&self, s: &str) -> bool {
        let c: Vec<char> = s.chars().collect();
        let mut budget = 200_000u32;
        m(&self.node, &c, 0, &mut budget, &mut |i, _| i == c.len())
    }
}

/// Matches `n` at `i`, then the continuation `k` from where it ended.
fn m(
    n: &Node,
    c: &[char],
    i: usize,
    budget: &mut u32,
    k: &mut dyn FnMut(usize, &mut u32) -> bool,
) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    match n {
        Node::Empty => k(i, budget),
        Node::Char(cl) => c.get(i).is_some_and(|&x| cl.has(x)) && k(i + 1, budget),
        Node::Seq(items) => seq(items, c, i, budget, k),
        Node::Alt(alts) => alts.iter().any(|a| m(a, c, i, budget, k)),
        Node::Repeat(inner, lo, hi) => rep(inner, *lo, *hi, 0, c, i, budget, k),
    }
}

fn seq(
    items: &[Node],
    c: &[char],
    i: usize,
    budget: &mut u32,
    k: &mut dyn FnMut(usize, &mut u32) -> bool,
) -> bool {
    match items.split_first() {
        None => k(i, budget),
        Some((first, rest)) => m(first, c, i, budget, &mut |j, b| seq(rest, c, j, b, k)),
    }
}

#[allow(clippy::too_many_arguments)]
fn rep(
    inner: &Node,
    lo: u32,
    hi: Option<u32>,
    done: u32,
    c: &[char],
    i: usize,
    budget: &mut u32,
    k: &mut dyn FnMut(usize, &mut u32) -> bool,
) -> bool {
    // Greedy: one more first (only when it consumes), then stop.
    if hi.is_none_or(|h| done < h)
        && m(inner, c, i, budget, &mut |j, b| {
            j > i && rep(inner, lo, hi, done + 1, c, j, b, k)
        })
    {
        return true;
    }
    done >= lo && k(i, budget)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(p: &str, s: &str) -> bool {
        Pattern::new(p).unwrap().matches(s)
    }

    #[test]
    fn xsd_patterns() {
        assert!(ok("abc", "abc"));
        assert!(!ok("abc", "abcd"), "whole string only");
        assert!(ok("a.c", "abc"));
        assert!(ok("[A-Z]{2}[0-9]+", "EW123"));
        assert!(!ok("[A-Z]{2}[0-9]+", "EW"));
        assert!(ok("(Foo|Bar)_.*", "Bar_x"));
        assert!(ok("\\d{3}-\\d{2}", "123-45"));
        assert!(ok("[a-z-[aeiou]]+", "bcd"));
        assert!(!ok("[a-z-[aeiou]]+", "bad"));
        assert!(ok("[^0-9]*", "abc"));
        assert!(!ok("[^0-9]*", "a1"));
        assert!(ok("\\p{Lu}\\p{Ll}+", "Wall"));
        assert!(ok("a{2,3}", "aaa"));
        assert!(!ok("a{2,3}", "aaaa"));
        assert!(ok("a{2,}", "aaaaa"));
        assert!(ok("x?y*z+", "zz"));
        assert!(ok("\\.\\*", ".*"));
        assert!(ok("", ""));
        assert!(ok("(a*)*b", "aaab"));
        // A pathological pattern ends within the budget.
        assert!(!ok("(a*)*b", &"a".repeat(60)));
        assert!(Pattern::new("(ab").is_err());
        assert!(Pattern::new("a{3,1}").is_err());
        assert!(Pattern::new("*a").is_err());
    }
}
