//! Navigation devices in the app (`specular_devices`): gamepads and 3Dconnexion SpaceMouse.
//!
//! - **Off by default** (`devices.enabled`): nothing is opened until it is turned on, and
//!   turning it off closes the readers.
//! - **Motion**: each turn of the event loop takes the devices' latest state and, while any
//!   is off-centre, moves the view by its velocity over the time since the last turn and asks
//!   for the next frame in 16 ms. At rest nothing runs: the reader threads wake the loop when
//!   a device sends something. The 2D view pans and zooms; a 3D model or MPR's 3D view
//!   turns and zooms (`specular_devices::nav`).
//! - **Buttons** are keys (`Pad-A`, `Puck-1`): the keymap's binding if it has one, else
//!   `specular_input::DEVICE_KEYS`. While Settings › Keys records, a button press is
//!   recorded like a key.
//! - Only while the window has the focus, and not under a dialog (but Back, to close it).
//! - **Settings › Devices**: what is connected and what each button does, in a panel under
//!   "Look again"; unreadable devices say which group to join (nothing is changed).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use specular_devices::nav::{self, Nav, Tuning};
use specular_devices::{Button, DeviceId, DeviceInfo, Devices, Event, Kind, Problem, State, Via};
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_i18n::{I18n, isolate_ltr, t};
use specular_input::{Action, DEVICE_KEYS, KeyCombo};
use specular_log::{INPUT, debug};
use specular_settings::Settings;
use specular_widgets::{Input, Modal, PanelRow, SettingPanel};

use crate::app::App;
use crate::app::frame_gate::Cause;

/// The setting the page's panel sits under.
pub(super) const UNDER: &str = "devices.look_again";

/// A SpaceMouse that sent nothing for this long is at rest (the kernel drops the zero
/// `EV_REL` values of a let-go cap, so its last frame can be the last word).
const PUCK_STALE: Duration = Duration::from_millis(250);
/// The longest step one frame moves by (after a stall, the view doesn't jump).
const MAX_STEP: f32 = 0.05;
/// The frame interval asked for while moving.
const FRAME: Duration = Duration::from_millis(16);

#[derive(Default)]
pub(super) struct DevicesState {
    run: Option<Devices>,
    rx: Option<crossbeam_channel::Receiver<Event>>,
    enabled: bool,
    tuning: Tuning,
    list: Vec<DeviceInfo>,
    states: HashMap<DeviceId, (State, Instant)>,
    problems: Vec<Problem>,
    /// When motion was last applied (none at rest).
    last: Option<Instant>,
    /// The window has the keyboard focus.
    pub(super) focused: bool,
    /// Changes to what the panel shows, and what it was built from (revision, language).
    rev: u64,
    built: Option<(u64, u64)>,
}

/// The settings' tuning.
fn tuning_of(s: &Settings) -> Tuning {
    Tuning {
        dead_zone: s.devices.dead_zone.clamp(0, 50) as f32 / 100.0,
        speed: s.devices.speed.clamp(25, 400) as f32 / 100.0,
    }
}

fn kind_name(k: Kind) -> String {
    match k {
        Kind::Gamepad => t!("devices-kind-gamepad"),
        Kind::Joystick => t!("devices-kind-joystick"),
        Kind::SpaceMouse => t!("devices-kind-spacemouse"),
    }
}

/// The rows of one problem.
fn problem_row(p: &Problem) -> PanelRow {
    let row = |label: String, lines: Vec<String>, detail: String| PanelRow {
        id: "problem".into(),
        label,
        lines,
        detail,
        attention: true,
        ..PanelRow::default()
    };
    match p {
        Problem::NoPermission {
            name,
            kind,
            path,
            group,
        } => {
            let mut lines = vec![match group.as_deref() {
                Some(g) if g != "root" => t!("devices-no-permission-group", group = isolate_ltr(g)),
                _ => t!("devices-no-permission-admin", path = isolate_ltr(path)),
            }];
            if *kind == Kind::SpaceMouse {
                lines.push(t!("devices-no-permission-spnav"));
            }
            row(
                t!("devices-no-permission", name = isolate_ltr(name)),
                lines,
                path.clone(),
            )
        }
        Problem::ReadFailed { name, error } => row(
            t!(
                "devices-read-failed",
                name = isolate_ltr(name),
                error = error.clone()
            ),
            Vec::new(),
            String::new(),
        ),
        Problem::Unsupported => row(t!("devices-unsupported"), Vec::new(), String::new()),
    }
}

