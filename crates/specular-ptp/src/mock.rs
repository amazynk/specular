//! A camera in memory that answers as ISO 15740 says a camera does: sessions and their
//! transaction ids, storages, objects, captures with their ObjectAdded and CaptureComplete
//! events, and properties with their forms. Data phases go out in USB transfers of the
//! packet size the test sets, ending at a short packet or a zero-length one, as on the wire.
//!
//! [`MockCamera`] is the transport; [`MockCamera::handle`] gives a second handle on the same
//! state, for the test to press the shutter, pull the card or unplug the camera while a
//! [`crate::Camera`] owns the transport.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::codes::{ALL, container, ev, format, op, prop, rc, ty};
use crate::container::{Header, data, params, read_params};
use crate::data::{DeviceInfo, Form, ObjectInfo, PropDesc, Reader, StorageInfo, Value, Writer};
use crate::transport::{Transport, TransportError};

/// A file the camera makes when it takes a picture.
#[derive(Clone, Debug)]
pub struct Makes {
    pub ext: &'static str,
    pub format: u16,
    pub bytes: usize,
}

/// One bulk-IN transfer sequence, ending with a short or zero-length packet.
struct Msg {
    bytes: Vec<u8>,
    pos: usize,
}

/// The camera's state.
pub struct State {
    pub info: DeviceInfo,
    pub storages: Vec<(u32, StorageInfo)>,
    pub objects: BTreeMap<u32, (ObjectInfo, Vec<u8>)>,
    pub props: BTreeMap<u16, PropDesc>,
    pub session: Option<u32>,
    /// The last transaction id seen in the session.
    last_tid: u32,
    pub packet: usize,
    /// Each command: operation, transaction id, parameters.
    pub log: Vec<(u16, u32, Vec<u32>)>,
    /// Reads whose size wasn't a multiple of the packet size.
    pub bad_reads: usize,
    /// The next call of an operation is answered with this code (then forgotten).
    pub fail: HashMap<u16, u16>,
    /// InitiateCapture answers Device_Busy this many times first.
    pub busy_captures: u32,
    /// What one shot makes.
    pub makes: Vec<Makes>,
    /// Real files the next shots make, one per shot, by name and contents (instead of
    /// [`State::makes`] while any are left).
    pub feed: VecDeque<(String, Vec<u8>)>,
    pub next_handle: u32,
    shots: u32,
    /// ObjectAdded events are sent.
    pub object_events: bool,
    pub unplugged: bool,
    /// The next GetObject sends only this many bytes of the data, then nothing.
    pub cut_next_object: Option<usize>,
    /// The next data-IN phase sends its header in a transfer of its own.
    pub split_header: bool,
    out: VecDeque<Msg>,
    events: VecDeque<Vec<u8>>,
    /// A command waiting for its data-OUT phase.
    waiting: Option<(u16, u32, Vec<u32>)>,
}

/// The in-memory camera (the transport side).
#[derive(Clone)]
pub struct MockCamera {
    s: Arc<Mutex<State>>,
}

fn storage(label: &str, free_images: u32) -> StorageInfo {
    StorageInfo {
        storage_type: 4,
        filesystem_type: 2,
        access: 0,
        max_capacity: 64 << 30,
        free_bytes: 32 << 30,
        free_images,
        description: "SD".into(),
        volume_label: label.into(),
    }
}

fn enum_prop(code: u16, t: u16, cur: Value, vals: Vec<Value>, writable: bool) -> PropDesc {
    PropDesc {
        code,
        data_type: t,
        writable,
        default: vals.first().cloned().unwrap_or(cur.clone()),
        current: cur,
        form: Form::Enum(vals),
    }
}

impl Default for MockCamera {
    fn default() -> MockCamera {
        MockCamera::new()
    }
}

