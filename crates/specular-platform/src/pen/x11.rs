//! Pen pressure on X11: XInput 2 `RawMotion` / `RawButtonPress` / `RawButtonRelease` on the
//! root window over our own x11rb connection, in safe Rust.
//!
//! Raw events carry each changed valuator of the slave device that moved. A device counts as a
//! pen when it is a slave pointer with a valuator labelled "Abs Pressure" (the label the
//! libinput and wacom drivers give it); its range comes from `XIQueryDevice`. Devices are
//! looked up when first seen and forgotten when the device hierarchy changes.
//!
//! A background thread waits for events; dropping the [`Listener`] sends a client message to a
//! small window of our own to wake it, then joins it (as the pinch listener does).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use x11rb::connection::{Connection, RequestConnection as _};
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{
    self, ChangeReason, ConnectionExt as _, Device, DeviceClassData, DeviceType,
    EventMask as XiEventMask, Fp3232, RawButtonPressEvent, XIEventMask,
};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, Window,
    WindowClass,
};
use x11rb::rust_connection::RustConnection;

use super::Sink;

pub(super) struct Listener {
    conn: Arc<RustConnection>,
    /// Our own input-only window, the target of the wake-up message.
    wake: Window,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl Listener {
    pub(super) fn new(sink: Sink) -> Result<Listener, String> {
        Self::setup(None, sink).map_err(|e| e.to_string())
    }

    fn setup(display: Option<&str>, sink: Sink) -> Res<Listener> {
        let (conn, screen) = RustConnection::connect(display)?;
        if conn
            .extension_information(xinput::X11_EXTENSION_NAME)?
            .is_none()
        {
            return Err("the X server has no XInput".into());
        }
        let v = conn.xinput_xi_query_version(2, 2)?.reply()?;
        if v.major_version < 2 {
            return Err(format!(
                "the X server has XInput {}.{}; pressure needs 2.0",
                v.major_version, v.minor_version
            )
            .into());
        }
        let label = conn.intern_atom(true, b"Abs Pressure")?.reply()?.atom;
        let root = conn.setup().roots[screen].root;
        let wake = conn.generate_id()?;
        conn.create_window(
            0,
            wake,
            root,
            -1,
            -1,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new(),
        )?
        .check()?;
        conn.xinput_xi_select_events(
            root,
            &[XiEventMask {
                deviceid: u16::from(Device::ALL),
                mask: vec![
                    XIEventMask::RAW_MOTION
                        | XIEventMask::RAW_BUTTON_PRESS
                        | XIEventMask::RAW_BUTTON_RELEASE
                        | XIEventMask::HIERARCHY
                        | XIEventMask::DEVICE_CHANGED,
                ],
            }],
        )?
        .check()?;
        conn.flush()?;
        specular_log::debug!(target: specular_log::INPUT, "pen pressure: XInput {}.{} raw events", v.major_version, v.minor_version);
        let conn = Arc::new(conn);
        let stop = Arc::new(AtomicBool::new(false));
        let (c, st) = (conn.clone(), stop.clone());
        let sink = Arc::new(sink);
        let thread = specular_thread::builder("x11-pen").spawn(move || {
            specular_log::service::supervise(
                "x11-pen",
                specular_log::service::Policy::RESTART,
                || st.load(Ordering::Acquire),
                || run(&c, label, &sink, &st),
                |_| {},
            );
        })?;
        Ok(Listener {
            conn,
            wake,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let ev = ClientMessageEvent::new(32, self.wake, AtomEnum::INTEGER, [0u32; 5]);
        let _ = self
            .conn
            .send_event(false, self.wake, EventMask::NO_EVENT, ev);
        let _ = self.conn.flush();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = self.conn.destroy_window(self.wake);
        let _ = self.conn.flush();
    }
}

/// A pen's pressure valuator: its number and range.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Axis {
    number: u16,
    min: f64,
    max: f64,
}

fn fp(v: Fp3232) -> f64 {
    f64::from(v.integral) + f64::from(v.frac) / 4_294_967_296.0
}

fn run(c: &RustConnection, label: Atom, sink: &Sink, stop: &AtomicBool) {
    // Per device: its pressure axis, or `None` for devices without one.
    let mut devices: HashMap<u16, Option<Axis>> = HashMap::new();
    let mut last: Option<f32> = None;
    while !stop.load(Ordering::Acquire) {
        let ev = match c.wait_for_event() {
            Ok(ev) => ev,
            Err(e) => {
                specular_log::debug!(target: specular_log::INPUT, "pen pressure: X connection lost: {e}");
                break;
            }
        };
        specular_log::service::trip();
        let raw = match &ev {
            Event::XinputRawMotion(e)
            | Event::XinputRawButtonPress(e)
            | Event::XinputRawButtonRelease(e) => e,
            Event::XinputHierarchy(_) => {
                devices.clear();
                continue;
            }
            Event::XinputDeviceChanged(e) if e.reason == ChangeReason::DEVICE_CHANGE => {
                devices.remove(&e.deviceid);
                devices.remove(&e.sourceid);
                continue;
            }
            _ => continue,
        };
        let axis = *devices
            .entry(raw.deviceid)
            .or_insert_with(|| query(c, raw.deviceid, label));
        let Some(axis) = axis else { continue };
        if let Some(p) = pressure(raw, axis)
            && last != Some(p)
        {
            last = Some(p);
            sink(p);
        }
    }
}

/// The pressure axis of `device`, if it is a slave pointer with one.
fn query(c: &RustConnection, device: u16, label: Atom) -> Option<Axis> {
    let reply = c.xinput_xi_query_device(device).ok()?.reply().ok()?;
    let info = reply.infos.into_iter().find(|d| d.deviceid == device)?;
    let axis = pen_axis(info.type_, &info.classes, label);
    specular_log::debug!(
        target: specular_log::INPUT,
        "pen pressure: device {device} {:?}: {axis:?}",
        String::from_utf8_lossy(&info.name)
    );
    axis
}

/// The valuator labelled `label` of a slave pointer, with a usable range.
fn pen_axis(ty: DeviceType, classes: &[xinput::DeviceClass], label: Atom) -> Option<Axis> {
    if label == u32::from(AtomEnum::NONE)
        || !matches!(ty, DeviceType::SLAVE_POINTER | DeviceType::FLOATING_SLAVE)
    {
        return None;
    }
    classes.iter().find_map(|cl| match &cl.data {
        DeviceClassData::Valuator(v) if v.label == label => {
            let (min, max) = (fp(v.min), fp(v.max));
            (max > min && min.is_finite() && max.is_finite()).then_some(Axis {
                number: v.number,
                min,
                max,
            })
        }
        _ => None,
    })
}

/// The pen's pressure (0 to 1) in a raw event, if the event carries that valuator. Raw events
/// list only the valuators set in the mask, in order.
fn pressure(ev: &RawButtonPressEvent, axis: Axis) -> Option<f32> {
    let n = usize::from(axis.number);
    let (word, bit) = (n / 32, n % 32);
    let mask = *ev.valuator_mask.get(word)?;
    if mask & (1 << bit) == 0 {
        return None;
    }
    let before: u32 = ev.valuator_mask[..word]
        .iter()
        .map(|m| m.count_ones())
        .sum::<u32>()
        + (mask & ((1u32 << bit) - 1)).count_ones();
    let v = fp(*ev.axisvalues.get(usize::try_from(before).ok()?)?);
    let p = ((v - axis.min) / (axis.max - axis.min)).clamp(0.0, 1.0) as f32;
    p.is_finite().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use x11rb::protocol::xinput::{DeviceClass, DeviceClassDataValuator, ValuatorMode};

    fn fx(v: f64) -> Fp3232 {
        Fp3232 {
            integral: v.floor() as i32,
            frac: ((v - v.floor()) * 4_294_967_296.0) as u32,
        }
    }

    fn raw(mask: Vec<u32>, values: &[f64]) -> RawButtonPressEvent {
        RawButtonPressEvent {
            response_type: 35,
            extension: 0,
            sequence: 0,
            length: 0,
            event_type: xinput::RAW_MOTION_EVENT,
            deviceid: 11,
            time: 0,
            detail: 0,
            sourceid: 11,
            flags: Default::default(),
            valuator_mask: mask,
            axisvalues: values.iter().map(|&v| fx(v)).collect(),
            axisvalues_raw: values.iter().map(|&v| fx(v)).collect(),
        }
    }

    const PEN: Axis = Axis {
        number: 2,
        min: 0.0,
        max: 65536.0,
    };

    #[test]
    fn pressure_is_found_among_the_set_valuators() {
        // x, y and pressure (axes 0, 1, 2).
        let e = raw(vec![0b111], &[100.0, 200.0, 16384.0]);
        assert_eq!(pressure(&e, PEN), Some(0.25));
        // Only pressure and tilt changed (axes 2, 3).
        let e = raw(vec![0b1100], &[49152.0, 10.0]);
        assert_eq!(pressure(&e, PEN), Some(0.75));
        // Motion without a pressure change.
        let e = raw(vec![0b11], &[100.0, 200.0]);
        assert_eq!(pressure(&e, PEN), None);
        // An empty mask or one cut short.
        assert_eq!(pressure(&raw(vec![], &[]), PEN), None);
        assert_eq!(pressure(&raw(vec![0b100], &[]), PEN), None);
    }

    #[test]
    fn pressure_on_a_high_axis_counts_earlier_mask_words() {
        let axis = Axis {
            number: 33,
            min: 100.0,
            max: 200.0,
        };
        let e = raw(vec![0b1, 0b11], &[1.0, 2.0, 150.0]);
        assert_eq!(pressure(&e, axis), Some(0.5));
        // Out of range is clamped.
        let e = raw(vec![0, 0b10], &[400.0]);
        assert_eq!(pressure(&e, axis), Some(1.0));
    }

    fn valuator(number: u16, label: Atom, min: f64, max: f64) -> DeviceClass {
        DeviceClass {
            len: 0,
            sourceid: 11,
            data: DeviceClassData::Valuator(DeviceClassDataValuator {
                number,
                label,
                min: fx(min),
                max: fx(max),
                value: fx(0.0),
                resolution: 0,
                mode: ValuatorMode::ABSOLUTE,
            }),
        }
    }

    /// On a private X server named by `SPECULAR_TEST_X11_DISPLAY` (e.g. Xvfb `:93`; never the
    /// desktop's): the listener selects raw events, sees the server's devices (none is a pen),
    /// and stops promptly. Skipped without the variable.
    #[test]
    fn listener_starts_and_stops_on_a_live_server() {
        let Some(display) = std::env::var_os("SPECULAR_TEST_X11_DISPLAY") else {
            return;
        };
        let display = display.to_string_lossy().into_owned();
        let sink: Sink = Box::new(|p| panic!("no pen on a virtual server, got {p}"));
        let l = Listener::setup(Some(&display), sink).expect("listener");
        // The server's own pointer and keyboard are no pens.
        let c = &l.conn;
        let label = c
            .intern_atom(true, b"Abs Pressure")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let all = c
            .xinput_xi_query_device(u16::from(Device::ALL))
            .unwrap()
            .reply()
            .unwrap();
        assert!(!all.infos.is_empty());
        for d in &all.infos {
            assert_eq!(query(c, d.deviceid, label), None);
        }
        let t = std::time::Instant::now();
        drop(l);
        assert!(t.elapsed().as_secs() < 2, "{:?}", t.elapsed());
    }

    #[test]
    fn only_slave_pointers_with_a_pressure_valuator_are_pens() {
        const ABS_X: Atom = 300;
        const PRESSURE: Atom = 301;
        let classes = [
            valuator(0, ABS_X, 0.0, 1000.0),
            valuator(2, PRESSURE, 0.0, 65536.0),
        ];
        assert_eq!(
            pen_axis(DeviceType::SLAVE_POINTER, &classes, PRESSURE),
            Some(PEN)
        );
        assert_eq!(
            pen_axis(DeviceType::MASTER_POINTER, &classes, PRESSURE),
            None,
            "master pointers repeat the slave's events"
        );
        assert_eq!(
            pen_axis(DeviceType::SLAVE_POINTER, &classes[..1], PRESSURE),
            None
        );
        // An empty range is not a pressure axis.
        let flat = [valuator(2, PRESSURE, 5.0, 5.0)];
        assert_eq!(pen_axis(DeviceType::SLAVE_POINTER, &flat, PRESSURE), None);
        // A server without the atom.
        assert_eq!(pen_axis(DeviceType::SLAVE_POINTER, &classes, 0), None);
    }
}
