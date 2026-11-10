//! Profiles: the capabilities in effect (Settings → Profile, plus what the toolbar chip or the
//! palette turned on for the session), the first-run question and the one-time "Always"
//! offer.

use std::collections::HashMap;

use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_i18n::t;
use specular_input::{Action, Capabilities, Capability, Profile};
use specular_log::{INPUT, debug};
use specular_settings::{Origin, ProfileCapability, Section, SettingEnum};

use super::App;

/// Undo keys of the "Always" offers (after the gallery's, studies' and video export's).
const OFFER_KEYS: u64 = 1 << 60;
/// What `profile.shown` records for the first-run question.
const QUESTION: &str = "question";

/// The session's side of the profile.
#[derive(Default)]
pub(super) struct ProfileState {
    /// Turned on for this session (the chip, the palette).
    session: Capabilities,
    /// Open "Always" offers by undo key.
    offers: HashMap<u64, Capability>,
    next_offer: u64,
    /// The profile the chrome and keymap were last synced to.
    synced: Option<Profile>,
}

impl App {
    /// The profile in effect without content. Benchmarks and screenshots see everything,
    /// so their output does not depend on a settings file.
    pub(super) fn profile(&self) -> Profile {
        if self.headless() {
            return self.welcome.shot_profile.unwrap_or(Profile::UNRESTRICTED);
        }
        let enabled = Capabilities::from_ids(
            self.settings
                .profile
                .capabilities
                .iter()
                .map(|c| c.as_str()),
        )
        .with(Capability::General);
        Profile {
            enabled,
            session: self.profile_state.session.with(Capability::General),
            content: Capabilities::GENERAL,
            mpr: false,
        }
    }

    /// Whether `profile.shown` records `what`.
    fn profile_shown(&self, what: &str) -> bool {
        self.settings
            .profile
            .shown
            .split(',')
            .any(|s| s.trim() == what)
    }

    /// Records `what` in `profile.shown`.
    fn profile_record(&mut self, what: &str) {
        if self.profile_shown(what) {
            return;
        }
        let what = what.to_owned();
        self.store.update(Origin::Program, |s| {
            if !s.profile.shown.is_empty() {
                s.profile.shown.push(',');
            }
            s.profile.shown.push_str(&what);
        });
        self.apply_settings(false);
    }

    /// Asks "What will you use Specular for?" once, on the first interactive start.
    pub(super) fn profile_first_run(&mut self) {
        if self.headless() || self.profile_shown(QUESTION) {
            return;
        }
        // Recorded when asked, so Esc keeps General and the question is not asked again.
        let current = self.profile().enabled;
        self.profile_record(QUESTION);
        self.chrome
            .set_profile_start(self.settings.general.on_start);
        self.chrome.ask_profile(current);
        self.redraw = true;
    }

    /// Follows a changed profile: the keymap (keys of what is off fall through) and the
    /// Settings pages of capabilities that are off.
    pub(super) fn profile_sync(&mut self) {
        let p = self.action_state().content_profile();
        if self.profile_state.synced == Some(p) {
            return;
        }
        self.profile_state.synced = Some(p);
        self.keymap.set_profile(p);
        // Settings → Medical images only while medical tools are on or a medical image is.
        let medical = p.is_on(Capability::Medical) || p.content.contains(Capability::Medical);
        for m in specular_settings::in_section(Section::Medical) {
            self.chrome.set_setting_hidden(m.key, !medical);
        }
        // Settings → Drawings with the engineering tools or a drawing open.
        let eng = p.is_on(Capability::Engineering) || p.content.contains(Capability::Engineering);
        for m in specular_settings::in_section(Section::Drawings) {
            self.chrome.set_setting_hidden(m.key, !eng);
        }
        // Settings → Maps likewise.
        let maps = p.is_on(Capability::Maps) || p.content.contains(Capability::Maps);
        for m in specular_settings::in_section(Section::Maps) {
            self.chrome.set_setting_hidden(m.key, !maps);
        }
        self.redraw = true;
    }

    /// The chip or the palette turned a capability on: for the session, or always. The
    /// first session enable offers "Always" once, in a toast.
    pub(super) fn capability_on(
        &mut self,
        c: Capability,
        always: bool,
        then: Option<Action>,
        el: &ActiveEventLoop,
    ) {
        debug!(target: INPUT, "capability {} on ({})", c.id(), if always { "always" } else { "session" });
        self.profile_state.session = self.profile_state.session.with(c);
        if always {
            self.profile_always(c);
        } else if !self.profile_shown(c.id()) && !self.headless() {
            self.profile_record(c.id());
            let key = OFFER_KEYS | self.profile_state.next_offer;
            self.profile_state.next_offer += 1;
            self.profile_state.offers.insert(key, c);
            let now = self.now();
            self.chrome.toast_action(
                t!("profile-tools-on", capability = c.id()),
                t!("profile-always"),
                key,
                now,
            );
        }
        self.profile_sync();
        self.redraw = true;
        if let Some(a) = then {
            self.action(a, el);
        }
    }

    /// Adds `c` to `profile.capabilities`.
    fn profile_always(&mut self, c: Capability) {
        let Some(pc) = ProfileCapability::from_name(c.id()) else {
            return;
        };
        self.store.update(Origin::Program, |s| {
            if !s.profile.capabilities.contains(&pc) {
                s.profile.capabilities.push(pc);
            }
        });
        self.apply_settings(false);
    }

    /// The toast's "Always": the capability stays on in later sessions.
    pub(super) fn profile_offer_taken(&mut self, key: u64) -> bool {
        let Some(c) = self.profile_state.offers.remove(&key) else {
            return false;
        };
        self.chrome.dismiss_undo(key);
        if self.last_undo == Some(key) {
            self.last_undo = None;
        }
        self.profile_always(c);
        self.profile_sync();
        true
    }

    /// An action of a capability that is off does nothing (remote control, menus, a stale
    /// palette): it is hidden, not disabled.
    pub(super) fn profile_hides(&self, a: Action) -> bool {
        let st = self.action_state();
        match a.hidden(&st) {
            Some(c) => {
                debug!(target: INPUT, "action {} hidden: {} tools are off", a.id(), c.id());
                true
            }
            None => false,
        }
    }
}
