//! The gallery's GPU half of the thumbnail cache: 2048² sRGB pages cut into square slots of
//! one size class each (128, 256 or 512 px), least-recently-drawn eviction, and premultiplied
//! uploads (the UI shader blends premultiplied colour).
//!
//! Tiles are drawn page by page so a screen of thumbnails costs one draw call per page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

use specular_gfx::{Gpu, TexId, UiRenderer, wgpu};
use specular_log::{UI, debug};
use specular_thumbs::pixels::Pixels;

pub(super) const PAGE: u32 = 2048;
/// Staging chunk size: a frame's upload budget (4 MB) fits in one.
const BELT_CHUNK: u64 = 4 << 20;

/// Where a thumbnail sits in the atlas.
#[derive(Clone, Copy, Debug)]
pub(super) struct Placed {
    pub tex: TexId,
    pub page: usize,
    /// Normalised x, y, w, h of the image (not the whole slot).
    pub uv: [f32; 4],
    pub width: u32,
    pub height: u32,
}

struct Slot {
    key: (PathBuf, u32),
    width: u32,
    height: u32,
    sharp: bool,
    last: u64,
}

struct Page {
    _tex: wgpu::Texture,
    id: TexId,
    class: u32,
    slots: Vec<Option<Slot>>,
}

impl Page {
    fn per_row(&self) -> u32 {
        PAGE / self.class
    }
}

/// A page texture made on a helper thread: the driver's memory allocation behind a new
/// texture took 20–55 ms now and then, a dropped frame in the middle of a scroll.
///
/// Off the draw thread is not enough: when the texture needs a new block of device memory
/// (every 128 MB or so; the other pages suballocate in 0.1 ms), the draw thread's next
/// `queue.submit` waits for that allocation (19–37 ms on a Radeon 860M, measured with the
/// scroll bench). Its `queue.write_buffer` calls waited too, on wgpu's allocator lock;
/// the UI renderer's staging belt took those off, but the submit still waits (likely
/// in the driver or kernel, which serialise a new memory block with command submission).
/// So pages are made while the view rests, all the folder wants in one run.
type Made = (wgpu::Texture, wgpu::TextureView);

pub(super) struct Atlas {
    pages: Vec<Page>,
    /// Pages made ahead of need.
    spare: Vec<Made>,
    /// The helper making pages: one per message, disconnected when it is done.
    making: Option<Receiver<Made>>,
    /// Pages the helper was asked for and has not sent yet.
    in_flight: usize,
    /// Uploads go through reused staging chunks: `queue.write_texture` allocated a staging
    /// buffer per call, and now and then that allocation slept 10–35 ms in the driver.
    belt: Option<wgpu::util::StagingBelt>,
    /// This frame's copies from the belt, submitted by [`Atlas::end_frame`].
    enc: Option<wgpu::CommandEncoder>,
    index: HashMap<(PathBuf, u32), (usize, usize)>,
    frame: u64,
    max_pages: usize,
    pub uploads: u64,
}

impl Atlas {
    pub fn new(max_pages: usize) -> Atlas {
        Atlas {
            pages: Vec::new(),
            spare: Vec::new(),
            making: None,
            in_flight: 0,
            belt: None,
            enc: None,
            index: HashMap::new(),
            frame: 1,
            max_pages: max_pages.max(2),
            uploads: 0,
        }
    }

    /// Starts a frame; picks up the pages made ahead, and while the view is `quiet` makes
    /// all of the `want` pages the folder can use that are missing, in one helper run. A new
    /// page's memory sometimes took the driver 15–50 ms to allocate, holding up this thread's
    /// submit meanwhile: better all at once while nothing moves (one idle frame each left
    /// the scroll that followed to pay for the rest) than one by one in the middle of a
    /// scroll. True if the `want` pages are there or being made.
    pub fn begin_frame(&mut self, gpu: &Gpu, quiet: bool, want: usize) -> bool {
        self.frame += 1;
        if self.belt.is_some() {
            // Without waiting: hands the staging chunks the GPU has finished with back to the
            // belt (otherwise it makes new ones until something else polls).
            let _ = gpu.poll(wgpu::PollType::Poll);
        }
        self.collect();
        let have = self.pages.len() + self.spare.len() + self.in_flight;
        let want = want.min(self.max_pages);
        if have >= want {
            return true;
        }
        // The helper busy with a page wanted now: the batch next frame.
        quiet && self.making.is_none() && self.start_making(gpu, want - have)
    }