impl MockCamera {
    /// A camera like a mirrorless body: one card holding two old shots, the exposure
    /// properties, events, a RAW and a JPEG per shot, 512-byte packets.
    pub fn new() -> MockCamera {
        let info = DeviceInfo {
            standard_version: 100,
            vendor_extension_id: 6,
            vendor_extension_version: 100,
            vendor_extension_desc: "microsoft.com: 1.0".into(),
            functional_mode: 0,
            operations: vec![
                op::GET_DEVICE_INFO,
                op::OPEN_SESSION,
                op::CLOSE_SESSION,
                op::GET_STORAGE_IDS,
                op::GET_STORAGE_INFO,
                op::GET_OBJECT_HANDLES,
                op::GET_OBJECT_INFO,
                op::GET_OBJECT,
                op::INITIATE_CAPTURE,
                op::GET_DEVICE_PROP_DESC,
                op::GET_DEVICE_PROP_VALUE,
                op::SET_DEVICE_PROP_VALUE,
            ],
            events: vec![
                ev::OBJECT_ADDED,
                ev::CAPTURE_COMPLETE,
                ev::DEVICE_PROP_CHANGED,
                ev::STORE_FULL,
            ],
            properties: vec![
                prop::BATTERY_LEVEL,
                prop::EXPOSURE_PROGRAM_MODE,
                prop::EXPOSURE_TIME,
                prop::F_NUMBER,
                prop::EXPOSURE_INDEX,
                prop::EXPOSURE_BIAS_COMPENSATION,
                prop::WHITE_BALANCE,
            ],
            capture_formats: vec![format::EXIF_JPEG],
            image_formats: vec![format::EXIF_JPEG, format::UNDEFINED],
            manufacturer: "Mock Optical".into(),
            model: "M-1".into(),
            device_version: "1.0".into(),
            serial_number: "0001".into(),
        };
        let u = |v: &[u64]| v.iter().map(|&x| Value::UInt(x)).collect::<Vec<_>>();
        let mut props = BTreeMap::new();
        for d in [
            PropDesc {
                code: prop::BATTERY_LEVEL,
                data_type: ty::UINT8,
                writable: false,
                default: Value::UInt(100),
                current: Value::UInt(80),
                form: Form::Range {
                    min: Value::UInt(0),
                    max: Value::UInt(100),
                    step: Value::UInt(1),
                },
            },
            enum_prop(
                prop::EXPOSURE_PROGRAM_MODE,
                ty::UINT16,
                Value::UInt(1),
                u(&[1, 2, 3, 4]),
                false,
            ),
            enum_prop(
                prop::EXPOSURE_TIME,
                ty::UINT32,
                Value::UInt(40),
                u(&[10, 20, 40, 80, 167, 333]),
                true,
            ),
            enum_prop(
                prop::F_NUMBER,
                ty::UINT16,
                Value::UInt(400),
                u(&[280, 400, 560, 800]),
                true,
            ),
            enum_prop(
                prop::EXPOSURE_INDEX,
                ty::UINT16,
                Value::UInt(100),
                u(&[100, 200, 400, 800, 1600, 0xFFFF]),
                true,
            ),
            PropDesc {
                code: prop::EXPOSURE_BIAS_COMPENSATION,
                data_type: ty::INT16,
                writable: true,
                default: Value::Int(0),
                current: Value::Int(0),
                form: Form::Range {
                    min: Value::Int(-3000),
                    max: Value::Int(3000),
                    step: Value::Int(1000),
                },
            },
            enum_prop(
                prop::WHITE_BALANCE,
                ty::UINT16,
                Value::UInt(2),
                u(&[2, 4, 6, 7]),
                true,
            ),
        ] {
            props.insert(d.code, d);
        }
        let mut s = State {
            info,
            storages: vec![(0x0001_0001, storage("CARD", 900))],
            objects: BTreeMap::new(),
            props,
            session: None,
            last_tid: 0,
            packet: 512,
            log: Vec::new(),
            bad_reads: 0,
            fail: HashMap::new(),
            busy_captures: 0,
            makes: vec![
                Makes {
                    ext: "RAW",
                    format: format::UNDEFINED,
                    bytes: 3000,
                },
                Makes {
                    ext: "JPG",
                    format: format::EXIF_JPEG,
                    bytes: 1024,
                },
            ],
            feed: VecDeque::new(),
            next_handle: 0x100,
            shots: 0,
            object_events: true,
            unplugged: false,
            cut_next_object: None,
            split_header: false,
            out: VecDeque::new(),
            events: VecDeque::new(),
            waiting: None,
        };
        // A folder and two old shots on the card.
        s.add_object(
            ObjectInfo {
                storage_id: 0x0001_0001,
                format: format::ASSOCIATION,
                association_type: 1,
                filename: "100MOCK".into(),
                ..ObjectInfo::default()
            },
            Vec::new(),
        );
        for _ in 0..2 {
            s.shoot();
        }
        MockCamera {
            s: Arc::new(Mutex::new(s)),
        }
    }

    /// The camera's state, to change or check.
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.s.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Another handle on the same camera.
    pub fn handle(&self) -> MockCamera {
        self.clone()
    }

