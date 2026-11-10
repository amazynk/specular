//! The sound chain in the player (specular-video `dsp`): the settings turned into the
//! sound's [`SoundSettings`], dialogue boost and mono from the sound menu and keys, the
//! equaliser's panel (built on Develop's panel), the levelling shown in Info, and gapless
//! play into the next sound file.
//!
//! **Gapless.** While a sound file plays, what plays after it (the play queue's next item,
//! else the folder's next sound file: `queue.rs`, [`App::queue_up_next`]) is handed to the
//! sound when it is a sound file; the sound opens and primes it and splices it in where
//! this one ends. When it is heard the queue moves on to that file and its player takes the
//! sound over ([`SoundFx::handoff`]): the sound never stops. Otherwise (a video next, a
//! sound that failed) the queue opens the next item when this one ends, as before.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use specular_i18n::t;
use specular_settings::{Origin, SoundLevelling};
use specular_video::audio::{AudioPlayer, Silence};
use specular_video::dsp::SoundSettings;
use specular_video::eq::{self, EqSettings};
use specular_video::levelling::{self, Levelling, Source as LevelSource};
use specular_widgets::develop_panel::{Body, DevelopPanel, Event, PanelButton};
use specular_widgets::eq_panel::{self, BAND, DELETE, PREAMP, PRESET, SAVE};

use super::App;
use crate::loader::ImageId;

/// The equaliser panel's screen-reader nodes.
pub(super) const A11Y_EQ: u64 = 0x6E80_0000_0000_0000;
/// "Delete the preset?" (the equaliser panel).
pub(super) const EQ_DELETE_ITEM: u64 = super::items::EQ_PRESET_DELETE;

/// A user preset's id in `viewing.sound_eq_preset`.
const USER: &str = "user:";

fn is_sound(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(specular_query::is_sound_ext)
}

/// The sound chain's state in the app.
#[derive(Default)]
pub(super) struct SoundFx {
    /// The equaliser's panel, while open.
    pub eq: Option<DevelopPanel>,
    /// The next file given to the sound of this image (`None`: none).
    next_for: Option<(ImageId, Option<PathBuf>)>,
    /// A sound handed on to the next file: it plays on until that file's player takes it.
    handoff: Option<(PathBuf, AudioPlayer)>,
    /// The levelling shown in Info for this image.
    shown: Option<(ImageId, Option<(i64, LevelSource)>)>,
    /// When the next file was last worked out (for this image).
    next_checked: Option<(ImageId, Instant)>,
    /// The file opened last is part of an album playing in order ("auto" levelling).
    album_run: Arc<AtomicBool>,
}

/// The sound as the settings have it; `album_run`: an album plays in order.
pub(super) fn sound_settings(v: &specular_settings::Viewing, album_run: bool) -> SoundSettings {
    let mode = match v.sound_levelling {
        SoundLevelling::Off => levelling::Mode::Off,
        SoundLevelling::Track => levelling::Mode::Track,
        SoundLevelling::Album => levelling::Mode::Album,
        SoundLevelling::Auto if album_run => levelling::Mode::Album,
        SoundLevelling::Auto => levelling::Mode::Track,
    };
    SoundSettings {
        levelling: Levelling {
            mode,
            preamp_db: v.sound_levelling_preamp as f32,
            prevent_clip: v.sound_prevent_clipping,
            measure: v.sound_measure_untagged,
        },
        eq: v.sound_equaliser.then(|| eq_settings(v)),
        dialogue: v
            .sound_dialogue_boost
            .then(|| v.sound_dialogue_strength.clamp(0, 100) as f32 / 100.0),
        balance: v.sound_balance.clamp(-100, 100) as f32 / 100.0,
        mono: v.sound_mono,
        crossfade: Duration::from_secs_f64(v.sound_crossfade.clamp(0.0, 12.0)),
    }
}

/// The equaliser's bands and preamp: the stored bands, else the preset's.
fn eq_settings(v: &specular_settings::Viewing) -> EqSettings {
    let bands = if v.sound_eq_bands.trim().is_empty() {
        preset_gains(v, &v.sound_eq_preset).unwrap_or([0.0; 10])
    } else {
        eq::parse_bands(&v.sound_eq_bands)
    };
    EqSettings {
        bands,
        preamp: v.sound_eq_preamp as f32,
    }
}

