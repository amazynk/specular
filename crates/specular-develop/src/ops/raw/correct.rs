//! The raw corrections in the front end ([`crate::ops::correct`] has the order and the
//! methods).
//!
//! 1. **Statistics** (opposed and rebuild only): the `rcell` pass reduces the stored samples
//!    (prepared and calibrated) to cells of whole CFA periods (4x4 Bayer, 6x6 X-Trans) with
//!    their clipped colours; `hltile` and `block` reduce those to tiles and blocks; the host
//!    reads them back and makes the chromaticity grid and the rebuild areas, which join the
//!    data buffer. They are kept per sensor, calibration and highlight settings
//!    ([`StatsKey`]), so nothing else reruns them. For the preview the read-back never
//!    blocks ([`Options::wait_stats`] false): the run uses the last grid of the same image
//!    (or a neutral one) and [`FrontEnd::stats_ready`] says when the new one has arrived.
//! 2. **Full size** (and the mosaic output): prepare (with calibration) → hot pixels →
//!    highlights → raw CA, in bands with halos like the demosaic, into the corrected mosaic
//!    (one f32 per crop pixel); the demosaic run then reads it instead of the stored samples
//!    (`U_PREPARED`).
//! 3. **Half size** (fit views): the same steps on the half-size pixels themselves, in three
//!    passes and no extra full-size buffer: `hcell` (the `half` window's colours, calibrated,
//!    clipped colours marked), `hhot` (hot pixels against the eight neighbouring pixels'
//!    same colour, then highlights with the pixel's own colours), `hout` (red and blue at
//!    half the raw CA shift, the output transform).
//!
//! When the statistics' buffers exceed one binding, the grid stays neutral (clipped
//! highlights then come out neutral, never magenta).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use specular_raw::SensorData;
use specular_raw::sensor::Layout;

use super::data::*;
use super::shaders::Pass;
use super::{Buf, Dst, FrontEnd, GpuSensor, Options, Output, Quality, Rec, Src, st};
use crate::gpu::Error;
use crate::ops::correct::calib::Calibration;
use crate::ops::correct::highlights::{
    self, BLOCK_CELLS, BLOCK_WORDS, BlockStats, Rebuild, TILE_CELLS, TILE_WORDS, TileStats,
};
use crate::ops::correct::{CaMode, HighlightMode};

/// What [`FrontEnd::correct`] recorded.
pub(crate) struct Corrected {
    /// The corrected mosaic (crop size, f32); `None` on the half-size path.
    pub mosaic: Option<wgpu::Buffer>,
    /// The correction run's dispatches (encoded with the final data buffer).
    pub recs: Vec<Rec>,
    pub stats_ms: Option<f64>,
    /// The statistics are still on their way (a neutral or older grid was used).
    pub pending: bool,
    /// Raw CA runs (the half-size path then needs its separate `hout` step).
    pub ca: bool,
}

/// Whether `o`'s corrections run on `s` (CFA sensors only).
pub(crate) fn applies(s: &SensorData, o: &Options) -> bool {
    o.corrections.any()
        && s.cpp == 1
        && matches!(s.meta.layout, Layout::Bayer(_) | Layout::Pattern(_))
}

/// Whether the corrections of `o` run on the half-size pixels.
pub(crate) fn half_path(o: &Options) -> bool {
    o.quality == Quality::Half && o.output != Output::Mosaic
}

/// Statistics cell side in pixels: whole CFA periods.
fn cell_size(l: &Layout) -> u32 {
    match l {
        Layout::Bayer(_) => 4,
        _ => 6,
    }
}

fn set(words: &mut [u32], i: usize, v: u32) {
    words[i] = v;
}

fn setf(words: &mut [u32], i: usize, v: f32) {
    words[i] = v.to_bits();
}

/// What the statistics depend on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StatsKey {
    sensor: u64,
    calibration: Option<u64>,
    mode: HighlightMode,
    threshold: f32,
}

/// The statistics' tables.
#[derive(Clone, Debug)]
pub(crate) struct StatTables {
    tw: u32,
    th: u32,
    ts: u32,
    grid: Vec<[f32; 3]>,
    /// Blocks wide and high, block size, the areas.
    rebuild: Option<(u32, u32, u32, Rebuild)>,
}