/// What a device button does: its keymap binding, else the default (unless switched off).
fn button_action(km: &specular_input::Keymap, c: &KeyCombo) -> Option<Action> {
    km.device_action(c)
}

/// The panel for what is connected, with what the sticks and buttons do.
pub(super) fn panel(
    enabled: bool,
    list: &[DeviceInfo],
    problems: &[Problem],
    km: &specular_input::Keymap,
) -> SettingPanel {
    let mut rows = Vec::new();
    if !enabled {
        rows.push(PanelRow {
            id: "off".into(),
            label: t!("devices-off"),
            ..PanelRow::default()
        });
    }
    rows.extend(problems.iter().map(problem_row));
    if enabled && list.is_empty() {
        rows.push(PanelRow {
            id: "none".into(),
            label: t!("devices-none"),
            ..PanelRow::default()
        });
    }
    for d in list {
        rows.push(PanelRow {
            id: format!("device:{}", d.id.0),
            label: if d.name.trim().is_empty() {
                t!("devices-unnamed")
            } else {
                isolate_ltr(d.name.trim())
            },
            value: kind_name(d.kind),
            lines: if d.via == Via::Spacenavd {
                vec![t!("devices-via-spacenavd")]
            } else {
                Vec::new()
            },
            detail: d.path.clone(),
            ..PanelRow::default()
        });
    }
    // What the sticks and buttons do: for the kinds connected, or both when none is.
    let has = |k: &[Kind]| list.is_empty() || list.iter().any(|d| k.contains(&d.kind));
    let mut section = |pad: bool| {
        let (sticks, value, buttons, prefix) = if pad {
            (
                t!("devices-sticks-pad"),
                t!("devices-sticks-pad-value"),
                t!("devices-buttons-pad"),
                "Pad-",
            )
        } else {
            (
                t!("devices-sticks-puck"),
                t!("devices-sticks-puck-value"),
                t!("devices-buttons-puck"),
                "Puck-",
            )
        };
        rows.push(PanelRow {
            id: format!("{prefix}sticks"),
            label: sticks,
            lines: vec![value],
            ..PanelRow::default()
        });
        rows.push(PanelRow {
            id: format!("{prefix}buttons"),
            label: buttons,
            ..PanelRow::default()
        });
        for (k, _) in DEVICE_KEYS.iter().filter(|(k, _)| k.starts_with(prefix)) {
            let Some(c) = KeyCombo::parse(k) else {
                continue;
            };
            let Some(a) = button_action(km, &c) else {
                continue;
            };
            rows.push(PanelRow {
                id: format!("button:{k}"),
                label: isolate_ltr(&c.to_string()),
                value: a.name(),
                indent: 1,
                ..PanelRow::default()
            });
        }
    };
    if has(&[Kind::Gamepad, Kind::Joystick]) {
        section(true);
    }
    if has(&[Kind::SpaceMouse]) {
        section(false);
    }
    SettingPanel {
        intro: t!("devices-intro"),
        rows,
        keywords: t!("devices-keywords"),
        ..SettingPanel::default()
    }
}

impl App {
    /// The settings changed (or the app starts): start or stop the readers, take the tuning.
    pub(super) fn devices_configure(&mut self, s: &Settings) {
        let d = &mut self.devices;
        d.tuning = tuning_of(s);
        if s.devices.enabled == d.enabled {
            return;
        }
        d.enabled = s.devices.enabled;
        d.rev += 1;
        if d.enabled {
            let (tx, rx) = crossbeam_channel::unbounded();
            let wake = self.loader.waker();
            let sink: specular_devices::Sink = std::sync::Arc::new(move |e| {
                if tx.send(e).is_ok() {
                    wake();
                }
            });
            d.run = Some(Devices::start(sink));
            d.rx = Some(rx);
            debug!(target: INPUT, "devices: on");
        } else {
            d.run = None;
            d.rx = None;
            d.list.clear();
            d.states.clear();
            d.problems.clear();
            d.last = None;
            debug!(target: INPUT, "devices: off");
        }
    }

    /// Settings › Devices › Look again.
    pub(super) fn devices_look_again(&mut self) {
        self.devices.problems.clear();
        self.devices.rev += 1;
        if let Some(r) = &self.devices.run {
            r.rescan();
        }
        self.redraw = true;
    }

