//! Linux's event interface (`/dev/input/eventN`): a stream of `struct input_event`
//! (`struct timeval time; __u16 type; __u16 code; __s32 value;`, native byte order; 24 bytes
//! with 64-bit `long`, 16 with 32-bit), grouped into frames ended by `SYN_REPORT`.
//!
//! Read without ioctls, so axis ranges (`EVIOCGABS`) are unknown: [`AutoRange`] guesses them
//! from the values seen. A SpaceMouse reports ±350 or so on each axis (`EV_REL` on most
//! models, `EV_ABS` on some); its raw frame has y towards the user and z down, turned here
//! into [`Puck`]'s y up, z towards the user.

use crate::pad::{Assembler, Layout, Role};
use crate::sysfs::Caps;
use crate::{Button, Event, Puck, State};

pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
pub const EV_REL: u16 = 2;
pub const EV_ABS: u16 = 3;
pub const SYN_REPORT: u16 = 0;
pub const SYN_DROPPED: u16 = 3;
/// SpaceMouse buttons: `BTN_0` is button 1.
pub const BTN_0: u16 = 0x100;

/// The size of one record on this system.
pub const RECORD: usize = if usize::BITS == 64 { 24 } else { 16 };
const TIME: usize = RECORD - 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputEvent {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

impl InputEvent {
    pub const fn new(kind: u16, code: u16, value: i32) -> InputEvent {
        InputEvent { kind, code, value }
    }

    /// From one record (the time is not used).
    pub fn parse(rec: &[u8; RECORD]) -> InputEvent {
        let t = TIME;
        InputEvent {
            kind: u16::from_ne_bytes([rec[t], rec[t + 1]]),
            code: u16::from_ne_bytes([rec[t + 2], rec[t + 3]]),
            value: i32::from_ne_bytes([rec[t + 4], rec[t + 5], rec[t + 6], rec[t + 7]]),
        }
    }

    /// One record as the kernel writes it (fake device feeds).
    pub fn encode(&self, usec: u64) -> [u8; RECORD] {
        let mut r = [0u8; RECORD];
        let half = TIME / 2;
        let sec = usec / 1_000_000;
        let us = usec % 1_000_000;
        r[..half].copy_from_slice(&sec.to_ne_bytes()[..half]);
        r[half..TIME].copy_from_slice(&us.to_ne_bytes()[..half]);
        r[TIME..TIME + 2].copy_from_slice(&self.kind.to_ne_bytes());
        r[TIME + 2..TIME + 4].copy_from_slice(&self.code.to_ne_bytes());
        r[TIME + 4..].copy_from_slice(&self.value.to_ne_bytes());
        r
    }
}

/// A byte stream of events, frames ended by `SYN_REPORT` (fake device feeds).
pub fn feed(frames: &[&[InputEvent]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut t = 1_000_000u64;
    for f in frames {
        for e in *f {
            out.extend_from_slice(&e.encode(t));
        }
        out.extend_from_slice(&InputEvent::new(EV_SYN, SYN_REPORT, 0).encode(t));
        t += 8_000;
    }
    out
}

/// An axis range guessed from the values seen (no `EVIOCGABS`): a stick that went negative
/// is centred on 0 with the smallest of ±127, ±511, ±2047, ±32767 that holds what was seen;
/// so is one whose first value was near 0. Any other is 0…255, 0…1023, 0…4095 or 0…65535 centred on its middle. Triggers
/// rest at 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoRange {
    min: i32,
    max: i32,
    /// The first value seen: a stick that starts near 0 is signed (unsigned ones rest at
    /// their middle).
    first: Option<i32>,
}

impl AutoRange {
    /// The value normalised: −1…1 for a stick, 0…1 for a trigger.
    pub fn norm(&mut self, v: i32, trigger: bool) -> f32 {
        let first = *self.first.get_or_insert(v);
        self.min = self.min.min(v);
        self.max = self.max.max(v);
        let signed = self.min < 0 || (!trigger && first.abs() <= 64);
        let pick = |m: i32, steps: &[i32]| {
            *steps
                .iter()
                .find(|s| **s >= m)
                .unwrap_or(&steps[steps.len() - 1])
        };
        if signed {
            let half = pick(self.max.max(-self.min), &[127, 511, 2047, 32767]) as f32;
            let x = (v as f32 / half).clamp(-1.0, 1.0);
            if trigger { (x + 1.0) / 2.0 } else { x }
        } else {
            let full = pick(self.max, &[255, 1023, 4095, 65535]) as f32;
            if trigger {
                (v as f32 / full).clamp(0.0, 1.0)
            } else {
                let c = (full + 1.0) / 2.0;
                ((v as f32 - c) / c).clamp(-1.0, 1.0)
            }
        }
    }
}

/// Turns a gamepad's event stream into [`Event`]s (sticks after each frame, buttons as they
/// come).
#[derive(Clone, Debug)]
pub struct PadReader {
    layout: Layout,
    ranges: [AutoRange; 0x12],
    asm: Assembler,
    sent: Option<crate::Pad>,
    /// After `SYN_DROPPED`, events up to the next report are dropped.
    dropping: bool,
}

impl PadReader {
    pub fn new(caps: &Caps) -> PadReader {
        PadReader {
            layout: Layout::from_abs(&caps.abs),
            ranges: [AutoRange::default(); 0x12],
            asm: Assembler::default(),
            sent: None,
            dropping: false,
        }
    }

    pub fn event(&mut self, e: InputEvent, id: crate::DeviceId, out: &mut Vec<Event>) {
        if self.dropping {
            if e.kind == EV_SYN && e.code == SYN_REPORT {
                self.dropping = false;
            }
            return;
        }
        match (e.kind, e.code) {
            (EV_SYN, SYN_DROPPED) => self.dropping = true,
            (EV_SYN, SYN_REPORT) => {
                if self.sent != Some(self.asm.pad) {
                    self.sent = Some(self.asm.pad);
                    out.push(Event::State(id, State::Pad(self.asm.pad)));
                }
            }
            (EV_KEY, c) if e.value != 2 => {
                if let Some((button, down)) = self.asm.key(c, e.value != 0) {
                    out.push(Event::Button { id, button, down });
                }
            }
            (EV_ABS, c) => {
                let role = self.layout.role(c);
                let v = match role {
                    Role::None => return,
                    Role::HatX | Role::HatY => e.value.signum() as f32,
                    r => self.ranges[usize::from(c)].norm(e.value, r.is_trigger()),
                };
                for (button, down) in self.asm.axis(role, v) {
                    out.push(Event::Button { id, button, down });
                }
            }
            _ => {}
        }
    }
}

/// The full scale of a SpaceMouse axis (spacenavd's too).
pub const PUCK_SCALE: f32 = 350.0;

/// A raw SpaceMouse frame (x right, y towards the user, z down; same for the turns) in
/// [`Puck`]'s frame.
pub fn puck_from_raw(raw: [i32; 6]) -> Puck {
    let n = |v: i32| (v as f32 / PUCK_SCALE).clamp(-1.0, 1.0);
    Puck {
        t: [n(raw[0]), -n(raw[2]), n(raw[1])],
        r: [n(raw[3]), -n(raw[5]), n(raw[4])],
    }
}

/// Turns a SpaceMouse's event stream into [`Event`]s. `EV_REL` axes count for the frame they
/// come in (an axis not sent is at rest: the kernel drops zero `EV_REL` values); `EV_ABS`
/// axes hold.
#[derive(Clone, Debug, Default)]
pub struct PuckReader {
    raw: [i32; 6],
    rel: bool,
    sent: Option<Puck>,
    dropping: bool,
}

impl PuckReader {
    pub fn event(&mut self, e: InputEvent, id: crate::DeviceId, out: &mut Vec<Event>) {
        if self.dropping {
            if e.kind == EV_SYN && e.code == SYN_REPORT {
                self.dropping = false;
            }
            return;
        }
        match (e.kind, e.code) {
            (EV_SYN, SYN_DROPPED) => self.dropping = true,
            (EV_SYN, SYN_REPORT) => {
                let p = puck_from_raw(self.raw);
                if self.sent != Some(p) {
                    self.sent = Some(p);
                    out.push(Event::State(id, State::Puck(p)));
                }
                if self.rel {
                    self.raw = [0; 6];
                }
            }
            (EV_REL | EV_ABS, c) if c < 6 => {
                self.rel = e.kind == EV_REL;
                self.raw[usize::from(c)] = e.value;
            }
            (EV_KEY, c) if e.value != 2 && (BTN_0..BTN_0 + 32).contains(&c) => {
                out.push(Event::Button {
                    id,
                    button: Button::Puck((c - BTN_0 + 1) as u8),
                    down: e.value != 0,
                });
            }
            _ => {}
        }
    }
}

/// Splits a byte stream into events (whole records only).
pub fn records(bytes: &[u8]) -> impl Iterator<Item = InputEvent> + '_ {
    bytes.chunks_exact(RECORD).map(|c| {
        let mut r = [0u8; RECORD];
        r.copy_from_slice(c);
        InputEvent::parse(&r)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::*;
    use crate::sysfs::Bits;
    use crate::{DeviceId, Pad};
    use specular_input::PadButton;

    const ID: DeviceId = DeviceId(1);

    fn run<R>(bytes: &[u8], mut f: impl FnMut(InputEvent, &mut Vec<Event>) -> R) -> Vec<Event> {
        let mut out = Vec::new();
        for e in records(bytes) {
            f(e, &mut out);
        }
        out
    }

    #[test]
    fn records_round_trip() {
        let e = InputEvent::new(EV_ABS, ABS_RX, -12345);
        assert_eq!(InputEvent::parse(&e.encode(1_234_567)), e);
        assert_eq!(feed(&[&[e]]).len(), 2 * RECORD);
    }

    #[test]
    fn xbox_style_pad_from_a_byte_stream() {
        let caps = Caps {
            abs: Bits::from_codes(&[0, 1, 2, 3, 4, 5, 0x10, 0x11]),
            ..Caps::default()
        };
        let mut r = PadReader::new(&caps);
        let a = |c, v| InputEvent::new(EV_ABS, c, v);
        let k = |c, v| InputEvent::new(EV_KEY, c, v);
        let bytes = feed(&[
            &[a(ABS_X, -32767), a(ABS_Y, -16384)],
            &[a(ABS_RZ, 255)],
            &[k(BTN_SOUTH, 1)],
            &[k(BTN_SOUTH, 2)],
            &[k(BTN_SOUTH, 0), a(ABS_HAT0X, 1)],
            &[a(ABS_HAT0X, 0), a(ABS_X, 0), a(ABS_Y, 0), a(ABS_RZ, 0)],
        ]);
        let out = run(&bytes, |e, o| r.event(e, ID, o));
        let states: Vec<Pad> = out
            .iter()
            .filter_map(|e| match e {
                Event::State(_, State::Pad(p)) => Some(*p),
                _ => None,
            })
            .collect();
        assert_eq!(states.len(), 3, "{out:?}");
        assert!((states[0].left[0] + 1.0).abs() < 1e-3);
        assert!((states[0].left[1] + 0.5).abs() < 1e-3);
        assert_eq!(states[1].rt, 1.0);
        assert_eq!(states[2], Pad::default());
        let buttons: Vec<_> = out
            .iter()
            .filter_map(|e| match e {
                Event::Button { button, down, .. } => Some((*button, *down)),
                _ => None,
            })
            .collect();
        assert_eq!(
            buttons,
            vec![
                (Button::Pad(PadButton::A), true),
                (Button::Pad(PadButton::A), false),
                (Button::Pad(PadButton::Right), true),
                (Button::Pad(PadButton::Right), false),
            ]
        );
    }

    #[test]
    fn unsigned_ranges_are_guessed() {
        let mut r = AutoRange::default();
        assert!(r.norm(128, false).abs() < 0.01);
        assert!((r.norm(255, false) - 1.0).abs() < 0.01);
        assert!((r.norm(0, false) + 1.0).abs() < 0.01);
        let mut t = AutoRange::default();
        assert_eq!(t.norm(255, true), 1.0);
        // An Xbox One trigger goes to 1023: the range grows.
        assert!((t.norm(1023, true) - 1.0).abs() < 1e-6);
        assert!((t.norm(512, true) - 0.5).abs() < 0.01);
    }

    #[test]
    fn syn_dropped_skips_to_the_next_report() {
        let caps = Caps {
            abs: Bits::from_codes(&[0, 1]),
            ..Caps::default()
        };
        let mut r = PadReader::new(&caps);
        let bytes = feed(&[
            &[
                InputEvent::new(EV_SYN, SYN_DROPPED, 0),
                InputEvent::new(EV_ABS, 0, -32767),
            ],
            &[InputEvent::new(EV_ABS, 0, -32767)],
        ]);
        let out = run(&bytes, |e, o| r.event(e, ID, o));
        assert_eq!(out.len(), 1, "{out:?}");
    }

    #[test]
    fn spacemouse_rel_frames() {
        let mut r = PuckReader::default();
        let rel = |c, v| InputEvent::new(EV_REL, c, v);
        let bytes = feed(&[
            // Pushed right and down (raw z down is +), twisted.
            &[rel(0, 350), rel(2, 175), rel(5, -350)],
            // The kernel drops zero REL values: an empty frame is the cap at rest.
            &[],
            &[InputEvent::new(EV_KEY, BTN_0 + 1, 1)],
        ]);
        let out = run(&bytes, |e, o| r.event(e, ID, o));
        let Event::State(_, State::Puck(p)) = out[0] else {
            panic!("{out:?}")
        };
        assert_eq!(p.t, [1.0, -0.5, 0.0]);
        assert_eq!(p.r, [0.0, 1.0, 0.0]);
        assert_eq!(out[1], Event::State(ID, State::Puck(Puck::default())));
        assert_eq!(
            out[2],
            Event::Button {
                id: ID,
                button: Button::Puck(2),
                down: true
            }
        );
    }
}
