//! A PTP session with one camera (ISO 15740 §9, §10): transactions with their data phases,
//! and the operations tethered capture uses.

use std::io::Write;
use std::time::Duration;

use crate::codes::{ALL, container, ev, op, prop, rc};
use crate::container::{HEADER, Header, data, params, read_params};
use crate::data::{
    DeviceInfo, Malformed, ObjectInfo, PropDesc, Reader, StorageInfo, Value, Writer,
};
use crate::transport::{Transport, TransportError};

/// The session id this crate opens.
pub const SESSION_ID: u32 = 1;

/// The properties tethered capture shows and changes, in the panel's order.
pub const EXPOSURE: [u16; 6] = [
    prop::EXPOSURE_PROGRAM_MODE,
    prop::EXPOSURE_TIME,
    prop::F_NUMBER,
    prop::EXPOSURE_INDEX,
    prop::EXPOSURE_BIAS_COMPENSATION,
    prop::WHITE_BALANCE,
];

/// Why an operation failed.
#[derive(Debug)]
pub enum Error {
    Transport(TransportError),
    /// The camera answered `code` (a response code other than OK) to operation `op`.
    Response {
        op: u16,
        code: u16,
    },
    /// The camera does not list the operation in its DeviceInfo.
    Unsupported(u16),
    /// A dataset the camera sent does not parse.
    Malformed(Malformed),
    /// The camera broke the protocol (a container out of place, a wrong transaction id).
    Protocol(&'static str),
    /// Writing the downloaded data failed.
    Write(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Transport(e) => write!(f, "{e}"),
            Error::Response { op, code } => match rc::name(*code) {
                Some(n) => write!(f, "operation {op:#06x}: {n}"),
                None => write!(f, "operation {op:#06x}: response {code:#06x}"),
            },
            Error::Unsupported(op) => write!(f, "the camera does not support operation {op:#06x}"),
            Error::Malformed(m) => write!(f, "{m}"),
            Error::Protocol(p) => write!(f, "protocol error: {p}"),
            Error::Write(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<TransportError> for Error {
    fn from(e: TransportError) -> Error {
        Error::Transport(e)
    }
}

impl From<Malformed> for Error {
    fn from(e: Malformed) -> Error {
        Error::Malformed(e)
    }
}

impl Error {
    /// The camera's response code, for a refused operation.
    pub fn code(&self) -> Option<u16> {
        match self {
            Error::Response { code, .. } => Some(*code),
            _ => None,
        }
    }

    /// The camera is gone: the session can't go on.
    pub fn is_gone(&self) -> bool {
        matches!(self, Error::Transport(TransportError::Disconnected))
    }

    /// Worth trying again shortly (busy, or a timeout).
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            Error::Response {
                code: rc::DEVICE_BUSY,
                ..
            } | Error::Transport(TransportError::Timeout)
        )
    }
}

/// The response to a transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub code: u16,
    pub params: Vec<u32>,
}

/// An event from the camera.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub code: u16,
    pub tid: u32,
    pub params: Vec<u32>,
}

impl Event {
    pub fn parse(b: &[u8]) -> Result<Event, Error> {
        let h = Header::parse(b).ok_or(Error::Protocol("short event"))?;
        if h.kind != container::EVENT {
            return Err(Error::Protocol("not an event container"));
        }
        Ok(Event {
            code: h.code,
            tid: h.tid,
            params: read_params(b, h.length),
        })
    }

    pub fn param(&self, i: usize) -> Option<u32> {
        self.params.get(i).copied()
    }
}

/// Where a data-IN phase goes: each part of the payload as it comes, and the payload's
/// length when the container says it.
type Sink<'a> = &'a mut dyn FnMut(&[u8], Option<u64>) -> Result<(), Error>;

