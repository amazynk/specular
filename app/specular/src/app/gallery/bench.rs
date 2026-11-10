//! `--show gallery-bench` (with `--screenshot`): once the first screen is sharp, scrolls the
//! grid at a steady fling speed and reports fill times, frame times, draw batches and memory
//! as one JSON line on stderr. The screenshot is taken when it is done.

use std::time::Instant;

use specular_log::info;

use super::Gallery;
use super::UI;

/// Scroll step per frame (logical px): 2400 px/s at 60 Hz, a hard fling.
const STEP: f32 = 40.0;
/// Frames to measure.
const FRAMES: usize = 600;

#[derive(Default)]
pub(super) struct Bench {
    last: Option<Instant>,
    /// Time between drawn frames (ms).
    intervals: Vec<f32>,
    /// The gallery's own CPU time per frame (ms), from `note_frame`.
    cpu_from: usize,
    /// Most draw batches in one frame.
    max_batches: usize,
    pub done: bool,
}

fn pct(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    s[((s.len() - 1) as f32 * p).round() as usize]
}

fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb >> 10)
}

impl Gallery {
    /// One bench frame; true while it runs (draw again at once).
    pub(super) fn bench_step(&mut self, batches: usize) -> bool {
        let Some(mut b) = self.bench.take() else {
            return false;
        };
        let running = self.bench_frame(&mut b, batches);
        self.bench = Some(b);
        running
    }

    fn bench_frame(&mut self, b: &mut Bench, batches: usize) -> bool {
        if b.done || self.stats.sharp.is_none() {
            return !b.done;
        }
        let now = Instant::now();
        match b.last {
            None => b.cpu_from = self.stats.frames.len(),
            Some(t) => b.intervals.push(t.elapsed().as_secs_f32() * 1000.0),
        }
        b.last = Some(now);
        if b.intervals.len() % 100 == 99 {
            info!(target: UI, "gallery bench: {} frames, p50 {:.1} ms, max {:.1} ms, scroll {:.0}",
                b.intervals.len() + 1, pct(&b.intervals, 0.5), pct(&b.intervals, 1.0), self.scroll);
        }
        b.max_batches = b.max_batches.max(batches);
        let max = self.layout.clamp_scroll(f32::MAX, self.grid_h());
        let at_end = self.scroll >= max - 0.5;
        if b.intervals.len() >= FRAMES || at_end && !b.intervals.is_empty() {
            b.done = true;
            let cpu: Vec<f32> = self.stats.frames.get(b.cpu_from..).unwrap_or(&[]).to_vec();
            let ms = |d: Option<std::time::Duration>| d.map_or(-1.0, |d| d.as_secs_f64() * 1000.0);
            let atlas = self.atlas.as_ref();
            let line = format!(
                "{{\"items\":{},\"first_grid_ms\":{:.1},\"filled_ms\":{:.1},\"sharp_ms\":{:.1},\
                 \"scroll_frames\":{},\"interval_p50_ms\":{:.2},\"interval_p95_ms\":{:.2},\"interval_max_ms\":{:.2},\
                 \"cpu_p50_ms\":{:.3},\"cpu_p95_ms\":{:.3},\"cpu_max_ms\":{:.3},\"max_batches\":{},\
                 \"atlas_pages\":{},\"atlas_thumbs\":{},\"atlas_mb\":{},\"rss_mb\":{}}}",
                self.tiles.len(),
                ms(self.stats.first_grid),
                ms(self.stats.filled),
                ms(self.stats.sharp),
                b.intervals.len(),
                pct(&b.intervals, 0.5),
                pct(&b.intervals, 0.95),
                pct(&b.intervals, 1.0),
                pct(&cpu, 0.5),
                pct(&cpu, 0.95),
                pct(&cpu, 1.0),
                b.max_batches,
                atlas.map_or(0, |a| a.pages()),
                atlas.map_or(0, |a| a.len()),
                atlas.map_or(0, |a| a.bytes() >> 20),
                rss_mb(),
            );
            info!(target: UI, "gallery bench: {line}");
            eprintln!("gallery-bench {line}");
            return false;
        }
        self.scroll = (self.scroll + STEP).min(max);
        self.scroll_target = self.scroll;
        true
    }
}
