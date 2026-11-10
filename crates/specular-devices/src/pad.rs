//! A gamepad's state from Linux axis and key codes, shared by the joydev and evdev readers.
//!
//! Layout (the kernel's gamepad conventions, `Documentation/input/gamepad.rst`): left stick
//! `ABS_X`/`ABS_Y`; right stick `ABS_RX`/`ABS_RY` with the triggers on `ABS_Z`/`ABS_RZ`, or,
//! on pads without `ABS_RX`/`ABS_RY` (many generic HID pads), the right stick on
//! `ABS_Z`/`ABS_RZ` and the triggers on `ABS_BRAKE`/`ABS_GAS`; the d-pad on `ABS_HAT0X`/
//! `ABS_HAT0Y` or the `BTN_DPAD_*` keys. Face buttons by position (`BTN_SOUTH` is A).

use specular_input::PadButton;

use crate::sysfs::Bits;
use crate::{Button, Pad};

pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_Z: u16 = 0x02;
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;
pub const ABS_RZ: u16 = 0x05;
pub const ABS_GAS: u16 = 0x09;
pub const ABS_BRAKE: u16 = 0x0a;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;

pub const BTN_MISC: u16 = 0x100;
pub const BTN_JOYSTICK: u16 = 0x120;
pub const BTN_SOUTH: u16 = 0x130;
pub const BTN_EAST: u16 = 0x131;
pub const BTN_NORTH: u16 = 0x133;
pub const BTN_WEST: u16 = 0x134;
pub const BTN_TL: u16 = 0x136;
pub const BTN_TR: u16 = 0x137;
pub const BTN_TL2: u16 = 0x138;
pub const BTN_TR2: u16 = 0x139;
pub const BTN_SELECT: u16 = 0x13a;
pub const BTN_START: u16 = 0x13b;
pub const BTN_MODE: u16 = 0x13c;
pub const BTN_THUMBL: u16 = 0x13d;
pub const BTN_THUMBR: u16 = 0x13e;
pub const BTN_DPAD_UP: u16 = 0x220;
pub const BTN_DPAD_DOWN: u16 = 0x221;
pub const BTN_DPAD_LEFT: u16 = 0x222;
pub const BTN_DPAD_RIGHT: u16 = 0x223;

/// What an axis does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    LeftX,
    LeftY,
    RightX,
    RightY,
    /// Analogue triggers, 0…1.
    Lt,
    Rt,
    HatX,
    HatY,
    None,
}

impl Role {
    pub fn is_trigger(self) -> bool {
        matches!(self, Role::Lt | Role::Rt)
    }
}

/// Which axis codes play which part, from the axes a device has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The right stick is on `ABS_RX`/`ABS_RY` (else on `ABS_Z`/`ABS_RZ`).
    rx_ry: bool,
}

impl Layout {
    pub fn from_abs(abs: &Bits) -> Layout {
        Layout {
            rx_ry: abs.has(ABS_RX) && abs.has(ABS_RY),
        }
    }

    pub fn role(self, code: u16) -> Role {
        match code {
            ABS_X => Role::LeftX,
            ABS_Y => Role::LeftY,
            ABS_RX if self.rx_ry => Role::RightX,
            ABS_RY if self.rx_ry => Role::RightY,
            ABS_Z if self.rx_ry => Role::Lt,
            ABS_RZ if self.rx_ry => Role::Rt,
            ABS_Z => Role::RightX,
            ABS_RZ => Role::RightY,
            ABS_BRAKE if !self.rx_ry => Role::Lt,
            ABS_GAS if !self.rx_ry => Role::Rt,
            ABS_HAT0X => Role::HatX,
            ABS_HAT0Y => Role::HatY,
            _ => Role::None,
        }
    }
}

/// The button of a key code (`BTN_TL2`/`BTN_TR2` are digital triggers, handled apart).
pub fn button_of(code: u16) -> Option<PadButton> {
    Some(match code {
        BTN_SOUTH => PadButton::A,
        BTN_EAST => PadButton::B,
        BTN_WEST => PadButton::X,
        BTN_NORTH => PadButton::Y,
        BTN_TL => PadButton::Lb,
        BTN_TR => PadButton::Rb,
        BTN_THUMBL => PadButton::Ls,
        BTN_THUMBR => PadButton::Rs,
        BTN_SELECT => PadButton::Back,
        BTN_START => PadButton::Start,
        BTN_MODE => PadButton::Guide,
        BTN_DPAD_UP => PadButton::Up,
        BTN_DPAD_DOWN => PadButton::Down,
        BTN_DPAD_LEFT => PadButton::Left,
        BTN_DPAD_RIGHT => PadButton::Right,
        // Joysticks: the trigger and the next buttons stand for A, B, X and Y.
        0x120 => PadButton::A,
        0x121 => PadButton::B,
        0x122 => PadButton::X,
        0x123 => PadButton::Y,
        _ => return None,
    })
}