    /// The camera's own shutter button: the shot is made, with its events.
    pub fn press_shutter(&self) -> Vec<u32> {
        let mut s = self.state();
        let hs = s.shoot();
        if s.object_events {
            for &h in &hs {
                s.event(ev::OBJECT_ADDED, 0, &[h]);
            }
        }
        hs
    }

    /// Queues an event as the camera would send it.
    pub fn send_event(&self, code: u16, ps: &[u32]) {
        self.state().event(code, 0, ps);
    }
}

/// Bytes `0, 1, 2…` (mod 251) after `seed`, so a cut or shifted transfer shows.
pub fn pattern(seed: u32, n: usize) -> Vec<u8> {
    (0..n)
        .map(|i| ((i as u32).wrapping_add(seed.wrapping_mul(7)) % 251) as u8)
        .collect()
}

impl State {
    fn add_object(&mut self, info: ObjectInfo, bytes: Vec<u8>) -> u32 {
        let h = self.next_handle;
        self.next_handle += 1;
        self.objects.insert(h, (info, bytes));
        h
    }

    /// Makes one shot's files on the first storage.
    fn shoot(&mut self) -> Vec<u32> {
        self.shots += 1;
        let n = self.shots;
        let sid = self.storages.first().map_or(0, |s| s.0);
        let mut hs = Vec::new();
        if let Some((name, bytes)) = self.feed.pop_front() {
            let jpeg = name.rsplit_once('.').is_some_and(|(_, e)| {
                e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg")
            });
            let info = ObjectInfo {
                storage_id: sid,
                format: if jpeg {
                    format::EXIF_JPEG
                } else {
                    format::UNDEFINED
                },
                compressed_size: bytes.len() as u32,
                parent: 0x100,
                filename: name,
                capture_date: format!("20241103T1425{:02}", n % 60),
                ..ObjectInfo::default()
            };
            hs.push(self.add_object(info, bytes));
            if let Some((_, st)) = self.storages.first_mut() {
                st.free_images = st.free_images.saturating_sub(1);
            }
            return hs;
        }
        let makes = self.makes.clone();
        for m in makes {
            let info = ObjectInfo {
                storage_id: sid,
                format: m.format,
                compressed_size: m.bytes as u32,
                parent: 0x100,
                filename: format!("IMG_{n:04}.{}", m.ext),
                capture_date: format!("20241103T1425{:02}", n % 60),
                ..ObjectInfo::default()
            };
            let h = self.next_handle;
            hs.push(self.add_object(info, pattern(h, m.bytes)));
        }
        if let Some((_, st)) = self.storages.first_mut() {
            st.free_images = st.free_images.saturating_sub(1);
        }
        hs
    }

    fn event(&mut self, code: u16, tid: u32, ps: &[u32]) {
        self.events
            .push_back(params(container::EVENT, code, tid, ps));
    }

    fn respond(&mut self, code: u16, tid: u32, ps: &[u32]) {
        self.out.push_back(Msg {
            bytes: params(container::RESPONSE, code, tid, ps),
            pos: 0,
        });
    }

    fn data_in(&mut self, code: u16, tid: u32, payload: &[u8]) {
        let d = data(code, tid, payload);
        if self.split_header {
            self.split_header = false;
            self.out.push_back(Msg {
                bytes: d[..12].to_vec(),
                pos: 0,
            });
            self.out.push_back(Msg {
                bytes: d[12..].to_vec(),
                pos: 0,
            });
        } else {
            self.out.push_back(Msg { bytes: d, pos: 0 });
        }
    }

