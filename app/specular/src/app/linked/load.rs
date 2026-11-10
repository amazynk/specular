//! Slices for linked views, decoded off the UI thread: one thread per comparison takes the
//! newest request of each viewport (older ones for a viewport are skipped while scrolling),
//! keeps the last file it parsed (a multi-frame file serves many slices) and sends back the
//! modality values, which the UI windows on the CPU into the viewport's texture.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use specular_dicom::{Dicom, ModalityFrame, Rendered};
use specular_render::Window as WlWindow;

use crate::loader::Wake;

/// Which slice: entry (series) and index into its stack.
pub(crate) type Key = (usize, usize);

/// A decoded slice.
#[derive(Debug)]
pub(crate) struct Slice {
    pub width: u32,
    pub height: u32,
    pub pixels: Pixels,
    /// The file's default window (`None`: a VOI LUT, or colour), its value range, and
    /// whether it shows inverted (MONOCHROME1).
    pub window: Option<WlWindow>,
    pub range: (f64, f64),
    pub inverted: bool,
}

#[derive(Debug)]
pub(crate) enum Pixels {
    /// Modality values, row-major.
    Grey(Vec<f32>),
    /// Colour, RGBA 8-bit.
    Rgba(Vec<u8>),
}

impl Slice {
    pub(crate) fn of(f: &ModalityFrame) -> Result<Slice, String> {
        let (voi, _) = f.default_voi();
        let window = super::super::medical::window_of(&voi, f.inverted());
        let range = f.value_range().unwrap_or((0.0, 1.0));
        let pixels = match f.values_f32() {
            Some(v) => Pixels::Grey(v),
            None => {
                let img = f
                    .render_default()
                    .map_err(|e| crate::error_text::dicom_reason(&e))?;
                Pixels::Rgba(match img.pixels {
                    Rendered::Rgba8(v) => v,
                    Rendered::Rgba16(v) => v.iter().map(|x| (x >> 8) as u8).collect(),
                    Rendered::Gray16(v) => v
                        .iter()
                        .flat_map(|x| {
                            let g = (x >> 8) as u8;
                            [g, g, g, 255]
                        })
                        .collect(),
                })
            }
        };
        Ok(Slice {
            width: f.frame.width,
            height: f.frame.height,
            pixels,
            window,
            range,
            inverted: f.inverted(),
        })
    }

    /// RGBA 8-bit through `w` (greys), or the colour as it is.
    pub(crate) fn rgba(&self, w: &WlWindow) -> Vec<u8> {
        match &self.pixels {
            Pixels::Rgba(v) => v.clone(),
            Pixels::Grey(v) => {
                let mut out = Vec::with_capacity(v.len() * 4);
                for &x in v {
                    let g = if x.is_finite() {
                        (w.grey(f64::from(x)).clamp(0.0, 1.0) * 255.0).round() as u8
                    } else {
                        0
                    };
                    out.extend_from_slice(&[g, g, g, 255]);
                }
                out
            }
        }
    }

    pub(crate) fn bytes(&self) -> usize {
        match &self.pixels {
            Pixels::Grey(v) => v.len() * 4,
            Pixels::Rgba(v) => v.len(),
        }
    }
}

/// A slice to decode for viewport `port`.
struct Request {
    port: usize,
    key: Key,
    path: PathBuf,
    frame: u32,
}

/// What came back.
pub(crate) struct Done {
    pub key: Key,
    pub slice: Result<Arc<Slice>, String>,
}

/// The decoding thread of one comparison (it ends when this is dropped).
pub(crate) struct Loader {
    tx: Option<Sender<Request>>,
    rx: Receiver<Done>,
}

impl Loader {
    pub(crate) fn start(wake: Wake) -> Loader {
        let (tx, req_rx) = channel::<Request>();
        let (done_tx, rx) = channel::<Done>();
        let _ = specular_thread::builder("dcm-linked").spawn(move || run(&req_rx, &done_tx, &wake));
        Loader { tx: Some(tx), rx }
    }

    pub(crate) fn ask(&self, port: usize, key: Key, path: PathBuf, frame: u32) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Request {
                port,
                key,
                path,
                frame,
            });
        }
    }

    /// What finished since the last call.
    pub(crate) fn poll(&self) -> Vec<Done> {
        let mut out = Vec::new();
        while let Ok(d) = self.rx.try_recv() {
            out.push(d);
        }
        out
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.tx = None;
    }
}

/// A file read and parsed (the last one is kept).
type Parsed = Arc<(Vec<u8>, Dicom)>;

fn parse(path: &PathBuf) -> Result<Parsed, String> {
    let bytes = std::fs::read(path).map_err(|e| crate::error_text::io_reason(&e))?;
    let d = Dicom::parse(&bytes).map_err(|e| crate::error_text::dicom_reason(&e))?;
    Ok(Arc::new((bytes, d)))
}

fn run(rx: &Receiver<Request>, tx: &Sender<Done>, wake: &Wake) {
    let mut last: Option<(PathBuf, Parsed)> = None;
    while let Ok(first) = rx.recv() {
        // The newest request of each viewport; the rest were scrolled past.
        let mut newest: HashMap<usize, Request> = HashMap::new();
        let mut order = vec![first.port];
        newest.insert(first.port, first);
        loop {
            match rx.try_recv() {
                Ok(r) => {
                    if !order.contains(&r.port) {
                        order.push(r.port);
                    }
                    newest.insert(r.port, r);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        for port in order {
            let Some(r) = newest.remove(&port) else {
                continue;
            };
            let slice = specular_core::isolate::run("linked slice", || {
                let file = match &last {
                    Some((p, f)) if *p == r.path => f.clone(),
                    _ => {
                        let f = parse(&r.path)?;
                        last = Some((r.path.clone(), f.clone()));
                        f
                    }
                };
                let m = file
                    .1
                    .modality_frame(&file.0, r.frame, None)
                    .map_err(|e| crate::error_text::dicom_reason(&e))?;
                Slice::of(&m).map(Arc::new)
            })
            .unwrap_or_else(|_| Err(specular_i18n::t!("error-reason-internal")));
            if tx.send(Done { key: r.key, slice }).is_err() {
                return;
            }
            wake();
        }
    }
}
