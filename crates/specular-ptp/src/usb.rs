//! Cameras on USB (`nusb`): the Still Image class interface (class 6, subclass 1, protocol 1)
//! with its bulk-IN, bulk-OUT and interrupt-IN endpoints.
//!
//! Opening is where the system gets in the way, and [`OpenError`] says how:
//! - **Linux**: the device node `/dev/bus/usb/BBB/DDD` is the user's only through a udev
//!   rule ([`udev_rule`]); a desktop's camera service (gvfs's gphoto2 monitor, KDE's
//!   kamera) or another program can hold the interface ([`OpenError::Busy`] names it, read
//!   from `/proc`).
//! - **macOS**: the system's PTP service (`ptpcamerad`) claims cameras for Photos and Image
//!   Capture.
//! - **Windows**: cameras get the system's WPD driver, which programs can't open through
//!   WinUSB.
//!
//! Nothing is changed on the system: the error carries what the user would do.

use std::time::Duration;

use nusb::descriptors::TransferType;
use nusb::transfer::{
    Buffer, Bulk, ControlOut, ControlType, In, Interrupt, Out, Recipient, TransferError,
};
use nusb::{Endpoint, ErrorKind, Interface, MaybeFuture};

use crate::transport::{Transport, TransportError};

/// The Still Image class.
const CLASS: (u8, u8, u8) = (6, 1, 1);

/// A camera found on USB.
#[derive(Clone, Debug)]
pub struct Found {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub interface: u8,
    /// Stays the same while the camera stays plugged in.
    pub key: String,
    /// Linux: the device node.
    pub node: Option<std::path::PathBuf>,
    info: nusb::DeviceInfo,
}

impl Found {
    /// The name the USB descriptors give (the PTP model name comes with the session).
    pub fn name(&self) -> String {
        match (&self.manufacturer, &self.product) {
            (_, Some(p)) if !p.trim().is_empty() => p.trim().to_owned(),
            (Some(m), _) => m.trim().to_owned(),
            _ => format!("{:04x}:{:04x}", self.vendor_id, self.product_id),
        }
    }
}

