//! Rules of a smart album: queries combined in order with and, or and except.
//!
//! Each rule is a query in the usual language. The first rule starts the set; each later
//! rule narrows it (and), widens it (or) or takes its matches out (except), strictly left to
//! right: `a or b and c` is `(a or b) and c`, the way a list of rules reads top to bottom.
//! Blank rules are left out.
//!
//! Unknown facts stay unknown through the combination (an `except` rule whose facts are not
//! read yet lets the file through), so results only shrink as metadata is read, as with a
//! single query.
//!
//! Stored as text, one rule per line: the word `and`, `or` or `except`, a tab, the query.

use crate::{Facts, Fold, Matcher, Query, Scope, Tri, parse_in};

/// How a rule joins the rules above it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Combine {
    #[default]
    And,
    Or,
    Except,
}

impl Combine {
    pub const ALL: [Combine; 3] = [Combine::And, Combine::Or, Combine::Except];

    /// The stored word.
    pub fn word(self) -> &'static str {
        match self {
            Combine::And => "and",
            Combine::Or => "or",
            Combine::Except => "except",
        }
    }

    pub fn from_word(w: &str) -> Option<Combine> {
        Combine::ALL
            .into_iter()
            .find(|c| c.word().eq_ignore_ascii_case(w.trim()))
    }
}

/// One rule: a query and how it joins the rules above (ignored for the first).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rule {
    pub combine: Combine,
    pub query: String,
}

impl Rule {
    pub fn new(combine: Combine, query: &str) -> Rule {
        Rule {
            combine,
            query: query.to_owned(),
        }
    }
}

/// A smart album's rules, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rules(pub Vec<Rule>);

impl Rules {
    /// One rule: what a saved search is.
    pub fn one(query: &str) -> Rules {
        Rules(vec![Rule::new(Combine::And, query)])
    }

    /// The rules that are not blank.
    pub fn active(&self) -> impl Iterator<Item = &Rule> {
        self.0.iter().filter(|r| !r.query.trim().is_empty())
    }

    /// No rule with any text: matches everything.
    pub fn is_empty(&self) -> bool {
        self.active().next().is_none()
    }

    /// The stored form: one rule per line, `word\tquery` (blank rules left out; line breaks
    /// in a query become spaces).
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for r in self.active() {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(r.combine.word());
            s.push('\t');
            let q: String = r
                .query
                .trim()
                .chars()
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                .collect();
            s.push_str(&q);
        }
        s
    }

    /// Reads [`Rules::to_text`]. A line without a known word before a tab is a rule joined
    /// with and (a plain query is one rule).
    pub fn from_text(s: &str) -> Rules {
        Rules(
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| match l.split_once('\t') {
                    Some((w, q)) => match Combine::from_word(w) {
                        Some(c) => Rule::new(c, q.trim()),
                        None => Rule::new(Combine::And, l.trim()),
                    },
                    None => Rule::new(Combine::And, l.trim()),
                })
                .collect(),
        )
    }

    /// Each active rule parsed (for its errors), in order.
    pub fn parsed(&self, scope: Scope) -> Vec<(Combine, Query)> {
        self.active()
            .map(|r| (r.combine, parse_in(&r.query, scope)))
            .collect()
    }

    /// Ready to run against many files.
    pub fn matcher(&self, scope: Scope, fold: Fold) -> RulesMatcher {
        let parts = self
            .parsed(scope)
            .into_iter()
            .map(|(c, q)| {
                let m = Matcher::new(&q, fold);
                (c, q, m)
            })
            .collect();
        RulesMatcher { parts }
    }
}

/// [`Rules`] compiled.
pub struct RulesMatcher {
    parts: Vec<(Combine, Query, Matcher)>,
}

impl RulesMatcher {
    /// No rules: everything matches.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Whether any rule reads image metadata.
    pub fn needs_meta(&self) -> bool {
        self.parts.iter().any(|p| p.2.needs_meta())
    }

    /// The file against the rules, left to right.
    pub fn tri(&self, f: &Facts) -> Tri {
        let mut it = self.parts.iter();
        let Some(first) = it.next() else {
            return Tri::Yes;
        };
        let mut acc = first.2.tri(f);
        for (c, _, m) in it {
            // Settled either way already: the rule cannot change it.
            match (c, acc) {
                (Combine::And | Combine::Except, Tri::No) | (Combine::Or, Tri::Yes) => continue,
                _ => {}
            }
            let r = m.tri(f);
            acc = match c {
                Combine::And => and(acc, r),
                Combine::Or => or(acc, r),
                Combine::Except => and(acc, r.not()),
            };
        }
        acc
    }

    /// Whether the file passes (unknown passes).
    pub fn matches(&self, f: &Facts) -> bool {
        self.tri(f) != Tri::No
    }

    /// The rules whose terms every match must pass, so a search may narrow its candidates
    /// by them (push them down): rule `i` when it is the first or joined with and, and no
    /// rule after it is joined with or. An except rule's terms never qualify.
    pub fn narrowing(&self) -> impl Iterator<Item = &Query> {
        // The first rule's word means nothing.
        let last_or = self
            .parts
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, p)| p.0 == Combine::Or)
            .map(|(i, _)| i)
            .next_back();
        self.parts
            .iter()
            .enumerate()
            .filter(move |(i, p)| match last_or {
                None => *i == 0 || p.0 == Combine::And,
                Some(k) => *i > k && p.0 == Combine::And,
            })
            .map(|(_, p)| &p.1)
    }

    /// The parsed rules, in order.
    pub fn queries(&self) -> impl Iterator<Item = (Combine, &Query)> {
        self.parts.iter().map(|p| (p.0, &p.1))
    }
}

