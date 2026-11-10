//! `--bench-loupe SECS`: the loupe (tapped on, so it stays) over the image at fit, driven by
//! a simulated pointer. First the pointer jumps to [`JUMPS`] places in turn, each timed until
//! the loupe is sharp there; then it sweeps a fixed path for SECS seconds at
//! [`EVENTS_PER_S`] pointer events a second, one sample per presented frame.

use std::time::{Duration, Instant};

use super::{ActiveEventLoop, App, ControlFlow};

/// Pointer events a second while sweeping (a common mouse report rate).
const EVENTS_PER_S: f64 = 250.0;
/// Sweep speed, logical px a second (an unhurried hover across a photo).
const SWEEP_SPEED: f64 = 600.0;
/// Where the jumps put the pointer (fractions of the image on screen).
const JUMPS: [(f32, f32); 6] = [
    (0.30, 0.30),
    (0.70, 0.62),
    (0.48, 0.52),
    (0.22, 0.78),
    (0.80, 0.22),
    (0.62, 0.40),
];
/// A jump that is not sharp after this long counts as stuck (and the bench goes on).
const JUMP_LIMIT: Duration = Duration::from_secs(30);

/// Tile and scene counters for the loupe bench and gate (always kept; a few adds a frame).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TileStats {
    /// Scene redraws (the image under the chrome and the loupe).
    pub scenes: u64,
    /// `request_tiles` calls and their main-thread time.
    pub calls: u64,
    pub ns: u64,
    /// Tile jobs started and cancelled.
    pub started: u64,
    pub cancelled: u64,
    /// Tiles the loupe still needs (missing or in flight) after the last call.
    pub loupe_missing: usize,
}

pub(super) struct LoupeBench {
    /// The sweep has run its time.
    pub done: bool,
    /// The frame just presented, for [`App::loupe_bench_presented`].
    pub pending: Option<FrameParts>,
    secs: f64,
    jump: usize,
    jump_t: Instant,
    jump_first: Option<f64>,
    /// Per jump: [ms to the first frame with the loupe there, ms until it is sharp].
    jumps: Vec<[f64; 2]>,
    sweep: Option<Instant>,
    events: u64,
    events_at_frame: u64,
    last_present: Option<Instant>,
    stats_at_frame: TileStats,
    /// Per presented frame while sweeping: [s, frame ms, present interval ms, request_tiles
    /// ms since the last frame, its calls, scene ms, ui ms, submit+present ms, acquire ms,
    /// loupe tiles missing, jobs started, jobs cancelled, pointer events].
    frames: Vec<[f64; 13]>,
    /// Loupe-level tile decodes: (ms, tiles).
    decodes: Vec<(f64, usize)>,
    uploads: Vec<f64>,
}

/// Parts of a frame's main-thread time (ms).
pub(super) struct FrameParts {
    pub total: f64,
    pub scene: f64,
    pub ui: f64,
    pub present: f64,
    pub acquire: f64,
}

impl App {
    /// Starts the loupe bench (the image is sharp at fit).
    pub(super) fn start_loupe_bench(&mut self, secs: f64) {
        let now = Instant::now();
        self.loupe_latched = true;
        let b = LoupeBench {
            done: false,
            pending: None,
            secs,
            jump: 0,
            jump_t: now,
            jump_first: None,
            jumps: Vec::new(),
            sweep: None,
            events: 0,
            events_at_frame: 0,
            last_present: None,
            stats_at_frame: self.tile_stats,
            frames: Vec::new(),
            decodes: Vec::new(),
            uploads: Vec::new(),
        };
        if let Some(bench) = self.bench.as_mut() {
            bench.loupe = Some(b);
        }
        self.loupe_jump(0);
    }

    fn loupe_bench(&mut self) -> Option<&mut LoupeBench> {
        self.bench.as_mut().and_then(|b| b.loupe.as_mut())
    }

