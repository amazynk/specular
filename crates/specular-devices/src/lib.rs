//! Navigation devices: gamepads (and simple joysticks) and 3Dconnexion SpaceMouse.
//!
//! Off by default (`devices.enabled`). [`Devices::start`] looks for what is connected and
//! sends [`Event`]s to a sink, from reader threads that block on the device (nothing polls
//! while a device rests, on Linux). The app turns the latest [`State`] into motion with
//! [`nav`] on each frame while a stick or the cap is off-centre, and runs device buttons
//! through the keymap ([`Button::combo`], `specular_input::device`).
//!
//! Sources:
//! - **Linux** (`linux.rs`): gamepads through the joystick interface `/dev/input/js*`
//!   ([`joydev`]: 8-byte records, axes already scaled to ±32767 by the kernel, no ioctl),
//!   or the event interface `/dev/input/event*` ([`evdev`]: `struct input_event` records,
//!   ranges guessed from the values seen) when there is no js node. Names, ids and what each
//!   device has come from sysfs ([`sysfs`]). A SpaceMouse goes through spacenavd's socket
//!   ([`spnav`]) when it runs, else its event node. `/dev/input` is watched for devices
//!   coming and going.
//! - **Windows** (`windows.rs`, compile-checked only): `Windows.Gaming.Input` (`Gamepad`, and
//!   `RawGameController` for a SpaceMouse), read at the frame rate while a stick is
//!   off-centre and four times a second otherwise.
//! - **macOS**: not yet ([`Problem::Unsupported`]): the GameController framework needs
//!   Objective-C calls this crate cannot make without `unsafe`.

pub mod evdev;
pub mod joydev;
pub mod nav;
pub mod pad;
pub mod spnav;
pub mod sysfs;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Roots;
#[cfg(windows)]
mod windows;

use std::sync::Arc;

use specular_input::{KeyCombo, PadButton};

/// A connected device, for as long as it stays connected (ids are not reused in a run).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(pub u32);

/// What a device is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Gamepad,
    /// A stick with buttons but no gamepad layout (flight sticks): the left stick only.
    Joystick,
    SpaceMouse,
}

/// How Specular reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Via {
    /// Linux `/dev/input/js*`.
    Joydev,
    /// Linux `/dev/input/event*`.
    Evdev,
    /// spacenavd's socket.
    Spacenavd,
    /// Windows.Gaming.Input.
    GamingInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    /// As the device names itself ("Xbox Wireless Controller").
    pub name: String,
    pub kind: Kind,
    pub via: Via,
    /// The device node or socket (empty where there is none).
    pub path: String,
}

/// A gamepad's sticks and triggers, normalised: sticks −1…1 with x right and y down,
/// triggers 0…1. The d-pad comes as buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    pub left: [f32; 2],
    pub right: [f32; 2],
    pub lt: f32,
    pub rt: f32,
}

/// A SpaceMouse cap, normalised to −1…1 per axis in a right-handed frame: x right, y up,
/// z towards the user; `r` turns about those axes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Puck {
    pub t: [f32; 3],
    pub r: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Pad(Pad),
    Puck(Puck),
}

/// A device button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Pad(PadButton),
    /// A SpaceMouse button, 1-based.
    Puck(u8),
}

impl Button {
    /// The bindable key it stands for (`None` past the last SpaceMouse button there is).
    pub fn combo(self) -> Option<KeyCombo> {
        match self {
            Button::Pad(b) => Some(b.combo()),
            Button::Puck(n) => specular_input::puck_combo(n),
        }
    }
}

/// Why a device cannot be used, for the Devices page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The device node exists but this user may not read it. `group` owns the node (join
    /// it to read it); `None` when the group could not be named.
    NoPermission {
        name: String,
        kind: Kind,
        path: String,
        group: Option<String>,
    },
    /// Reading stopped with an error other than the device going away.
    ReadFailed { name: String, error: String },
    /// This system has no device support yet (macOS).
    Unsupported,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Added(DeviceInfo),
    Removed(DeviceId),
    /// The device's sticks or cap now.
    State(DeviceId, State),
    Button {
        id: DeviceId,
        button: Button,
        down: bool,
    },
    Problem(Problem),
}

/// Where events go: called from reader threads (wake the UI from it).
pub type Sink = Arc<dyn Fn(Event) + Send + Sync>;

/// The device readers. Dropping it stops them (a Linux reader blocked on a device that sends
/// nothing ends with its next event).
pub struct Devices {
    #[cfg(target_os = "linux")]
    inner: linux::Backend,
    #[cfg(windows)]
    inner: windows::Backend,
}

impl Devices {
    /// Looks for devices now and as they come and go.
    pub fn start(sink: Sink) -> Devices {
        #[cfg(target_os = "linux")]
        {
            Devices {
                inner: linux::Backend::start(linux::Roots::default(), sink),
            }
        }
        #[cfg(windows)]
        {
            Devices {
                inner: windows::Backend::start(sink),
            }
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            sink(Event::Problem(Problem::Unsupported));
            Devices {}
        }
    }

    /// Linux with other places for sysfs, `/dev/input`, the spacenavd socket and the group
    /// file: fake trees in tests.
    #[cfg(target_os = "linux")]
    pub fn start_at(roots: Roots, sink: Sink) -> Devices {
        Devices {
            inner: linux::Backend::start(roots, sink),
        }
    }

    /// Looks again now (Settings › Devices › Look again).
    pub fn rescan(&self) {
        #[cfg(any(target_os = "linux", windows))]
        self.inner.rescan();
    }
}

impl std::fmt::Debug for Devices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Devices")
    }
}
