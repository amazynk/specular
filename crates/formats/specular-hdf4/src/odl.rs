//! An ODL (Object Description Language) parser for HDF-EOS structure, core and archive
//! metadata: `GROUP`/`OBJECT` blocks of `KEY = VALUE` statements, where a value is a quoted
//! string, a number or word, or a parenthesised list (which may span lines and nest);
//! `/* … */` comments are skipped and `END` ends the text.

use crate::{Error, Result, limits};

/// A value.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    /// A quoted string.
    Str(String),
    /// A number, word or anything else unquoted.
    Word(String),
    List(Vec<Val>),
}

impl Val {
    /// The text of a string or word.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Val::Str(s) | Val::Word(s) => Some(s),
            Val::List(_) => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Val::Word(s) | Val::Str(s) => parse_num(s),
            Val::List(v) if v.len() == 1 => v[0].as_f64(),
            Val::List(_) => None,
        }
    }

    /// The numbers of a list (a single number gives one).
    pub fn numbers(&self) -> Vec<f64> {
        match self {
            Val::List(v) => v.iter().filter_map(Val::as_f64).collect(),
            v => v.as_f64().into_iter().collect(),
        }
    }

    /// The strings of a list (a single value gives one).
    pub fn strings(&self) -> Vec<String> {
        match self {
            Val::List(v) => v
                .iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect(),
            v => v.as_str().map(str::to_owned).into_iter().collect(),
        }
    }

    /// A display form: lists joined with ", ".
    pub fn display(&self) -> String {
        match self {
            Val::Str(s) | Val::Word(s) => s.clone(),
            Val::List(v) => {
                let s: Vec<String> = v.iter().map(Val::display).collect();
                format!("({})", s.join(", "))
            }
        }
    }
}

fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    // Fortran-style exponents ("1.0D+02") appear in some metadata.
    let t = t.replace(['D', 'd'], "E");
    t.parse().ok()
}

/// A `GROUP` or `OBJECT` block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub name: String,
    /// An `OBJECT` (not a `GROUP`).
    pub object: bool,
    pub items: Vec<(String, Val)>,
    pub groups: Vec<Group>,
}

impl Group {
    /// The value of a statement (keys compare without case).
    pub fn get(&self, key: &str) -> Option<&Val> {
        self.items
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    pub fn num(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_f64()
    }

    /// The child block of that name (without case).
    pub fn group(&self, name: &str) -> Option<&Group> {
        self.groups
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(name))
    }

