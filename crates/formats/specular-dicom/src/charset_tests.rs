use super::*;

fn decode(charset: &str, bytes: &[u8], vr: Vr) -> String {
    Charset::parse(charset.as_bytes()).decode(bytes, vr)
}

// PS3.5 Annex H, example 1: Japanese, JIS X 0208 by code extension.
#[test]
fn annex_h_jis_x_0208() {
    let pn =
        b"Yamada^Tarou=\x1b$B;3ED\x1b(B^\x1b$BB@O:\x1b(B=\x1b$B$d$^$@\x1b(B^\x1b$B$?$m$&\x1b(B";
    assert_eq!(
        decode("\\ISO 2022 IR 87", pn, Vr::PN),
        "Yamada^Tarou=山田^太郎=やまだ^たろう"
    );
}

// PS3.5 Annex H, example 2: JIS X 0201 katakana in G1 as the initial set, then JIS X 0208.
#[test]
fn annex_h_jis_x_0201_and_0208() {
    let pn = b"\xd4\xcf\xc0\xde^\xc0\xdb\xb3=\x1b$B;3ED\x1b(J^\x1b$BB@O:\x1b(J=\x1b$B$d$^$@\x1b(J^\x1b$B$?$m$&\x1b(J";
    assert_eq!(
        decode("ISO 2022 IR 13\\ISO 2022 IR 87", pn, Vr::PN),
        "ﾔﾏﾀﾞ^ﾀﾛｳ=山田^太郎=やまだ^たろう"
    );
}

#[test]
fn jis_x_0212() {
    assert_eq!(
        decode("\\ISO 2022 IR 159", b"\x1b$(D0!\x1b(B", Vr::LO),
        "丂"
    );
}

// PS3.5 Annex I: Korean, KS X 1001 in G1, designated again after each delimiter.
#[test]
fn annex_i_korean() {
    let pn = b"Hong^Gildong=\x1b$)C\xfb\xf3^\x1b$)C\xd1\xce\xd4\xd7=\x1b$)C\xc8\xab^\x1b$)C\xb1\xe6\xb5\xbf";
    assert_eq!(
        decode("\\ISO 2022 IR 149", pn, Vr::PN),
        "Hong^Gildong=洪^吉洞=홍^길동"
    );
}

// PS3.5 Annex J: Chinese, UTF-8 and GB18030.
#[test]
fn annex_j_chinese() {
    let utf8 = "Wang^XiaoDong=王^小東=";
    assert_eq!(decode("ISO_IR 192", utf8.as_bytes(), Vr::PN), utf8);
    assert_eq!(
        decode(
            "GB18030",
            b"Wang^XiaoDong=\xcd\xf5^\xd0\xa1\xb6\xab=",
            Vr::PN
        ),
        "Wang^XiaoDong=王^小东="
    );
    assert_eq!(
        decode("GBK", b"Wang^XiaoDong=\xcd\xf5^\xd0\xa1\xb6\xab=", Vr::PN),
        "Wang^XiaoDong=王^小东="
    );
}

// PS3.5 Annex K: Chinese, GB 2312 by code extension.
#[test]
fn annex_k_gb2312() {
    let pn = b"Zhang^XiaoDong=\x1b$)A\xd5\xc5^\x1b$)A\xd0\xa1\xb6\xab=";
    assert_eq!(
        decode("\\ISO 2022 IR 58", pn, Vr::PN),
        "Zhang^XiaoDong=张^小东="
    );
}

#[test]
fn gb18030_four_byte() {
    assert_eq!(
        decode(
            "GB18030",
            b"\x81\x30\x81\x30\x95\x32\x82\x36\x84\x31\xa4\x39\x81\x35\xf4\x37",
            Vr::LO
        ),
        // The WHATWG index (GB18030-2022): 0x81 0x35 0xF4 0x37 is U+E7C7 (older tables: U+1E3F).
        "\u{80}\u{20000}\u{FFFF}\u{E7C7}"
    );
    // Truncated sequences don't swallow what follows.
    assert_eq!(decode("GB18030", b"\x81\x30A", Vr::LO), "\u{FFFD}0A");
    assert_eq!(decode("GB18030", b"\xcd", Vr::LO), "\u{FFFD}");
}

#[test]
fn single_byte_sets() {
    let cases: [(&str, &[u8], &str); 9] = [
        (
            "ISO_IR 126",
            b"\xc4\xe9\xef\xed\xf5\xf3\xe9\xef\xf2",
            "Διονυσιος",
        ),
        ("ISO_IR 144", b"\xb1\xe3\xda\xd2\xeb", "Буквы"),
        ("ISO_IR 138", b"\xf9\xf8\xe5\xef", "שרון"),
        ("ISO_IR 166", b"\xc0\xd2\xc9\xd2\xe4\xb7\xc2", "ภาษาไทย"),
        ("ISO_IR 127", b"\xe2\xc8\xc7\xe6\xea", "قباني"),
        ("ISO_IR 148", b"\xc7avu\xfe", "Çavuş"),
        ("ISO_IR 101", b"\xa3\xf3d\xbc", "Łódź"),
        ("ISO_IR 203", b"\xa4", "€"),
        ("ISO_IR 100", b"Buc^J\xe9r\xf4me", "Buc^Jérôme"),
    ];
    for (cs, bytes, want) in cases {
        assert_eq!(decode(cs, bytes, Vr::PN), want, "{cs}");
    }
}

