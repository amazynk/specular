//! The sound's output device in the player: the choice in Settings and on the sound menu
//! (`crate::sound::Prefs`), the devices offered, the sound moved when the default changes
//! or the chosen device goes or comes back (`crate::sound::decide`; faded out and in, at
//! the same place), and the output shown in Info.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use specular_i18n::t;
use specular_settings::{Origin, SoundChannels};
use specular_video::audio::{AudioPlayer, OutputInfo, Silence};
use specular_video::speakers::ChannelMode;
use specular_widgets::Modal;

use super::App;
use crate::loader::ImageId;
use crate::sound::{self, Devices, Move, Prefs};

/// The output device's state in the app.
#[derive(Default)]
pub(super) struct OutDev {
    /// Lists the devices (started with the first sound).
    watch: Option<sound::Watch>,
    /// The choice outputs open with, as last set.
    applied: Option<Prefs>,
    /// The devices last offered to Settings and the menu.
    offered: Option<Devices>,
    /// Every device seen, by id: a device that has gone is still named.
    known: HashMap<String, String>,
    /// The sound last moved (not again for a moment: a device that won't settle).
    moved: Option<Instant>,
    /// The output shown in Info for this image.
    shown: Option<(ImageId, OutputInfo)>,
}

/// Moves no more often than this.
const SETTLE: Duration = Duration::from_secs(2);

/// The channel setting as the speakers' mode.
pub(super) fn channel_mode(c: SoundChannels) -> ChannelMode {
    match c {
        SoundChannels::Auto => ChannelMode::Auto,
        SoundChannels::Stereo => ChannelMode::Stereo,
        SoundChannels::Surround51 => ChannelMode::Surround51,
        SoundChannels::Surround71 => ChannelMode::Surround71,
    }
}

/// The channel settings in the menu's order.
pub(super) const CHANNELS: [SoundChannels; 4] = [
    SoundChannels::Auto,
    SoundChannels::Stereo,
    SoundChannels::Surround51,
    SoundChannels::Surround71,
];

/// A channel count in words: mono, stereo, 5.1, 7.1, or "N channels".
pub(super) fn channels_text(n: u16) -> String {
    match n {
        1 => t!("player-channels-mono"),
        2 => t!("sound-output-stereo"),
        6 => "5.1".to_owned(),
        8 => "7.1".to_owned(),
        n => t!("player-channels", count = i64::from(n)),
    }
}

/// A rate in kHz ("48", "44.1").
fn khz(rate: u32) -> String {
    let k = f64::from(rate) / 1000.0;
    let digits = if rate.is_multiple_of(1000) { 0 } else { 1 };
    specular_i18n::I18n::global().localizer().number(k, digits)
}

/// Info's output row: the device, its rate and channels, and what was done to fit.
pub(super) fn info_text(o: &OutputInfo) -> (String, Option<String>) {
    let device = if o.device.name.is_empty() {
        t!("sound-output-default")
    } else {
        specular_i18n::isolate_ltr(&o.device.name)
    };
    let value = t!(
        "info-sound-output-value",
        device = device,
        rate = khz(o.format.rate),
        channels = channels_text(o.format.channels)
    );
    let mut notes = Vec::new();
    if o.resampled {
        notes.push(t!("info-sound-output-resampled", rate = khz(o.source_rate)));
    }
    if o.downmixed {
        notes.push(t!(
            "info-sound-output-downmixed",
            channels = channels_text(o.source_channels)
        ));
    }
    (value, (!notes.is_empty()).then(|| notes.join("\n")))
}

