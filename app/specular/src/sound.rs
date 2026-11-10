//! The sound device for movies and sound files: the device chosen in Settings (or the
//! system's default) through cpal, owned by its own thread (a cpal stream may not move
//! between threads on every platform), feeding `specular_video::audio`'s renderer. Tests
//! never open it; they use `specular_video::audio::FakeOutput`.
//!
//! [`Watch`] lists the devices now and then, off the UI thread, and [`decide`] says when
//! the sound moves: the default changed (following it), the chosen device went (to the
//! default) or came back (back to it).
//!
//! Channel order: cpal passes samples straight through in the platform's order (no
//! speaker mask of its own); `specular_video::speakers` puts 5.1 and 7.1 in that order.
//! Platform paths new here: on Windows (WASAPI) and macOS (Core Audio) the device list,
//! the device by id, the default device's id, and a stream opened at 6 or 8 channels.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use parking_lot::Mutex;
use specular_video::audio::{AudioOutput, DeviceFormat, OutputDevice, Render};
use specular_video::speakers::{ChannelMode, pick_channels};

/// What the listener chose: a device by its id (empty: the system's default) and the
/// speakers' channels.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    pub device: String,
    pub channels: ChannelMode,
}

/// The choice new outputs open with (set from the settings).
static PREFS: Mutex<Prefs> = Mutex::new(Prefs {
    device: String::new(),
    channels: ChannelMode::Auto,
});

pub fn set_prefs(p: Prefs) {
    *PREFS.lock() = p;
}

pub fn prefs() -> Prefs {
    PREFS.lock().clone()
}

/// The chosen device, or the system's default.
pub struct CpalOutput {
    lost: Arc<AtomicBool>,
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
    prefs: Prefs,
    opened: Option<OutputDevice>,
}

impl CpalOutput {
    /// An output for the choice as set now.
    pub fn new() -> CpalOutput {
        CpalOutput {
            lost: Arc::default(),
            stop: None,
            thread: None,
            prefs: prefs(),
            opened: None,
        }
    }
}

/// A stream of samples of type `T`, converted from the renderer's f32.
fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut render: Render,
    lost: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let mut scratch: Vec<f32> = Vec::new();
    let on_data = move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
        let ts = info.timestamp();
        let latency = ts.playback.duration_since(&ts.callback).unwrap_or_default();
        scratch.clear();
        scratch.resize(data.len(), 0.0);
        render(&mut scratch, Instant::now() + latency);
        for (d, s) in data.iter_mut().zip(&scratch) {
            *d = T::from_sample(*s);
        }
    };
    let on_error = move |e: cpal::StreamError| {
        specular_log::warn!(target: specular_log::DECODE, "sound device: {e}");
        if matches!(
            e,
            cpal::StreamError::DeviceNotAvailable | cpal::StreamError::StreamInvalidated
        ) {
            lost.store(true, Ordering::Relaxed);
        }
    };
    device
        .build_output_stream(config, on_data, on_error, Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())
}

/// A device's name for the listener.
fn name_of(d: &cpal::Device) -> String {
    d.description()
        .map(|x| x.name().to_owned())
        .unwrap_or_default()
}

fn id_of(d: &cpal::Device) -> Option<String> {
    d.id().ok().map(|i| i.to_string())
}

/// The chosen device (by id) when it is there, else the default; and whether it is the
/// default.
fn device_for(host: &cpal::Host, chosen: &str) -> Option<(cpal::Device, bool)> {
    if !chosen.is_empty()
        && let Ok(id) = chosen.parse::<cpal::DeviceId>()
        && let Some(d) = host.device_by_id(&id)
    {
        return Some((d, false));
    }
    host.default_output_device().map(|d| (d, true))
}

/// The stream's configuration: the device's default, at the channel count
/// [`pick_channels`] asks for when it takes it.
fn config_for(
    device: &cpal::Device,
    mode: ChannelMode,
    source_channels: u16,
) -> Result<cpal::SupportedStreamConfig, String> {
    let default = device.default_output_config().map_err(|e| e.to_string())?;
    let ranges: Vec<cpal::SupportedStreamConfigRange> = device
        .supported_output_configs()
        .map(Iterator::collect)
        .unwrap_or_default();
    let mut counts: Vec<u16> = ranges.iter().map(|r| r.channels()).collect();
    counts.sort_unstable();
    counts.dedup();
    let want = pick_channels(mode, source_channels, &counts, default.channels());
    if want == default.channels() {
        return Ok(default);
    }
    let rate = default.sample_rate();
    let fits = |r: &&cpal::SupportedStreamConfigRange| r.channels() == want;
    let pick = ranges
        .iter()
        .filter(fits)
        .find(|r| r.sample_format() == default.sample_format())
        .or_else(|| ranges.iter().find(fits));
    Ok(match pick {
        Some(r) => {
            let rate = rate.clamp(r.min_sample_rate(), r.max_sample_rate());
            r.with_sample_rate(rate)
        }
        None => default,
    })
}