    /// The image's rectangle on screen, clipped to the viewport (physical px).
    fn loupe_bench_area(&self) -> (f32, f32, f32, f32) {
        let vp = self.view.viewport;
        let (dw, dh) = self.view.display_size();
        let a = self.view.display_to_screen((0.0, 0.0));
        let b = self.view.display_to_screen((dw, dh));
        let x0 = a.0.min(b.0).max(vp.x);
        let y0 = a.1.min(b.1).max(vp.y);
        let x1 = a.0.max(b.0).min(vp.x + vp.w);
        let y1 = a.1.max(b.1).min(vp.y + vp.h);
        (x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0))
    }

    fn loupe_jump(&mut self, i: usize) {
        let (x, y, w, h) = self.loupe_bench_area();
        let (fx, fy) = JUMPS[i];
        self.on_cursor_moved((x + fx * w, y + fy * h));
        if let Some(b) = self.loupe_bench() {
            b.jump = i;
            b.jump_t = Instant::now();
            b.jump_first = None;
        }
    }

    /// Pointer events due by now (sweep), from `about_to_wait`.
    pub(super) fn drive_loupe_bench(&mut self) {
        let Some(b) = self.loupe_bench() else { return };
        let Some(t0) = b.sweep else { return };
        let due = (t0.elapsed().as_secs_f64() * EVENTS_PER_S) as u64;
        let from = b.events;
        if due <= from {
            return;
        }
        b.events = due;
        let (x, y, w, h) = self.loupe_bench_area();
        let ui = f64::from(self.dl.scale);
        for e in from + 1..=due {
            let p = sweep_at(e as f64 / EVENTS_PER_S, (w as f64, h as f64), ui);
            self.on_cursor_moved((x + p.0 as f32, y + p.1 as f32));
        }
    }

    /// While sweeping, wake for the next pointer event even when a frame is pending.
    pub(super) fn loupe_bench_control_flow(&self, el: &ActiveEventLoop) {
        let Some(b) = self.bench.as_ref().and_then(|b| b.loupe.as_ref()) else {
            return;
        };
        if let Some(t0) = b.sweep {
            let next = t0 + Duration::from_secs_f64((b.events + 1) as f64 / EVENTS_PER_S);
            let at = match el.control_flow() {
                ControlFlow::WaitUntil(t) => t.min(next),
                ControlFlow::Poll => return,
                ControlFlow::Wait => next,
            };
            el.set_control_flow(ControlFlow::WaitUntil(at));
        }
    }

    /// A tile result arrived: kept when it is the loupe's level.
    pub(super) fn loupe_bench_tiles(&mut self, level: u8, ms: f64, tiles: usize, upload: f64) {
        let lv = self.loupe_view().map(|l| l.0);
        let Some(v) = lv else { return };
        let ll = specular_render::tiles::level_for_scale(v.scale(), u8::MAX);
        if level != ll {
            return;
        }
        if let Some(b) = self.loupe_bench() {
            b.decodes.push((ms, tiles));
            b.uploads.push(upload);
        }
    }

    /// After each presented frame; whether the bench is done.
    pub(super) fn loupe_bench_presented(&mut self, parts: FrameParts) -> bool {
        let stats = self.tile_stats;
        let tiles_done = stats.loupe_missing == 0;
        let Some(b) = self.loupe_bench() else {
            return false;
        };
        let now = Instant::now();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        match b.sweep {
            None => {
                let since = ms(now - b.jump_t);
                b.jump_first.get_or_insert(since);
                let stuck = now - b.jump_t > JUMP_LIMIT;
                if !(tiles_done || stuck) {
                    return false;
                }
                let first = b.jump_first.unwrap_or(since);
                // A stuck jump counts its whole wait.
                b.jumps.push([first, since]);
                let next = b.jump + 1;
                if next < JUMPS.len() {
                    self.loupe_jump(next);
                } else {
                    b.sweep = Some(now);
                    b.events = 0;
                    b.events_at_frame = 0;
                    b.stats_at_frame = stats;
                    b.last_present = Some(now);
                    b.decodes.clear();
                    b.uploads.clear();
                }
                self.redraw = true;
                false
            }
            Some(t0) => {
                let d = |a: u64, b: u64| a.saturating_sub(b) as f64;
                let s0 = b.stats_at_frame;
                b.frames.push([
                    (now - t0).as_secs_f64(),
                    parts.total,
                    b.last_present.map_or(0.0, |l| ms(now - l)),
                    d(stats.ns, s0.ns) / 1e6,
                    d(stats.calls, s0.calls),
                    parts.scene,
                    parts.ui,
                    parts.present,
                    parts.acquire,
                    stats.loupe_missing as f64,
                    d(stats.started, s0.started),
                    d(stats.cancelled, s0.cancelled),
                    d(b.events, b.events_at_frame),
                ]);
                b.stats_at_frame = stats;
                b.events_at_frame = b.events;
                b.last_present = Some(now);
                (now - t0).as_secs_f64() >= b.secs
            }
        }
    }
}

/// The sweep's pointer position at `t` s in an area of `size` physical px (`ui` physical px
/// per logical px): a Lissajous curve over the middle 80 %, at about [`SWEEP_SPEED`].
fn sweep_at(t: f64, size: (f64, f64), ui: f64) -> (f64, f64) {
    use std::f64::consts::TAU;
    let (ax, ay) = (size.0 * 0.4, size.1 * 0.4);
    // Mean speed of (ax sin(a t), ay sin(b t)) is about (2/π)·hypot(a·ax, b·ay).
    let base = (ax.powi(2) + (0.77 * ay).powi(2)).sqrt() * 2.0 / std::f64::consts::PI;
    let w = SWEEP_SPEED * ui / base.max(1.0);
    (
        size.0 * 0.5 + ax * (w * t).sin(),
        size.1 * 0.5 + ay * (0.77 * w * t + TAU / 8.0).sin(),
    )
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(((v.len() as f64 - 1.0) * q).round() as usize)
        .copied()
        .unwrap_or(0.0)
}

