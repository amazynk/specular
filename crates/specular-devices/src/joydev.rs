//! Linux's joystick interface (`/dev/input/jsN`): 8-byte records `struct js_event { __u32
//! time; __s16 value; __u8 type; __u8 number; }`. The kernel scales every axis to ±32767
//! from the device's own range, so nothing has to be guessed, and on open it sends each
//! axis's and button's current value (flagged `JS_EVENT_INIT`).
//!
//! Records number axes and buttons, not codes. Without the `JSIOCGAXMAP`/`JSIOCGBTNMAP`
//! ioctls the numbering is rebuilt from sysfs's capability bitmaps the way `joydev.c`
//! numbers them: axes in code order; buttons from `BTN_JOYSTICK` up to `KEY_MAX`, then
//! `BTN_MISC` up to `BTN_JOYSTICK`.

use crate::pad::{Assembler, BTN_JOYSTICK, BTN_MISC, Layout, Role};
use crate::sysfs::Caps;
use crate::{DeviceId, Event, State};

pub const RECORD: usize = 8;
pub const JS_EVENT_BUTTON: u8 = 0x01;
pub const JS_EVENT_AXIS: u8 = 0x02;
pub const JS_EVENT_INIT: u8 = 0x80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsEvent {
    pub time: u32,
    pub value: i16,
    pub kind: u8,
    pub number: u8,
}

impl JsEvent {
    pub fn parse(r: &[u8; RECORD]) -> JsEvent {
        JsEvent {
            time: u32::from_ne_bytes([r[0], r[1], r[2], r[3]]),
            value: i16::from_ne_bytes([r[4], r[5]]),
            kind: r[6],
            number: r[7],
        }
    }

    pub fn encode(&self) -> [u8; RECORD] {
        let mut r = [0u8; RECORD];
        r[..4].copy_from_slice(&self.time.to_ne_bytes());
        r[4..6].copy_from_slice(&self.value.to_ne_bytes());
        r[6] = self.kind;
        r[7] = self.number;
        r
    }
}

/// joydev's numbering for a device with these capabilities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JsMap {
    pub axes: Vec<u16>,
    pub buttons: Vec<u16>,
}

/// `KEY_MAX`.
const KEY_MAX: u16 = 0x2ff;
/// `ABS_CNT`.
const ABS_CNT: u16 = 0x40;

impl JsMap {
    pub fn from_caps(caps: &Caps) -> JsMap {
        let axes = (0..ABS_CNT).filter(|c| caps.abs.has(*c)).collect();
        let buttons = (BTN_JOYSTICK..=KEY_MAX)
            .chain(BTN_MISC..BTN_JOYSTICK)
            .filter(|c| caps.key.has(*c))
            .collect();
        JsMap { axes, buttons }
    }
}

/// Turns a joystick-interface stream into [`Event`]s.
#[derive(Clone, Debug)]
pub struct JsReader {
    map: JsMap,
    layout: Layout,
    asm: Assembler,
    sent: Option<crate::Pad>,
}

impl JsReader {
    pub fn new(caps: &Caps) -> JsReader {
        JsReader {
            map: JsMap::from_caps(caps),
            layout: Layout::from_abs(&caps.abs),
            asm: Assembler::default(),
            sent: None,
        }
    }

    pub fn event(&mut self, e: JsEvent, id: DeviceId, out: &mut Vec<Event>) {
        let init = e.kind & JS_EVENT_INIT != 0;
        match e.kind & !JS_EVENT_INIT {
            JS_EVENT_AXIS => {
                let Some(&code) = self.map.axes.get(usize::from(e.number)) else {
                    return;
                };
                let role = self.layout.role(code);
                let x = f32::from(e.value) / 32767.0;
                let v = match role {
                    Role::None => return,
                    r if r.is_trigger() => (x + 1.0) / 2.0,
                    _ => x,
                };
                for (button, down) in self.asm.axis(role, v) {
                    if !init {
                        out.push(Event::Button { id, button, down });
                    }
                }
            }
            JS_EVENT_BUTTON => {
                let Some(&code) = self.map.buttons.get(usize::from(e.number)) else {
                    return;
                };
                // A button already held when the device was opened is not a press.
                if let Some((button, down)) = self.asm.key(code, e.value != 0)
                    && !init
                {
                    out.push(Event::Button { id, button, down });
                }
            }
            _ => return,
        }
        if self.sent != Some(self.asm.pad) {
            self.sent = Some(self.asm.pad);
            out.push(Event::State(id, State::Pad(self.asm.pad)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::*;
    use crate::sysfs::Bits;
    use crate::{Button, Pad};
    use specular_input::PadButton;

    fn xbox_caps() -> Caps {
        Caps {
            key: Bits::from_codes(&[
                BTN_SOUTH, BTN_EAST, BTN_NORTH, BTN_WEST, BTN_TL, BTN_TR, BTN_SELECT, BTN_START,
                BTN_MODE, BTN_THUMBL, BTN_THUMBR,
            ]),
            abs: Bits::from_codes(&[0, 1, 2, 3, 4, 5, 0x10, 0x11]),
            ..Caps::default()
        }
    }

    #[test]
    fn numbering_follows_joydev() {
        let mut caps = xbox_caps();
        caps.key = Bits::from_codes(&[0x100, 0x101, 0x130, 0x220]);
        let m = JsMap::from_caps(&caps);
        assert_eq!(m.axes, vec![0, 1, 2, 3, 4, 5, 0x10, 0x11]);
        // BTN_JOYSTICK and up first, then BTN_MISC.
        assert_eq!(m.buttons, vec![0x130, 0x220, 0x100, 0x101]);
    }

    #[test]
    fn stream_with_init_burst() {
        let mut r = JsReader::new(&xbox_caps());
        let ev = |kind, number, value| JsEvent {
            time: 1,
            value,
            kind,
            number,
        };
        let a = |n, v| ev(JS_EVENT_AXIS, n, v);
        let mut bytes = Vec::new();
        // Init: triggers at rest read −32767, button 0 (A) held.
        for e in [
            ev(JS_EVENT_AXIS | JS_EVENT_INIT, 2, -32767),
            ev(JS_EVENT_AXIS | JS_EVENT_INIT, 5, -32767),
            ev(JS_EVENT_BUTTON | JS_EVENT_INIT, 0, 1),
            a(0, 16384),
            a(5, 32767),
            ev(JS_EVENT_BUTTON, 0, 0),
            ev(JS_EVENT_BUTTON, 3, 1),
            a(7, 32767),
        ] {
            bytes.extend_from_slice(&e.encode());
        }
        let mut out = Vec::new();
        for c in bytes.chunks_exact(RECORD) {
            r.event(JsEvent::parse(c.try_into().unwrap()), DeviceId(2), &mut out);
        }
        let last = out
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::State(_, State::Pad(p)) => Some(*p),
                _ => None,
            })
            .unwrap();
        assert!((last.left[0] - 0.5).abs() < 1e-3);
        assert_eq!(last.rt, 1.0);
        assert_eq!(last.lt, 0.0);
        let buttons: Vec<_> = out
            .iter()
            .filter_map(|e| match e {
                Event::Button { button, down, .. } => Some((*button, *down)),
                _ => None,
            })
            .collect();
        // Buttons are numbered in code order: 0 A (south), 1 B, 2 Y (north), 3 X (west).
        assert_eq!(
            buttons,
            vec![
                (Button::Pad(PadButton::A), false),
                (Button::Pad(PadButton::X), true),
                (Button::Pad(PadButton::Down), true),
            ]
        );
        assert_ne!(last, Pad::default());
    }
}