/// Opens the device and starts it; runs on the thread that keeps the stream.
fn open(
    render: Render,
    lost: Arc<AtomicBool>,
    prefs: &Prefs,
    source_channels: u16,
) -> Result<(cpal::Stream, DeviceFormat, OutputDevice), String> {
    let host = cpal::default_host();
    let (device, default) =
        device_for(&host, &prefs.device).ok_or_else(|| "no output device".to_owned())?;
    let supported = config_for(&device, prefs.channels, source_channels)?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, &config, render, lost)?,
        SampleFormat::I16 => build::<i16>(&device, &config, render, lost)?,
        SampleFormat::U16 => build::<u16>(&device, &config, render, lost)?,
        SampleFormat::I32 => build::<i32>(&device, &config, render, lost)?,
        SampleFormat::F64 => build::<f64>(&device, &config, render, lost)?,
        f => return Err(format!("unsupported sample format {f}")),
    };
    stream.play().map_err(|e| e.to_string())?;
    let opened = OutputDevice {
        id: id_of(&device).unwrap_or_default(),
        name: name_of(&device),
        default,
    };
    Ok((
        stream,
        DeviceFormat {
            rate: config.sample_rate,
            channels: config.channels,
        },
        opened,
    ))
}

impl AudioOutput for CpalOutput {
    fn start(&mut self, render: Render, source_channels: u16) -> Result<DeviceFormat, String> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (fmt_tx, fmt_rx) = mpsc::channel();
        let lost = Arc::clone(&self.lost);
        let prefs = self.prefs.clone();
        let thread = specular_thread::builder("anim-sound")
            .spawn(move || match open(render, lost, &prefs, source_channels) {
                Ok((stream, fmt, opened)) => {
                    let _ = fmt_tx.send(Ok((fmt, opened)));
                    // Keep the stream until the player goes (or the sender is dropped).
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = fmt_tx.send(Err(e));
                }
            })
            .map_err(|e| e.to_string())?;
        self.thread = Some(thread);
        self.stop = Some(stop_tx);
        match fmt_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok((fmt, opened))) => {
                specular_log::info!(target: specular_log::DECODE, "sound device: {} ({} Hz, {} channels)", opened.name, fmt.rate, fmt.channels);
                self.opened = Some(opened);
                Ok(fmt)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                // A device that hangs: leave its thread behind rather than wait on it when
                // the player goes (it ends once the device answers, the sender gone).
                self.stop = None;
                drop(self.thread.take());
                Err("the sound device did not answer".into())
            }
        }
    }

    fn lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }

    fn device(&self) -> OutputDevice {
        self.opened.clone().unwrap_or_default()
    }
}

impl Drop for CpalOutput {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// --- The devices there are -----------------------------------------------------------------

/// The output devices there are: (id, name) in the system's order, and the default's id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Devices {
    pub list: Vec<(String, String)>,
    pub default: Option<String>,
}

impl Devices {
    pub fn has(&self, id: &str) -> bool {
        self.list.iter().any(|(i, _)| i == id)
    }

    pub fn name(&self, id: &str) -> Option<&str> {
        self.list
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, n)| n.as_str())
    }
}

/// ALSA devices not offered: the default (offered as "System default"), the null sink,
/// raw hardware and plugins that only convert or duplicate another device.
const ALSA_HIDDEN: [&str; 19] = [
    "default",
    "null",
    "hw:",
    "plughw:",
    "front:",
    "dmix",
    "dsnoop",
    "surround21:",
    "surround40:",
    "surround41:",
    "surround50:",
    "lavrate",
    "samplerate",
    "speexrate",
    "upmix",
    "vdownmix",
    "usbstream:",
    "a52",
    "oss",
];

/// The devices worth offering, from all cpal lists (id, name, a second line of description),
/// each with a name of its own: names two devices share get the second line, or the ALSA
/// device's kind, or a number.
pub fn offered(all: Vec<(String, String, Option<String>)>) -> Vec<(String, String)> {
    let shown: Vec<_> = all
        .into_iter()
        .filter(|(id, _, _)| match id.strip_prefix("alsa:") {
            Some(pcm) => !ALSA_HIDDEN.iter().any(|h| pcm.starts_with(h)),
            None => true,
        })
        .collect();
    let mut out: Vec<(String, String)> = Vec::with_capacity(shown.len());
    for (id, name, more) in &shown {
        let twins = shown.iter().filter(|x| x.1 == *name).count();
        let name = if name.trim().is_empty() {
            id.clone()
        } else {
            name.clone()
        };
        let label = if twins > 1 {
            let kind = id
                .strip_prefix("alsa:")
                .and_then(|p| p.split(':').next())
                .map(str::to_owned);
            match (more.as_deref().filter(|m| !m.is_empty()), kind) {
                (_, Some(k)) => format!("{name} ({k})"),
                (Some(m), None) => format!("{name} ({m})"),
                (None, None) => name,
            }
        } else {
            name
        };
        let label = if out.iter().any(|(_, l)| *l == label) {
            let n = out.iter().filter(|(_, l)| l.starts_with(&label)).count() + 1;
            format!("{label} ({n})")
        } else {
            label
        };
        out.push((id.clone(), label));
    }
    out
}

