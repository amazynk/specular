//! Picture Transfer Protocol (ISO 15740, PTP 1.0/1.1) for tethered capture, written from the
//! standard and the USB Still Image Capture Device Definition (PIMA 15740 USB). libgphoto2 is
//! not used (at most as a test oracle).
//!
//! - [`Camera`]: a session with its transactions (command, data phase, response), and the
//!   operations tethered capture needs: DeviceInfo, storages, object handles and info,
//!   downloads, InitiateCapture, property descriptions and values, events.
//! - [`tether`]: telling new shots from what the card held, from ObjectAdded events or by
//!   listing handles when the camera sends none; downloading into a folder.
//! - [`props`]: what exposure values mean (shutter, aperture, ISO, bias, white balance).
//! - [`Transport`]: the wire. [`usb`] is USB through `nusb`; [`mock`] is an in-memory
//!   camera for tests.
//!
//! Generic PTP only: vendor extensions (Canon EOS, Nikon, Sony) are not publicly documented,
//! so cameras that need them for remote capture or events (many Canon EOS and Sony bodies)
//! get downloads of shots taken with their own button, found by listing handles.

pub mod camera;
pub mod codes;
pub mod container;
pub mod data;
pub mod mock;
pub mod props;
pub mod tether;
pub mod transport;
pub mod usb;

pub use camera::{Camera, EXPOSURE, Error, Event, Response};
pub use data::{DeviceInfo, Form, ObjectInfo, PropDesc, StorageInfo, Value};
pub use transport::{Transport, TransportError};
