//! Screenshot tests of the text view, from hand-made files: a log with long lines unwrapped
//! and wrapped (dark, 1.5×), DOS art in code page 437, a CSV table with its header row, a
//! subtitle file's cues with a picture cue, a Markdown file shown formatted (only its switch)
//! and a Hebrew table in a right-to-left interface. Rendered offscreen on lavapipe and
//! compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular text_view::shots`, then review and
//! commit the rewritten PNGs.

use specular_decode::specular_subs::{Bitmap, Body, Cue, Span, TextCue, Track};
use specular_decode::specular_textdoc::{TableFile, TextFile};
use specular_shots::{Goldens, Harness};
use specular_theme::{Mode, Palette};

use super::*;

const SIZE: (f32, f32) = (560.0, 360.0);

const LOG: &str = "2026-11-07 10:00:01 INFO  server: listening on 0.0.0.0:8080\n\
2026-11-07 10:00:02 INFO  cache: warmed 1200 entries in 35 ms\n\
2026-11-07 10:00:05 WARN  db: slow query (812 ms): SELECT id, name, created FROM items WHERE owner = $1 ORDER BY created DESC LIMIT 50\n\
\tat db::query (src/db.rs:120)\n\
2026-11-07 10:00:09 ERROR http: request failed: connection reset by peer\n\
2026-11-07 10:00:10 INFO  http: GET /status 200 1.2 ms\n";

const CSV: &str = "city,country,population,area km²\n\
Lisbon,Portugal,545923,100.05\n\
\"Washington, D.C.\",United States,689545,177\n\
Reykjavík,Iceland,139875,273\n\
Ulaanbaatar,Mongolia,1645000,4704.4\n";

const CSV_HE: &str = "עיר,מדינה,אוכלוסייה\nירושלים,ישראל,981711\nחיפה,ישראל,285316\n";

fn text_model(s: &str, dos: bool) -> TextModel {
    let file = TextFile::open(Arc::new(s.as_bytes().to_vec()), dos, &|| false).unwrap();
    TextModel::Plain {
        file: Arc::new(file),
        dos,
    }
}

fn nfo() -> TextModel {
    // Code page 437 bytes: a box, shading and a name.
    let mut b = Vec::new();
    b.extend_from_slice(b"\xC9");
    b.extend(std::iter::repeat_n(0xCD, 30));
    b.extend_from_slice(
        b"\xBB\n\xBA  \xDB\xDB\xB2\xB2\xB1\xB1\xB0\xB0  RELEASE NOTES    \xBA\n\xC8",
    );
    b.extend(std::iter::repeat_n(0xCD, 30));
    b.extend_from_slice(b"\xBC\n");
    let file = TextFile::open(Arc::new(b), true, &|| false).unwrap();
    TextModel::Plain {
        file: Arc::new(file),
        dos: true,
    }
}

fn table(s: &str) -> TextModel {
    let t = TableFile::open(Arc::new(s.as_bytes().to_vec()), None, &|| false).unwrap();
    TextModel::Table(Arc::new(t))
}

fn cues() -> TextModel {
    let text = |a: i64, b: i64, l: &[&str]| Cue {
        start_us: a,
        end_us: b,
        body: Body::Text(TextCue {
            lines: l.iter().map(|s| vec![Span::plain(*s)]).collect(),
            align: 2,
            pos: None,
            margin_v: None,
        }),
    };
    // A picture cue: white letters' stand-in on transparency.
    let (w, h) = (120u32, 24u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 6..18 {
        for x in 8..112 {
            if (x / 6) % 2 == 0 {
                let i = ((y * w + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&[240, 240, 240, 255]);
            }
        }
    }
    let pic = Cue {
        start_us: 9_000_000,
        end_us: 11_000_000,
        body: Body::Bitmap(Bitmap {
            x: 0,
            y: 0,
            w,
            h,
            canvas: (720, 480),
            rgba: rgba.into(),
        }),
    };
    let track = Track::new(vec![
        text(1_000_000, 2_500_000, &["Where are we going?"]),
        text(
            3_000_000,
            5_250_000,
            &["North, past the harbour,", "before the tide turns."],
        ),
        text(6_000_000, 8_000_000, &["[waves]"]),
        pic,
    ]);
    TextModel::Subtitles(Arc::new(SubtitleModel {
        track,
        format: "SubRip",
        charset: Some("UTF-8"),
    }))
}

fn markdown() -> TextModel {
    let file = TextFile::open(
        Arc::new(b"# Notes\n\nSome *text*.\n".to_vec()),
        false,
        &|| false,
    )
    .unwrap();
    TextModel::Markdown(Arc::new(file))
}

struct Case {
    name: &'static str,
    model: TextModel,
    file: &'static str,
    scale: f32,
    dark: bool,
    rtl: bool,
    wrap: Option<bool>,
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, c: &Case) {
    let pal = if c.dark {
        Palette::for_mode(Mode::Dark)
    } else {
        Palette::for_mode(Mode::Light)
    };
    let mut v = TextView::default();
    v.show(1, &c.model, c.file);
    if let Some(w) = c.wrap {
        v.wrap = w;
    }
    // Picture cues are uploaded before the frame that shows them.
    if matches!(c.model, TextModel::Subtitles(_)) {
        v.want = vec![3];
        v.prepare(&h.gpu, &mut h.ui);
    }
    let rtl = c.rtl;
    let shot = h.render(
        SIZE,
        c.scale,
        |_| {},
        |dl, text| {
            draw(
                dl,
                text,
                &pal,
                pal.canvas,
                &mut v,
                Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
                rtl,
                1.0,
            );
        },
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    super::super::a11y_audit::audit_screen(c.name, SIZE, rtl, |t| a11y(t, &v, 1.0));
}

#[test]
fn text_views_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-text",
    );
    let cases = [
        Case {
            name: "text-log",
            model: text_model(LOG, false),
            file: "server.log",
            scale: 1.0,
            dark: false,
            rtl: false,
            wrap: Some(false),
        },
        Case {
            name: "text-log-wrapped-dark",
            model: text_model(LOG, false),
            file: "server.log",
            scale: 1.5,
            dark: true,
            rtl: false,
            wrap: Some(true),
        },
        Case {
            name: "text-nfo",
            model: nfo(),
            file: "release.nfo",
            scale: 1.0,
            dark: true,
            rtl: false,
            wrap: None,
        },
        Case {
            name: "text-csv",
            model: table(CSV),
            file: "cities.csv",
            scale: 1.0,
            dark: false,
            rtl: false,
            wrap: None,
        },
        Case {
            name: "text-subtitles",
            model: cues(),
            file: "film.srt",
            scale: 1.0,
            dark: false,
            rtl: false,
            wrap: None,
        },
        Case {
            name: "text-markdown-formatted",
            model: markdown(),
            file: "notes.md",
            scale: 1.0,
            dark: false,
            rtl: false,
            wrap: None,
        },
    ];
    for c in &cases {
        shoot(&mut h, &mut goldens, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let c = Case {
            name: "text-csv-he",
            model: table(CSV_HE),
            file: "ערים.csv",
            scale: 1.0,
            dark: false,
            rtl: true,
            wrap: None,
        };
        shoot(&mut h, &mut goldens, &c);
    });
    goldens.finish();
}