/// A camera with a session open.
pub struct Camera<T: Transport> {
    t: T,
    tid: u32,
    open: bool,
    info: DeviceInfo,
    /// For responses and each data transfer.
    pub timeout: Duration,
    /// The bulk-IN transfer size for data phases.
    chunk: usize,
}

impl<T: Transport> Camera<T> {
    /// Reads the DeviceInfo and opens a session. A session left open (by a program that
    /// quit) is closed and opened again.
    pub fn open(t: T) -> Result<Camera<T>, Error> {
        let ps = t.packet_size().max(64);
        let mut c = Camera {
            t,
            tid: 0,
            open: false,
            info: DeviceInfo::default(),
            timeout: Duration::from_secs(10),
            chunk: (1 << 20) / ps * ps,
        };
        c.info = c.device_info()?;
        match c.call(op::OPEN_SESSION, &[SESSION_ID]) {
            Ok(_) => {}
            Err(Error::Response {
                code: rc::SESSION_ALREADY_OPEN,
                ..
            }) => {
                c.open = true;
                let _ = c.call(op::CLOSE_SESSION, &[]);
                c.open = false;
                c.tid = 0;
                match c.call(op::OPEN_SESSION, &[SESSION_ID]) {
                    Ok(_)
                    | Err(Error::Response {
                        code: rc::SESSION_ALREADY_OPEN,
                        ..
                    }) => {}
                    Err(e) => return Err(e),
                }
            }
            Err(e) => return Err(e),
        }
        c.open = true;
        c.tid = 1;
        Ok(c)
    }

    /// The DeviceInfo read when the session opened.
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn transport(&mut self) -> &mut T {
        &mut self.t
    }

    /// The camera lists the operation.
    pub fn supports(&self, operation: u16) -> bool {
        self.info.operations.contains(&operation)
    }

    /// The camera lists the property.
    pub fn has_prop(&self, code: u16) -> bool {
        self.info.properties.contains(&code)
    }

    /// The camera lists the event.
    pub fn has_event(&self, code: u16) -> bool {
        self.info.events.contains(&code)
    }

    fn next_tid(&mut self) -> u32 {
        if !self.open {
            return 0;
        }
        let t = self.tid;
        self.tid = if t >= 0xFFFF_FFFE { 1 } else { t + 1 };
        t
    }

    /// One transaction: the command, then `out` as a data-OUT phase, or a data-IN phase into
    /// `sink`, then the response. A response other than OK is an [`Error::Response`].
    pub fn transaction(
        &mut self,
        code: u16,
        ps: &[u32],
        out: Option<&[u8]>,
        sink: Option<Sink<'_>>,
    ) -> Result<Response, Error> {
        let tid = self.next_tid();
        let to = self.timeout;
        self.t
            .send(&params(container::COMMAND, code, tid, ps), to)?;
        if let Some(d) = out {
            self.t.send(&data(code, tid, d), to)?;
        }
        let r = self.finish(code, tid, sink);
        if let Err(Error::Transport(TransportError::Timeout)) = &r {
            let _ = self.t.cancel(tid);
        }
        if let Err(Error::Transport(TransportError::Stall)) = &r {
            let _ = self.t.clear_stall();
        }
        let r = r?;
        if r.code != rc::OK {
            return Err(Error::Response {
                op: code,
                code: r.code,
            });
        }
        Ok(r)
    }

    /// Receives one transfer, skipping up to two zero-length packets (one may be left from
    /// a data phase that filled its last packet).
    fn receive_some(&mut self, max: usize) -> Result<Vec<u8>, Error> {
        for _ in 0..3 {
            let b = self.t.receive(max, self.timeout)?;
            if !b.is_empty() {
                return Ok(b);
            }
        }
        Err(Error::Protocol("only empty transfers"))
    }