#[test]
fn spellings_and_defaults() {
    // Real-world spellings of the defined terms.
    for cs in ["ISO_IR100", "ISO-IR 100", "iso_ir 100", "ISO 2022 IR 100"] {
        assert_eq!(decode(cs, b"J\xe9r\xf4me", Vr::PN), "Jérôme", "{cs}");
    }
    assert_eq!(decode("UTF-8", "Jérôme".as_bytes(), Vr::PN), "Jérôme");
    assert_eq!(decode("GB2312", b"\xd5\xc5", Vr::PN), "张");
    // None or unknown: ASCII, with ISO_IR 100 for high bytes.
    let none = Charset::default();
    assert_eq!(none.decode(b"J\xe9r\xf4me", Vr::PN), "Jérôme");
    assert_eq!(decode("ISO_IR 999", b"\xe9", Vr::LO), "é");
    // High bytes with only a multi-byte G1 set named, but not designated.
    assert_eq!(decode("\\ISO 2022 IR 149", b"\xc8\xab", Vr::PN), "홍");
    // Invalid UTF-8 doesn't fail.
    assert_eq!(decode("ISO_IR 192", b"a\xffb", Vr::LO), "a\u{FFFD}b");
}

#[test]
fn delimiters_return_to_the_initial_state() {
    let cs = "ISO 2022 IR 100\\ISO 2022 IR 144";
    // '\' separates values of LO: the second value starts in ISO_IR 100 again.
    assert_eq!(decode(cs, b"\x1b-L\xb1\\\xb1", Vr::LO), "Б\\±");
    // LT has a single value: '\' is a character and the Cyrillic set stays.
    assert_eq!(decode(cs, b"\x1b-L\xb1\\\xb1", Vr::LT), "Б\\Б");
    // Control characters end lines in text VRs.
    assert_eq!(decode(cs, b"\x1b-L\xb1\r\n\xb1", Vr::LT), "Б\r\n±");
    // '^' and '=' only reset in PN.
    assert_eq!(decode(cs, b"\x1b-L\xb1^\xb1", Vr::PN), "Б^±");
    assert_eq!(decode(cs, b"\x1b-L\xb1^\xb1", Vr::LO), "Б^Б");
    // In JIS X 0208, 0x5C is half of a character, not a delimiter.
    assert_eq!(
        decode("\\ISO 2022 IR 87", b"\x1b$B\x30\x5c\x1b(B\\a", Vr::LO),
        "\u{79FB}\\a"
    );
}

#[test]
fn malformed_escapes_and_truncation() {
    let jis = "\\ISO 2022 IR 87";
    // Unknown escape sequence.
    assert_eq!(decode(jis, b"a\x1b%Gb", Vr::LO), "a\u{FFFD}%Gb");
    // Escape at the end, and half a JIS character.
    assert_eq!(decode(jis, b"a\x1b", Vr::LO), "a\u{FFFD}");
    assert_eq!(decode(jis, b"\x1b$B;", Vr::LO), "\u{FFFD}");
    assert_eq!(
        decode("\\ISO 2022 IR 149", b"\x1b$)C\xc8", Vr::LO),
        "\u{FFFD}"
    );
}

#[test]
fn summary_and_tag_browser_use_the_character_set() {
    use crate::tests::{W, file};
    use crate::{Dicom, TagTree, TagValue};
    let mut item = W::new(true, false);
    item.s(0x0008_0005, b"CS", "ISO_IR 144")
        .raw(0x0008_0104, b"LO", b"\xb1\xe3\xda\xd2\xeb ");
    let mut w = W::new(true, false);
    w.raw(0x0008_0005, b"CS", b"\\ISO 2022 IR 87 ")
        .s(0x0008_0060, b"CS", "CT")
        .raw(0x0008_1030, b"LO", b"\x1b$B;3ED\x1b(B")
        .seq(0x0008_1032, &[item])
        .image(2, 3, 1, "MONOCHROME2", (16, 12, 11), false);
    let f = file("1.2.840.10008.1.2.1", &w.out);
    let d = Dicom::parse(&f).unwrap();
    assert_eq!(d.summary(&f).study_description.as_deref(), Some("山田"));
    let t = TagTree::build(&d, &f);
    let text = |tag| {
        t.rows
            .iter()
            .find(|r| r.tag == tag)
            .map(|r| match &r.value {
                TagValue::Text { text, .. } => text.clone(),
                v => format!("{v:?}"),
            })
    };
    assert_eq!(text(0x0008_1030).as_deref(), Some("山田"));
    // The item's own Specific Character Set applies inside it.
    assert_eq!(text(0x0008_0104).as_deref(), Some("Буквы"));
}