    /// Every block below this one, depth first.
    pub fn walk(&self) -> Vec<&Group> {
        let mut out = Vec::new();
        let mut stack: Vec<&Group> = self.groups.iter().rev().collect();
        while let Some(g) = stack.pop() {
            out.push(g);
            stack.extend(g.groups.iter().rev());
        }
        out
    }
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    /// Skips blanks (not newlines) and comments.
    fn blanks(&mut self) {
        loop {
            while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\r') {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"/*") {
                match find(&self.s[self.i + 2..], b"*/") {
                    Some(k) => self.i += 2 + k + 2,
                    None => self.i = self.s.len(),
                }
                continue;
            }
            break;
        }
    }

    /// Skips blanks, comments and newlines.
    fn space(&mut self) {
        loop {
            self.blanks();
            if self.i < self.s.len() && self.s[self.i] == b'\n' {
                self.i += 1;
                continue;
            }
            break;
        }
    }

    fn word(&mut self) -> String {
        let st = self.i;
        while self.i < self.s.len()
            && !matches!(
                self.s[self.i],
                b'=' | b'\n' | b' ' | b'\t' | b'\r' | b',' | b')'
            )
            && !self.s[self.i..].starts_with(b"/*")
        {
            self.i += 1;
        }
        latin1(&self.s[st..self.i])
    }

    fn value(&mut self, depth: usize) -> Result<Val> {
        if depth > 32 {
            return Err(Error::Malformed("ODL lists nested too deep".into()));
        }
        self.blanks();
        match self.s.get(self.i) {
            Some(b'"') => {
                let st = self.i + 1;
                let k = self.s[st..]
                    .iter()
                    .position(|&c| c == b'"')
                    .ok_or_else(|| Error::Malformed("ODL string not closed".into()))?;
                self.i = st + k + 1;
                Ok(Val::Str(latin1(&self.s[st..st + k])))
            }
            Some(b'\'') => {
                let st = self.i + 1;
                let k = self.s[st..]
                    .iter()
                    .position(|&c| c == b'\'')
                    .ok_or_else(|| Error::Malformed("ODL string not closed".into()))?;
                self.i = st + k + 1;
                Ok(Val::Str(latin1(&self.s[st..st + k])))
            }
            Some(b'(') | Some(b'{') => {
                let close = if self.s[self.i] == b'(' { b')' } else { b'}' };
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.space();
                    match self.s.get(self.i) {
                        None => return Err(Error::Malformed("ODL list not closed".into())),
                        Some(&c) if c == close => {
                            self.i += 1;
                            break;
                        }
                        Some(b',') => {
                            self.i += 1;
                        }
                        Some(_) => {
                            let st = self.i;
                            v.push(self.value(depth + 1)?);
                            if self.i == st {
                                // A character no value starts with: skip it.
                                self.i += 1;
                            }
                        }
                    }
                }
                Ok(Val::List(v))
            }
            _ => {
                // An unquoted value: up to the end of the line (or the list item).
                let st = self.i;
                let in_list = depth > 0;
                while self.i < self.s.len() {
                    let c = self.s[self.i];
                    if c == b'\n' || (in_list && matches!(c, b',' | b')' | b'}')) {
                        break;
                    }
                    if self.s[self.i..].starts_with(b"/*") {
                        break;
                    }
                    self.i += 1;
                }
                Ok(Val::Word(latin1(&self.s[st..self.i]).trim().to_owned()))
            }
        }
    }
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// Parses ODL text into a root block holding the top-level statements and blocks.
pub fn parse(text: &str) -> Result<Group> {
    parse_bytes(text.as_bytes())
}