/// The `"loupe"` part of the bench report.
pub(super) fn report(b: &LoupeBench) -> String {
    let refresh = 1000.0 / 60.0;
    let col = |i: usize| -> Vec<f64> { b.frames.iter().skip(1).map(|f| f[i]).collect() };
    let dist = |mut v: Vec<f64>| {
        format!(
            "{{\"p50\": {:.2}, \"p95\": {:.2}, \"max\": {:.2}}}",
            pct(&mut v, 0.5),
            pct(&mut v, 0.95),
            pct(&mut v, 1.0)
        )
    };
    let n = b.frames.len().saturating_sub(1);
    let span = b.frames.last().map_or(0.0, |f| f[0]) - b.frames.first().map_or(0.0, |f| f[0]);
    let sum = |i: usize| col(i).iter().sum::<f64>();
    let over = col(1).iter().filter(|&&v| v > refresh).count();
    let gaps = col(2).iter().filter(|&&v| v > refresh * 1.5).count();
    let blurry = col(9).iter().filter(|&&v| v > 0.0).count();
    let mut jf: Vec<f64> = b.jumps.iter().map(|j| j[0]).collect();
    let mut js: Vec<f64> = b.jumps.iter().map(|j| j[1]).collect();
    let dec: Vec<f64> = b.decodes.iter().map(|d| d.0).collect();
    let tiles: usize = b.decodes.iter().map(|d| d.1).sum();
    let list = |v: &[f64]| {
        v.iter()
            .map(|x| format!("{x:.1}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut s = String::from("  \"loupe\": {\n");
    s += &format!(
        "    \"jumps_ms\": {{\"first_frame\": [{}], \"sharp\": [{}]}},\n",
        list(&b.jumps.iter().map(|j| j[0]).collect::<Vec<_>>()),
        list(&b.jumps.iter().map(|j| j[1]).collect::<Vec<_>>())
    );
    s += &format!(
        "    \"jump_first_frame_p50_ms\": {:.1}, \"jump_sharp_p50_ms\": {:.1}, \"jump_sharp_max_ms\": {:.1},\n",
        pct(&mut jf, 0.5),
        pct(&mut js, 0.5),
        pct(&mut js, 1.0)
    );
    s += &format!(
        "    \"sweep\": {{\"secs\": {:.1}, \"frames\": {n}, \"fps\": {:.1}, \"pointer_events\": {}, \"events_per_s\": {:.0},\n",
        span,
        if span > 0.0 { n as f64 / span } else { 0.0 },
        b.events,
        if span > 0.0 {
            b.events as f64 / span
        } else {
            0.0
        }
    );
    s += &format!(
        "      \"frame_cpu_ms\": {}, \"over_16_7\": {over},\n",
        dist(col(1))
    );
    s += &format!(
        "      \"present_interval_ms\": {}, \"over_25\": {gaps},\n",
        dist(col(2))
    );
    s += &format!(
        "      \"request_tiles_ms_per_frame\": {}, \"request_tiles_calls\": {:.0},\n",
        dist(col(3)),
        sum(4)
    );
    s += &format!(
        "      \"scene_ms\": {}, \"ui_ms\": {}, \"submit_present_ms\": {}, \"acquire_ms\": {},\n",
        dist(col(5)),
        dist(col(6)),
        dist(col(7)),
        dist(col(8))
    );
    s += &format!(
        "      \"frames_loupe_not_sharp\": {blurry}, \"tile_jobs_started\": {:.0}, \"tile_jobs_cancelled\": {:.0},\n",
        sum(10),
        sum(11)
    );
    s += &format!(
        "      \"loupe_decodes\": {}, \"loupe_tiles\": {tiles}, \"loupe_decode_ms\": {}, \"loupe_upload_ms\": {}}}\n",
        b.decodes.len(),
        dist(dec),
        dist(b.uploads.clone())
    );
    s += "  },\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_stays_inside_and_moves_at_about_the_speed() {
        let size = (1600.0, 900.0);
        let mut len = 0.0;
        let mut prev = sweep_at(0.0, size, 1.0);
        let steps = 10_000;
        for i in 1..=steps {
            let p = sweep_at(i as f64 * 10.0 / steps as f64, size, 1.0);
            assert!(p.0 >= 0.0 && p.0 <= size.0 && p.1 >= 0.0 && p.1 <= size.1);
            len += ((p.0 - prev.0).powi(2) + (p.1 - prev.1).powi(2)).sqrt();
            prev = p;
        }
        let speed = len / 10.0;
        assert!((speed - SWEEP_SPEED).abs() < SWEEP_SPEED * 0.3, "{speed}");
    }
}