    fn command(&mut self, code: u16, tid: u32, ps: Vec<u32>, payload: Option<Vec<u8>>) {
        self.log.push((code, tid, ps.clone()));
        let p = |i: usize| ps.get(i).copied().unwrap_or(0);
        if self.session.is_none() {
            if code != op::GET_DEVICE_INFO && code != op::OPEN_SESSION {
                self.respond(rc::SESSION_NOT_OPEN, tid, &[]);
                return;
            }
        } else if code != op::OPEN_SESSION
            && code != op::CLOSE_SESSION
            && !(code == op::GET_DEVICE_INFO && tid == 0)
        {
            // CloseSession is taken with any id (a host that lost the count after a crash
            // can still close the session it left open), and GetDeviceInfo with id 0 (its
            // form outside a session).
            // Transaction ids count up from 1 after OpenSession (wrapping past 0xFFFFFFFE).
            let want = if self.last_tid >= 0xFFFF_FFFE {
                1
            } else {
                self.last_tid + 1
            };
            if tid != want {
                self.respond(rc::INVALID_TRANSACTION_ID, tid, &[]);
                return;
            }
            self.last_tid = tid;
        }
        if let Some(f) = self.fail.remove(&code) {
            self.respond(f, tid, &[]);
            return;
        }
        let mut w = Writer::default();
        match code {
            op::GET_DEVICE_INFO => {
                self.info.write(&mut w);
                self.data_in(code, tid, &w.b);
                self.respond(rc::OK, tid, &[]);
            }
            op::OPEN_SESSION => {
                if self.session.is_some() {
                    self.respond(rc::SESSION_ALREADY_OPEN, tid, &[self.session.unwrap_or(0)]);
                } else if p(0) == 0 {
                    self.respond(rc::INVALID_PARAMETER, tid, &[]);
                } else {
                    self.session = Some(p(0));
                    self.last_tid = 0;
                    self.respond(rc::OK, tid, &[]);
                }
            }
            op::CLOSE_SESSION => {
                self.session = None;
                self.respond(rc::OK, tid, &[]);
            }
            op::GET_STORAGE_IDS => {
                let ids: Vec<u32> = self.storages.iter().map(|s| s.0).collect();
                w.u32_array(&ids);
                self.data_in(code, tid, &w.b);
                self.respond(rc::OK, tid, &[]);
            }
            op::GET_STORAGE_INFO => match self.storages.iter().find(|s| s.0 == p(0)) {
                Some((_, st)) => {
                    st.write(&mut w);
                    self.data_in(code, tid, &w.b);
                    self.respond(rc::OK, tid, &[]);
                }
                None => self.respond(rc::INVALID_STORAGE_ID, tid, &[]),
            },
            op::GET_OBJECT_HANDLES => {
                let sid = p(0);
                if sid != ALL && !self.storages.iter().any(|s| s.0 == sid) {
                    self.respond(rc::INVALID_STORAGE_ID, tid, &[]);
                    return;
                }
                if self.storages.is_empty() {
                    self.respond(rc::STORE_NOT_AVAILABLE, tid, &[]);
                    return;
                }
                let hs: Vec<u32> = self
                    .objects
                    .iter()
                    .filter(|(_, (i, _))| sid == ALL || i.storage_id == sid)
                    .map(|(h, _)| *h)
                    .collect();
                w.u32_array(&hs);
                self.data_in(code, tid, &w.b);
                self.respond(rc::OK, tid, &[]);
            }
            op::GET_OBJECT_INFO => match self.objects.get(&p(0)) {
                Some((i, _)) => {
                    i.write(&mut w);
                    self.data_in(code, tid, &w.b);
                    self.respond(rc::OK, tid, &[]);
                }
                None => self.respond(rc::INVALID_OBJECT_HANDLE, tid, &[]),
            },
            op::GET_OBJECT => match self.objects.get(&p(0)) {
                Some((_, b)) => {
                    let b = b.clone();
                    if let Some(n) = self.cut_next_object.take() {
                        let d = data(code, tid, &b);
                        self.out.push_back(Msg {
                            bytes: d[..(12 + n).min(d.len())].to_vec(),
                            pos: 0,
                        });
                        // Then nothing: the camera hangs.
                        self.out.push_back(Msg {
                            bytes: Vec::new(),
                            pos: usize::MAX,
                        });
                        return;
                    }
                    self.data_in(code, tid, &b);
                    self.respond(rc::OK, tid, &[]);
                }
                None => self.respond(rc::INVALID_OBJECT_HANDLE, tid, &[]),
            },
            op::INITIATE_CAPTURE => {
                if self.busy_captures > 0 {
                    self.busy_captures -= 1;
                    self.respond(rc::DEVICE_BUSY, tid, &[]);
                    return;
                }
                match self.storages.first() {
                    None => {
                        self.respond(rc::STORE_NOT_AVAILABLE, tid, &[]);
                        return;
                    }
                    Some((_, st)) if st.free_images == 0 => {
                        self.respond(rc::STORE_FULL, tid, &[]);
                        return;
                    }
                    _ => {}
                }
                self.respond(rc::OK, tid, &[]);
                let hs = self.shoot();
                if self.object_events {
                    for h in hs {
                        self.event(ev::OBJECT_ADDED, tid, &[h]);
                    }
                }
                self.event(ev::CAPTURE_COMPLETE, tid, &[tid]);
            }
            op::GET_DEVICE_PROP_DESC => match self.props.get(&(p(0) as u16)) {
                Some(d) => {
                    d.write(&mut w);
                    self.data_in(code, tid, &w.b);
                    self.respond(rc::OK, tid, &[]);
                }
                None => self.respond(rc::DEVICE_PROP_NOT_SUPPORTED, tid, &[]),
            },
            op::GET_DEVICE_PROP_VALUE => match self.props.get(&(p(0) as u16)) {
                Some(d) => {
                    w.value(d.data_type, &d.current);
                    self.data_in(code, tid, &w.b);
                    self.respond(rc::OK, tid, &[]);
                }
                None => self.respond(rc::DEVICE_PROP_NOT_SUPPORTED, tid, &[]),
            },
            op::SET_DEVICE_PROP_VALUE => {
                let pc = p(0) as u16;
                let Some(d) = self.props.get(&pc).cloned() else {
                    self.respond(rc::DEVICE_PROP_NOT_SUPPORTED, tid, &[]);
                    return;
                };
                if !d.writable {
                    self.respond(rc::ACCESS_DENIED, tid, &[]);
                    return;
                }
                let payload = payload.unwrap_or_default();
                let Ok(v) = Reader::new(&payload, "value").value(d.data_type) else {
                    self.respond(rc::INVALID_DEVICE_PROP_FORMAT, tid, &[]);
                    return;
                };
                let ok = d.choices(4096).is_none_or(|c| c.contains(&v));
                if !ok {
                    self.respond(rc::INVALID_DEVICE_PROP_VALUE, tid, &[]);
                    return;
                }
                if let Some(d) = self.props.get_mut(&pc) {
                    d.current = v;
                }
                self.respond(rc::OK, tid, &[]);
                self.event(ev::DEVICE_PROP_CHANGED, 0, &[pc as u32]);
            }
            _ => self.respond(rc::OPERATION_NOT_SUPPORTED, tid, &[]),
        }
    }
}

