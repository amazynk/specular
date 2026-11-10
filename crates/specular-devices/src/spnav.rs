//! spacenavd's socket (`/var/run/spnav.sock`, a Unix stream socket), protocol 0: the one
//! every spacenavd speaks, used until a client asks for another. The daemon sends fixed
//! 32-byte packets of eight native-endian `int`s:
//!
//! - motion: `0, x, y, z, rx, ry, rz, period` (period: milliseconds since the last one);
//! - button pressed: `1, number, …`; released: `2, number, …` (numbers from 0).
//!
//! Axes are in spacenavd's frame (x right, y up, z towards the user), about ±350 at full
//! deflection after its sensitivity. The client sends nothing.

use crate::evdev::PUCK_SCALE;
use crate::{Button, DeviceId, Event, Puck, State};

/// Where spacenavd listens.
pub const SOCKET: &str = "/var/run/spnav.sock";
pub const PACKET: usize = 32;

const UEV_MOTION: i32 = 0;
const UEV_PRESS: i32 = 1;
const UEV_RELEASE: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packet {
    Motion {
        t: [i32; 3],
        r: [i32; 3],
        period: i32,
    },
    Button {
        number: i32,
        down: bool,
    },
}

impl Packet {
    pub fn parse(p: &[u8; PACKET]) -> Option<Packet> {
        let w = |i: usize| i32::from_ne_bytes([p[4 * i], p[4 * i + 1], p[4 * i + 2], p[4 * i + 3]]);
        match w(0) {
            UEV_MOTION => Some(Packet::Motion {
                t: [w(1), w(2), w(3)],
                r: [w(4), w(5), w(6)],
                period: w(7),
            }),
            UEV_PRESS | UEV_RELEASE => Some(Packet::Button {
                number: w(1),
                down: w(0) == UEV_PRESS,
            }),
            _ => None,
        }
    }

    /// As spacenavd sends it (fake daemons).
    pub fn encode(&self) -> [u8; PACKET] {
        let words: [i32; 8] = match *self {
            Packet::Motion { t, r, period } => {
                [UEV_MOTION, t[0], t[1], t[2], r[0], r[1], r[2], period]
            }
            Packet::Button { number, down } => [
                if down { UEV_PRESS } else { UEV_RELEASE },
                number,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
        };
        let mut out = [0u8; PACKET];
        for (i, w) in words.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&w.to_ne_bytes());
        }
        out
    }

    /// The events it stands for.
    pub fn event(&self, id: DeviceId) -> Option<Event> {
        match *self {
            Packet::Motion { t, r, .. } => {
                let n = |v: i32| (v as f32 / PUCK_SCALE).clamp(-1.0, 1.0);
                Some(Event::State(
                    id,
                    State::Puck(Puck {
                        t: t.map(n),
                        r: r.map(n),
                    }),
                ))
            }
            Packet::Button { number, down } => {
                let n = u8::try_from(number.checked_add(1)?)
                    .ok()
                    .filter(|n| *n >= 1)?;
                Some(Event::Button {
                    id,
                    button: Button::Puck(n),
                    down,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_round_trip() {
        let m = Packet::Motion {
            t: [350, -175, 0],
            r: [0, 700, -1],
            period: 16,
        };
        assert_eq!(Packet::parse(&m.encode()), Some(m));
        let Some(Event::State(_, State::Puck(p))) = m.event(DeviceId(1)) else {
            panic!()
        };
        assert_eq!(p.t, [1.0, -0.5, 0.0]);
        assert_eq!(p.r[1], 1.0);
        let b = Packet::Button {
            number: 1,
            down: false,
        };
        assert_eq!(Packet::parse(&b.encode()), Some(b));
        assert_eq!(
            b.event(DeviceId(1)),
            Some(Event::Button {
                id: DeviceId(1),
                button: Button::Puck(2),
                down: false
            })
        );
        let mut junk = [0u8; PACKET];
        junk[..4].copy_from_slice(&9i32.to_ne_bytes());
        assert_eq!(Packet::parse(&junk), None);
        assert_eq!(
            Packet::Button {
                number: -1,
                down: true
            }
            .event(DeviceId(1)),
            None
        );
    }
}