    fn finish(
        &mut self,
        code: u16,
        tid: u32,
        mut sink: Option<Sink<'_>>,
    ) -> Result<Response, Error> {
        let ps = self.t.packet_size().max(1);
        let mut first = self.receive_some(self.chunk)?;
        let mut h = Header::parse(&first).ok_or(Error::Protocol("short container"))?;
        if h.kind == container::DATA {
            if h.tid != tid {
                return Err(Error::Protocol("data for another transaction"));
            }
            // 0xFFFFFFFF: 4 GiB or more; read until a short transfer.
            let unknown = h.length == u32::MAX;
            let total = h.length as u64;
            if !unknown && total < HEADER as u64 {
                return Err(Error::Protocol("data container shorter than its header"));
            }
            let payload_len = (!unknown).then(|| total - HEADER as u64);
            let mut got = first.len() as u64;
            let keep = if unknown {
                first.len()
            } else {
                first.len().min(total as usize)
            };
            if let Some(s) = sink.as_mut() {
                s(&first[HEADER..keep], payload_len)?;
            }
            let mut last_short = first.len() < self.chunk;
            loop {
                let more = if unknown { !last_short } else { got < total };
                if !more {
                    break;
                }
                let want = if unknown {
                    self.chunk
                } else {
                    let left = (total - got) as usize;
                    left.div_ceil(ps).saturating_mul(ps).min(self.chunk)
                };
                let b = self.t.receive(want, self.timeout)?;
                if b.is_empty() {
                    if unknown {
                        break;
                    }
                    return Err(Error::Protocol("data ended early"));
                }
                last_short = b.len() < want;
                let keep = if unknown {
                    b.len()
                } else {
                    b.len().min((total - got) as usize)
                };
                got += b.len() as u64;
                if let Some(s) = sink.as_mut() {
                    s(&b[..keep], payload_len)?;
                }
            }
            first = self.receive_some(self.chunk)?;
            h = Header::parse(&first).ok_or(Error::Protocol("short container"))?;
        }
        if h.kind != container::RESPONSE {
            return Err(Error::Protocol("expected a response"));
        }
        if h.tid != tid && code != op::OPEN_SESSION {
            return Err(Error::Protocol("response for another transaction"));
        }
        Ok(Response {
            code: h.code,
            params: read_params(&first, h.length),
        })
    }

    /// A transaction without data.
    pub fn call(&mut self, code: u16, ps: &[u32]) -> Result<Response, Error> {
        self.transaction(code, ps, None, None)
    }

    /// A transaction with a data-IN phase, collected.
    pub fn call_in(&mut self, code: u16, ps: &[u32]) -> Result<(Response, Vec<u8>), Error> {
        let mut buf = Vec::new();
        let r = self.transaction(
            code,
            ps,
            None,
            Some(&mut |b: &[u8], _| {
                buf.extend_from_slice(b);
                Ok(())
            }),
        )?;
        Ok((r, buf))
    }

    pub fn device_info(&mut self) -> Result<DeviceInfo, Error> {
        let (_, b) = self.call_in(op::GET_DEVICE_INFO, &[])?;
        Ok(DeviceInfo::parse(&b)?)
    }

    pub fn close(&mut self) -> Result<(), Error> {
        if self.open {
            self.call(op::CLOSE_SESSION, &[])?;
            self.open = false;
        }
        Ok(())
    }

    /// The storages with a medium in them (a low half of 0 means an empty slot).
    pub fn storage_ids(&mut self) -> Result<Vec<u32>, Error> {
        let (_, b) = self.call_in(op::GET_STORAGE_IDS, &[])?;
        let ids = Reader::new(&b, "StorageIDs").u32_array()?;
        Ok(ids.into_iter().filter(|id| id & 0xFFFF != 0).collect())
    }

    pub fn storage_info(&mut self, id: u32) -> Result<StorageInfo, Error> {
        let (_, b) = self.call_in(op::GET_STORAGE_INFO, &[id])?;
        Ok(StorageInfo::parse(&b)?)
    }