impl StatTables {
    fn append(&self, words: &mut Vec<u32>) {
        let at = words.len() as u32;
        words.extend(self.grid.iter().flatten().map(|v| v.to_bits()));
        set(words, H_HL_GRID, at);
        set(words, H_HL_GW, self.tw);
        set(words, H_HL_GH, self.th);
        set(words, H_HL_TS, self.ts);
        if let Some((bw, bh, bs, rb)) = &self.rebuild {
            let at = words.len() as u32;
            for (a, b) in rb.area.iter().zip(&rb.boost) {
                words.push(*a);
                words.push(b.to_bits());
            }
            set(words, H_RB_OFF, at);
            set(words, H_RB_BW, *bw);
            set(words, H_RB_BH, *bh);
            set(words, H_RB_BS, *bs);
            let at = words.len() as u32;
            words.extend(rb.chroma.iter().flatten().map(|v| v.to_bits()));
            set(words, H_RB_SEG, at);
        }
    }
}

/// Statistics on their way back from the GPU.
pub(crate) struct PendingStats {
    key: StatsKey,
    staging: wgpu::Buffer,
    bytes: u64,
    /// 0 waiting, 1 mapped, 2 failed.
    state: Arc<AtomicU8>,
    dims: (u32, u32, u32, u32, u32),
    rebuild: bool,
    started: Instant,
}

/// The statistics a front end keeps: the newest tables of at most two sensors (the image
/// shown and one developed ahead, `Developer::prefetch`) and what they were made from, and
/// a read-back under way.
#[derive(Default)]
pub(crate) struct StatsCache {
    /// Newest first, one per sensor.
    pub(crate) done: Vec<(StatsKey, Arc<StatTables>)>,
    pub(crate) pending: Option<PendingStats>,
}

impl StatsCache {
    fn has(&self, key: StatsKey) -> bool {
        self.done.iter().any(|d| d.0 == key)
    }

    fn keep(&mut self, key: StatsKey, t: Arc<StatTables>) {
        self.done.retain(|d| d.0.sensor != key.sensor);
        self.done.insert(0, (key, t));
        self.done.truncate(2);
    }
}

impl FrontEnd {
    /// Sets (or clears) the flat-field and dark-frame words of `s`.
    pub fn set_calibration(
        &mut self,
        s: &mut GpuSensor,
        cal: Option<&Calibration>,
    ) -> Result<(), Error> {
        s.cal = match cal {
            None => None,
            Some(c) => {
                if c.width != s.data.crop.w
                    || c.height != s.data.crop.h
                    || c.words.len() != c.width as usize * c.height as usize
                {
                    return Err(Error::BadRequest(
                        "the calibration does not match the sensor",
                    ));
                }
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("raw calibration"),
                    size: (c.words.len() as u64 * 4).div_ceil(16) * 16,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue
                    .write_buffer(&b, 0, bytemuck::cast_slice(&c.words));
                Some(b)
            }
        };
        s.cal_generation = next_id();
        Ok(())
    }

    /// Whether statistics a preview run started have arrived (polls the device, never
    /// waits): the caller then runs the front end again to use them.
    pub fn stats_ready(&mut self) -> bool {
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.stats
            .pending
            .as_ref()
            .is_some_and(|p| p.state.load(Ordering::Acquire) != 0)
    }

    /// Whether statistics are on their way.
    pub fn stats_pending(&self) -> bool {
        self.stats.pending.is_some()
    }

    /// [`FrontEnd::stats_ready`] for the statistics of `s` only.
    pub fn stats_ready_of(&mut self, s: &GpuSensor) -> bool {
        self.stats_pending_of(s) && self.stats_ready()
    }

    /// [`FrontEnd::stats_pending`] for the statistics of `s` only.
    pub fn stats_pending_of(&self, s: &GpuSensor) -> bool {
        self.stats
            .pending
            .as_ref()
            .is_some_and(|p| p.key.sensor == s.id)
    }