/// A gamepad being read: the state so far and the d-pad hat's last position.
#[derive(Clone, Debug, Default)]
pub struct Assembler {
    pub pad: Pad,
    hat: [i8; 2],
    /// Analogue triggers seen (digital trigger buttons then don't override them).
    analog_lt: bool,
    analog_rt: bool,
}

impl Assembler {
    /// An axis in its normalised form: sticks −1…1, triggers 0…1, hats −1, 0 or 1. Returns
    /// the d-pad buttons a hat move pressed or let go.
    pub fn axis(&mut self, role: Role, v: f32) -> Vec<(Button, bool)> {
        let p = &mut self.pad;
        match role {
            Role::LeftX => p.left[0] = v.clamp(-1.0, 1.0),
            Role::LeftY => p.left[1] = v.clamp(-1.0, 1.0),
            Role::RightX => p.right[0] = v.clamp(-1.0, 1.0),
            Role::RightY => p.right[1] = v.clamp(-1.0, 1.0),
            Role::Lt => {
                self.analog_lt = true;
                p.lt = v.clamp(0.0, 1.0);
            }
            Role::Rt => {
                self.analog_rt = true;
                p.rt = v.clamp(0.0, 1.0);
            }
            Role::HatX | Role::HatY => {
                let i = usize::from(role == Role::HatY);
                let new = if v < -0.5 {
                    -1
                } else if v > 0.5 {
                    1
                } else {
                    0
                };
                let old = std::mem::replace(&mut self.hat[i], new);
                let (neg, pos) = if i == 0 {
                    (PadButton::Left, PadButton::Right)
                } else {
                    (PadButton::Up, PadButton::Down)
                };
                let mut out = Vec::new();
                let b = |d: i8| if d < 0 { neg } else { pos };
                if old != new {
                    if old != 0 {
                        out.push((Button::Pad(b(old)), false));
                    }
                    if new != 0 {
                        out.push((Button::Pad(b(new)), true));
                    }
                }
                return out;
            }
            Role::None => {}
        }
        Vec::new()
    }

    /// A key code went down or up: the button it is, if any (digital triggers set the
    /// trigger instead).
    pub fn key(&mut self, code: u16, down: bool) -> Option<(Button, bool)> {
        let v = if down { 1.0 } else { 0.0 };
        match code {
            BTN_TL2 => {
                if !self.analog_lt {
                    self.pad.lt = v;
                }
                None
            }
            BTN_TR2 => {
                if !self.analog_rt {
                    self.pad.rt = v;
                }
                None
            }
            c => button_of(c).map(|b| (Button::Pad(b), down)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        let xbox = Layout::from_abs(&Bits::from_codes(&[0, 1, 2, 3, 4, 5, 0x10, 0x11]));
        assert_eq!(xbox.role(ABS_RX), Role::RightX);
        assert_eq!(xbox.role(ABS_Z), Role::Lt);
        assert_eq!(xbox.role(ABS_RZ), Role::Rt);
        let generic = Layout::from_abs(&Bits::from_codes(&[0, 1, 2, 5, 9, 10]));
        assert_eq!(generic.role(ABS_Z), Role::RightX);
        assert_eq!(generic.role(ABS_RZ), Role::RightY);
        assert_eq!(generic.role(ABS_GAS), Role::Rt);
        assert_eq!(generic.role(ABS_BRAKE), Role::Lt);
    }

    #[test]
    fn hat_presses_and_releases() {
        let mut a = Assembler::default();
        assert_eq!(
            a.axis(Role::HatX, 1.0),
            vec![(Button::Pad(PadButton::Right), true)]
        );
        assert_eq!(
            a.axis(Role::HatX, -1.0),
            vec![
                (Button::Pad(PadButton::Right), false),
                (Button::Pad(PadButton::Left), true)
            ]
        );
        assert_eq!(
            a.axis(Role::HatX, 0.0),
            vec![(Button::Pad(PadButton::Left), false)]
        );
        assert!(a.axis(Role::HatX, 0.0).is_empty());
    }

    #[test]
    fn digital_triggers_only_without_analogue() {
        let mut a = Assembler::default();
        assert_eq!(a.key(BTN_TR2, true), None);
        assert_eq!(a.pad.rt, 1.0);
        a.axis(Role::Rt, 0.25);
        a.key(BTN_TR2, false);
        assert_eq!(a.pad.rt, 0.25);
        assert_eq!(
            a.key(BTN_SOUTH, true),
            Some((Button::Pad(PadButton::A), true))
        );
        assert_eq!(
            a.key(BTN_NORTH, true),
            Some((Button::Pad(PadButton::Y), true))
        );
        assert_eq!(
            a.key(BTN_WEST, true),
            Some((Button::Pad(PadButton::X), true))
        );
    }
}