    /// Takes the pages the helper has made so far.
    fn collect(&mut self) {
        let Some(rx) = &self.making else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(made) => {
                    self.spare.push(made);
                    self.in_flight = self.in_flight.saturating_sub(1);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.making = None;
                    self.in_flight = 0;
                    break;
                }
            }
        }
    }

    /// Makes `n` pages on a helper thread, unless it is already at work. False if it could
    /// not be started.
    fn start_making(&mut self, gpu: &Gpu, n: usize) -> bool {
        if self.making.is_some() {
            return true;
        }
        let (device, queue) = (gpu.device.clone(), gpu.queue.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = specular_thread::builder("gal-atlas").spawn(move || {
            let t = std::time::Instant::now();
            for _ in 0..n {
                if tx.send(make_page(&device, &queue)).is_err() {
                    break;
                }
            }
            debug!(target: UI, "gallery atlas: {n} pages made in {:.1} ms",
                    t.elapsed().as_secs_f64() * 1000.0);
        });
        if spawned.is_ok() {
            self.making = Some(rx);
            self.in_flight = n;
        }
        spawned.is_ok()
    }

    /// Submits this frame's uploads (before the frame's own commands, which sample them).
    pub fn end_frame(&mut self, gpu: &Gpu) {
        let Some(enc) = self.enc.take() else {
            return;
        };
        if let Some(belt) = self.belt.as_mut() {
            belt.finish();
        }
        gpu.queue.submit([enc.finish()]);
        if let Some(belt) = self.belt.as_mut() {
            belt.recall();
        }
    }

    /// A page is being made on the helper thread.
    pub fn growing(&self) -> bool {
        self.making.is_some()
    }

    pub fn pages(&self) -> usize {
        self.pages.len()
    }

    pub fn bytes(&self) -> u64 {
        self.pages.len() as u64 * PAGE as u64 * PAGE as u64 * 4
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    fn placed(&self, p: usize, s: usize) -> Placed {
        let page = &self.pages[p];
        let slot = page.slots[s].as_ref().expect("indexed slot");
        let per = page.per_row();
        let (sx, sy) = ((s as u32 % per) * page.class, (s as u32 / per) * page.class);
        let f = PAGE as f32;
        // Half a texel inside so linear filtering never reads the neighbouring slot.
        Placed {
            tex: page.id,
            page: p,
            uv: [
                (sx as f32 + 0.5) / f,
                (sy as f32 + 0.5) / f,
                (slot.width as f32 - 1.0) / f,
                (slot.height as f32 - 1.0) / f,
            ],
            width: slot.width,
            height: slot.height,
        }
    }

    /// The thumbnail of `path` at `class`, marked as drawn this frame.
    pub fn get(&mut self, path: &Path, class: u32) -> Option<Placed> {
        let &(p, s) = self.index.get(&(path.to_path_buf(), class))?;
        if let Some(slot) = self.pages[p].slots[s].as_mut() {
            slot.last = self.frame;
        }
        Some(self.placed(p, s))
    }

    /// The best thumbnail of `path` for `class`: that class, else the largest other one.
    pub fn best(&mut self, path: &Path, class: u32) -> Option<Placed> {
        if let Some(p) = self.get(path, class) {
            return Some(p);
        }
        specular_thumbs::CLASSES
            .iter()
            .rev()
            .filter(|&&c| c != class)
            .find_map(|&c| self.get(path, c))
    }

    pub fn contains(&self, path: &Path, class: u32) -> bool {
        self.index.contains_key(&(path.to_path_buf(), class))
    }

    pub fn is_sharp(&self, path: &Path, class: u32) -> bool {
        self.index
            .get(&(path.to_path_buf(), class))
            .and_then(|&(p, s)| self.pages[p].slots[s].as_ref())
            .is_some_and(|s| s.sharp)
    }

    /// Drops every class of `path` (the file changed).
    pub fn remove(&mut self, path: &Path) {
        for &c in &specular_thumbs::CLASSES {
            if let Some((p, s)) = self.index.remove(&(path.to_path_buf(), c)) {
                self.pages[p].slots[s] = None;
            }
        }
    }

    pub fn clear(&mut self) {
        self.index.clear();
        for p in &mut self.pages {
            p.slots.iter_mut().for_each(|s| *s = None);
        }
    }

    /// Releases every page (the gallery closed).
    pub fn release(&mut self, ui: &mut UiRenderer) {
        for p in self.pages.drain(..) {
            ui.unregister_texture(p.id);
        }
        self.spare.clear();
        self.making = None;
        self.in_flight = 0;
        self.enc = None;
        self.belt = None;
        self.index.clear();
    }

    /// Uploads `px` as the thumbnail of `path` at `class`. `None` when every slot of the
    /// class is in use this frame and no page can be added.
    pub fn insert(
        &mut self,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        path: &Path,
        class: u32,
        px: &Pixels,
        sharp: bool,
    ) -> Option<Placed> {
        let key = (path.to_path_buf(), class);
        let (p, s) = match self.index.get(&key) {
            Some(&at) => at,
            None => {
                let at = self.free_slot(gpu, ui, class)?;
                self.index.insert(key.clone(), at);
                at
            }
        };
        let (w, h) = (px.width.min(class), px.height.min(class));
        if w == 0 || h == 0 {
            return None;
        }
        let page = &mut self.pages[p];
        page.slots[s] = Some(Slot {
            key,
            width: w,
            height: h,
            sharp,
            last: self.frame,
        });
        let per = page.per_row();
        let (sx, sy) = ((s as u32 % per) * class, (s as u32 / per) * class);
        let (data, src_stride) = (upload_source(px), px.width as usize * 4);
        let row = w as usize * 4;
        let stride = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
        let size = (stride * h as usize) as u64;
        let belt = self
            .belt
            .get_or_insert_with(|| wgpu::util::StagingBelt::new(gpu.device.clone(), BELT_CHUNK));
        let enc = self.enc.get_or_insert_with(|| {
            gpu.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gallery uploads"),
                })
        });
        let slice = belt.allocate(
            wgpu::BufferSize::new(size)?,
            wgpu::BufferSize::new(wgpu::COPY_BUFFER_ALIGNMENT.max(256))?,
        );
        {
            let mut view = slice.get_mapped_range_mut().ok()?;
            for y in 0..h as usize {
                view.slice(y * stride..y * stride + row)
                    .copy_from_slice(&data[y * src_stride..y * src_stride + row]);
            }
        }
        enc.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: slice.buffer(),
                layout: wgpu::TexelCopyBufferLayout {
                    offset: slice.offset(),
                    bytes_per_row: Some(stride as u32),
                    rows_per_image: Some(h),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: &page._tex,
                mip_level: 0,
                origin: wgpu::Origin3d { x: sx, y: sy, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.uploads += 1;
        Some(self.placed(p, s))
    }

    fn free_slot(&mut self, gpu: &Gpu, ui: &mut UiRenderer, class: u32) -> Option<(usize, usize)> {
        // A free slot in a page of this class.
        for (pi, page) in self.pages.iter().enumerate() {
            if page.class == class
                && let Some(si) = page.slots.iter().position(Option::is_none)
            {
                return Some((pi, si));
            }
        }
        // A new page: one made ahead, or wait for it (the caller tries again next frame)
        // rather than allocate on this thread.
        if self.pages.len() < self.max_pages {
            let (tex, view) = match self.spare.pop() {
                Some(made) => made,
                // Wanted now: made on the helper (the caller tries again next frame), or
                // here if it could not be started.
                None if self.start_making(gpu, 1) => return None,
                None => make_page(&gpu.device, &gpu.queue),
            };
            let per = PAGE / class;
            let id = ui.register_texture(gpu, &view);
            self.pages.push(Page {
                _tex: tex,
                id,
                class,
                slots: (0..per * per).map(|_| None).collect(),
            });
            return Some((self.pages.len() - 1, 0));
        }
        // The least recently drawn slot of this class not drawn this frame.
        let frame = self.frame;
        let victim = self
            .pages
            .iter()
            .enumerate()
            .filter(|(_, p)| p.class == class)
            .flat_map(|(pi, p)| {
                p.slots
                    .iter()
                    .enumerate()
                    .filter_map(move |(si, s)| s.as_ref().map(|s| (s.last, pi, si)))
            })
            .filter(|(last, _, _)| *last < frame)
            .min_by_key(|e| e.0);
        if let Some((_, pi, si)) = victim {
            if let Some(s) = self.pages[pi].slots[si].take() {
                self.index.remove(&s.key);
            }
            return Some((pi, si));
        }
        // Repurpose the page of another class drawn least recently.
        let victim = self
            .pages
            .iter()
            .enumerate()
            .filter(|(_, p)| p.class != class)
            .map(|(pi, p)| {
                let last = p.slots.iter().flatten().map(|s| s.last).max().unwrap_or(0);
                (last, pi)
            })
            .filter(|(last, _)| *last < frame)
            .min_by_key(|e| e.0);
        let (_, pi) = victim?;
        let page = &mut self.pages[pi];
        for s in page.slots.iter_mut().filter_map(Option::take) {
            self.index.remove(&s.key);
        }
        let per = PAGE / class;
        page.class = class;
        page.slots = (0..per * per).map(|_| None).collect();
        Some((pi, 0))
    }
}

/// A new page, its memory initialised.
///
/// wgpu zero-fills a texture's memory before the first copy that covers only part of it:
/// for a page, 16 MB written by the GPU as part of the frame that uploads its first
/// thumbnail. A one-texel copy submitted here moves that fill to the helper, so the
/// draw thread's uploads find the page initialised.
fn make_page(device: &wgpu::Device, queue: &wgpu::Queue) -> Made {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("gallery atlas"),
        size: wgpu::Extent3d {
            width: PAGE,
            height: PAGE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let src = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gallery atlas init"),
        size: 4,
        usage: wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("gallery atlas init"),
    });
    enc.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
            buffer: &src,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            },
        },
        tex.as_image_copy(),
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    (tex, view)
}