/// The devices there are now (cpal; ALSA lists them without opening any).
pub fn list() -> Devices {
    let host = cpal::default_host();
    let all = host
        .output_devices()
        .map(|it| {
            it.filter_map(|d| {
                let id = id_of(&d)?;
                let desc = d.description().ok();
                let name = desc
                    .as_ref()
                    .map(|x| x.name().to_owned())
                    .unwrap_or_default();
                let more = desc.and_then(|x| x.extended().first().cloned());
                Some((id, name, more))
            })
            .collect()
        })
        .unwrap_or_default();
    Devices {
        list: offered(all),
        default: host.default_output_device().as_ref().and_then(id_of),
    }
}

/// How often the devices are listed while sound plays.
const POLL: Duration = Duration::from_secs(2);

/// Lists the devices on its own thread: every [`POLL`] while `active`, and once when asked.
pub struct Watch {
    latest: Arc<Mutex<Option<Devices>>>,
    pub active: Arc<AtomicBool>,
    poke: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Watch {
    /// Starts listing (`wake` runs after each list that changed).
    pub fn start(wake: Arc<dyn Fn() + Send + Sync>) -> Watch {
        let latest: Arc<Mutex<Option<Devices>>> = Arc::default();
        let active = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<()>();
        let thread = {
            let latest = Arc::clone(&latest);
            let active = Arc::clone(&active);
            specular_thread::builder("sound-devices")
                .spawn(move || {
                    loop {
                        let d = list();
                        let changed = latest.lock().as_ref() != Some(&d);
                        *latest.lock() = Some(d);
                        if changed {
                            wake();
                        }
                        // Asked (or every POLL while active); gone when the watch goes.
                        loop {
                            match rx.recv_timeout(POLL) {
                                Ok(()) => break,
                                Err(mpsc::RecvTimeoutError::Disconnected) => return,
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    if active.load(Ordering::Relaxed) {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                })
                .ok()
        };
        Watch {
            latest,
            active,
            poke: Some(tx),
            thread,
        }
    }

    /// Lists again now.
    pub fn refresh(&self) {
        if let Some(p) = &self.poke {
            let _ = p.send(());
        }
    }

    /// The last list (none before the first).
    pub fn devices(&self) -> Option<Devices> {
        self.latest.lock().clone()
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.poke = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// --- When the sound moves ------------------------------------------------------------------

/// Why the sound moves to another device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// The system's default changed, and the sound follows it.
    FollowDefault,
    /// The chosen device went (named): the sound goes to the default.
    FallBack(String),
    /// The chosen device is back (named).
    Return(String),
}

/// Whether the sound playing on `opened` moves: `chosen` is the device set (empty: the
/// default), `lost` says the stream failed, `devices` is the last list, `known` names
/// devices seen before (for one that has gone).
pub fn decide(
    chosen: &str,
    opened: &OutputDevice,
    lost: bool,
    devices: &Devices,
    known: &dyn Fn(&str) -> Option<String>,
) -> Option<Move> {
    let default = devices.default.as_deref();
    let name = |id: &str| {
        devices
            .name(id)
            .map(str::to_owned)
            .or_else(|| known(id))
            .unwrap_or_else(|| id.to_owned())
    };
    if !chosen.is_empty() {
        if opened.id == chosen {
            // On the chosen device: it went.
            return (lost || !devices.has(chosen))
                .then(|| default.map(|_| Move::FallBack(name(chosen))))
                .flatten();
        }
        // On the default in its place: the chosen one is back.
        if devices.has(chosen) {
            return Some(Move::Return(name(chosen)));
        }
    }
    let default = default?;
    (lost || opened.id != default).then_some(Move::FollowDefault)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn devs(ids: &[&str], default: Option<&str>) -> Devices {
        Devices {
            list: ids
                .iter()
                .map(|i| (i.to_string(), format!("Name {i}")))
                .collect(),
            default: default.map(str::to_owned),
        }
    }

    fn on(id: &str, default: bool) -> OutputDevice {
        OutputDevice {
            id: id.into(),
            name: format!("Name {id}"),
            default,
        }
    }

    fn none(_: &str) -> Option<String> {
        None
    }

    /// Following the default: the sound moves when the default changes or the stream
    /// fails, and stays otherwise.
    #[test]
    fn follows_the_default() {
        let d = devs(&["a", "b"], Some("a"));
        assert_eq!(decide("", &on("a", true), false, &d, &none), None);
        let d2 = devs(&["a", "b"], Some("b"));
        assert_eq!(
            decide("", &on("a", true), false, &d2, &none),
            Some(Move::FollowDefault)
        );
        // The device went with the default: on to the new default.
        assert_eq!(
            decide("", &on("a", true), true, &d, &none),
            Some(Move::FollowDefault)
        );
        // No device at all: nothing to move to (the sound is silent, said once).
        assert_eq!(
            decide("", &on("a", true), true, &devs(&[], None), &none),
            None
        );
    }

    /// A chosen device that goes: to the default, named; when it comes back: back to it.
    #[test]
    fn chosen_device_falls_back_and_returns() {
        let known = |id: &str| (id == "usb").then(|| "USB DAC".to_owned());
        let with = devs(&["a", "usb"], Some("a"));
        let without = devs(&["a"], Some("a"));
        // Playing on it: stays.
        assert_eq!(decide("usb", &on("usb", false), false, &with, &known), None);
        // Unplugged (gone from the list, or the stream failed first).
        assert_eq!(
            decide("usb", &on("usb", false), false, &without, &known),
            Some(Move::FallBack("USB DAC".into()))
        );
        assert_eq!(
            decide("usb", &on("usb", false), true, &with, &known),
            Some(Move::FallBack("Name usb".into()))
        );
        // On the default meanwhile: stays until it is back, then returns.
        assert_eq!(decide("usb", &on("a", true), false, &without, &known), None);
        assert_eq!(
            decide("usb", &on("a", true), false, &with, &known),
            Some(Move::Return("Name usb".into()))
        );
        // The default changes meanwhile: the fallback follows it.
        assert_eq!(
            decide(
                "usb",
                &on("a", true),
                false,
                &devs(&["b"], Some("b")),
                &known
            ),
            Some(Move::FollowDefault)
        );
        // Chosen but never there (opened the default at the start): returns once it is.
        assert_eq!(decide("usb", &on("a", true), false, &without, &none), None);
        // Gone and no default either: nothing to move to.
        assert_eq!(
            decide("usb", &on("usb", false), true, &devs(&[], None), &known),
            None
        );
    }

    /// ALSA's duplicates and converters are left out; twins get a name of their own.
    #[test]
    fn offered_devices() {
        let all = vec![
            ("alsa:default".into(), "Default".into(), None),
            ("alsa:null".into(), "Discard all samples".into(), None),
            ("alsa:pipewire".into(), "PipeWire Sound Server".into(), None),
            (
                "alsa:sysdefault:CARD=PCH".into(),
                "HDA Intel PCH, ALC892 Analog".into(),
                Some("Default Audio Device".into()),
            ),
            (
                "alsa:front:CARD=PCH,DEV=0".into(),
                "HDA Intel PCH, ALC892 Analog".into(),
                None,
            ),
            (
                "alsa:surround51:CARD=PCH,DEV=0".into(),
                "HDA Intel PCH, ALC892 Analog".into(),
                None,
            ),
            (
                "alsa:hdmi:CARD=PCH,DEV=0".into(),
                "HDA Intel PCH, HDMI 0".into(),
                None,
            ),
            (
                "alsa:hw:CARD=PCH,DEV=0".into(),
                "HDA Intel PCH".into(),
                None,
            ),
            (
                "alsa:plughw:CARD=PCH,DEV=0".into(),
                "HDA Intel PCH".into(),
                None,
            ),
        ];
        let o = offered(all);
        let labels: Vec<_> = o.iter().map(|x| x.1.as_str()).collect();
        assert_eq!(
            labels,
            [
                "PipeWire Sound Server",
                "HDA Intel PCH, ALC892 Analog (sysdefault)",
                "HDA Intel PCH, ALC892 Analog (surround51)",
                "HDA Intel PCH, HDMI 0",
            ]
        );
        // Elsewhere: everything, numbered when names repeat.
        let o = offered(vec![
            ("wasapi:{1}".into(), "Speakers".into(), None),
            ("wasapi:{2}".into(), "Speakers".into(), None),
            ("wasapi:{3}".into(), "".into(), None),
        ]);
        let labels: Vec<_> = o.iter().map(|x| x.1.as_str()).collect();
        assert_eq!(labels, ["Speakers", "Speakers (2)", "wasapi:{3}"]);
    }
}