    /// Once per event-loop turn: takes what the devices sent, moves the view while any is
    /// off-centre and keeps the Settings panel up to date.
    pub(super) fn devices_tick(&mut self, el: &ActiveEventLoop) {
        let events: Vec<Event> = match &self.devices.rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        let now = Instant::now();
        for e in events {
            match e {
                Event::Added(info) => {
                    debug!(target: INPUT, "device added: {info:?}");
                    let d = &mut self.devices;
                    d.problems.retain(
                        |p| !matches!(p, Problem::NoPermission { path, .. } if *path == info.path),
                    );
                    d.list.push(info);
                    d.rev += 1;
                }
                Event::Removed(id) => {
                    debug!(target: INPUT, "device removed: {id:?}");
                    let d = &mut self.devices;
                    d.list.retain(|i| i.id != id);
                    d.states.remove(&id);
                    d.rev += 1;
                }
                Event::State(id, s) => {
                    self.devices.states.insert(id, (s, now));
                }
                Event::Button { button, down, .. } => self.device_button(button, down, el),
                Event::Problem(p) => {
                    debug!(target: INPUT, "device problem: {p:?}");
                    let d = &mut self.devices;
                    if !d.problems.contains(&p) {
                        d.problems.push(p);
                        d.rev += 1;
                    }
                }
            }
        }
        self.devices_motion(now);
        if self.chrome.modal() == Some(Modal::Settings) {
            let built = (self.devices.rev, I18n::global().generation());
            if self.devices.built != Some(built) {
                self.devices.built = Some(built);
                let d = &self.devices;
                let p = panel(d.enabled, &d.list, &d.problems, &self.keymap);
                if self.chrome.set_setting_panel(UNDER, Some(p)) {
                    self.redraw = true;
                }
            }
        }
    }

    /// The velocity of all devices together.
    fn devices_nav(&self, now: Instant, three_d: bool) -> Nav {
        let t = &self.devices.tuning;
        let mut sum = Nav::default();
        for (s, at) in self.devices.states.values() {
            let n = match s {
                State::Pad(p) => nav::pad(p, t, three_d),
                State::Puck(_) if now.duration_since(*at) > PUCK_STALE => continue,
                State::Puck(p) => nav::puck(p, t, three_d),
            };
            sum.pan[0] += n.pan[0];
            sum.pan[1] += n.pan[1];
            sum.zoom += n.zoom;
            sum.orbit[0] += n.orbit[0];
            sum.orbit[1] += n.orbit[1];
        }
        sum
    }

    fn devices_motion(&mut self, now: Instant) {
        if self.devices.states.is_empty() {
            self.devices.last = None;
            return;
        }
        let mpr = self.mpr_active();
        let three_d = self.mesh_active() || (mpr && self.mpr_device_3d());
        let n = self.devices_nav(now, three_d);
        if n.is_still() || !self.devices.focused || self.chrome.modal().is_some() {
            self.devices.last = None;
            // A SpaceMouse that may still go stale: look again then.
            if self.devices.focused
                && let Some(at) = self
                    .devices
                    .states
                    .values()
                    .filter(|(s, at)| {
                        matches!(s, State::Puck(p) if *p != Default::default())
                            && now.duration_since(*at) <= PUCK_STALE
                    })
                    .map(|(_, at)| *at)
                    .max()
            {
                self.wake_by(at + PUCK_STALE, Cause::Gesture);
            }
            return;
        }
        let dt = self
            .devices
            .last
            .map_or(0.0, |l| now.duration_since(l).as_secs_f32().min(MAX_STEP));
        self.devices.last = Some(now);
        self.wake_by(now + FRAME, Cause::Gesture);
        if dt <= 0.0 {
            return;
        }
        let (pan, zoom, orbit) = n.over(dt);
        let vp = self.view.viewport;
        if three_d || mpr {
            let px = (f64::from(pan[0] * vp.w), f64::from(pan[1] * vp.h));
            let orbit = (f64::from(orbit[0]), f64::from(orbit[1]));
            if !self.mesh_device_nav(px, f64::from(zoom), orbit) {
                self.mpr_device_nav(px, f64::from(zoom), orbit);
            }
            return;
        }
        if self.gallery_active() || self.shown.is_none() {
            return;
        }
        // The view moves over the image: the image moves the other way.
        if pan != [0.0, 0.0] {
            self.view.pan_by(-pan[0] * vp.w, -pan[1] * vp.h, false);
        }
        if zoom != 1.0 {
            self.view.zoom_direct_by(zoom, None);
        }
        self.after_view_change();
    }