    /// The handles of every object on `storage` ([`ALL`] for every storage), of any format,
    /// in any folder.
    pub fn object_handles(&mut self, storage: u32) -> Result<Vec<u32>, Error> {
        let (_, b) = self.call_in(op::GET_OBJECT_HANDLES, &[storage, 0, 0])?;
        Ok(Reader::new(&b, "ObjectHandles").u32_array()?)
    }

    pub fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, Error> {
        let (_, b) = self.call_in(op::GET_OBJECT_INFO, &[handle])?;
        Ok(ObjectInfo::parse(&b)?)
    }

    /// Downloads the object into `out`, calling `progress(done, total)` after each part
    /// (`total` from the data container, 0 when unknown). Returns the bytes written.
    pub fn get_object(
        &mut self,
        handle: u32,
        out: &mut dyn Write,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<u64, Error> {
        let mut done = 0u64;
        self.transaction(
            op::GET_OBJECT,
            &[handle],
            None,
            Some(&mut |b: &[u8], total| {
                out.write_all(b).map_err(Error::Write)?;
                done += b.len() as u64;
                progress(done, total.unwrap_or(0));
                Ok(())
            }),
        )?;
        Ok(done)
    }

    /// Takes a picture to the camera's default storage, in its default format. The new
    /// objects arrive as ObjectAdded events, then CaptureComplete.
    pub fn initiate_capture(&mut self) -> Result<(), Error> {
        if !self.supports(op::INITIATE_CAPTURE) {
            return Err(Error::Unsupported(op::INITIATE_CAPTURE));
        }
        self.call(op::INITIATE_CAPTURE, &[0, 0])?;
        Ok(())
    }

    pub fn prop_desc(&mut self, code: u16) -> Result<PropDesc, Error> {
        let (_, b) = self.call_in(op::GET_DEVICE_PROP_DESC, &[code as u32])?;
        let d = PropDesc::parse(&b)?;
        if d.code != code {
            return Err(Error::Protocol("description of another property"));
        }
        Ok(d)
    }

    pub fn prop_value(&mut self, code: u16, data_type: u16) -> Result<Value, Error> {
        let (_, b) = self.call_in(op::GET_DEVICE_PROP_VALUE, &[code as u32])?;
        Ok(Reader::new(&b, "DevicePropValue").value(data_type)?)
    }

    pub fn set_prop_value(&mut self, code: u16, data_type: u16, v: &Value) -> Result<(), Error> {
        let mut w = Writer::default();
        w.value(data_type, v);
        self.transaction(op::SET_DEVICE_PROP_VALUE, &[code as u32], Some(&w.b), None)?;
        Ok(())
    }

    /// The exposure properties of [`EXPOSURE`] the camera has and describes.
    pub fn exposure(&mut self) -> Vec<PropDesc> {
        let mut out = Vec::new();
        for code in EXPOSURE {
            if self.has_prop(code)
                && let Ok(d) = self.prop_desc(code)
            {
                out.push(d);
            }
        }
        out
    }

    /// The battery level in percent, when the camera reports one.
    pub fn battery(&mut self) -> Option<u8> {
        if !self.has_prop(prop::BATTERY_LEVEL) {
            return None;
        }
        let d = self.prop_desc(prop::BATTERY_LEVEL).ok()?;
        crate::props::battery_percent(&d)
    }

    /// The next event, waiting at most `timeout`.
    pub fn poll_event(&mut self, timeout: Duration) -> Result<Option<Event>, Error> {
        match self.t.event(timeout)? {
            Some(b) => Event::parse(&b).map(Some),
            None => Ok(None),
        }
    }

    /// Every object handle the camera holds now (for telling new shots from old).
    pub fn all_handles(&mut self) -> Result<Vec<u32>, Error> {
        self.object_handles(ALL)
    }

    /// The camera announces new objects by ObjectAdded events.
    pub fn sends_object_events(&self) -> bool {
        self.has_event(ev::OBJECT_ADDED)
    }
}

impl<T: Transport> Drop for Camera<T> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