impl Transport for MockCamera {
    fn send(&mut self, bytes: &[u8], _timeout: Duration) -> Result<(), TransportError> {
        let mut s = self.state();
        if s.unplugged {
            return Err(TransportError::Disconnected);
        }
        let h = Header::parse(bytes).ok_or(TransportError::Stall)?;
        if h.length as usize != bytes.len() {
            return Err(TransportError::Stall);
        }
        match h.kind {
            container::COMMAND => {
                let ps = read_params(bytes, h.length);
                if h.code == op::SET_DEVICE_PROP_VALUE {
                    s.waiting = Some((h.code, h.tid, ps));
                } else {
                    s.command(h.code, h.tid, ps, None);
                }
            }
            container::DATA => match s.waiting.take() {
                Some((code, tid, ps)) if code == h.code && tid == h.tid => {
                    s.command(code, tid, ps, Some(bytes[12..].to_vec()));
                }
                _ => return Err(TransportError::Stall),
            },
            _ => return Err(TransportError::Stall),
        }
        Ok(())
    }

    fn receive(&mut self, max: usize, _timeout: Duration) -> Result<Vec<u8>, TransportError> {
        let mut s = self.state();
        if s.unplugged {
            return Err(TransportError::Disconnected);
        }
        let packet = s.packet;
        if max == 0 || !max.is_multiple_of(packet) {
            s.bad_reads += 1;
        }
        let Some(m) = s.out.front_mut() else {
            return Err(TransportError::Timeout);
        };
        if m.pos == usize::MAX {
            // A camera that stopped mid-transfer.
            return Err(TransportError::Timeout);
        }
        let rem = m.bytes.len() - m.pos;
        if rem == 0 {
            // The zero-length packet after a transfer that filled its last packet.
            s.out.pop_front();
            return Ok(Vec::new());
        }
        let n = rem.min(max);
        let out = m.bytes[m.pos..m.pos + n].to_vec();
        m.pos += n;
        if m.pos == m.bytes.len() {
            let full_packets = m.bytes.len() % packet == 0;
            // A short packet ends the sequence; so does the zero-length one when this read
            // asked for more. A read of exactly the rest leaves the zero-length packet.
            if !(full_packets && n == max) {
                s.out.pop_front();
            }
        }
        Ok(out)
    }

    fn event(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        let mut s = self.state();
        if s.unplugged {
            return Err(TransportError::Disconnected);
        }
        Ok(s.events.pop_front())
    }

    fn packet_size(&self) -> usize {
        self.state().packet
    }

    fn cancel(&mut self, _tid: u32) -> Result<(), TransportError> {
        let mut s = self.state();
        s.out.clear();
        Ok(())
    }
}
