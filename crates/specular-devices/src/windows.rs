//! Windows: `Windows.Gaming.Input` (safe WinRT calls). XInput and Raw Input would need
//! `unsafe` Win32 calls; Gaming.Input covers Xbox and most other pads, and a SpaceMouse as a
//! `RawGameController` (3Dconnexion's vendor ids).
//!
//! Gaming.Input has no "reading changed" event for pads, so one thread reads them: at about
//! the frame rate while a stick, trigger or cap is off-centre or a button is held, and four
//! times a second otherwise (also to notice devices coming and going). Compile-checked only.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use specular_input::PadButton;
use windows::Gaming::Input::{
    GameControllerSwitchPosition, Gamepad, GamepadButtons, RawGameController,
};

use crate::evdev::puck_from_raw;
use crate::sysfs::is_spacemouse;
use crate::{Button, DeviceId, DeviceInfo, Event, Kind, Pad, Puck, Sink, State, Via};

pub(crate) struct Backend {
    stop: Arc<AtomicBool>,
    rescan: Arc<AtomicBool>,
}

impl Backend {
    pub(crate) fn start(sink: Sink) -> Backend {
        let stop = Arc::new(AtomicBool::new(false));
        let rescan = Arc::new(AtomicBool::new(true));
        let (s, r) = (Arc::clone(&stop), Arc::clone(&rescan));
        let _ = specular_thread::builder("plat-pads").spawn(move || run(&sink, &s, &r));
        Backend { stop, rescan }
    }