impl App {
    /// Each tick: the settings to the outputs, the devices to Settings, and the sound moved
    /// when its device changed.
    pub(super) fn outdev_tick(&mut self) {
        let v = &self.settings.viewing;
        let want = Prefs {
            device: v.sound_output_device.trim().to_owned(),
            channels: channel_mode(v.sound_channels),
        };
        if self.outdev.applied.as_ref() != Some(&want) {
            let first = self.outdev.applied.is_none();
            sound::set_prefs(want.clone());
            self.outdev.applied = Some(want.clone());
            if !first {
                self.outdev_move();
            }
        }
        let playing = self.outdev_sound().is_some();
        let settings_open = self.chrome.modal() == Some(Modal::Settings);
        if (playing || settings_open) && self.outdev.watch.is_none() {
            self.outdev.watch = Some(sound::Watch::start(self.loader.waker()));
        }
        let Some(w) = &self.outdev.watch else { return };
        w.active.store(playing || settings_open, Ordering::Relaxed);
        let devices = w.devices();
        if let Some(d) = &devices
            && self.outdev.offered.as_ref() != Some(d)
        {
            for (id, name) in &d.list {
                self.outdev.known.insert(id.clone(), name.clone());
            }
            let pairs = d
                .list
                .iter()
                .map(|(id, name)| (id.clone(), specular_i18n::isolate_ltr(name)))
                .collect();
            self.chrome
                .set_setting_choices("viewing.sound_output_device", Some(pairs));
            self.outdev.offered = Some(d.clone());
            self.redraw = true;
        }
        // Info follows the output.
        let cur = self.current_id();
        if let (Some(id), Some(s)) = (cur, self.outdev_sound()) {
            let o = s.output();
            if self.outdev.shown.as_ref() != Some(&(id, o.clone())) {
                self.outdev.shown = Some((id, o));
                self.video_patch(id);
            }
        }
        // The sound moves when its device changed.
        let (Some(d), Some(s)) = (devices, self.outdev_sound()) else {
            return;
        };
        let now = Instant::now();
        if self
            .outdev
            .moved
            .is_some_and(|t| now.duration_since(t) < SETTLE)
        {
            return;
        }
        let known = |id: &str| self.outdev.known.get(id).cloned();
        let lost = s.lost();
        let Some(m) = sound::decide(&want.device, &s.output().device, lost, &d, &known) else {
            return;
        };
        if lost {
            // Said here, not as a silent movie.
            s.take_lost();
        }
        let msg = match &m {
            Move::FollowDefault if !lost => None,
            Move::FollowDefault => Some(t!("toast-sound-output-default")),
            Move::FallBack(name) => Some(t!(
                "toast-sound-output-gone",
                device = specular_i18n::isolate_ltr(name)
            )),
            Move::Return(name) => Some(t!(
                "toast-sound-output-back",
                device = specular_i18n::isolate_ltr(name)
            )),
        };
        specular_log::info!(target: specular_log::DECODE, "sound device: moving ({m:?})");
        self.outdev_move();
        if let Some(msg) = msg {
            let now = self.now();
            self.chrome.toast(msg, None, now);
        }
    }

    /// The sound of the file shown, once it is open.
    fn outdev_sound(&self) -> Option<&AudioPlayer> {
        let cur = self.current_id();
        self.anim
            .as_ref()
            .filter(|a| Some(a.id) == cur)
            .and_then(|a| a.sound())
    }

    /// Opens the sound of the file shown again on the device as chosen now, where the
    /// picture is: the old sound fades out, the new one in.
    pub(super) fn outdev_move(&mut self) {
        self.outdev.moved = Some(Instant::now());
        if std::env::var_os("SPECULAR_NO_SOUND").is_some() || self.outdev_sound().is_none() {
            return;
        }
        let Some(id) = self.anim.as_ref().map(|a| a.id) else {
            return;
        };
        let Some(source) = self
            .images
            .get(&id)
            .and_then(|i| i.session.as_ref())
            .map(|s| Arc::clone(s.source()))
        else {
            return;
        };
        // A chosen track opens as chosen; otherwise as the file opened it.
        let open: Box<dyn FnOnce() -> Result<AudioPlayer, Silence> + Send> = match self.player.sound
        {
            Some(track) => Box::new(move || {
                specular_video::session::open_sound_track(
                    source,
                    Box::new(crate::sound::CpalOutput::new()),
                    Some(track),
                )
            }),
            None => self.sound_opener(&source),
        };
        let movie = crate::anim::Movie::new(
            true,
            self.loop_forever,
            crate::anim::volume(&self.settings),
            self.settings.viewing.video_muted,
            Some(open),
        );
        if let Some(a) = self.anim.as_mut() {
            a.move_sound(movie);
        }
        self.sound_fx_new_player();
        self.redraw = true;
    }

    /// Info's output row for the sound of image `id`, once it plays.
    pub(super) fn outdev_info(&self, id: ImageId) -> Option<specular_metadata::SummaryItem> {
        let o = self
            .anim
            .as_ref()
            .filter(|a| a.id == id)
            .and_then(|a| a.sound())?
            .output();
        let (value, detail) = info_text(&o);
        Some(specular_metadata::SummaryItem {
            key: "sound-output",
            label: t!("info-sound-output"),
            value,
            detail,
        })
    }

