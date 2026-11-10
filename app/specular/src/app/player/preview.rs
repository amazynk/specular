//! The seek bar's preview frame: while the pointer is over the bar, a scrub strip frame near
//! that place when the player has one, else the frame there decoded on a worker of its own (its own decoder: playback never waits for it), made
//! small, and kept (the last few are cached). Moving on cancels a decode that is no longer
//! wanted.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

use specular_core::CancelToken;
use specular_decode::{Frames, Session};
use specular_gfx::{Gpu, TexId, UiRenderer, wgpu};

use super::super::App;

/// The preview's width in pixels (the chrome shows it at 160 logical px).
pub(crate) const WIDTH: u32 = 320;
/// How many previews are kept.
const KEEP: usize = 24;
/// Places along the movie the previews snap to.
const STEPS: f64 = 200.0;

struct Done {
    bucket: u32,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

/// A worker decoding previews for one movie.
struct Worker {
    tx: mpsc::Sender<(u64, u32, u32)>,
    rx: mpsc::Receiver<Done>,
    /// The newest request: older ones are dropped, and their decodes cancelled.
    latest: Arc<AtomicU64>,
    cancel: Arc<std::sync::Mutex<CancelToken>>,
}

#[derive(Default)]
pub(crate) struct Preview {
    /// The movie (by its session's address) the worker decodes.
    movie: Option<usize>,
    worker: Option<Worker>,
    serial: u64,
    asked: Option<u32>,
    held: VecDeque<(u32, wgpu::Texture, TexId, u32, u32)>,
    /// Waiting to be uploaded.
    ready: Vec<Done>,
    /// The one to show: bucket, fraction.
    shown: Option<(u32, f64)>,
}

/// A small copy of straight RGBA, `w`×`h` to `WIDTH` wide (box filter).
pub(crate) fn shrink(rgba: &[u8], w: u32, h: u32) -> (u32, u32, Vec<u8>) {
    if w <= WIDTH || w == 0 || h == 0 {
        return (w, h, rgba.to_vec());
    }
    let nw = WIDTH;
    let nh = ((u64::from(h) * u64::from(nw)) / u64::from(w)).max(1) as u32;
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let y0 = (y * h / nh) as usize;
        let y1 = (((y + 1) * h / nh) as usize).max(y0 + 1);
        for x in 0..nw {
            let x0 = (x * w / nw) as usize;
            let x1 = (((x + 1) * w / nw) as usize).max(x0 + 1);
            let mut acc = [0u32; 4];
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let p = (yy * w as usize + xx) * 4;
                    for c in 0..4 {
                        acc[c] += u32::from(rgba[p + c]);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (acc[c] / n) as u8;
            }
        }
    }
    (nw, nh, out)
}

fn spawn(session: Arc<dyn Session>, waker: specular_anim::Waker) -> Option<Worker> {
    let (tx, req) = mpsc::channel::<(u64, u32, u32)>();
    let (done_tx, rx) = mpsc::channel();
    let latest = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(std::sync::Mutex::new(CancelToken::new()));
    let (l2, c2) = (Arc::clone(&latest), Arc::clone(&cancel));
    specular_thread::builder("player-preview")
        .spawn(move || {
            let Ok(mut frames) = session.frames(1) else {
                return;
            };
            while let Ok(mut r) = req.recv() {
                // Only the newest request counts.
                while let Ok(n) = req.try_recv() {
                    r = n;
                }
                let (serial, bucket, index) = r;
                if serial != l2.load(Ordering::Acquire) {
                    continue;
                }
                let token = c2.lock().map(|t| t.clone()).unwrap_or_default();
                if let Some(d) = decode(&mut *frames, index, &token) {
                    let (w, h, rgba) = shrink(&d.2, d.0, d.1);
                    let _ = done_tx.send(Done { bucket, w, h, rgba });
                    waker();
                }
            }
        })
        .ok()?;
    Some(Worker {
        tx,
        rx,
        latest,
        cancel,
    })
}

fn decode(
    frames: &mut dyn Frames,
    index: u32,
    cancel: &CancelToken,
) -> Option<(u32, u32, Vec<u8>)> {
    if !frames.seek(index).ok()? {
        frames.rewind().ok()?;
    }
    let f = frames.next_frame(cancel)?.ok()?;
    let rgba = crate::clipboard::to_rgba(&f.raster);
    Some((f.raster.width, f.raster.height, rgba))
}