    /// A device button went down or up.
    fn device_button(&mut self, b: Button, down: bool, el: &ActiveEventLoop) {
        let Some(combo) = b.combo() else {
            return;
        };
        debug!(target: INPUT, "device button {combo} {}", if down { "down" } else { "up" });
        if !down || !self.devices.focused {
            return;
        }
        // Settings › Keys is recording: the button is the key to bind.
        if self.chrome.recording_keys() {
            let r = self.chrome_input(Input::Key {
                combo,
                repeat: false,
            });
            for c in r.commands {
                self.command(c, el);
            }
            self.redraw = true;
            return;
        }
        let Some(a) = button_action(&self.keymap, &combo) else {
            return;
        };
        if self.chrome.modal().is_some() && a != Action::Back {
            return;
        }
        if let Some(why) = a.disabled(&self.action_state()) {
            debug!(target: INPUT, "action {} disabled: {why:?}", a.id());
            return;
        }
        self.action(a, el);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_input::{Keymap, PadButton};

    fn info(id: u32, kind: Kind, via: Via) -> DeviceInfo {
        DeviceInfo {
            id: DeviceId(id),
            name: "Xbox Wireless Controller".into(),
            kind,
            via,
            path: "/dev/input/js0".into(),
        }
    }

    #[test]
    fn panel_lists_devices_and_buttons() {
        let km = Keymap::viewer_default();
        let p = panel(true, &[info(1, Kind::Gamepad, Via::Joydev)], &[], &km);
        let labels: Vec<&str> = p.rows.iter().map(|r| r.label.as_str()).collect();
        assert!(
            labels
                .iter()
                .any(|l| l.contains("Xbox Wireless Controller"))
        );
        // Only the gamepad's section with a gamepad connected.
        assert!(p.rows.iter().any(|r| r.id == "button:Pad-A"));
        assert!(!p.rows.iter().any(|r| r.id == "button:Puck-1"));
        // A binding in the keymap shows instead of the default.
        let mut km2 = Keymap::viewer_default();
        assert!(
            km2.apply_toml("[viewer]\n\"Pad-A\" = \"next\"\n")
                .is_empty()
        );
        let p2 = panel(true, &[info(1, Kind::Gamepad, Via::Joydev)], &[], &km2);
        let a = |p: &SettingPanel| {
            p.rows
                .iter()
                .find(|r| r.id == "button:Pad-A")
                .map(|r| r.value.clone())
        };
        assert_ne!(a(&p), a(&p2));
        assert_eq!(a(&p2), Some(Action::NextImage.name()));
        assert_eq!(
            button_action(&km2, &PadButton::A.combo()),
            Some(Action::NextImage)
        );
        // None connected: both sections, and the "none found" row.
        let p3 = panel(true, &[], &[], &km);
        assert!(p3.rows.iter().any(|r| r.id == "none"));
        assert!(p3.rows.iter().any(|r| r.id == "button:Puck-1"));
        // Off: says so.
        assert!(
            panel(false, &[], &[], &km)
                .rows
                .iter()
                .any(|r| r.id == "off")
        );
    }

    #[test]
    fn permission_problem_names_the_group() {
        let p = Problem::NoPermission {
            name: "3Dconnexion SpaceMouse Pro".into(),
            kind: Kind::SpaceMouse,
            path: "/dev/input/event7".into(),
            group: Some("input".into()),
        };
        let r = problem_row(&p);
        assert!(r.attention);
        assert!(r.lines[0].contains("input"), "{:?}", r.lines);
        assert_eq!(r.lines.len(), 2, "SpaceMouse: also spacenavd");
        let root = Problem::NoPermission {
            name: "Pad".into(),
            kind: Kind::Gamepad,
            path: "/dev/input/event7".into(),
            group: Some("root".into()),
        };
        let r = problem_row(&root);
        assert!(r.lines[0].contains("/dev/input/event7"), "{:?}", r.lines);
        assert_eq!(r.lines.len(), 1);
    }
}