/// A preset's gains: built in, or the user's (`user:name`).
fn preset_gains(v: &specular_settings::Viewing, id: &str) -> Option<[f32; 10]> {
    match id.strip_prefix(USER) {
        Some(name) => eq::parse_user_presets(&v.sound_eq_user_presets)
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, g)| g),
        None => eq::preset(id),
    }
}

/// The album of the sound file at `path` (album and album artist), from its tags.
fn album_of(path: &Path) -> Option<String> {
    let src = specular_core::Source::open(path).ok()?;
    let t = specular_video::sound_tags::read(src.bytes());
    let album = t.get("album")?;
    Some(format!(
        "{album}\u{1f}{}",
        t.get("album-artist")
            .or_else(|| t.get("artist"))
            .unwrap_or_default()
    ))
}

impl App {
    /// The sound of a movie or sound file, opened on another thread: the sound handed on
    /// from the file before when it is this one (gapless), else a new one with the sound
    /// settings (working out first whether an album plays in order).
    pub(super) fn sound_opener(
        &mut self,
        source: &Arc<specular_core::Source>,
    ) -> Box<dyn FnOnce() -> Result<AudioPlayer, Silence> + Send> {
        if let Some((p, player)) = self.sound_fx.handoff.take() {
            if p == source.path() {
                return Box::new(move || Ok(player));
            }
            self.sound_fx.handoff = Some((p, player));
        }
        // The neighbours in the folder: an album plays in order when one of them is from
        // the same album.
        let neighbours: Vec<PathBuf> = self
            .current
            .map(|c| {
                [c.checked_sub(1), Some(c + 1)]
                    .into_iter()
                    .flatten()
                    .filter_map(|i| self.entries.get(i))
                    .filter(|p| is_sound(p))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let album_run = Arc::new(AtomicBool::new(false));
        self.sound_fx.album_run = Arc::clone(&album_run);
        let auto = self.settings.viewing.sound_levelling == SoundLevelling::Auto;
        let viewing = self.settings.viewing.clone();
        let source = Arc::clone(source);
        Box::new(move || {
            let run = auto
                && is_sound(source.path())
                && album_of(source.path())
                    .is_some_and(|a| neighbours.iter().any(|n| album_of(n).as_ref() == Some(&a)));
            album_run.store(run, Ordering::Relaxed);
            specular_video::session::open_sound_with(
                source,
                Box::new(crate::sound::CpalOutput::new()),
                &sound_settings(&viewing, run),
            )
        })
    }

    /// Each tick: the settings to the sound, the levelling to Info, and the next file
    /// (primed, taken over when heard, or opened when this one ended).
    pub(super) fn sound_fx_tick(&mut self) {
        // The equaliser's panel goes when the file shown is no movie or sound file (it
        // stays open from one track to the next).
        let media = self
            .current
            .and_then(|c| self.entries.get(c))
            .is_some_and(|p| {
                is_sound(p)
                    || p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(specular_query::is_video_ext)
            });
        if self.sound_fx.eq.is_some() && !media {
            self.sound_fx.eq = None;
            self.redraw = true;
        }
        // A handed-on sound whose file is no longer the one shown goes.
        // (A stream's next piece keeps it until the piece shown ends and it opens.)
        let shown = self.current.and_then(|c| self.entries.get(c)).cloned();
        if let Some((p, _)) = &self.sound_fx.handoff
            && shown.as_ref() != Some(p)
            && shown
                .as_deref()
                .and_then(|s| self.stream_next_for(s))
                .as_ref()
                != Some(p)
        {
            self.sound_fx.handoff = None;
        }
        let cur = self.current_id();
        let Some(a) = self.anim.as_ref().filter(|a| Some(a.id) == cur) else {
            return;
        };
        let id = a.id;
        let Some(m) = a.movie.as_ref() else { return };
        let sound_file = !m.picture;
        let s = sound_settings(
            &self.settings.viewing,
            self.sound_fx.album_run.load(Ordering::Relaxed),
        );
        if a.sound().is_none() {
            return;
        }
        let path = self
            .images
            .get(&id)
            .and_then(|i| i.session.as_ref())
            .map(|s| s.source().path().to_path_buf());
        // What plays next, worked out again now and then (the queue may change).
        let now = Instant::now();
        let recheck = self
            .sound_fx
            .next_checked
            .is_none_or(|(i, t)| i != id || now.duration_since(t) > Duration::from_millis(500));
        // A network stream's movie piece: its sound goes on into the next piece's (one
        // decoder across the boundary when coded alike), as a sound file's does.
        let piece = !sound_file && path.as_deref().is_some_and(|p| self.stream_owns(p));
        let next = match (&path, recheck) {
            (Some(p), true) if sound_file || piece => {
                self.sound_fx.next_checked = Some((id, now));
                self.queue_up_next(p)
            }
            (_, true) => None,
            (_, false) => self
                .sound_fx
                .next_for
                .as_ref()
                .filter(|(i, _)| *i == id)
                .and_then(|(_, n)| n.clone()),
        };
        let Some(sound) = self
            .anim
            .as_ref()
            .filter(|a| a.id == id)
            .and_then(|a| a.sound())
        else {
            return;
        };
        sound.set_sound(&s);
        // Info's levelling row follows what is applied (to a tenth of a dB).
        let gain = sound
            .levelling()
            .map(|(db, src)| ((db * 10.0).round() as i64, src));
        // The next file, primed.
        let want = Some((id, next.clone()));
        if (sound_file || piece) && self.sound_fx.next_for != want {
            let src = next
                .as_ref()
                .and_then(|p| specular_core::Source::open(p).ok())
                .map(Arc::new);
            if piece {
                sound.set_next_piece(src);
            } else {
                sound.set_next(src);
            }
            self.sound_fx.next_for = want;
        }
        let advanced = sound.take_advanced() > 0;
        if self.sound_fx.shown != Some((id, gain)) {
            self.sound_fx.shown = Some((id, gain));
            self.video_patch(id);
        }
        if piece {
            // The next piece is heard: its player (opened when this picture ends) keeps
            // the sound.
            if advanced
                && let Some(p) = next
                && let Some(player) = self.anim.as_mut().and_then(|a| a.take_sound())
            {
                self.sound_fx.handoff = Some((p, player));
            }
            return;
        }
        if !sound_file {
            return;
        }
        // The next file is heard: the queue moves on to it, and its player keeps the sound.
        if advanced
            && let (Some(p), Some(cur)) = (next, path)
            && let Some(player) = self.anim.as_mut().and_then(|a| a.take_sound())
        {
            self.sound_fx.handoff = Some((p, player));
            self.queue_heard_next(id, &cur);
        }
    }

    /// Info's levelling row for the sound of image `id`, when levelling applies.
    pub(super) fn sound_fx_info(&self, id: ImageId) -> Option<specular_metadata::SummaryItem> {
        let (db, src) = self
            .anim
            .as_ref()
            .filter(|a| a.id == id)
            .and_then(|a| a.sound())
            .and_then(AudioPlayer::levelling)?;
        let gain = specular_i18n::isolate_ltr(&format!("{db:+.1} dB").replace('-', "\u{2212}"));
        let value = match src {
            LevelSource::Track => t!("info-sound-levelling-track", gain = gain),
            LevelSource::Album => t!("info-sound-levelling-album", gain = gain),
            LevelSource::Measured => t!("info-sound-levelling-measured", gain = gain),
        };
        Some(specular_metadata::SummaryItem {
            key: "sound-levelling",
            label: t!("info-sound-levelling"),
            value,
            detail: None,
        })
    }

    /// The sound of the file shown was opened again (another device): the next file is
    /// given to the new one, and Info shows its levelling.
    pub(super) fn sound_fx_new_player(&mut self) {
        self.sound_fx.next_for = None;
        self.sound_fx.shown = None;
    }

    /// Dialogue boost on or off (the d key, the sound menu).
    pub(super) fn sound_fx_dialogue(&mut self) {
        let on = !self.settings.viewing.sound_dialogue_boost;
        self.store
            .update(Origin::Program, |s| s.viewing.sound_dialogue_boost = on);
        Arc::make_mut(&mut self.settings)
            .viewing
            .sound_dialogue_boost = on;
        let now = self.now();
        let msg = if on {
            t!("toast-dialogue-boost-on")
        } else {
            t!("toast-dialogue-boost-off")
        };
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// Mono on or off (the sound menu).
    pub(super) fn sound_fx_mono(&mut self) {
        let on = !self.settings.viewing.sound_mono;
        self.store
            .update(Origin::Program, |s| s.viewing.sound_mono = on);
        Arc::make_mut(&mut self.settings).viewing.sound_mono = on;
        self.redraw = true;
    }

    // --- The equaliser's panel -------------------------------------------------------------

    /// Opens or closes the equaliser's panel (the e key, the sound menu).
    pub(super) fn eq_toggle_panel(&mut self) {
        if self.sound_fx.eq.take().is_none() {
            let mut p = eq_panel(&self.settings.viewing);
            p.set_focus(Some(specular_widgets::develop_panel::Item::Switch(0)));
            self.sound_fx.eq = Some(p);
        }
        self.redraw = true;
    }

    /// Fills the open panel from the settings again (a preset chosen, saved or deleted).
    fn eq_refresh(&mut self) {
        if let Some(p) = &mut self.sound_fx.eq {
            let focus = p.focus();
            let mut n = eq_panel(&self.settings.viewing);
            n.set_focus(focus);
            *p = n;
        }
    }

    fn eq_events(&mut self, ev: Vec<Event>) {
        for e in ev {
            match e {
                Event::Enabled { on, .. } => {
                    self.eq_store(|v| v.sound_equaliser = on);
                }
                Event::Value { control, .. } => self.eq_value(control),
                Event::Button(PanelButton::Close) | Event::Left => {
                    self.sound_fx.eq = None;
                }
                _ => {}
            }
        }
        self.redraw = true;
    }

    fn eq_store(&mut self, f: impl Fn(&mut specular_settings::Viewing)) {
        f(&mut Arc::make_mut(&mut self.settings).viewing);
        self.store.update(Origin::Program, |s| f(&mut s.viewing));
    }

    /// A control of the panel changed.
    fn eq_value(&mut self, control: u32) {
        let Some(p) = &self.sound_fx.eq else { return };
        let Some(c) = p.control(control) else { return };
        match (&c.body, control) {
            (Body::Choice(ch), PRESET) => {
                let ids = preset_ids(&self.settings.viewing);
                let Some(id) = ids.get(ch.selected).cloned() else {
                    return;
                };
                let gains = preset_gains(&self.settings.viewing, &id);
                self.eq_store(|v| {
                    v.sound_eq_preset = id.clone();
                    if let Some(g) = gains {
                        v.sound_eq_bands = eq::format_bands(&g);
                    }
                    v.sound_equaliser = true;
                });
                self.eq_refresh();
            }
            (Body::Slider(sl), PREAMP) => {
                let v = f64::from(sl.value);
                self.eq_store(|s| s.sound_eq_preamp = v);
            }
            (Body::Slider(sl), k) if (BAND..BAND + 10).contains(&k) => {
                let mut g = eq_settings(&self.settings.viewing).bands;
                g[(k - BAND) as usize] = sl.value;
                let bands = eq::format_bands(&g);
                let custom = !preset_ids(&self.settings.viewing)
                    .iter()
                    .any(|id| preset_gains(&self.settings.viewing, id) == Some(g));
                self.eq_store(|v| {
                    v.sound_eq_bands = bands.clone();
                    v.sound_equaliser = true;
                    if custom {
                        v.sound_eq_preset.clear();
                    }
                });
                if custom {
                    // The preset row says "Custom" now.
                    let opts = preset_names(&self.settings.viewing);
                    if let Some(Body::Choice(ch)) = self
                        .sound_fx
                        .eq
                        .as_mut()
                        .and_then(|p| p.control_mut(PRESET))
                        .map(|c| &mut c.body)
                        && ch.options != opts
                    {
                        ch.options = opts;
                        ch.selected = ch.options.len() - 1;
                    }
                }
            }
            (Body::Choice(_), SAVE) => self.eq_save_preset(),
            (Body::Choice(_), DELETE) => self.eq_delete_ask(),
            _ => {}
        }
    }

    /// Saves the bands as a new user preset ("My preset 1", "My preset 2"…).
    fn eq_save_preset(&mut self) {
        let v = &self.settings.viewing;
        let mut presets = eq::parse_user_presets(&v.sound_eq_user_presets);
        let name = (1..)
            .map(|n| t!("eq-user-preset-name", n = n))
            .find(|n| !presets.iter().any(|(p, _)| p == n))
            .unwrap_or_default();
        presets.push((name.clone(), eq_settings(v).bands));
        let text = eq::format_user_presets(&presets);
        self.eq_store(|v| {
            v.sound_eq_user_presets = text.clone();
            v.sound_eq_preset = format!("{USER}{name}");
        });
        self.eq_refresh();
    }

    /// Asks before deleting the chosen user preset.
    fn eq_delete_ask(&mut self) {
        let Some(name) = self.settings.viewing.sound_eq_preset.strip_prefix(USER) else {
            return;
        };
        self.chrome.confirm_destructive(
            t!("eq-confirm-delete", name = name.to_owned()),
            t!("eq-confirm-delete-detail"),
            t!("eq-delete-preset"),
            EQ_DELETE_ITEM,
        );
        self.redraw = true;
    }

    /// The deletion was confirmed: the preset goes; the bands stay as they are.
    pub(super) fn eq_delete_confirmed(&mut self) {
        let v = &self.settings.viewing;
        let Some(name) = v.sound_eq_preset.strip_prefix(USER).map(str::to_owned) else {
            return;
        };
        let mut presets = eq::parse_user_presets(&v.sound_eq_user_presets);
        presets.retain(|(n, _)| *n != name);
        let text = eq::format_user_presets(&presets);
        self.eq_store(|v| {
            v.sound_eq_user_presets = text.clone();
            v.sound_eq_preset.clear();
        });
        self.eq_refresh();
    }

    /// A key for the panel; whether it took it.
    pub(super) fn eq_key(&mut self, combo: specular_input::KeyCombo) -> bool {
        let Some(ev) = self.sound_fx.eq.as_mut().and_then(|p| p.key(combo)) else {
            return false;
        };
        self.eq_events(ev);
        true
    }

    /// The primary button at `lp` (logical px); whether the panel took it.
    pub(super) fn eq_button(&mut self, lp: (f32, f32), down: bool) -> bool {
        let shift = self.mods.shift_key();
        let now = self.now();
        let Some(p) = self.sound_fx.eq.as_mut() else {
            return false;
        };
        if down {
            let inside = p.contains(lp);
            if let Some(ev) = p.press(lp, shift, now) {
                self.eq_events(ev);
            }
            self.redraw |= inside;
            inside
        } else {
            let Some(ev) = p.release() else { return false };
            self.eq_events(ev);
            true
        }
    }

    /// The pointer moved; whether a drag in the panel is going on.
    pub(super) fn eq_pointer_move(&mut self, lp: Option<(f32, f32)>) -> bool {
        let shift = self.mods.shift_key();
        let Some(p) = self.sound_fx.eq.as_mut() else {
            return false;
        };
        let dragging = p.dragging();
        let hot = p.bar_hot();
        let ev = p.pointer_move(lp, shift);
        let rehot = p.bar_hot() != hot;
        if !ev.is_empty() {
            self.eq_events(ev);
        }
        if dragging || rehot {
            self.redraw = true;
        }
        dragging
    }

    /// A wheel turn over the panel; whether it took it.
    pub(super) fn eq_wheel(&mut self, lp: (f32, f32), dy: f32) -> bool {
        let took = self.sound_fx.eq.as_mut().is_some_and(|p| {
            // Over the panel the image does not zoom, scrolled or not.
            p.wheel(lp, dy);
            p.contains(lp)
        });
        self.redraw |= took;
        took
    }

    /// A screen reader's request on the panel; whether it was one.
    pub(super) fn eq_a11y_request(
        &mut self,
        req: &specular_widgets::accesskit::ActionRequest,
    ) -> bool {
        let value = match req.data {
            Some(specular_widgets::accesskit::ActionData::NumericValue(v)) => Some(v),
            _ => None,
        };
        let Some(ev) = self
            .sound_fx
            .eq
            .as_mut()
            .and_then(|p| p.a11y_action(A11Y_EQ, req.target_node.0, req.action, value))
        else {
            return false;
        };
        self.eq_events(ev);
        true
    }
}

/// The ids of the presets offered, built-in then the user's.
fn preset_ids(v: &specular_settings::Viewing) -> Vec<String> {
    eq::PRESETS
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .chain(
            eq::parse_user_presets(&v.sound_eq_user_presets)
                .into_iter()
                .map(|(n, _)| format!("{USER}{n}")),
        )
        .collect()
}

/// The presets' names as offered, and "Custom" when the bands are none of them.
fn preset_names(v: &specular_settings::Viewing) -> Vec<String> {
    let mut names: Vec<String> = preset_ids(v)
        .iter()
        .map(|id| match id.strip_prefix(USER) {
            Some(n) => n.to_owned(),
            None => specular_i18n::I18n::global().tr(&format!("eq-preset-{id}")),
        })
        .collect();
    if !preset_ids(v).contains(&v.sound_eq_preset) {
        names.push(t!("eq-custom"));
    }
    names
}

/// The equaliser's panel as the settings have it.
pub(super) fn eq_panel(v: &specular_settings::Viewing) -> DevelopPanel {
    let e = eq_settings(v);
    let ids = preset_ids(v);
    let presets = preset_names(v);
    let preset = ids
        .iter()
        .position(|id| *id == v.sound_eq_preset)
        .unwrap_or(presets.len() - 1);
    eq_panel::panel(&eq_panel::EqView {
        on: v.sound_equaliser,
        presets,
        preset,
        preamp: e.preamp,
        labels: eq::BANDS
            .iter()
            .map(|&hz| eq_panel::band_label(hz))
            .collect(),
        bands: e.bands.to_vec(),
        can_delete: v.sound_eq_preset.starts_with(USER),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_become_the_sound() {
        let mut v = specular_settings::Settings::default().viewing;
        let s = sound_settings(&v, false);
        assert_eq!(s.levelling.mode, levelling::Mode::Track);
        assert_eq!(s.eq, None);
        assert_eq!(s.dialogue, None);
        assert_eq!(
            sound_settings(&v, true).levelling.mode,
            levelling::Mode::Album
        );
        v.sound_equaliser = true;
        v.sound_eq_preset = "rock".into();
        assert_eq!(
            sound_settings(&v, false).eq.unwrap().bands,
            eq::preset("rock").unwrap()
        );
        v.sound_eq_bands = "1 2 3".into();
        assert_eq!(sound_settings(&v, false).eq.unwrap().bands[2], 3.0);
        v.sound_dialogue_boost = true;
        v.sound_balance = -50;
        let s = sound_settings(&v, false);
        assert_eq!(s.dialogue, Some(0.5));
        assert_eq!(s.balance, -0.5);
    }

    #[test]
    fn the_panel_has_ten_bands_and_presets() {
        let mut v = specular_settings::Settings::default().viewing;
        v.sound_eq_user_presets = "Car = 1 1 1 1 1 1 1 1 1 1".into();
        v.sound_eq_preset = "user:Car".into();
        let p = eq_panel(&v);
        let c = &p.cards[0];
        // Preset, preamp, ten bands, save, delete.
        assert_eq!(c.controls.len(), 14);
        let Body::Choice(ch) = &c.controls[0].body else {
            panic!()
        };
        assert_eq!(ch.options.len(), eq::PRESETS.len() + 1);
        assert_eq!(ch.selected, eq::PRESETS.len());
        assert_eq!(preset_gains(&v, "user:Car"), Some([1.0; 10]));
    }
}
