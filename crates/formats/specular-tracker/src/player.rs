//! The sequencer and mixer: rows and ticks (a tick is 2.5 / tempo seconds), effects in
//! Scream Tracker's letters, envelopes and fade-out, and a linearly interpolating stereo
//! mixer. The player is `Clone`: a copy is a resume point.

use std::sync::Arc;

use crate::{
    Cell, Envelope, FX_ENVELOPE_POS, FX_FINE_VOL_DOWN, FX_FINE_VOL_UP, FX_KEY_OFF, FX_MOD_VOLSLIDE,
    FX_SET_VOLUME, Kind, LoopKind, Module, NOTE_CUT, NOTE_FADE, NOTE_OFF, VolCmd,
};

/// Amiga clock in Scream Tracker's period units (four to ProTracker's): a period of 1712
/// plays at 8363 Hz.
const AMIGA_CLOCK: f64 = 14_317_456.0;
/// The most rows a song plays before it is taken to loop for ever.
const MAX_ROWS: u64 = 1 << 20;

/// A sine of 64 steps, ±255.
fn sine(pos: u8) -> i32 {
    const T: [i16; 32] = [
        0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250,
        244, 235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24,
    ];
    let p = usize::from(pos & 63);
    let v = i32::from(T[p & 31]);
    if p < 32 { v } else { -v }
}

/// The last parameter of each effect, by its letter.
#[derive(Clone, Debug)]
struct Mem([u8; 128]);

impl Default for Mem {
    fn default() -> Mem {
        Mem([0; 128])
    }
}

#[derive(Clone, Debug, Default)]
struct Chan {
    /// 1-based sample and instrument, 0 none.
    sample: usize,
    instr: usize,
    active: bool,
    pos: f64,
    backwards: bool,
    period: f64,
    target: f64,
    note: u8,
    vol: i32,
    chan_vol: i32,
    pan: i32,
    key_on: bool,
    fade: i32,
    fading: bool,
    env_vol: u32,
    env_pan: u32,
    /// The last parameter of each effect letter.
    mem: Mem,
    vib_pos: u8,
    trem_pos: u8,
    vib_speed: u8,
    vib_depth: u8,
    /// This row's cell and the tick its note waits for (SDx).
    cell: Cell,
    delay: u32,
    retrig: u32,
    loop_row: usize,
    loop_count: u8,
    /// What the tick plays: period and volume after vibrato, tremolo and arpeggio.
    play_period: f64,
    play_vol: i32,
    /// The gains last used (ramped towards the new ones over a tick).
    gain: (f32, f32),
}

/// A module being played.
#[derive(Clone)]
pub struct Player {
    m: Arc<Module>,
    rate: u32,
    order: usize,
    row: usize,
    tick: u32,
    speed: u32,
    tempo: u32,
    global: i32,
    pattern_delay: u32,
    next: Option<(usize, usize)>,
    visited: Vec<bool>,
    rows_played: u64,
    ended: bool,
    chans: Vec<Chan>,
    /// Frames left in the current tick (fractional).
    left: f64,
    started: bool,
}