    /// The whole-mosaic buffer, at least `bytes` (kept between runs).
    fn mosaic_buffer(&mut self, bytes: u64) -> Result<wgpu::Buffer, Error> {
        if let Some(b) = &self.mosaic
            && b.size() >= bytes
        {
            return Ok(b.clone());
        }
        if bytes > self.max_buffer {
            return Err(Error::TooLarge { needs: bytes });
        }
        let device = self.device.clone();
        let b = super::scoped(&device, || {
            Ok(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("raw corrected mosaic"),
                size: bytes.div_ceil(256) * 256,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            }))
        })?;
        self.mosaic = Some(b.clone());
        Ok(b)
    }

    fn storage(&self, label: &'static str, bytes: u64, copy: bool) -> wgpu::Buffer {
        let mut usage = wgpu::BufferUsages::STORAGE;
        if copy {
            usage |= wgpu::BufferUsages::COPY_SRC;
        }
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: bytes.div_ceil(16) * 16,
            usage,
            mapped_at_creation: false,
        })
    }

    /// Takes the pending statistics if they have arrived (or waits for them with `wait`)
    /// into the cache.
    fn collect_stats(&mut self, wait: bool) -> Result<(), Error> {
        let Some(p) = self.stats.pending.as_ref() else {
            return Ok(());
        };
        if wait {
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| Error::Gpu(e.to_string()))?;
        } else {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        match p.state.load(Ordering::Acquire) {
            0 => return Ok(()),
            2 => {
                self.stats.pending = None;
                return Err(Error::Gpu("reading the statistics back failed".into()));
            }
            _ => {}
        }
        let Some(p) = self.stats.pending.take() else {
            return Ok(());
        };
        let all: Vec<f32> = {
            let v = p
                .staging
                .slice(..p.bytes)
                .get_mapped_range()
                .map_err(|e| Error::Gpu(format!("{e:?}")))?;
            bytemuck::pod_collect_to_vec(&v[..])
        };
        p.staging.unmap();
        let (tw, th, bw, bh, cs) = p.dims;
        let nt = (tw * th) as usize * TILE_WORDS;
        let grid = highlights::chroma_grid(&TileStats {
            w: tw,
            h: th,
            words: all[..nt].to_vec(),
        });
        let rebuild = p
            .rebuild
            .then(|| {
                let bs = BlockStats {
                    w: bw,
                    h: bh,
                    words: all[nt..nt + (bw * bh) as usize * BLOCK_WORDS].to_vec(),
                };
                highlights::rebuild(&bs, &grid, tw, th)
            })
            .filter(|rb| !rb.chroma.is_empty())
            .map(|rb| (bw, bh, BLOCK_CELLS * cs, rb));
        let t = StatTables {
            tw,
            th,
            ts: TILE_CELLS * cs,
            grid,
            rebuild,
        };
        self.stats.keep(p.key, Arc::new(t));
        self.last_stats_ms = Some(p.started.elapsed().as_secs_f64() * 1e3);
        Ok(())
    }

    /// Starts the statistics of `s` for `key` (see the module documentation); `false` when
    /// they don't fit the bindings.
    fn start_stats(
        &mut self,
        s: &GpuSensor,
        o: &Options,
        key: StatsKey,
        words: &[u32],
        cal: Option<&wgpu::Buffer>,
    ) -> Result<bool, Error> {
        let sd = &s.data;
        let (cw, ch) = (sd.crop.w, sd.crop.h);
        let cs = cell_size(&sd.meta.layout);
        let (nw, nh) = (cw.div_ceil(cs), ch.div_ceil(cs));
        let (tw, th) = (nw.div_ceil(TILE_CELLS), nh.div_ceil(TILE_CELLS));
        let (bw, bh) = (nw.div_ceil(BLOCK_CELLS), nh.div_ceil(BLOCK_CELLS));
        let cells_bytes = u64::from(nw) * u64::from(nh) * 8;
        if s.raw.size() > self.max_binding
            || cells_bytes > self.max_binding
            || cal.is_some_and(|c| c.size() > self.max_binding)
        {
            return Ok(false);
        }
        let rebuild = key.mode == HighlightMode::Rebuild;
        let mut w = words.to_vec();
        set(&mut w, H_CS, cs);
        set(&mut w, H_CELLS_W, nw);
        set(&mut w, H_CELLS_H, nh);
        let d0 = self.upload_words(&w);
        let cells = self.storage("raw stat cells", cells_bytes, false);
        let tile_bytes = u64::from(tw * th) * TILE_WORDS as u64 * 4;
        let block_bytes = u64::from(bw * bh) * BLOCK_WORDS as u64 * 4;
        let tiles = self.storage("raw stat tiles", tile_bytes, true);
        let blocks = self.storage("raw stat blocks", block_bytes.max(16), true);
        let dummy = (self.dummy.clone(), 0, 64);
        let whole = |b: &wgpu::Buffer| (b.clone(), 0, b.size());
        let u = |w: u32, h: u32| {
            let mut u = [0u32; 16];
            u[1] = h;
            u[2] = w;
            u[3] = h;
            u[10] = sd.height;
            u
        };
        let mut recs = vec![
            Rec {
                pass: Pass::RCell,
                precision: o.precision,
                u: u(nw, nh),
                bind: [
                    whole(&cells),
                    dummy.clone(),
                    dummy.clone(),
                    whole(&s.raw),
                    cal.map_or(dummy.clone(), whole),
                ],
                groups: [nw.div_ceil(super::WG), nh.div_ceil(super::WG)],
            },
            Rec {
                pass: Pass::HlTile,
                precision: o.precision,
                u: u(tw, th),
                bind: [
                    whole(&tiles),
                    whole(&cells),
                    dummy.clone(),
                    dummy.clone(),
                    dummy.clone(),
                ],
                groups: [tw.div_ceil(super::WG), th.div_ceil(super::WG)],
            },
        ];
        if rebuild {
            recs.push(Rec {
                pass: Pass::Block,
                precision: o.precision,
                u: u(bw, bh),
                bind: [
                    whole(&blocks),
                    whole(&cells),
                    dummy.clone(),
                    dummy.clone(),
                    dummy,
                ],
                groups: [bw.div_ceil(super::WG), bh.div_ceil(super::WG)],
            });
        }
        let bytes = tile_bytes + if rebuild { block_bytes } else { 0 };
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("raw stat read"),
            size: bytes.div_ceil(16) * 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("raw statistics"),
            });
        self.encode(&recs, &d0, &mut enc, None)?;
        enc.copy_buffer_to_buffer(&tiles, 0, &staging, 0, tile_bytes);
        if rebuild {
            enc.copy_buffer_to_buffer(&blocks, 0, &staging, tile_bytes, block_bytes);
        }
        self.queue.submit([enc.finish()]);
        let state = Arc::new(AtomicU8::new(0));
        let st2 = state.clone();
        staging
            .slice(..bytes)
            .map_async(wgpu::MapMode::Read, move |r| {
                st2.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
        self.stats.pending = Some(PendingStats {
            key,
            staging,
            bytes,
            state,
            dims: (tw, th, bw, bh, cs),
            rebuild,
            started: Instant::now(),
        });
        Ok(true)
    }

    /// The statistics for `key` into `words`: kept ones, arrived ones, or (waiting with
    /// `o.wait_stats`) new ones; otherwise the last tables of the same image, or none
    /// (neutral). Returns whether newer statistics are still on their way.
    fn stats_into(
        &mut self,
        s: &GpuSensor,
        o: &Options,
        key: StatsKey,
        words: &mut Vec<u32>,
        cal: Option<&wgpu::Buffer>,
    ) -> Result<bool, Error> {
        if !self.stats.has(key) {
            let waiting = self.stats.pending.as_ref().is_some_and(|p| p.key == key);
            if !waiting {
                // A read-back for other settings (or another image) is dropped.
                self.stats.pending = None;
                if !self.stats.has(key) && !self.start_stats(s, o, key, words, cal)? {
                    return Ok(false);
                }
            }
            #[cfg(test)]
            let late = self.stats_late && !waiting && !o.wait_stats;
            #[cfg(not(test))]
            let late = false;
            if !self.stats.has(key) && !late {
                self.collect_stats(o.wait_stats)?;
            }
        }
        match self.stats.done.iter().find(|d| d.0.sensor == key.sensor) {
            Some((k, t)) => {
                t.append(words);
                Ok(*k != key)
            }
            None => Ok(true),
        }
    }

    /// Records the corrections of `s` per `o` (see the module documentation); adds their
    /// header and tables to `words`.
    pub(crate) fn correct(
        &mut self,
        s: &GpuSensor,
        o: &Options,
        raw: &Src,
        words: &mut Vec<u32>,
    ) -> Result<Corrected, Error> {
        let sd = &s.data;
        let meta = &sd.meta;
        let c = o.corrections;
        let (cw, ch) = (sd.crop.w, sd.crop.h);
        // Float samples (HDR merges) run past the white level: they clip where the merge
        // says (its brightest frame's saturation), not at white.
        let clip = if s.float && meta.clip_white > 0.0 {
            meta.clip_white
        } else if meta.clip_white > 0.0 {
            meta.clip_white.min(meta.white)
        } else {
            meta.white
        };
        setf(words, H_CLIP, clip);
        let cal = s.cal.as_ref().filter(|_| c.calibrate);
        set(words, H_CAL, u32::from(cal.is_some()));
        setf(words, H_HOT, c.hot.unwrap_or(0.0));
        set(
            words,
            H_HL,
            match c.highlights {
                HighlightMode::Off => 0,
                HighlightMode::Clip => 1,
                HighlightMode::Opposed => 2,
                HighlightMode::Rebuild => 3,
            },
        );
        let threshold = c.highlight_threshold.clamp(0.5, 1.0);
        setf(words, H_HL_T, threshold);
        // Raw CA (Bayer and X-Trans).
        let fit = match c.ca {
            CaMode::Off => None,
            CaMode::Auto => s.ca_fit().map(|f| (f.red, f.blue)),
            CaMode::Manual { red, blue } => Some((
                crate::ops::correct::ca::Radial {
                    a: red / 100.0,
                    b: 0.0,
                },
                crate::ops::correct::ca::Radial {
                    a: blue / 100.0,
                    b: 0.0,
                },
            )),
        };
        let ca = fit.is_some();
        if let Some((r, b)) = fit {
            let (fw, fh) = (cw as f32, ch as f32);
            set(words, H_CA, 1);
            setf(words, H_CA_CX, (fw - 1.0) / 2.0);
            setf(words, H_CA_CY, (fh - 1.0) / 2.0);
            setf(words, H_CA_R, (fw * fw + fh * fh).sqrt() / 2.0);
            setf(words, H_CA_RA, r.a);
            setf(words, H_CA_RB, r.b);
            setf(words, H_CA_BA, b.a);
            setf(words, H_CA_BB, b.b);
        }
        let mut stats_ms = None;
        let mut pending = false;
        if o.stats
            && matches!(
                c.highlights,
                HighlightMode::Opposed | HighlightMode::Rebuild
            )
        {
            let t0 = Instant::now();
            let key = StatsKey {
                sensor: s.id,
                calibration: cal.map(|_| s.cal_generation),
                mode: c.highlights,
                threshold,
            };
            pending = self.stats_into(s, o, key, words, cal)?;
            stats_ms = Some(t0.elapsed().as_secs_f64() * 1e3);
        }
        if half_path(o) {
            return Ok(Corrected {
                mosaic: None,
                recs: Vec::new(),
                stats_ms,
                pending,
                ca,
            });
        }
        let mosaic = self.mosaic_buffer(u64::from(cw) * u64::from(ch) * 4)?;
        let src = Src {
            mosaic_domain: true,
            ..*raw
        };
        let o1 = Options { dual: None, ..*o };
        let mut recs = Vec::new();
        // X-Trans raw CA reads a green plane (made after the highlights, into A: free by
        // then) instead of tent-averaging green around every shifted site.
        let xt = ca && matches!(meta.layout, Layout::Pattern(_));
        // Preparation, hot pixels and highlights in one pass (each tile reads its samples
        // once), then raw CA.
        let mut steps = vec![st(Pass::CFuse, Buf::M, Buf::None, Buf::None, 12)];
        #[cfg(test)]
        if self.unfused_full {
            steps = vec![
                st(Pass::CPrep, Buf::A, Buf::None, Buf::None, 15),
                st(Pass::Hot, Buf::B, Buf::A, Buf::None, 13),
                st(Pass::Hl, Buf::M, Buf::B, Buf::None, 12),
            ];
        }
        if xt {
            steps.push(st(Pass::XtG, Buf::A, Buf::M, Buf::None, 4));
            steps.push(st(Pass::Ca, Buf::Out, Buf::M, Buf::A, 0));
        } else {
            steps.push(st(Pass::Ca, Buf::Out, Buf::M, Buf::None, 0));
        }
        self.bands(
            &steps,
            (cw, ch),
            &src,
            &Dst {
                buf: &mosaic,
                bpp: 4,
                rows: None,
            },
            cal,
            &o1,
            &mut recs,
        )?;
        Ok(Corrected {
            mosaic: Some(mosaic),
            recs,
            stats_ms,
            pending,
            ca,
        })
    }
}