/// Why a camera can't be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// The user may not open the device. Linux: `rule` is the udev rule that allows it.
    Permission { node: Option<String>, rule: String },
    /// Another program has the camera; `holders` are the names of those found.
    Busy { holders: Vec<String> },
    /// The camera went away.
    Gone,
    /// The system's driver for it doesn't let programs in (Windows).
    Driver,
    /// Anything else, as the system put it.
    Other(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Permission { node, .. } => {
                write!(f, "permission denied")?;
                if let Some(n) = node {
                    write!(f, " on {n}")?;
                }
                Ok(())
            }
            OpenError::Busy { holders } if holders.is_empty() => {
                write!(f, "in use by another program")
            }
            OpenError::Busy { holders } => write!(f, "in use by {}", holders.join(", ")),
            OpenError::Gone => write!(f, "disconnected"),
            OpenError::Driver => write!(f, "the system driver does not allow access"),
            OpenError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for OpenError {}

/// The udev rule giving the logged-in user the camera `vendor:product`, for
/// `/etc/udev/rules.d/70-specular-camera.rules`.
pub fn udev_rule(vendor: u16, product: u16) -> String {
    format!(
        "SUBSYSTEM==\"usb\", ATTR{{idVendor}}==\"{vendor:04x}\", ATTR{{idProduct}}==\"{product:04x}\", MODE=\"0660\", TAG+=\"uaccess\""
    )
}

/// Where the rule goes.
pub const UDEV_RULE_FILE: &str = "/etc/udev/rules.d/70-specular-camera.rules";

/// The cameras plugged in now.
pub fn cameras() -> Result<Vec<Found>, OpenError> {
    let list = nusb::list_devices()
        .wait()
        .map_err(|e| OpenError::Other(e.to_string()))?;
    let mut out = Vec::new();
    for d in list {
        let Some(i) = d
            .interfaces()
            .find(|i| (i.class(), i.subclass(), i.protocol()) == CLASS)
        else {
            continue;
        };
        #[cfg(target_os = "linux")]
        let (key, node) = (
            d.sysfs_path().display().to_string(),
            Some(std::path::PathBuf::from(format!(
                "/dev/bus/usb/{:03}/{:03}",
                d.busnum(),
                d.device_address()
            ))),
        );
        #[cfg(not(target_os = "linux"))]
        let (key, node) = (format!("{:?}", d.id()), None);
        out.push(Found {
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            manufacturer: d.manufacturer_string().map(str::to_owned),
            product: d.product_string().map(str::to_owned),
            serial: d.serial_number().map(str::to_owned),
            interface: i.interface_number(),
            key,
            node,
            info: d,
        });
    }
    Ok(out)
}

fn open_error(e: &nusb::Error, f: &Found) -> OpenError {
    match e.kind() {
        ErrorKind::PermissionDenied => OpenError::Permission {
            node: f.node.as_ref().map(|n| n.display().to_string()),
            rule: udev_rule(f.vendor_id, f.product_id),
        },
        ErrorKind::Busy => OpenError::Busy {
            holders: holders(f),
        },
        ErrorKind::Disconnected | ErrorKind::NotFound => OpenError::Gone,
        ErrorKind::Unsupported if cfg!(windows) => OpenError::Driver,
        _ if cfg!(windows) => OpenError::Driver,
        _ => OpenError::Other(e.to_string()),
    }
}

/// The programs with the camera's device node open (Linux, from `/proc`; only processes of
/// this user can be seen).
pub fn holders(f: &Found) -> Vec<String> {
    let Some(node) = &f.node else {
        return Vec::new();
    };
    let me = std::process::id().to_string();
    let mut out: Vec<String> = Vec::new();
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return out;
    };
    for p in procs.flatten() {
        let pid = p.file_name();
        let pid = pid.to_string_lossy();
        if !pid.bytes().all(|b| b.is_ascii_digit()) || pid == me {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(p.path().join("fd")) else {
            continue;
        };
        let holds = fds
            .flatten()
            .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|l| l == *node));
        if holds {
            let comm = std::fs::read_to_string(p.path().join("comm")).unwrap_or_default();
            let name = comm.trim().to_owned();
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// A camera's PTP interface, claimed.
pub struct UsbTransport {
    iface: Interface,
    bulk_in: Endpoint<Bulk, In>,
    bulk_out: Endpoint<Bulk, Out>,
    intr: Option<Endpoint<Interrupt, In>>,
    ps_in: usize,
    ps_out: usize,
    intr_size: usize,
}

/// Opens and claims the camera's PTP interface.
pub fn open(f: &Found) -> Result<UsbTransport, OpenError> {
    let dev = f.info.open().wait().map_err(|e| open_error(&e, f))?;
    let iface = dev
        .claim_interface(f.interface)
        .wait()
        .map_err(|e| open_error(&e, f))?;
    let desc = iface
        .descriptor()
        .ok_or_else(|| OpenError::Other("no interface descriptor".into()))?;
    let (mut bi, mut bo, mut ii) = (None, None, None);
    for ep in desc.endpoints() {
        let input = ep.address() & 0x80 != 0;
        match (ep.transfer_type(), input) {
            (TransferType::Bulk, true) if bi.is_none() => bi = Some(ep.address()),
            (TransferType::Bulk, false) if bo.is_none() => bo = Some(ep.address()),
            (TransferType::Interrupt, true) if ii.is_none() => ii = Some(ep.address()),
            _ => {}
        }
    }
    let (Some(bi), Some(bo)) = (bi, bo) else {
        return Err(OpenError::Other("the camera has no bulk endpoints".into()));
    };
    let err = |e: nusb::Error| open_error(&e, f);
    let bulk_in = iface.endpoint::<Bulk, In>(bi).map_err(err)?;
    let bulk_out = iface.endpoint::<Bulk, Out>(bo).map_err(err)?;
    let intr = ii.and_then(|a| iface.endpoint::<Interrupt, In>(a).ok());
    let ps_in = bulk_in.max_packet_size().max(8);
    let ps_out = bulk_out.max_packet_size().max(8);
    let intr_size = intr
        .as_ref()
        .map_or(64, |e| e.max_packet_size().max(8))
        .max(64)
        .div_ceil(8)
        * 8;
    Ok(UsbTransport {
        iface,
        bulk_in,
        bulk_out,
        intr,
        ps_in,
        ps_out,
        intr_size,
    })
}

fn transfer_error(e: TransferError) -> TransportError {
    match e {
        // `transfer_blocking` cancels on its timeout.
        TransferError::Cancelled => TransportError::Timeout,
        TransferError::Stall => TransportError::Stall,
        TransferError::Disconnected => TransportError::Disconnected,
        e => TransportError::Io(e.to_string()),
    }
}

impl Transport for UsbTransport {
    fn send(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), TransportError> {
        let c = self
            .bulk_out
            .transfer_blocking(Buffer::from(bytes.to_vec()), timeout);
        c.status.map_err(transfer_error)?;
        if bytes.len().is_multiple_of(self.ps_out) {
            let c = self.bulk_out.transfer_blocking(Buffer::new(0), timeout);
            c.status.map_err(transfer_error)?;
        }
        Ok(())
    }

    fn receive(&mut self, max: usize, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        let len = max.max(self.ps_in).div_ceil(self.ps_in) * self.ps_in;
        let buf = self.bulk_in.allocate(len);
        let c = self.bulk_in.transfer_blocking(buf, timeout);
        c.status.map_err(transfer_error)?;
        Ok(c.buffer[..c.actual_len.min(c.buffer.len())].to_vec())
    }

    fn event(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        let Some(ep) = self.intr.as_mut() else {
            if !timeout.is_zero() {
                // No interrupt endpoint: wait as asked, so callers pace themselves.
                std::thread::sleep(timeout.min(Duration::from_millis(200)));
            }
            return Ok(None);
        };
        // One transfer stays pending between calls.
        if ep.pending() == 0 {
            let b = ep.allocate(self.intr_size);
            ep.submit(b);
        }
        match ep.wait_next_complete(timeout) {
            None => Ok(None),
            Some(c) => match c.status {
                Ok(()) => Ok(Some(c.buffer[..c.actual_len.min(c.buffer.len())].to_vec())),
                Err(TransferError::Cancelled) => Ok(None),
                Err(e) => Err(transfer_error(e)),
            },
        }
    }

    fn packet_size(&self) -> usize {
        self.ps_in
    }

    fn cancel(&mut self, tid: u32) -> Result<(), TransportError> {
        // Still Image class Cancel Request (bRequest 0x64): the cancel event code and the id.
        let mut d = [0u8; 6];
        d[..2].copy_from_slice(&0x4001u16.to_le_bytes());
        d[2..].copy_from_slice(&tid.to_le_bytes());
        let n = self.iface.interface_number();
        self.iface
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: 0x64,
                    value: 0,
                    index: n as u16,
                    data: &d,
                },
                Duration::from_secs(2),
            )
            .wait()
            .map_err(transfer_error)?;
        self.bulk_in.cancel_all();
        Ok(())
    }

    fn clear_stall(&mut self) -> Result<(), TransportError> {
        let a = self.bulk_in.clear_halt().wait();
        let b = self.bulk_out.clear_halt().wait();
        a.and(b).map_err(|e| TransportError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_text() {
        assert_eq!(
            udev_rule(0x04a9, 0x32b4),
            "SUBSYSTEM==\"usb\", ATTR{idVendor}==\"04a9\", ATTR{idProduct}==\"32b4\", MODE=\"0660\", TAG+=\"uaccess\""
        );
    }

    /// Listing works without a camera and without any permission (sysfs on Linux).
    #[test]
    fn listing_does_not_need_a_camera() {
        let found = cameras().expect("list USB devices");
        for f in &found {
            assert!(!f.name().is_empty());
        }
    }
}
