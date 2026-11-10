//! Writes Office 97–2003 fixtures (compound files with summary information) into a folder,
//! for `tools/corpus/gen-office.py`, which reads them with olefile as the oracle.
//!
//! Usage: cargo run -p specular-office --example write_fixtures -- OUT_DIR

use specular_office::fixture::{self, FMTID_DOC_SUMMARY, FMTID_SUMMARY, Prop};

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("OUT_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    let ft = |unix: u64| (unix + 11_644_473_600) * 10_000_000;
    let dib = fixture::dib_2x2();
    // A METAFILEPICT header (mapping mode, extents in 0.01 mm, handle), then the metafile.
    let mut wmf = vec![8, 0, 0x10, 0x27, 0x10, 0x27, 0, 0];
    wmf.extend_from_slice(&fixture::wmf_square());
    let emf = {
        let mut e = vec![0u8; 88];
        e[0] = 1;
        e[4] = 88;
        e[40..44].copy_from_slice(b" EMF");
        e
    };
    let body = vec![0u8; 6000];
    type Case<'a> = (&'a str, &'a str, Vec<(u32, Prop<'a>)>, Vec<(u32, Prop<'a>)>);
    let cases: Vec<Case> = vec![
        (
            "word97-dib.doc",
            "WordDocument",
            vec![
                (2, Prop::Str("Caf\u{e9} notes")),
                (3, Prop::Str("Menus")),
                (4, Prop::Str("Ann Smith")),
                (5, Prop::Str("food, plans")),
                (6, Prop::Str("A comment")),
                (8, Prop::Str("Bob")),
                (9, Prop::Str("4")),
                (12, Prop::Time(ft(1_000_000_000))),
                (13, Prop::Time(ft(1_100_000_000))),
                (14, Prop::I4(5)),
                (15, Prop::I4(1234)),
                (16, Prop::I4(7000)),
                (17, Prop::Clip(-1, 8, &dib)),
                (18, Prop::Str("Microsoft Word 8.0")),
            ],
            vec![(2, Prop::Str("Food")), (15, Prop::Str("ACME"))],
        ),
        (
            "ppt97-wmf.ppt",
            "PowerPoint Document",
            vec![
                (2, Prop::Str("Launch")),
                (4, Prop::Str("Carol")),
                (17, Prop::Clip(-1, 3, &wmf)),
            ],
            vec![(7, Prop::I4(12)), (8, Prop::I4(2)), (9, Prop::I4(1))],
        ),
        (
            "excel97-emf.xls",
            "Workbook",
            vec![(2, Prop::Str("Budget")), (17, Prop::Clip(-1, 14, &emf))],
            vec![],
        ),
        (
            "excel97-none.xls",
            "Workbook",
            vec![(2, Prop::Str("No preview")), (4, Prop::Str("Dan"))],
            vec![],
        ),
    ];
    for (name, main, s, d) in cases {
        let s = fixture::property_set(FMTID_SUMMARY, &s);
        let d = fixture::property_set(FMTID_DOC_SUMMARY, &d);
        let f = fixture::cfb(&[
            (main, &body[..]),
            ("\u{5}SummaryInformation", &s),
            ("\u{5}DocumentSummaryInformation", &d),
        ]);
        std::fs::write(out.join(name), f).unwrap();
    }
    let enc = fixture::cfb(&[
        ("EncryptionInfo", &[4, 0, 4, 0, 0x40, 0, 0, 0][..]),
        ("EncryptedPackage", &[0u8; 64][..]),
    ]);
    std::fs::write(out.join("protected.docx"), enc).unwrap();
}