/// The half-size steps with corrections: one fused pass, or two with raw CA (its shifted
/// samples need the corrected neighbours first).
/// `xtrans`: the passes that read each X-Trans sample once per workgroup.
pub(crate) fn half_steps(ca: bool, xtrans: bool) -> Vec<super::Step> {
    let (fuse, fuseb) = if xtrans {
        (Pass::HFuseX, Pass::HFuseBX)
    } else {
        (Pass::HFuse, Pass::HFuseB)
    };
    if ca {
        vec![
            st(fuseb, Buf::B, Buf::None, Buf::None, 5),
            st(Pass::HOut, Buf::Out, Buf::B, Buf::None, 0),
        ]
    } else {
        vec![st(fuse, Buf::Out, Buf::None, Buf::None, 0)]
    }
}

/// The three-pass half-size steps the fused ones replace (tests compare them).
#[cfg(test)]
pub(crate) fn half_steps_unfused() -> Vec<super::Step> {
    vec![
        st(Pass::HCell, Buf::A, Buf::None, Buf::None, 6),
        st(Pass::HHot, Buf::B, Buf::A, Buf::None, 5),
        st(Pass::HOut, Buf::Out, Buf::B, Buf::None, 0),
    ]
}

/// A new process-wide id (sensors, calibrations).
pub(crate) fn next_id() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed)
}