impl Player {
    pub fn new(m: Arc<Module>, rate: u32) -> Player {
        let chans = (0..m.channels)
            .map(|c| Chan {
                pan: i32::from(m.pans.get(c).copied().unwrap_or(128)),
                chan_vol: i32::from(m.channel_volumes.get(c).copied().unwrap_or(64)),
                fade: 65_536,
                ..Chan::default()
            })
            .collect();
        let mut p = Player {
            rate: rate.max(1000),
            order: 0,
            row: 0,
            tick: 0,
            speed: u32::from(m.speed.max(1)),
            tempo: u32::from(m.tempo.max(32)),
            global: i32::from(m.global_volume),
            pattern_delay: 0,
            next: None,
            visited: vec![false; m.orders.len() * 256],
            rows_played: 0,
            ended: false,
            chans,
            left: 0.0,
            started: false,
            m,
        };
        p.order = p.skip_markers(0).unwrap_or(usize::MAX);
        if p.order == usize::MAX {
            p.ended = true;
        }
        p
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    /// The first playable order at or after `o`.
    fn skip_markers(&self, mut o: usize) -> Option<usize> {
        while o < self.m.orders.len() {
            match self.m.orders[o] {
                255 => return None,
                p if usize::from(p) < self.m.patterns.len() => return Some(o),
                _ => o += 1,
            }
        }
        None
    }

    fn tick_frames(&self) -> f64 {
        f64::from(self.rate) * 2.5 / f64::from(self.tempo)
    }

    /// Renders up to `frames` stereo frames (interleaved) to `out`; the frames rendered
    /// (fewer only at the song's end).
    pub fn render(&mut self, frames: usize, out: &mut Vec<f32>) -> usize {
        self.run(frames as u64, Some(out)) as usize
    }

    /// Plays `frames` frames without mixing (positions still move); the frames played.
    pub fn skip(&mut self, frames: u64) -> u64 {
        self.run(frames, None)
    }

    fn run(&mut self, frames: u64, mut out: Option<&mut Vec<f32>>) -> u64 {
        let mut done = 0u64;
        while done < frames && !self.ended {
            if self.left < 1.0 {
                if self.started {
                    self.advance_tick();
                    if self.ended {
                        break;
                    }
                } else {
                    self.started = true;
                    self.enter_row();
                    if self.ended {
                        break;
                    }
                    self.process_tick();
                }
                self.left += self.tick_frames();
            }
            let n = (self.left.floor() as u64).min(frames - done).max(1);
            match out.as_deref_mut() {
                Some(o) => self.mix(n as usize, o),
                None => self.advance(n as usize),
            }
            self.left -= n as f64;
            done += n;
        }
        done
    }

    /// The next tick: the next row when the row's ticks are done.
    fn advance_tick(&mut self) {
        self.tick += 1;
        if self.tick >= self.speed * (1 + self.pattern_delay) {
            self.tick = 0;
            self.pattern_delay = 0;
            self.next_row();
            if self.ended {
                return;
            }
            self.enter_row();
            if self.ended {
                return;
            }
        }
        self.process_tick();
    }

    fn next_row(&mut self) {
        let rows = self.pattern_rows();
        let (o, r) = match self.next.take() {
            Some(j) => j,
            None if self.row + 1 >= rows => (self.order + 1, 0),
            None => (self.order, self.row + 1),
        };
        let o = if o >= self.m.orders.len() || self.m.orders[o] == 255 {
            // The song's end (or its restart, which is a repeat).
            self.ended = true;
            return;
        } else {
            match self.skip_markers(o) {
                Some(o) => o,
                None => {
                    self.ended = true;
                    return;
                }
            }
        };
        self.order = o;
        self.row = r;
        if self.row >= self.pattern_rows() {
            self.row = 0;
        }
    }

    fn pattern_rows(&self) -> usize {
        self.m
            .orders
            .get(self.order)
            .and_then(|&p| self.m.patterns.get(usize::from(p)))
            .map_or(64, |p| p.rows)
    }

    /// Reads the row's cells (and stops a song that comes back to a row it played).
    fn enter_row(&mut self) {
        self.rows_played += 1;
        let key = self.order * 256 + self.row.min(255);
        let looping = self.chans.iter().any(|c| c.loop_count > 0);
        if self.rows_played > MAX_ROWS || (!looping && self.visited.get(key) == Some(&true)) {
            self.ended = true;
            return;
        }
        if let Some(v) = self.visited.get_mut(key) {
            *v = true;
        }
        let p = usize::from(self.m.orders[self.order]);
        let n = self.m.channels;
        for c in 0..n {
            let cell = self.m.patterns[p]
                .cells
                .get(self.row * n + c)
                .copied()
                .unwrap_or_default();
            let ch = &mut self.chans[c];
            ch.cell = cell;
            ch.delay = 0;
            if cell.fx == b'S' && cell.param >> 4 == 0xD {
                ch.delay = u32::from(cell.param & 15);
            }
        }
    }

    /// An effect's parameter with the channel's memory (0: the last one).
    fn param(&mut self, c: usize, fx: u8, p: u8) -> u8 {
        let mem = &mut self.chans[c].mem.0[usize::from(fx & 127)];
        if p != 0 {
            *mem = p;
            p
        } else if self.m.fx_memory {
            *mem
        } else {
            0
        }
    }

    fn sample_of(&self, c: usize) -> Option<&crate::Sample> {
        self.m.samples.get(self.chans[c].sample.checked_sub(1)?)
    }

    fn period_of(&self, note: u8, c_speed: f64) -> f64 {
        if self.m.linear {
            7680.0 - f64::from(note) * 64.0
        } else {
            1712.0 * 8363.0 / c_speed.max(1.0) * 2f64.powf((60.0 - f64::from(note)) / 12.0)
        }
    }

    fn freq(&self, c: usize, period: f64) -> f64 {
        let Some(s) = self.sample_of(c) else {
            return 0.0;
        };
        if self.m.linear {
            s.c_speed * 2f64.powf((3840.0 - period) / 768.0)
        } else {
            AMIGA_CLOCK / period.max(1.0)
        }
    }

    /// A note, instrument and volume column starting on this tick.
    fn trigger(&mut self, c: usize) {
        let cell = self.chans[c].cell;
        let porta = matches!(cell.fx, b'G' | b'L') || matches!(cell.vol, VolCmd::TonePorta(_));
        if cell.instr > 0 {
            let i = usize::from(cell.instr);
            if self.m.use_instruments {
                self.chans[c].instr = i;
            } else {
                self.chans[c].sample = i;
            }
        }
        let mut note = cell.note;
        let mut sample = self.chans[c].sample;
        if note < 120 && self.m.use_instruments {
            let i = self.chans[c].instr;
            match self.m.instruments.get(i.wrapping_sub(1)) {
                Some(ins) => {
                    let (s, n) = ins
                        .keymap
                        .get(usize::from(note))
                        .copied()
                        .unwrap_or((0, note));
                    sample = usize::from(s);
                    note = n;
                }
                None => sample = 0,
            }
        }
        if note < 120 && porta && self.chans[c].active {
            let cs = self.sample_of(c).map_or(8363.0, |s| s.c_speed);
            self.chans[c].target = self.period_of(note, cs);
            self.chans[c].note = note;
        } else if note < 120 {
            let Some(s) = self.m.samples.get(sample.wrapping_sub(1)) else {
                self.chans[c].active = false;
                return;
            };
            let period = self.period_of(note, s.c_speed);
            let ch = &mut self.chans[c];
            ch.sample = sample;
            ch.note = note;
            ch.period = period;
            ch.target = period;
            ch.pos = 0.0;
            ch.backwards = false;
            ch.active = !s.data.is_empty();
            ch.vib_pos = 0;
            ch.trem_pos = 0;
            if cell.fx == b'O' {
                let p = self.param(c, b'O', cell.param);
                let off = f64::from(p) * 256.0;
                let ch = &mut self.chans[c];
                ch.pos = off;
                if let Some(s) = self.m.samples.get(sample - 1)
                    && off >= s.data.len() as f64
                {
                    ch.active = s.loop_kind != LoopKind::None;
                    ch.pos = s.loop_start as f64;
                }
            }
        }
        if cell.instr > 0 || note < 120 {
            // A new instrument or note restarts the envelopes.
            if cell.instr > 0 {
                let sv = self
                    .m
                    .samples
                    .get(sample.wrapping_sub(1))
                    .map(|s| (s.volume, s.pan));
                let ip = self
                    .m
                    .instruments
                    .get(self.chans[c].instr.wrapping_sub(1))
                    .and_then(|i| i.pan);
                let ch = &mut self.chans[c];
                if let Some((v, sp)) = sv {
                    ch.vol = i32::from(v);
                    if let Some(p) = ip.or(sp) {
                        ch.pan = i32::from(p);
                    }
                }
            }
            if !(porta && self.chans[c].active) || cell.instr > 0 {
                let ch = &mut self.chans[c];
                ch.key_on = true;
                ch.fade = 65_536;
                ch.fading = false;
                ch.env_vol = 0;
                ch.env_pan = 0;
            }
        }
        match cell.note {
            NOTE_OFF => self.key_off(c),
            NOTE_CUT => {
                self.chans[c].active = false;
            }
            NOTE_FADE => self.chans[c].fading = true,
            _ => {}
        }
        // The volume column's first-tick part.
        let ch = &mut self.chans[c];
        match cell.vol {
            VolCmd::Set(v) => ch.vol = i32::from(v.min(64)),
            VolCmd::FineUp(x) => ch.vol = (ch.vol + i32::from(x)).min(64),
            VolCmd::FineDown(x) => ch.vol = (ch.vol - i32::from(x)).max(0),
            VolCmd::Pan(p) => ch.pan = i32::from(p),
            VolCmd::VibratoSpeed(x) => ch.vib_speed = x * 4,
            VolCmd::Vibrato(x) => ch.vib_depth = x * 4,
            VolCmd::TonePorta(x) if x > 0 => ch.mem.0[usize::from(b'G')] = x,
            _ => {}
        }
    }

    fn key_off(&mut self, c: usize) {
        let env = self
            .m
            .instruments
            .get(self.chans[c].instr.wrapping_sub(1))
            .is_some_and(|i| i.volume.enabled);
        let ch = &mut self.chans[c];
        ch.key_on = false;
        match self.m.kind.style() {
            Kind::Xm if !env => ch.vol = 0,
            Kind::It if !env => ch.fading = true,
            Kind::Mod | Kind::S3m => ch.active = false,
            _ => {}
        }
    }

    fn vol_slide(&mut self, c: usize, p: u8, first: bool) {
        let (x, y) = (i32::from(p >> 4), i32::from(p & 15));
        let ch = &mut self.chans[c];
        let d = if y == 15 && x != 0 {
            if first { x } else { 0 }
        } else if x == 15 && y != 0 {
            if first { -y } else { 0 }
        } else if first {
            0
        } else if y == 0 {
            x
        } else {
            -y
        };
        ch.vol = (ch.vol + d).clamp(0, 64);
    }

    fn mod_vol_slide(&mut self, c: usize, p: u8) {
        let (x, y) = (i32::from(p >> 4), i32::from(p & 15));
        let ch = &mut self.chans[c];
        ch.vol = (ch.vol + if x > 0 { x } else { -y }).clamp(0, 64);
    }

    fn porta(&mut self, c: usize, p: u8, down: bool, first: bool) {
        let (x, y) = (p >> 4, p & 15);
        let amount = match x {
            0xF if first => f64::from(y) * 4.0,
            0xE if first => f64::from(y),
            0xE | 0xF => 0.0,
            _ if first => 0.0,
            _ => f64::from(p) * 4.0,
        };
        let ch = &mut self.chans[c];
        ch.period += if down { amount } else { -amount };
        ch.period = ch.period.clamp(1.0, 1e6);
    }

    fn tone_porta(&mut self, c: usize, speed: u8) {
        let ch = &mut self.chans[c];
        let step = f64::from(speed) * 4.0;
        if ch.period < ch.target {
            ch.period = (ch.period + step).min(ch.target);
        } else {
            ch.period = (ch.period - step).max(ch.target);
        }
    }

    fn vibrato(&mut self, c: usize, speed: u8, depth: u8, fine: bool) {
        let ch = &mut self.chans[c];
        let d = sine(ch.vib_pos) * i32::from(depth);
        let delta = if fine { d >> 7 } else { d >> 5 };
        ch.play_period = ch.period + f64::from(delta);
        ch.vib_pos = ch.vib_pos.wrapping_add(speed);
    }

    /// Effects on the current tick.
    fn process_tick(&mut self) {
        let first = self.tick == 0;
        let tick_in_row = self.tick % self.speed.max(1);
        for c in 0..self.m.channels {
            {
                let ch = &mut self.chans[c];
                ch.play_period = ch.period;
            }
            let cell = self.chans[c].cell;
            if (first && self.chans[c].delay == 0)
                || (self.chans[c].delay > 0
                    && tick_in_row == self.chans[c].delay
                    && self.tick < self.speed)
            {
                self.trigger(c);
            }
            let fx = cell.fx;
            if fx == 0 && matches!(cell.vol, VolCmd::None) {
                self.envelopes(c);
                continue;
            }
            // The volume column's per-tick part.
            if !first {
                let ch = &mut self.chans[c];
                match cell.vol {
                    VolCmd::SlideUp(x) => ch.vol = (ch.vol + i32::from(x)).min(64),
                    VolCmd::SlideDown(x) => ch.vol = (ch.vol - i32::from(x)).max(0),
                    VolCmd::PanLeft(x) => ch.pan = (ch.pan - i32::from(x)).max(0),
                    VolCmd::PanRight(x) => ch.pan = (ch.pan + i32::from(x)).min(255),
                    VolCmd::PortaUp(x) => ch.period = (ch.period - f64::from(x) * 4.0).max(1.0),
                    VolCmd::PortaDown(x) => ch.period += f64::from(x) * 4.0,
                    _ => {}
                }
                if matches!(cell.vol, VolCmd::TonePorta(_)) {
                    let s = ch.mem.0[usize::from(b'G')];
                    self.tone_porta(c, s);
                }
            }
            if matches!(cell.vol, VolCmd::Vibrato(_)) {
                let (s, d) = (self.chans[c].vib_speed, self.chans[c].vib_depth);
                self.vibrato(c, s, d, false);
            }
            let raw = cell.param;
            match fx {
                b'A' if first && raw > 0 => self.speed = u32::from(raw),
                b'T' if first && raw >= 0x20 => self.tempo = u32::from(raw),
                b'B' if first => self.next = Some((usize::from(raw), 0)),
                b'C' if first => {
                    let o = self.next.map_or(self.order + 1, |n| n.0);
                    self.next = Some((o, usize::from(raw)));
                }
                b'D' => {
                    let p = self.param(c, b'D', raw);
                    self.vol_slide(c, p, first);
                }
                FX_MOD_VOLSLIDE if !first => {
                    let p = self.param(c, b'D', raw);
                    self.mod_vol_slide(c, p);
                }
                FX_FINE_VOL_UP if first => {
                    let p = self.param(c, FX_FINE_VOL_UP, raw);
                    let ch = &mut self.chans[c];
                    ch.vol = (ch.vol + i32::from(p)).min(64);
                }
                FX_FINE_VOL_DOWN if first => {
                    let p = self.param(c, FX_FINE_VOL_DOWN, raw);
                    let ch = &mut self.chans[c];
                    ch.vol = (ch.vol - i32::from(p)).max(0);
                }
                b'E' | b'F' => {
                    let p = self.param(c, fx, raw);
                    self.porta(c, p, fx == b'E', first);
                }
                b'G' => {
                    let p = self.param(c, b'G', raw);
                    if !first {
                        self.tone_porta(c, p);
                    }
                }
                b'H' | b'U' => {
                    let p = self.param(c, b'H', raw);
                    let ch = &mut self.chans[c];
                    if p >> 4 != 0 {
                        ch.vib_speed = p >> 4;
                    }
                    if p & 15 != 0 {
                        ch.vib_depth = p & 15;
                    }
                    let (s, d) = (ch.vib_speed, ch.vib_depth);
                    if !first || self.m.kind.style() == Kind::It {
                        self.vibrato(c, s, d, fx == b'U');
                    }
                }
                b'J' => {
                    let p = self.param(c, b'J', raw);
                    let k = match self.tick % 3 {
                        0 => 0,
                        1 => p >> 4,
                        _ => p & 15,
                    };
                    let ch = &mut self.chans[c];
                    ch.play_period = if self.m.linear {
                        ch.period - f64::from(k) * 64.0
                    } else {
                        ch.period * 2f64.powf(-f64::from(k) / 12.0)
                    };
                }
                b'K' | b'L' => {
                    if fx == b'K' {
                        let (s, d) = (self.chans[c].vib_speed, self.chans[c].vib_depth);
                        if !first {
                            self.vibrato(c, s, d, false);
                        }
                    } else if !first {
                        let s = self.chans[c].mem.0[usize::from(b'G')];
                        self.tone_porta(c, s);
                    }
                    let p = self.param(c, b'D', raw);
                    if matches!(self.m.kind.style(), Kind::Mod | Kind::Xm) {
                        if !first {
                            self.mod_vol_slide(c, p);
                        }
                    } else {
                        self.vol_slide(c, p, first);
                    }
                }
                b'M' if first => self.chans[c].chan_vol = i32::from(raw.min(64)),
                b'N' => {
                    let p = self.param(c, b'N', raw);
                    let v = self.chans[c].vol;
                    self.chans[c].vol = self.chans[c].chan_vol;
                    self.vol_slide(c, p, first);
                    let ch = &mut self.chans[c];
                    ch.chan_vol = ch.vol;
                    ch.vol = v;
                }
                b'P' if !first => {
                    let p = self.param(c, b'P', raw);
                    let ch = &mut self.chans[c];
                    let step = if self.m.kind.style() == Kind::It {
                        4
                    } else {
                        1
                    };
                    ch.pan =
                        (ch.pan + step * (i32::from(p >> 4) - i32::from(p & 15))).clamp(0, 255);
                }
                b'Q' => {
                    let p = self.param(c, b'Q', raw);
                    let every = u32::from(p & 15);
                    if every > 0 && !first {
                        let ch = &mut self.chans[c];
                        ch.retrig += 1;
                        if ch.retrig >= every {
                            ch.retrig = 0;
                            ch.pos = 0.0;
                            ch.backwards = false;
                            ch.vol = match p >> 4 {
                                1..=5 => ch.vol - (1 << ((p >> 4) - 1)),
                                6 => ch.vol * 2 / 3,
                                7 => ch.vol / 2,
                                9..=13 => ch.vol + (1 << ((p >> 4) - 9)),
                                14 => ch.vol * 3 / 2,
                                15 => ch.vol * 2,
                                _ => ch.vol,
                            }
                            .clamp(0, 64);
                        }
                    } else if first {
                        self.chans[c].retrig = 0;
                    }
                }
                b'R' => {
                    let p = self.param(c, b'R', raw);
                    let ch = &mut self.chans[c];
                    let d = (sine(ch.trem_pos) * i32::from(p & 15)) >> 6;
                    ch.play_vol = (ch.vol + d).clamp(0, 64);
                    if !first {
                        ch.trem_pos = ch.trem_pos.wrapping_add(p >> 4);
                    }
                }
                b'S' => {
                    let (x, y) = (raw >> 4, raw & 15);
                    match x {
                        8 if first => self.chans[c].pan = i32::from(y) * 17,
                        0xB if first => {
                            let ch = &mut self.chans[c];
                            if y == 0 {
                                ch.loop_row = self.row;
                            } else if ch.loop_count == 0 {
                                ch.loop_count = y;
                                self.next = Some((self.order, ch.loop_row));
                            } else {
                                ch.loop_count -= 1;
                                if ch.loop_count > 0 {
                                    self.next = Some((self.order, ch.loop_row));
                                }
                            }
                        }
                        0xC if self.tick == u32::from(y) => {
                            let ch = &mut self.chans[c];
                            if self.m.kind.style() == Kind::It || self.m.kind.style() == Kind::S3m {
                                ch.active = false;
                            } else {
                                ch.vol = 0;
                            }
                        }
                        0xE if first && self.pattern_delay == 0 => {
                            self.pattern_delay = u32::from(y);
                        }
                        _ => {}
                    }
                }
                b'V' if first => {
                    let v = if self.m.kind.style() == Kind::S3m {
                        raw.min(64) * 2
                    } else {
                        raw.min(128)
                    };
                    self.global = i32::from(v);
                }
                b'W' if !first => {
                    let p = self.param(c, b'W', raw);
                    let k = if self.m.kind.style() == Kind::Xm {
                        2
                    } else {
                        1
                    };
                    let (x, y) = (i32::from(p >> 4), i32::from(p & 15));
                    let d = if x > 0 { x } else { -y };
                    self.global = (self.global + k * d).clamp(0, 128);
                }
                b'X' if first => {
                    self.chans[c].pan = if self.m.kind.style() == Kind::S3m {
                        (i32::from(raw.min(0x80)) * 2).min(255)
                    } else {
                        i32::from(raw)
                    };
                }
                FX_SET_VOLUME if first => self.chans[c].vol = i32::from(raw.min(64)),
                FX_KEY_OFF if self.tick == u32::from(raw) => self.key_off(c),
                FX_ENVELOPE_POS if first => {
                    self.chans[c].env_vol = u32::from(raw);
                    self.chans[c].env_pan = u32::from(raw);
                }
                _ => {}
            }
            self.envelopes(c);
        }
    }

    /// Envelope positions and fade-out, once a tick.
    fn envelopes(&mut self, c: usize) {
        if !self.chans[c].active {
            return;
        }
        let Some(ins) = self.m.instruments.get(self.chans[c].instr.wrapping_sub(1)) else {
            return;
        };
        let key_on = self.chans[c].key_on;
        let step = |e: &Envelope, t: u32| -> (u32, bool) {
            if !e.enabled || e.points.is_empty() {
                return (t, false);
            }
            let tick_of = |k: usize| u32::from(e.points.get(k).map_or(0, |p| p.0));
            let mut t = t + 1;
            if let Some((s, se)) = e.sustain
                && key_on
                && t > tick_of(se)
            {
                t = tick_of(s);
            } else if let Some((ls, le)) = e.looped
                && t > tick_of(le)
            {
                t = tick_of(ls);
            }
            let end = t >= tick_of(e.points.len() - 1);
            (t.min(tick_of(e.points.len() - 1)), end)
        };
        let (tv, end) = step(&ins.volume, self.chans[c].env_vol);
        let (tp, _) = step(&ins.panning, self.chans[c].env_pan);
        let fadeout = ins.fadeout as i32;
        let vol_env = ins.volume.enabled;
        let looped = ins.volume.looped.is_some();
        let last_zero = ins.volume.points.last().is_some_and(|p| p.1 == 0);
        let ch = &mut self.chans[c];
        ch.env_vol = tv;
        ch.env_pan = tp;
        if self.m.kind.style() == Kind::It && vol_env && end && !looped && !key_on {
            ch.fading = true;
        }
        if self.m.kind.style() == Kind::It && vol_env && end && last_zero {
            ch.active = false;
        }
        if (!key_on && self.m.kind.style() == Kind::Xm && vol_env) || ch.fading {
            ch.fade = (ch.fade - fadeout).max(0);
            if ch.fade == 0 {
                ch.active = false;
            }
        }
    }

    fn env_value(e: &Envelope, t: u32, default: i32) -> i32 {
        if !e.enabled || e.points.is_empty() {
            return default;
        }
        let t = t as i64;
        let p = &e.points;
        if t <= i64::from(p[0].0) {
            return i32::from(p[0].1);
        }
        for w in p.windows(2) {
            let (t0, v0) = (i64::from(w[0].0), i64::from(w[0].1));
            let (t1, v1) = (i64::from(w[1].0), i64::from(w[1].1));
            if t <= t1 {
                if t1 <= t0 {
                    return v1 as i32;
                }
                return (v0 + (v1 - v0) * (t - t0) / (t1 - t0)) as i32;
            }
        }
        i32::from(p[p.len() - 1].1)
    }

    /// The channel's left and right gains for this tick.
    fn gains(&self, c: usize) -> (f32, f32) {
        let ch = &self.chans[c];
        let Some(s) = self.sample_of(c) else {
            return (0.0, 0.0);
        };
        let vol = if matches!(ch.cell.fx, b'R') {
            ch.play_vol
        } else {
            ch.vol
        };
        let mut g = f64::from(vol) / 64.0 * f64::from(ch.chan_vol) / 64.0 * f64::from(self.global)
            / 128.0
            * f64::from(s.global_volume)
            / 64.0;
        let mut pan = f64::from(ch.pan);
        if let Some(ins) = self.m.instruments.get(ch.instr.wrapping_sub(1)) {
            g *= f64::from(Self::env_value(&ins.volume, ch.env_vol, 64)) / 64.0;
            g *= f64::from(ins.global_volume) / 128.0;
            g *= f64::from(ch.fade) / 65_536.0;
            let pe = Self::env_value(&ins.panning, ch.env_pan, 0);
            if ins.panning.enabled {
                let room = 128.0 - (pan - 128.0).abs();
                pan += f64::from(pe) / 32.0 * room;
            }
        }
        let master = f64::from(self.m.mix_volume) / 128.0 * 0.5;
        let g = (g * master) as f32;
        let p = (pan.clamp(0.0, 255.0) / 255.0) as f32;
        // Linear panning (a centred channel at half power on each side).
        (g * (1.0 - p) * 2f32.sqrt(), g * p * 2f32.sqrt())
    }

    /// Moves a channel's position by `n` frames at `step`, looping as the sample says; the
    /// samples read are given to `f` (index, fraction) when mixing.
    fn walk(&mut self, c: usize, n: usize, step: f64, mut f: impl FnMut(usize, f64, usize, usize)) {
        let Some(s) = self.m.samples.get(self.chans[c].sample.wrapping_sub(1)) else {
            self.chans[c].active = false;
            return;
        };
        let ch = &mut self.chans[c];
        let (lk, ls, le) = match s.sustain {
            Some((a, b, k)) if ch.key_on => (k, a, b),
            _ => (s.loop_kind, s.loop_start, s.loop_end),
        };
        let len = s.data.len();
        if len == 0 {
            // An empty slot (an unread synthetic sound, a missing sample) plays nothing.
            ch.active = false;
            return;
        }
        let (ls, le) = if lk != LoopKind::None && le > ls && le <= len {
            (ls as f64, le as f64)
        } else {
            (0.0, len as f64)
        };
        for k in 0..n {
            if !ch.active {
                return;
            }
            let i = ch.pos as usize;
            f(i.min(len.saturating_sub(1)), ch.pos.fract(), k, len);
            if ch.backwards {
                ch.pos -= step;
                if ch.pos < ls {
                    ch.pos = ls + (ls - ch.pos);
                    ch.backwards = false;
                }
            } else {
                ch.pos += step;
                if ch.pos >= le {
                    match lk {
                        LoopKind::None => ch.active = false,
                        _ if le <= ls => ch.active = false,
                        LoopKind::Forward => {
                            ch.pos = ls + (ch.pos - le) % (le - ls);
                        }
                        LoopKind::PingPong => {
                            ch.pos = (le - (ch.pos - le)).max(ls);
                            ch.backwards = true;
                        }
                    }
                }
            }
        }
    }

    fn step(&self, c: usize) -> f64 {
        let ch = &self.chans[c];
        self.freq(c, ch.play_period) / f64::from(self.rate)
    }

    fn advance(&mut self, n: usize) {
        for c in 0..self.m.channels {
            if self.chans[c].active {
                let step = self.step(c);
                self.walk(c, n, step, |_, _, _, _| {});
            }
        }
    }

    fn mix(&mut self, n: usize, out: &mut Vec<f32>) {
        let base = out.len();
        out.resize(base + 2 * n, 0.0);
        let m = Arc::clone(&self.m);
        for c in 0..self.m.channels {
            if !self.chans[c].active {
                continue;
            }
            let (gl, gr) = self.gains(c);
            let (pl, pr) = self.chans[c].gain;
            self.chans[c].gain = (gl, gr);
            let step = self.step(c);
            let Some(s) = m.samples.get(self.chans[c].sample.wrapping_sub(1)) else {
                continue;
            };
            let data = &s.data;
            let ramp = n.min(64) as f32;
            let looped = s.loop_kind == LoopKind::Forward && s.loop_end > s.loop_start;
            let (ls, le) = (s.loop_start, s.loop_end);
            let o = &mut out[base..];
            self.walk(c, n, step, |i, frac, k, len| {
                let a = data[i];
                let next = if i + 1 < len && !(looped && i + 1 >= le) {
                    data[i + 1]
                } else if looped {
                    data[ls]
                } else {
                    0.0
                };
                let v = a + (next - a) * frac as f32;
                let t = (k as f32 / ramp).min(1.0);
                o[2 * k] += v * (pl + (gl - pl) * t);
                o[2 * k + 1] += v * (pr + (gr - pr) * t);
            });
        }
        for v in &mut out[base..] {
            *v = v.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_shape() {
        assert_eq!(sine(0), 0);
        assert_eq!(sine(16), 255);
        assert_eq!(sine(48), -255);
    }
}