/// The bytes the atlas takes: premultiplied (in linear light, as the shader blends) and
/// sRGB-encoded. The thumbnail engine made that copy on its worker; opaque thumbnails have
/// none and their own pixels are copied straight into the staging belt.
fn upload_source(px: &Pixels) -> &[u8] {
    debug_assert!(px.premul_known(), "alpha scan left to the draw thread");
    px.premultiplied().unwrap_or(&px.rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_thumbnails_upload_their_own_pixels() {
        let opaque = Pixels::opaque(2, 2, vec![255; 16]);
        assert!(std::ptr::eq(upload_source(&opaque), opaque.rgba.as_slice()));
        let px = Pixels::new(2, 1, vec![200, 100, 50, 255, 255, 255, 255, 0]);
        px.prepare();
        let out = upload_source(&px);
        assert_eq!(&out[..4], &[200, 100, 50, 255]);
        assert_eq!(&out[4..], &[0, 0, 0, 0]);
        let half = Pixels::new(1, 1, vec![255, 255, 255, 128]);
        half.prepare();
        // Linear 0.5 encodes to about 188 in sRGB.
        let out = upload_source(&half);
        assert!((185..=190).contains(&out[0]), "{out:?}");
    }

    /// While the view rests, the pages the folder wants are made in one helper run, none
    /// while it moves; a page arrives initialised to transparent black. Skips without a GPU.
    #[test]
    fn pages_are_made_ahead_in_one_run_and_arrive_cleared() {
        let Ok(gpu) = Gpu::new() else {
            eprintln!("no GPU, skipped");
            return;
        };
        let mut atlas = Atlas::new(4);
        atlas.begin_frame(&gpu, false, 3);
        assert!(!atlas.growing(), "nothing is made while the view moves");
        atlas.begin_frame(&gpu, true, 9);
        assert_eq!(atlas.in_flight, 4, "capped at the page budget");
        let t = std::time::Instant::now();
        while atlas.growing() && t.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            atlas.begin_frame(&gpu, true, 9);
        }
        assert_eq!(atlas.spare.len(), 4);
        assert!(!atlas.growing(), "nothing more once the budget is met");

        let (tex, _) = &atlas.spare[0];
        let row = PAGE * 4;
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row) * 2,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(2),
                },
            },
            wgpu::Extent3d {
                width: PAGE,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().expect("mapped");
        assert!(data.iter().all(|&b| b == 0));
    }

    /// Thumbnails uploaded on a device that is then lost: the next frame neither panics nor
    /// waits; a new atlas on a new device takes them again.
    #[test]
    fn a_lost_device_is_left_and_the_pages_made_again() {
        let new_gpu = || match Gpu::new() {
            Ok(g) => Some(g),
            Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_some() => panic!("{e}"),
            Err(_) => None,
        };
        let (Some(old), Some(new)) = (new_gpu(), new_gpu()) else {
            return;
        };
        // A page made ahead while the view rests, as the gallery does.
        let ready = |atlas: &mut Atlas, gpu: &Gpu| {
            atlas.begin_frame(gpu, true, 1);
            let t = std::time::Instant::now();
            while atlas.growing() && t.elapsed() < std::time::Duration::from_secs(10) {
                std::thread::sleep(std::time::Duration::from_millis(5));
                atlas.begin_frame(gpu, true, 1);
            }
        };
        let px = Pixels::opaque(64, 48, vec![200; 64 * 48 * 4]);
        let path = Path::new("/a.jpg");
        let mut ui = UiRenderer::new(
            &old,
            &specular_gfx::Compositor::new(&old, specular_gfx::HDR, 16, 64, 64),
        );
        let mut atlas = Atlas::new(4);
        ready(&mut atlas, &old);
        let before = atlas.insert(&old, &mut ui, path, 128, &px, true).unwrap();
        atlas.end_frame(&old);
        old.device.destroy();
        let t = std::time::Instant::now();
        atlas.begin_frame(&old, false, 1);
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
        // The gallery's reset: a new atlas on the new device.
        drop(atlas);
        let mut ui = UiRenderer::new(
            &new,
            &specular_gfx::Compositor::new(&new, specular_gfx::HDR, 16, 64, 64),
        );
        let mut atlas = Atlas::new(4);
        ready(&mut atlas, &new);
        let after = atlas.insert(&new, &mut ui, path, 128, &px, true).unwrap();
        atlas.end_frame(&new);
        assert_eq!(new.wait(), Ok(()));
        assert_ne!(before.tex, after.tex);
        assert_eq!((after.width, after.height), (64, 48));
    }
}