impl Preview {
    /// The GPU device was replaced: the thumbnails are decoded and uploaded again.
    pub(crate) fn gpu_reset(&mut self) {
        self.held.clear();
        self.asked = None;
    }

    /// The texture for the place shown, for the chrome.
    pub(crate) fn view(&self) -> Option<(TexId, u32, u32, f64)> {
        let (b, f) = self.shown?;
        self.held
            .iter()
            .find(|h| h.0 == b)
            .map(|h| (h.2, h.3, h.4, f))
    }

    pub(crate) fn clear(&mut self, ui: &mut UiRenderer) {
        for h in self.held.drain(..) {
            ui.unregister_texture(h.2);
        }
        *self = Preview::default();
    }

    fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) {
        for d in self.ready.drain(..) {
            let (tex, id) = super::subs::upload_texture(gpu, ui, d.w, d.h, &d.rgba);
            self.held.push_back((d.bucket, tex, id, d.w, d.h));
            if self.held.len() > KEEP
                && let Some(old) = self.held.pop_front()
            {
                ui.unregister_texture(old.2);
            }
        }
    }
}

impl App {
    /// Each frame before drawing: asks for the frame under the pointer on the seek bar and
    /// uploads finished ones.
    pub(in crate::app) fn preview_frame(&mut self) {
        let hover = self.chrome.seek_hover();
        let Some((a, count)) = self
            .anim
            .as_ref()
            .filter(|a| a.movie.as_ref().is_some_and(|m| m.picture))
            .and_then(|a| a.playback().count.map(|c| (a.id, c)))
        else {
            self.player.preview.shown = None;
            return;
        };
        let Some(session) = self.images.get(&a).and_then(|i| i.session.clone()) else {
            return;
        };
        let key = Arc::as_ptr(&session) as *const () as usize;
        let p = &mut self.player.preview;
        if p.movie != Some(key) {
            p.movie = Some(key);
            p.worker = None;
            p.asked = None;
            p.shown = None;
        }
        if let Some(w) = &p.worker {
            while let Ok(d) = w.rx.try_recv() {
                p.ready.push(d);
            }
        }
        if let Some(gfx) = self.gfx.as_mut() {
            self.player.preview.upload(&gfx.gpu, &mut gfx.ui);
        }
        let p = &mut self.player.preview;
        let Some(f) = hover else {
            p.shown = None;
            return;
        };
        let bucket = (f * STEPS).round() as u32;
        p.shown = Some((bucket, f));
        if p.held.iter().any(|h| h.0 == bucket) || p.asked == Some(bucket) {
            return;
        }
        let index =
            ((f64::from(bucket) / STEPS) * f64::from(count.saturating_sub(1))).round() as u32;
        // A scrub strip frame near the place serves without a decode.
        let within = (count / STEPS as u32 / 2).max(1);
        if let Some(f) = self
            .anim
            .as_mut()
            .and_then(|a| a.player().strip_frame(index, within))
        {
            let rgba = crate::clipboard::to_rgba(&f.raster);
            let (w, h, rgba) = shrink(&rgba, f.raster.width, f.raster.height);
            let p = &mut self.player.preview;
            p.asked = Some(bucket);
            p.ready.push(Done { bucket, w, h, rgba });
            self.redraw = true;
            return;
        }
        let p = &mut self.player.preview;
        if p.worker.is_none() {
            p.worker = spawn(session, self.loader.waker());
        }
        let Some(w) = &p.worker else { return };
        p.serial += 1;
        p.asked = Some(bucket);
        w.latest.store(p.serial, Ordering::Release);
        // The decode in flight is for a place no longer wanted.
        if let Ok(mut c) = w.cancel.lock() {
            c.cancel();
            *c = CancelToken::new();
        }
        let _ = w.tx.send((p.serial, bucket, index));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinks_to_the_preview_width() {
        let (w, h) = (1280u32, 720u32);
        let rgba = vec![200u8; (w * h * 4) as usize];
        let (nw, nh, out) = shrink(&rgba, w, h);
        assert_eq!((nw, nh), (WIDTH, 180));
        assert_eq!(out.len(), (nw * nh * 4) as usize);
        assert!(out.iter().all(|&v| v == 200));
        let (sw, sh, _) = shrink(&rgba[..(64 * 36 * 4)], 64, 36);
        assert_eq!((sw, sh), (64, 36), "small pictures stay");
    }
}
