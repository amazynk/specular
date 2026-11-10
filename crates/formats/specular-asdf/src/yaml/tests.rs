use super::*;

fn p(s: &str) -> Value {
    parse(s).unwrap_or_else(|e| panic!("{e}\n{s}"))
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

#[test]
fn asdf_header_and_tags() {
    let v = p(
        "#ASDF 1.0.0\n#ASDF_STANDARD 1.6.0\n%YAML 1.1\n%TAG ! tag:stsci.edu:asdf/\n\
               --- !core/asdf-1.1.0\nasdf_library: !core/software-1.0.0 {author: The ASDF \
               Developers, homepage: 'http://github.com/asdf-format/asdf',\n  name: asdf, \
               version: 5.4.0}\nx: !core/ndarray-1.1.0\n  source: 0\n  datatype: int16\n  \
               byteorder: big\n  shape: [3, 4]\nr: !<asdf://stsci.edu/datamodels/roman/tags/\
               wfi_image-2.1.0>\n  meta: {telescope: ROMAN}\n...\n",
    );
    assert_eq!(v.tag.as_deref(), Some("tag:stsci.edu:asdf/core/asdf-1.1.0"));
    let lib = v.get("asdf_library").unwrap();
    assert!(lib.tag_is("software"));
    assert_eq!(s(lib.get("author").unwrap()), "The ASDF Developers");
    assert_eq!(s(lib.get("version").unwrap()), "5.4.0");
    assert_eq!(
        s(lib.get("homepage").unwrap()),
        "http://github.com/asdf-format/asdf"
    );
    let x = v.get("x").unwrap();
    assert!(x.tag_is("ndarray"));
    assert_eq!(x.get("source").unwrap().as_i64(), Some(0));
    let shape: Vec<i64> = x
        .get("shape")
        .unwrap()
        .as_seq()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(shape, [3, 4]);
    let r = v.get("r").unwrap();
    assert!(r.tag_is("wfi_image"));
    assert_eq!(s(r.path(&["meta", "telescope"]).unwrap()), "ROMAN");
}

#[test]
fn block_collections() {
    let v = p(
        "a:\n- 1\n- 2\nb:\n  c: x\n  d:\n    - e: 1\n      f: 2\n    - - g\n      - h\n\
               k: [1, [2, 3], {m: n}]\n",
    );
    assert_eq!(v.get("a").unwrap().as_seq().unwrap().len(), 2);
    let d = v.path(&["b", "d"]).unwrap().as_seq().unwrap();
    assert_eq!(d[0].get("f").unwrap().as_i64(), Some(2));
    assert_eq!(s(&d[1].as_seq().unwrap()[1]), "h");
    let k = v.get("k").unwrap().as_seq().unwrap();
    assert_eq!(k[1].as_seq().unwrap()[1].as_i64(), Some(3));
    assert_eq!(s(k[2].get("m").unwrap()), "n");
}

#[test]
fn empty_values_and_comments() {
    let v = p("# c\na:   # comment\nb: ~\nc: 'x # not a comment' # comment\nd: x#y\ne:\n");
    assert!(v.get("a").unwrap().is_null());
    assert!(v.get("b").unwrap().is_null());
    assert_eq!(s(v.get("c").unwrap()), "x # not a comment");
    assert_eq!(s(v.get("d").unwrap()), "x#y");
    assert!(v.get("e").unwrap().is_null());
}

#[test]
fn scalars_resolve_like_pyyaml() {
    let v = p(
        "[yes, No, on, OFF, true, ~, null, 12, -0x1F, 0o17, 017, 0b101, 1_000, 190:20:30, \
               1.5, -.inf, .NaN, 1.0e+20, 6.8e-3, 1e5, '12', \"yes\", 1.2.3, 0.5.0, 2020-01-01]",
    );
    let a = v.as_seq().unwrap();
    let n = |i: usize| a[i].node.clone();
    assert_eq!(n(0), Node::Bool(true));
    assert_eq!(n(1), Node::Bool(false));
    assert_eq!(n(2), Node::Bool(true));
    assert_eq!(n(3), Node::Bool(false));
    assert_eq!(n(4), Node::Bool(true));
    assert_eq!(n(5), Node::Null);
    assert_eq!(n(6), Node::Null);
    assert_eq!(n(7), Node::Int(12));
    assert_eq!(n(8), Node::Int(-31));
    assert_eq!(n(9), Node::Int(15));
    assert_eq!(n(10), Node::Int(15));
    assert_eq!(n(11), Node::Int(5));
    assert_eq!(n(12), Node::Int(1000));
    assert_eq!(n(13), Node::Int(190 * 3600 + 20 * 60 + 30));
    assert_eq!(n(14), Node::Float(1.5));
    assert_eq!(n(15), Node::Float(f64::NEG_INFINITY));
    assert!(matches!(n(16), Node::Float(f) if f.is_nan()));
    assert_eq!(n(17), Node::Float(1.0e20));
    assert_eq!(n(18), Node::Float(6.8e-3));
    assert_eq!(n(19), Node::Float(1e5));
    assert_eq!(n(20), Node::Str("12".into()));
    assert_eq!(n(21), Node::Str("yes".into()));
    assert_eq!(n(22), Node::Str("1.2.3".into()));
    assert_eq!(n(23), Node::Str("0.5.0".into()));
    assert_eq!(n(24), Node::Str("2020-01-01".into()));
}

#[test]
fn core_tags() {
    let v = p("[!!str 12, !!int '7', !!float 3, !!null '', ! 12, !!bool yes]");
    let a = v.as_seq().unwrap();
    assert_eq!(a[0].node, Node::Str("12".into()));
    assert_eq!(a[1].node, Node::Int(7));
    assert_eq!(a[2].node, Node::Float(3.0));
    assert_eq!(a[3].node, Node::Null);
    assert_eq!(a[4].node, Node::Str("12".into()));
    assert_eq!(a[5].node, Node::Bool(true));
    assert!(a.iter().all(|v| v.tag.is_none()));
}

#[test]
fn quoted_scalars() {
    let v = p(
        "a: 'it''s'\nb: \"tab\\there \\x41\\u00e9\\U0001F600 \\\"q\\\" \\\\\"\n\
               c: \"folded\n  line\n\n  para\"\nd: \"joined\\\n   here\"\ne: 'multi\n  line'\n",
    );
    assert_eq!(s(v.get("a").unwrap()), "it's");
    assert_eq!(s(v.get("b").unwrap()), "tab\there Aé😀 \"q\" \\");
    assert_eq!(s(v.get("c").unwrap()), "folded line\npara");
    assert_eq!(s(v.get("d").unwrap()), "joinedhere");
    assert_eq!(s(v.get("e").unwrap()), "multi line");
}

#[test]
fn plain_multiline() {
    let v = p("a: one\n  two\n\n  three\nb: [x\n  y, z]\n");
    assert_eq!(s(v.get("a").unwrap()), "one two\nthree");
    let b = v.get("b").unwrap().as_seq().unwrap();
    assert_eq!(s(&b[0]), "x y");
    assert_eq!(s(&b[1]), "z");
}

#[test]
fn block_scalars() {
    let v = p(
        "lit: |\n  line 1\n    indented\n  line 3\n\nfold: >\n  a\n  b\n\n  c\n    more\n  d\n\
               strip: |-\n  x\n\n\nkeep: |+\n  y\n\n\nexplicit: |2\n    four\n  two\nlast: 1\n",
    );
    assert_eq!(s(v.get("lit").unwrap()), "line 1\n  indented\nline 3\n");
    assert_eq!(s(v.get("fold").unwrap()), "a b\nc\n  more\nd\n");
    assert_eq!(s(v.get("strip").unwrap()), "x");
    assert_eq!(s(v.get("keep").unwrap()), "y\n\n\n");
    assert_eq!(s(v.get("explicit").unwrap()), "  four\ntwo\n");
    assert_eq!(v.get("last").unwrap().as_i64(), Some(1));
}

#[test]
fn anchors_aliases_and_merge() {
    let v = p(
        "base: &b {x: 1, y: 2}\nref: *b\nmerged:\n  <<: *b\n  y: 3\nlist: &l [1, 2]\n\
               again: *l\nscalar: &s hello\nsame: *s\n",
    );
    assert_eq!(v.get("ref"), v.get("base"));
    assert_eq!(v.path(&["merged", "x"]).unwrap().as_i64(), Some(1));
    assert_eq!(v.path(&["merged", "y"]).unwrap().as_i64(), Some(3));
    assert_eq!(v.get("again").unwrap().as_seq().unwrap().len(), 2);
    assert_eq!(s(v.get("same").unwrap()), "hello");
}

#[test]
fn json_style_flow() {
    let v = p("{\"a\":1, \"b\": [true, false, null], \"c\" : {\"d\": \"e\"}}");
    assert_eq!(v.get("a").unwrap().as_i64(), Some(1));
    assert_eq!(
        v.get("b").unwrap().as_seq().unwrap()[0].node,
        Node::Bool(true)
    );
    assert_eq!(s(v.path(&["c", "d"]).unwrap()), "e");
}

#[test]
fn document_end_and_trailing_blocks() {
    let v = p("--- {a: 1}\n...\n\u{1}garbage that is never read");
    assert_eq!(v.get("a").unwrap().as_i64(), Some(1));
    let v = p("--- \na: 1\n---\nb: 2\n");
    assert!(v.get("b").is_none());
}

#[test]
fn tag_names() {
    assert_eq!(tag_name("tag:stsci.edu:asdf/core/ndarray-1.1.0"), "ndarray");
    assert_eq!(
        tag_name("asdf://stsci.edu/datamodels/roman/tags/wfi_image-2.1.0"),
        "wfi_image"
    );
    assert_eq!(
        tag_name("tag:stsci.edu:asdf/unit/quantity-1.1.0"),
        "quantity"
    );
    assert_eq!(tag_name("!custom"), "!custom");
}

#[test]
fn refuses_deep_nesting() {
    let deep = "[".repeat(MAX_DEPTH + 10) + &"]".repeat(MAX_DEPTH + 10);
    assert_eq!(parse(&deep), Err(YamlError::Limit("nesting too deep")));
    let mut block = String::new();
    for d in 0..MAX_DEPTH + 10 {
        block.push_str(&" ".repeat(d));
        block.push_str("k:\n");
    }
    assert_eq!(parse(&block), Err(YamlError::Limit("nesting too deep")));
    // Just under the cap is fine.
    let ok = "[".repeat(MAX_DEPTH - 1) + &"]".repeat(MAX_DEPTH - 1);
    assert!(parse(&ok).is_ok());
}

#[test]
fn refuses_alias_bombs() {
    let mut y = String::from("a0: &a0 [x, x, x, x, x, x, x, x, x]\n");
    for i in 1..12 {
        let prev = format!("*a{}", i - 1);
        let items = vec![prev; 9].join(", ");
        y.push_str(&format!("a{i}: &a{i} [{items}]\n"));
    }
    assert_eq!(parse(&y), Err(YamlError::Limit("too many nodes")));
    // Long strings copied by aliases count against the text cap.
    let big = "x".repeat(4 << 20);
    let mut y = format!("s: &s {big}\n");
    for i in 0..20 {
        y.push_str(&format!("c{i}: *s\n"));
    }
    assert_eq!(parse(&y), Err(YamlError::Limit("too much text")));
}

#[test]
fn refuses_huge_scalars() {
    let big = "y".repeat(MAX_SCALAR + 10);
    assert_eq!(
        parse(&format!("a: {big}\n")),
        Err(YamlError::Limit("scalar too long"))
    );
    assert_eq!(
        parse(&format!("a: '{big}'\n")),
        Err(YamlError::Limit("scalar too long"))
    );
    assert_eq!(
        parse(&format!("a: |\n  {big}\n")),
        Err(YamlError::Limit("scalar too long"))
    );
}

#[test]
fn refuses_unsupported_and_bad_input() {
    assert!(matches!(
        parse("? complex\n: key\n"),
        Err(YamlError::Unsupported {
            what: "complex mapping keys",
            ..
        })
    ));
    assert!(matches!(
        parse("[a, b]: c\n"),
        Err(YamlError::Unsupported {
            what: "a collection as a mapping key",
            ..
        })
    ));
    assert!(matches!(
        parse("a: *nowhere\n"),
        Err(YamlError::Syntax { .. })
    ));
    assert!(matches!(parse("a: [1, 2\n"), Err(YamlError::Syntax { .. })));
    assert!(matches!(parse("a: 'open\n"), Err(YamlError::Syntax { .. })));
    assert!(matches!(
        parse("a: !x!y 1\n"),
        Err(YamlError::Syntax { line: 1, .. })
    ));
    assert!(matches!(
        parse("a: 1\n   b: 2\n"),
        Err(YamlError::Syntax { line: 2, .. })
    ));
}

mod hostile {
    use super::*;

    /// Chained anchors, each nesting the previous one ~150 levels deeper: before the alias
    /// height check this built a tree thousands of levels deep (stack overflow on clone/drop).
    #[test]
    fn chained_aliases_cannot_exceed_max_depth() {
        let d = 150;
        let mut s = format!("- &a0 {}0{}\n", "[".repeat(d), "]".repeat(d));
        for k in 1..40 {
            s.push_str(&format!(
                "- &a{k} {}*a{}{}\n",
                "[".repeat(d),
                k - 1,
                "]".repeat(d)
            ));
        }
        assert_eq!(parse(&s), Err(YamlError::Limit("nesting too deep")));
        // Shallow reuse still works.
        let v = parse("a: &x [[1, 2]]\nb: *x\n").unwrap();
        assert_eq!(v.get("b"), v.get("a"));
    }
}

/// 150 nested anchors around one 20 000-scalar list: each anchor kept its own uncharged copy
/// of everything inside it (3 million nodes here; 200 × the caps at worst).
#[test]
fn nested_anchor_copies_are_charged() {
    let leaf = format!("[{}1]", "1,".repeat(20_000));
    let mut y = String::new();
    for i in 0..150 {
        y.push_str(&format!("&a{i} ["));
    }
    y.push_str(&leaf);
    y.push_str(&"]".repeat(150));
    assert_eq!(parse(&y), Err(YamlError::Limit("too many nodes")));
}