pub fn parse_bytes(b: &[u8]) -> Result<Group> {
    if b.len() > limits::MAX_ODL {
        return Err(Error::TooLarge);
    }
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let mut p = P { s: &b[..end], i: 0 };
    let mut stack: Vec<Group> = vec![Group::default()];
    loop {
        p.space();
        if p.i >= p.s.len() {
            break;
        }
        let key = p.word();
        if key.is_empty() {
            // A stray character: skip the line.
            while p.i < p.s.len() && p.s[p.i] != b'\n' {
                p.i += 1;
            }
            continue;
        }
        p.blanks();
        let has_value = p.s.get(p.i) == Some(&b'=');
        if has_value {
            p.i += 1;
        }
        let upper = key.to_ascii_uppercase();
        if upper == "END" && !has_value {
            break;
        }
        let val = if has_value {
            p.value(0)?
        } else {
            Val::Word(String::new())
        };
        match upper.as_str() {
            "GROUP" | "OBJECT" | "BEGIN_GROUP" | "BEGIN_OBJECT" => {
                if stack.len() > 64 {
                    return Err(Error::Malformed("ODL nested too deep".into()));
                }
                stack.push(Group {
                    name: val.as_str().unwrap_or("").to_owned(),
                    object: upper.ends_with("OBJECT"),
                    ..Default::default()
                });
            }
            "END_GROUP" | "END_OBJECT" => {
                if stack.len() > 1 {
                    let g = stack.pop().unwrap_or_default();
                    if let Some(top) = stack.last_mut() {
                        top.groups.push(g);
                    }
                } else {
                    return Err(Error::Malformed(format!("ODL {upper} without a block")));
                }
            }
            _ => {
                if let Some(top) = stack.last_mut() {
                    top.items.push((key, val));
                }
            }
        }
    }
    // Blocks left open close at the end.
    while stack.len() > 1 {
        let g = stack.pop().unwrap_or_default();
        if let Some(top) = stack.last_mut() {
            top.groups.push(g);
        }
    }
    Ok(stack.pop().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_metadata() {
        let t = "GROUP=GridStructure\n\tGROUP=GRID_1\n\t\tGridName=\"MOD_Grid\"\n\t\tXDim=2400\n\
                 \t\tUpperLeftPointMtrs=(-11119505.196667,4447802.078667)\n\
                 \t\tProjection=GCTP_SNSOID\n\t\tProjParams=(6371007.181000,0,0,0,0,0,0,0,0,0,0,0,0)\n\
                 \t\tGROUP=DataField\n\t\t\tOBJECT=DataField_1\n\t\t\t\tDataFieldName=\"sur_refl_b01\"\n\
                 \t\t\t\tDimList=(\"YDim\",\"XDim\")\n\t\t\tEND_OBJECT=DataField_1\n\t\tEND_GROUP=DataField\n\
                 \tEND_GROUP=GRID_1\nEND_GROUP=GridStructure\nEND\n";
        let r = parse(t).unwrap();
        let g = r.group("GridStructure").unwrap().group("GRID_1").unwrap();
        assert_eq!(g.str("GridName"), Some("MOD_Grid"));
        assert_eq!(g.num("XDim"), Some(2400.0));
        assert_eq!(
            g.get("UpperLeftPointMtrs").unwrap().numbers(),
            vec![-11119505.196667, 4447802.078667]
        );
        assert_eq!(g.str("Projection"), Some("GCTP_SNSOID"));
        assert_eq!(g.get("ProjParams").unwrap().numbers().len(), 13);
        let f = g.group("DataField").unwrap().group("DataField_1").unwrap();
        assert!(f.object);
        assert_eq!(f.get("DimList").unwrap().strings(), vec!["YDim", "XDim"]);
    }

    #[test]
    fn core_metadata() {
        let t = "\nGROUP                  = INVENTORYMETADATA\n  GROUPTYPE            = MASTERGROUP\n\n\
                 \x20 OBJECT                 = SHORTNAME /* the product */\n    NUM_VAL              = 1\n\
                 \x20   VALUE                = \"MOD09A1\"\n  END_OBJECT             = SHORTNAME\n\
                 \x20 OBJECT = GRINGPOINTLATITUDE\n    VALUE = (29.9, 30.0,\n      40.0, 39.9)\n\
                 \x20 END_OBJECT = GRINGPOINTLATITUDE\n\
                 \x20 OBJECT = NESTED\n    VALUE = ((1, 2), (3, 4))\n  END_OBJECT = NESTED\n\
                 \x20 OBJECT = DATE\n    VALUE = 1.5D+02\n  END_OBJECT = DATE\n\
                 END_GROUP              = INVENTORYMETADATA\n\nEND\n";
        let r = parse(t).unwrap();
        let g = r.group("INVENTORYMETADATA").unwrap();
        assert_eq!(g.str("GROUPTYPE"), Some("MASTERGROUP"));
        assert_eq!(g.group("SHORTNAME").unwrap().str("VALUE"), Some("MOD09A1"));
        assert_eq!(
            g.group("GRINGPOINTLATITUDE")
                .unwrap()
                .get("VALUE")
                .unwrap()
                .numbers(),
            vec![29.9, 30.0, 40.0, 39.9]
        );
        match g.group("NESTED").unwrap().get("VALUE").unwrap() {
            Val::List(v) => assert_eq!(v[1].numbers(), vec![3.0, 4.0]),
            v => panic!("{v:?}"),
        }
        assert_eq!(g.group("DATE").unwrap().num("VALUE"), Some(150.0));
        assert_eq!(r.walk().len(), 5);
    }

    #[test]
    fn refusals() {
        assert!(parse("A = \"open string\n").is_err());
        assert!(parse("A = (1, 2\n").is_err());
        assert!(parse("END_GROUP = X\n").is_err());
        // Blocks left open are closed; text after END is ignored.
        let r = parse("GROUP = G\n A = 1\nEND\nB = 2").unwrap();
        assert_eq!(r.group("G").unwrap().num("A"), Some(1.0));
        assert!(r.get("B").is_none());
    }
}