    /// The devices for the sound menu: labels ("System default" first) and the one chosen.
    pub(super) fn outdev_menu(&self) -> (Vec<String>, usize) {
        let mut labels = vec![t!("sound-output-default")];
        let chosen = self.settings.viewing.sound_output_device.trim();
        let mut at = 0;
        if let Some(d) = &self.outdev.offered {
            for (i, (id, name)) in d.list.iter().enumerate() {
                labels.push(specular_i18n::isolate_ltr(name));
                if id == chosen {
                    at = i + 1;
                }
            }
        }
        // A chosen device not there now still shows (ticked).
        if !chosen.is_empty() && at == 0 {
            let name = self
                .outdev
                .known
                .get(chosen)
                .cloned()
                .unwrap_or_else(|| chosen.to_owned());
            labels.push(specular_i18n::isolate_ltr(&name));
            at = labels.len() - 1;
        }
        (labels, at)
    }

    /// The sound menu's device `i` (0: the system's default): saved, said, and the sound
    /// moved to it.
    pub(super) fn outdev_choose(&mut self, i: usize) {
        let (labels, _) = self.outdev_menu();
        let id = match i {
            0 => String::new(),
            i => match self.outdev.offered.as_ref().and_then(|d| d.list.get(i - 1)) {
                Some((id, _)) => id.clone(),
                // The chosen device that is not there: stays chosen.
                None => self.settings.viewing.sound_output_device.clone(),
            },
        };
        if id != self.settings.viewing.sound_output_device {
            self.store.update(Origin::Program, |s| {
                s.viewing.sound_output_device = id.clone()
            });
            Arc::make_mut(&mut self.settings)
                .viewing
                .sound_output_device = id;
        }
        let label = labels.get(i).cloned().unwrap_or_default();
        let now = self.now();
        self.chrome
            .toast(t!("toast-sound-output", device = label), None, now);
        self.redraw = true;
    }

    /// The sound menu's speakers `c`: saved and said; the sound opens again for them.
    pub(super) fn outdev_channels(&mut self, c: SoundChannels) {
        if c != self.settings.viewing.sound_channels {
            self.store
                .update(Origin::Program, |s| s.viewing.sound_channels = c);
            Arc::make_mut(&mut self.settings).viewing.sound_channels = c;
        }
        let now = self.now();
        self.chrome.toast(
            t!("toast-sound-speakers", speakers = speakers_text(c)),
            None,
            now,
        );
        self.redraw = true;
    }

    /// Lists the devices again (the sound menu opened).
    pub(super) fn outdev_refresh(&self) {
        if let Some(w) = &self.outdev.watch {
            w.refresh();
        }
    }
}

/// A channel setting in words, as the menu shows it.
pub(super) fn speakers_text(c: SoundChannels) -> String {
    match c {
        SoundChannels::Auto => t!("settings-viewing-sound-channels-auto"),
        SoundChannels::Stereo => t!("settings-viewing-sound-channels-stereo"),
        SoundChannels::Surround51 => t!("settings-viewing-sound-channels-surround-5-1"),
        SoundChannels::Surround71 => t!("settings-viewing-sound-channels-surround-7-1"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_video::audio::{DeviceFormat, OutputDevice};

    /// Info says the device, its rate and channels, and what was done to fit.
    #[test]
    fn info_row() {
        let o = OutputInfo {
            device: OutputDevice {
                id: "alsa:default".into(),
                name: "Built-in Audio".into(),
                default: true,
            },
            format: DeviceFormat {
                rate: 48_000,
                channels: 2,
            },
            source_channels: 6,
            source_rate: 44_100,
            resampled: true,
            downmixed: true,
        };
        let (value, detail) = info_text(&o);
        assert!(
            value.contains("Built-in Audio") && value.contains("48"),
            "{value}"
        );
        let detail = detail.unwrap();
        assert!(
            detail.contains("44.1") && detail.contains("5.1"),
            "{detail}"
        );
        let plain = OutputInfo {
            resampled: false,
            downmixed: false,
            ..o
        };
        assert_eq!(info_text(&plain).1, None);
        assert_eq!(channels_text(8), "7.1");
        assert_eq!(
            CHANNELS.map(channel_mode),
            [
                ChannelMode::Auto,
                ChannelMode::Stereo,
                ChannelMode::Surround51,
                ChannelMode::Surround71
            ]
        );
    }
}
