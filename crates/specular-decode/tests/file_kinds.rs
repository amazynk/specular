//! Files whose extension several formats share (`specular_decode::ident`): a signature
//! settles it whatever was remembered; content no check matches leaves the formats claiming
//! the extension, asked about; a choice or a rule settles it; the folder orders the
//! formats; a thumbnail never asks.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::Source;
use specular_decode::ident::{self, Confidence, FolderHint, Rule};
use specular_decode::{DecodeError, Format, Registry};

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("file-kinds-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn open(reg: &Registry, p: &Path) -> Result<Format, DecodeError> {
    reg.open(Arc::new(Source::open(p).unwrap()))
        .map(|s| s.info().format)
}

const PLS: &[u8] = b"[playlist]\nFile1=song.mp3\nTitle1=Song\nNumberOfEntries=1\nVersion=2\n";
/// Neither a PLS playlist nor Gerber commands.
const NEITHER: &[u8] = b"just some words\nand more words\n";

/// One test: the rules and choices are the process's own.
#[test]
fn shared_extensions_are_asked_about_unless_settled() {
    let reg = Registry::new();
    let d = scratch("ask");
    let odd = d.join("paste.pls");
    std::fs::write(&odd, NEITHER).unwrap();

    // Content no check matches: the two formats claiming .pls, asked about.
    let a = match open(&reg, &odd) {
        Err(DecodeError::Ambiguous(a)) => a,
        r => panic!("not asked about: {:?}", r.map_err(|e| e.to_string())),
    };
    assert_eq!(a.ext, "pls");
    let formats: Vec<Format> = a.candidates.iter().map(|c| c.format).collect();
    // The commoner first (ident::SHARED's order), both guesses by name.
    assert_eq!(formats, [Format::Playlist, Format::Gerber]);
    assert!(
        a.candidates
            .iter()
            .all(|c| c.confidence == Confidence::Guess)
    );
    assert_eq!(ident::ambiguity_of(&odd), Some((*a).clone()));
    // A thumbnail never asks: the likeliest that opens, else the question.
    match reg.open_for_thumbnail(Arc::new(Source::open(&odd).unwrap())) {
        Ok(_) | Err(DecodeError::Ambiguous(_)) => {}
        Err(e) => panic!("a thumbnail failed otherwise: {e}"),
    }

    // A rule for everywhere settles it; one for the folder wins over it.
    ident::set_rules(vec![
        Rule {
            ext: "pls".into(),
            folder: None,
            format: Format::Playlist,
        },
        Rule {
            ext: "pls".into(),
            folder: Some(d.clone()),
            format: Format::Gerber,
        },
    ]);
    assert!(!matches!(open(&reg, &odd), Err(DecodeError::Ambiguous(_))));
    assert_eq!(ident::rule_for(&odd, &a.candidates), Some(Format::Gerber));

    // A signature wins over any rule.
    let real = d.join("radio.pls");
    std::fs::write(&real, PLS).unwrap();
    assert_eq!(open(&reg, &real).ok(), Some(Format::Playlist));
    ident::set_rules(Vec::new());

    // A choice for the file settles it too, and answers the question.
    ident::choose(&odd, Format::Playlist);
    assert!(!matches!(open(&reg, &odd), Err(DecodeError::Ambiguous(_))));
    assert!(ident::ambiguity_of(&odd).is_none());
    ident::unchoose(&odd);

    // The folder: another .pls here read by its signature leans this one the same way.
    let a = match open(&reg, &odd) {
        Err(DecodeError::Ambiguous(a)) => a,
        r => panic!("not asked about: {:?}", r.map_err(|e| e.to_string())),
    };
    assert_eq!(a.candidates[0].format, Format::Playlist);
    assert_eq!(a.candidates[0].hint, Some(FolderHint::SameKind));

    // Board files beside it, nothing identified: Gerber first.
    let b = scratch("board");
    for n in ["board.gtl", "board.gbl", "board.drl"] {
        std::fs::write(b.join(n), b"x").unwrap();
    }
    let paste = b.join("paste.pls");
    std::fs::write(&paste, NEITHER).unwrap();
    let a = match open(&reg, &paste) {
        Err(DecodeError::Ambiguous(a)) => a,
        r => panic!("not asked about: {:?}", r.map_err(|e| e.to_string())),
    };
    assert_eq!(a.candidates[0].format, Format::Gerber);
    assert_eq!(a.candidates[0].hint, Some(FolderHint::Companions));

    // Open as…: every format the file may be.
    let all = reg.identify(&Source::open(&real).unwrap());
    assert_eq!(all[0].format, Format::Playlist);
    assert_eq!(all[0].confidence, Confidence::Certain);
    assert!(all.iter().any(|c| c.format == Format::Gerber));
    // Forced: whatever the content says.
    let forced = reg.open_as(Arc::new(Source::open(&real).unwrap()), Format::Playlist);
    assert!(forced.is_ok());

    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&b);
}

/// An extension one format has keeps opening on its name alone, never asked about.
#[test]
fn an_extension_of_one_format_is_never_asked_about() {
    let reg = Registry::new();
    let d = scratch("one");
    let p = d.join("x.tga");
    std::fs::write(&p, [0u8; 40]).unwrap();
    assert!(!matches!(open(&reg, &p), Err(DecodeError::Ambiguous(_))));
    let _ = std::fs::remove_dir_all(&d);
}
