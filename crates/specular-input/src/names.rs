//! An action's name: the one way the palette, the cheat sheet, Settings → Keys, the manual,
//! toasts and assistive technologies name an action. Its message id is private to this
//! crate, so nothing names an action by translating the id itself (`gallery-stars` alone,
//! without the stars it takes, once showed as "{$stars} stars").

use specular_i18n::{FluentArgs, I18n, Localizer};

use crate::{Action, ActionState};

/// What can translate (the global handle, or one localizer).
trait Tr {
    fn tr(&self, id: &str) -> String;
    fn tr_args(&self, id: &str, args: &FluentArgs<'_>) -> String;
}

impl Tr for I18n {
    fn tr(&self, id: &str) -> String {
        I18n::tr(self, id)
    }
    fn tr_args(&self, id: &str, args: &FluentArgs<'_>) -> String {
        I18n::tr_args(self, id, args)
    }
}

impl Tr for Localizer {
    fn tr(&self, id: &str) -> String {
        Localizer::tr(self, id)
    }
    fn tr_args(&self, id: &str, args: &FluentArgs<'_>) -> String {
        Localizer::tr_args(self, id, args)
    }
}

/// A colour label's name (1–4: red, yellow, green, blue; else none).
fn label<T: Tr + ?Sized>(t: &T, n: u8) -> String {
    t.tr(match n {
        1 => "gallery-label-red",
        2 => "gallery-label-yellow",
        3 => "gallery-label-green",
        4 => "gallery-label-blue",
        _ => "gallery-label-none",
    })
}

fn one<T: Tr + ?Sized>(t: &T, id: &str, name: &'static str, value: i64) -> String {
    let mut a = FluentArgs::new();
    a.set(name, value);
    t.tr_args(id, &a)
}

impl Action {
    /// The action's name in the interface's language.
    pub fn name(self) -> String {
        self.name_by(I18n::global())
    }

    /// The action's name for what is shown now: a tractogram's own names for the point size
    /// and colouring keys ([`Action::tract_message_id`]).
    pub fn name_in(self, st: &ActionState) -> String {
        match self.tract_message_id() {
            Some(id) if st.tracts && !st.points => I18n::global().tr(id),
            _ => self.name(),
        }
    }

    /// The action's name for a tractogram where it has one of its own, else its name.
    pub fn tract_name(self) -> String {
        match self.tract_message_id() {
            Some(id) => I18n::global().tr(id),
            None => self.name(),
        }
    }

    /// The action's name in `loc`'s language.
    pub fn name_with(self, loc: &Localizer) -> String {
        self.name_by(loc)
    }

    fn name_by<T: Tr + ?Sized>(self, t: &T) -> String {
        match self {
            Action::CullPick => t.tr("keys-action-cull-pick"),
            Action::CullReject => t.tr("keys-action-cull-reject"),
            Action::CullUnflag => t.tr("keys-action-cull-unflag"),
            Action::CullRate(n) => one(t, "keys-action-cull-rate", "stars", i64::from(n)),
            Action::CullLabel(n) => {
                let mut a = FluentArgs::new();
                a.set("label", label(t, n));
                t.tr_args("keys-action-cull-label", &a)
            }
            Action::CullCopyTo(n) => one(t, "keys-action-cull-copy-to", "slot", i64::from(n)),
            Action::SeekPercent(n) => one(t, "palette-seek-percent", "percent", i64::from(n)),
            Action::CullMoveTo(n) => one(t, "keys-action-cull-move-to", "slot", i64::from(n)),
            a => t.tr(a.message_id()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bindable_action_has_a_whole_name() {
        let en = Localizer::english();
        for a in Action::BINDABLE.iter().chain(Action::LISTED) {
            let n = a.name_with(&en);
            assert!(!n.is_empty() && !n.contains('{'), "{a:?}: {n:?}");
            assert_ne!(n, a.message_id(), "{a:?} has no English name");
        }
        for n in 0..=5 {
            let s = Action::CullRate(n).name_with(&en);
            assert!(!s.contains('{') && !s.contains("$"), "{s}");
        }
        assert!(Action::CullLabel(1).name_with(&en).contains("Red"));
        assert!(Action::CullCopyTo(2).name_with(&en).contains('2'));
    }

    /// Names with a number or a label pass what their English message uses
    /// (`message_args.rs` cannot read `one`'s ids and names from the source).
    #[test]
    fn action_names_get_their_arguments() {
        let en = Localizer::english();
        specular_i18n::record_argument_problems(true);
        for a in Action::BINDABLE.iter().chain(Action::LISTED) {
            let _ = a.name_with(&en);
        }
        for n in 0..=9 {
            for a in [
                Action::CullRate(n),
                Action::CullLabel(n),
                Action::CullCopyTo(n),
                Action::CullMoveTo(n),
            ] {
                let _ = a.name_with(&en);
            }
        }
        let problems = specular_i18n::argument_problems();
        specular_i18n::record_argument_problems(false);
        assert!(problems.is_empty(), "{problems:#?}");
    }
}
