//! The Optics card's Lensfun rows: for a raw without
//! lens corrections of its own, which Lensfun profile applies, and a picker to choose
//! another lens or none.
//!
//! - **Lens profile** (a choice): Automatic, the lenses the lookup offers (best first), None.
//!   It writes the lens module's `lensfun` field: removed for Automatic, `none`, or the lens's
//!   key ([`specular_develop::ops::geometry::lensfun`]).
//! - **The note** under it says what applies: "Lensfun: Canon EF 24-70mm f/2.8L II USM", or
//!   that no profile fits, that the file does not name the lens, or that several fit and one
//!   must be picked. Automatic never chooses between two equally good matches.
//!
//! Both rows show only for a raw the lookup ran on (they follow the Lens corrections row).

use std::sync::Arc;

use specular_develop::model::{Fields, Value};
use specular_develop::ops::geometry::lensfun::{NONE, Shot};
use specular_i18n::t;
use specular_lensfun::lookup::Pick;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Control};

/// The rows' control ids.
pub(super) mod id {
    pub const PROFILE: u32 = 85;
    pub const NOTE: u32 = 86;
}

/// The picker's options: Automatic, the candidates, None.
pub(super) fn options(shot: &Shot) -> Vec<String> {
    let mut v = vec![t!("develop-lensfun-auto")];
    v.extend(shot.candidates.iter().map(|c| c.name.clone()));
    v.push(t!("develop-lensfun-off"));
    v
}

/// The option of the `lensfun` field's value.
pub(super) fn index_of(shot: &Shot, setting: &str) -> usize {
    match setting {
        "" => 0,
        NONE => shot.candidates.len() + 1,
        key => shot
            .candidates
            .iter()
            .position(|c| c.key == key)
            .map_or(0, |i| i + 1),
    }
}

/// Writes option `i` into the lens module's fields.
pub(super) fn write(shot: &Shot, i: usize, f: &mut Fields) {
    if i == 0 {
        f.remove("lensfun");
    } else if let Some(c) = shot.candidates.get(i - 1) {
        f.set("lensfun", Value::Text(c.key.clone()));
    } else {
        f.set("lensfun", Value::Text(NONE.into()));
    }
}

/// What applies, in a line.
pub(super) fn note(shot: &Shot, setting: &str) -> String {
    if let Some(c) = shot.chosen(setting) {
        return t!("develop-lensfun-used", lens = c.name.clone());
    }
    if setting == NONE {
        return t!("develop-lensfun-none-chosen");
    }
    let lens = shot.lens_name.clone().unwrap_or_default();
    match shot.pick {
        Pick::Ambiguous => t!("develop-lensfun-ambiguous", count = shot.tied as i64),
        Pick::NoLensName if shot.candidates.is_empty() => t!("develop-lensfun-no-profile"),
        Pick::NoLensName => t!("develop-lensfun-no-name"),
        _ if lens.is_empty() => t!("develop-lensfun-no-profile"),
        _ => t!("develop-lensfun-no-match", lens = lens),
    }
}

/// The two rows for `shot`, set to `setting`.
pub(super) fn rows(shot: &Shot, setting: &str) -> Vec<Control> {
    let mut c = Choice::new(t!("develop-lensfun-profile"), options(shot));
    c.set(index_of(shot, setting));
    vec![
        Control {
            id: id::PROFILE,
            body: Body::Choice(c),
        },
        Control {
            id: id::NOTE,
            body: Body::Note(note(shot, setting)),
        },
    ]
}

/// The rows for the card: none without a lookup.
pub(super) fn rows_for(shot: Option<&Arc<Shot>>, setting: &str) -> Vec<Control> {
    shot.map_or_else(Vec::new, |s| rows(s, setting))
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::ops::geometry::lensfun::Candidate;
    use specular_lensfun::model::Profile;

    fn shot(pick: Pick, n: usize) -> Shot {
        let p = Arc::new(Profile::new(None, None, None, 1.0, 1.5, 1.0));
        Shot {
            lens_name: Some("EF50mm f/1.8".into()),
            candidates: (0..n)
                .map(|i| Candidate {
                    key: format!("k{i}"),
                    name: format!("Lens {i}"),
                    profile: p.clone(),
                })
                .collect(),
            pick,
            tied: if pick == Pick::Ambiguous { 2 } else { 1 },
        }
    }

    #[test]
    fn picker_round_trip() {
        let s = shot(Pick::One, 3);
        assert_eq!(options(&s).len(), 5);
        for (setting, i) in [("", 0), ("k0", 1), ("k2", 3), (NONE, 4), ("gone", 0)] {
            assert_eq!(index_of(&s, setting), i, "{setting}");
        }
        let mut f = Fields::new();
        write(&s, 2, &mut f);
        assert_eq!(f.text("lensfun"), Some("k1"));
        write(&s, 4, &mut f);
        assert_eq!(f.text("lensfun"), Some(NONE));
        write(&s, 0, &mut f);
        assert_eq!(f.get("lensfun"), None);
    }

    #[test]
    fn notes_say_what_applies() {
        let one = shot(Pick::One, 2);
        assert!(note(&one, "").contains("Lens 0"));
        assert!(note(&one, "k1").contains("Lens 1"));
        assert!(!note(&one, NONE).contains("Lens"));
        // Ambiguous: nothing applies until a lens is picked; the note says to pick.
        let amb = shot(Pick::Ambiguous, 2);
        assert!(amb.chosen("").is_none());
        assert!(note(&amb, "").contains('2'));
        assert!(note(&amb, "k1").contains("Lens 1"));
        let none = shot(Pick::NoMatch, 0);
        assert!(note(&none, "").contains("EF50mm"));
        assert_eq!(rows(&none, "").len(), 2);
        assert!(rows_for(None, "").is_empty());
    }
}