    pub(crate) fn rescan(&self) {
        self.rescan.store(true, Ordering::SeqCst);
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

const BUTTONS: [(GamepadButtons, PadButton); 14] = [
    (GamepadButtons::A, PadButton::A),
    (GamepadButtons::B, PadButton::B),
    (GamepadButtons::X, PadButton::X),
    (GamepadButtons::Y, PadButton::Y),
    (GamepadButtons::LeftShoulder, PadButton::Lb),
    (GamepadButtons::RightShoulder, PadButton::Rb),
    (GamepadButtons::LeftThumbstick, PadButton::Ls),
    (GamepadButtons::RightThumbstick, PadButton::Rs),
    (GamepadButtons::View, PadButton::Back),
    (GamepadButtons::Menu, PadButton::Start),
    (GamepadButtons::DPadUp, PadButton::Up),
    (GamepadButtons::DPadDown, PadButton::Down),
    (GamepadButtons::DPadLeft, PadButton::Left),
    (GamepadButtons::DPadRight, PadButton::Right),
];

struct PadSlot {
    pad: Gamepad,
    id: DeviceId,
    state: Pad,
    buttons: u32,
}

struct PuckSlot {
    ctl: RawGameController,
    id: DeviceId,
    state: Puck,
    buttons: Vec<bool>,
}

fn run(sink: &Sink, stop: &AtomicBool, rescan: &AtomicBool) {
    let mut pads: Vec<PadSlot> = Vec::new();
    let mut pucks: Vec<PuckSlot> = Vec::new();
    let mut next = 1u32;
    let mut ticks = 0u32;
    while !stop.load(Ordering::SeqCst) {
        if rescan.swap(false, Ordering::SeqCst) || ticks.is_multiple_of(4) {
            refresh(sink, &mut pads, &mut pucks, &mut next);
        }
        let mut active = false;
        for s in &mut pads {
            let Ok(r) = s.pad.GetCurrentReading() else {
                continue;
            };
            let p = Pad {
                left: [r.LeftThumbstickX as f32, -(r.LeftThumbstickY as f32)],
                right: [r.RightThumbstickX as f32, -(r.RightThumbstickY as f32)],
                lt: r.LeftTrigger as f32,
                rt: r.RightTrigger as f32,
            };
            if p != s.state {
                s.state = p;
                sink(Event::State(s.id, State::Pad(p)));
            }
            let bits = r.Buttons.0;
            for (flag, b) in BUTTONS {
                let now = bits & flag.0 != 0;
                if now != (s.buttons & flag.0 != 0) {
                    sink(Event::Button {
                        id: s.id,
                        button: Button::Pad(b),
                        down: now,
                    });
                }
            }
            s.buttons = bits;
            active |= p != Pad::default() || bits != 0;
        }
        for s in &mut pucks {
            let n = s.ctl.AxisCount().unwrap_or(0).clamp(0, 16) as usize;
            let nb = s.ctl.ButtonCount().unwrap_or(0).clamp(0, 64) as usize;
            let ns = s.ctl.SwitchCount().unwrap_or(0).clamp(0, 8) as usize;
            let mut axes = vec![0.5f64; n];
            let mut buttons = vec![false; nb];
            let mut switches = vec![GameControllerSwitchPosition::Center; ns];
            if s.ctl
                .GetCurrentReading(&mut buttons, &mut switches, &mut axes)
                .is_err()
            {
                continue;
            }
            // Axes are 0…1 with the centre at 0.5, in the device's raw frame.
            let mut raw = [0i32; 6];
            for (i, a) in axes.iter().take(6).enumerate() {
                raw[i] = ((a - 0.5) * 2.0 * f64::from(crate::evdev::PUCK_SCALE)) as i32;
            }
            let p = puck_from_raw(raw);
            if p != s.state {
                s.state = p;
                sink(Event::State(s.id, State::Puck(p)));
            }
            for (i, (&now, was)) in buttons.iter().zip(s.buttons.iter_mut()).enumerate() {
                if now != *was {
                    *was = now;
                    sink(Event::Button {
                        id: s.id,
                        button: Button::Puck((i + 1) as u8),
                        down: now,
                    });
                }
            }
            active |= p != Puck::default() || buttons.iter().any(|b| *b);
        }
        ticks = ticks.wrapping_add(1);
        std::thread::sleep(Duration::from_millis(if active { 16 } else { 250 }));
    }
}

fn refresh(sink: &Sink, pads: &mut Vec<PadSlot>, pucks: &mut Vec<PuckSlot>, next: &mut u32) {
    let mut now_pads = Vec::new();
    if let Ok(list) = Gamepad::Gamepads() {
        for i in 0..list.Size().unwrap_or(0) {
            if let Ok(g) = list.GetAt(i) {
                now_pads.push(g);
            }
        }
    }
    pads.retain(|s| {
        let keep = now_pads.contains(&s.pad);
        if !keep {
            sink(Event::Removed(s.id));
        }
        keep
    });
    for g in now_pads {
        if pads.iter().any(|s| s.pad == g) {
            continue;
        }
        let id = DeviceId(*next);
        *next += 1;
        let name = RawGameController::FromGameController(&g)
            .and_then(|r| r.DisplayName())
            .map(|h| h.to_string())
            .unwrap_or_default();
        sink(Event::Added(DeviceInfo {
            id,
            name,
            kind: Kind::Gamepad,
            via: Via::GamingInput,
            path: String::new(),
        }));
        pads.push(PadSlot {
            pad: g,
            id,
            state: Pad::default(),
            buttons: 0,
        });
    }
    let mut now_pucks = Vec::new();
    if let Ok(list) = RawGameController::RawGameControllers() {
        for i in 0..list.Size().unwrap_or(0) {
            if let Ok(c) = list.GetAt(i)
                && is_spacemouse(
                    c.HardwareVendorId().unwrap_or(0),
                    c.HardwareProductId().unwrap_or(0),
                )
            {
                now_pucks.push(c);
            }
        }
    }
    pucks.retain(|s| {
        let keep = now_pucks.contains(&s.ctl);
        if !keep {
            sink(Event::Removed(s.id));
        }
        keep
    });
    for c in now_pucks {
        if pucks.iter().any(|s| s.ctl == c) {
            continue;
        }
        let id = DeviceId(*next);
        *next += 1;
        sink(Event::Added(DeviceInfo {
            id,
            name: c.DisplayName().map(|h| h.to_string()).unwrap_or_default(),
            kind: Kind::SpaceMouse,
            via: Via::GamingInput,
            path: String::new(),
        }));
        let nb = c.ButtonCount().unwrap_or(0).clamp(0, 64) as usize;
        pucks.push(PuckSlot {
            ctl: c,
            id,
            state: Puck::default(),
            buttons: vec![false; nb],
        });
    }
}