fn and(a: Tri, b: Tri) -> Tri {
    match (a, b) {
        (Tri::No, _) | (_, Tri::No) => Tri::No,
        (Tri::Yes, Tri::Yes) => Tri::Yes,
        _ => Tri::Unknown,
    }
}

fn or(a: Tri, b: Tri) -> Tri {
    match (a, b) {
        (Tri::Yes, _) | (_, Tri::Yes) => Tri::Yes,
        (Tri::No, Tri::No) => Tri::No,
        _ => Tri::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ascii_fold;

    fn photo<'a>(name: &'a str, rating: u8, albums: Option<&'a str>) -> Facts<'a> {
        Facts {
            name,
            ext: "jpg",
            rating,
            known: true,
            albums,
            tags: Some(""),
            ..Facts::default()
        }
    }

    fn rules(r: &[(Combine, &str)]) -> RulesMatcher {
        Rules(r.iter().map(|(c, q)| Rule::new(*c, q)).collect()).matcher(Scope::Library, ascii_fold)
    }

    #[test]
    fn rules_combine_left_to_right() {
        use Combine::*;
        let a = photo("a.jpg", 5, Some("Client X/selects"));
        let b = photo("b.jpg", 2, Some("Client X/all\nPortfolio"));
        let c = photo("c.jpg", 4, Some(""));
        let m = rules(&[(And, "rating:>=4"), (Or, "album:portfolio")]);
        assert!(m.matches(&a) && m.matches(&b) && m.matches(&c));
        let m = rules(&[(And, "rating:>=4"), (Except, "album:\"client x\"")]);
        assert!(!m.matches(&a) && !m.matches(&b) && m.matches(&c));
        // (a or b) and c, not a or (b and c).
        let m = rules(&[(And, "a.jpg"), (Or, "b.jpg"), (And, "rating:>=3")]);
        assert!(m.matches(&a) && !m.matches(&b) && !m.matches(&c));
        let m = rules(&[(And, "album=\"Client X/selects\"")]);
        assert!(m.matches(&a) && !m.matches(&b));
        // Blank rules are left out; none matches everything.
        let m = rules(&[(And, "  "), (Except, "")]);
        assert!(m.is_empty() && m.matches(&a));
        // The first rule's word means nothing.
        let m = rules(&[(Except, "a.jpg")]);
        assert!(m.matches(&a) && !m.matches(&b));
    }

    #[test]
    fn unknown_stays_unknown() {
        use Combine::*;
        // Albums not known: the except rule lets it through.
        let x = photo("x.jpg", 5, None);
        let m = rules(&[(And, "rating:5"), (Except, "album:selects")]);
        assert_eq!(m.tri(&x), Tri::Unknown);
        assert!(m.matches(&x));
        let m = rules(&[(And, "rating:1"), (Or, "album:selects")]);
        assert_eq!(m.tri(&x), Tri::Unknown);
        let m = rules(&[(And, "rating:5"), (Or, "album:selects")]);
        assert_eq!(m.tri(&x), Tri::Yes);
    }

    #[test]
    fn narrowing_rules_are_those_every_match_passes() {
        use Combine::*;
        let n = |r: &[(Combine, &str)]| -> Vec<String> {
            let m = rules(r);
            m.narrowing()
                .map(|q| {
                    q.terms
                        .iter()
                        .map(|t| format!("{:?}", t.field()))
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect()
        };
        assert_eq!(n(&[(And, "rating:5"), (And, "iso:100")]).len(), 2);
        assert_eq!(n(&[(And, "rating:5"), (Except, "iso:100")]).len(), 1);
        assert!(n(&[(And, "rating:5"), (Or, "iso:100")]).is_empty());
        assert_eq!(
            n(&[(And, "rating:5"), (Or, "iso:100"), (And, "ext:jpg")]),
            vec!["Some(Ext)".to_owned()]
        );
        assert_eq!(n(&[(Or, "rating:5")]).len(), 1);
    }

    #[test]
    fn rules_round_trip_as_text() {
        let r = Rules(vec![
            Rule::new(Combine::And, " rating:>=4 "),
            Rule::new(Combine::Or, ""),
            Rule::new(Combine::Except, "tag:\"out\ntakes\""),
        ]);
        let t = r.to_text();
        assert_eq!(t, "and\trating:>=4\nexcept\ttag:\"out takes\"");
        let back = Rules::from_text(&t);
        assert_eq!(back.0.len(), 2);
        assert_eq!(back.0[1], Rule::new(Combine::Except, "tag:\"out takes\""));
        assert_eq!(Rules::from_text("rating:5"), Rules::one("rating:5"));
        assert_eq!(Rules::from_text("maybe\tx").0[0].query, "maybe\tx");
    }

    #[test]
    fn album_is_library_only() {
        let q = crate::parse_in("album:selects", Scope::Gallery);
        assert!(q.terms.is_empty() && !q.errors.is_empty());
        let q = crate::parse_in("albums:selects", Scope::Library);
        assert_eq!(q.terms[0].field(), Some(crate::Field::Album));
        assert!(!crate::Field::Album.needs_meta());
    }
}
