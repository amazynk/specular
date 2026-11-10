//! Digital television closed captions (CEA-708 / CTA-708-E) as movies carry them in their
//! video: `cc_data` triplets of types 2 and 3 make caption channel packets, which hold
//! service blocks for services 1 to 63 (1 to 6 standard).
//!
//! Rendered (a sensible subset of what the standard can say):
//! - Windows: define, set current, clear, show, hide, toggle, delete and reset; their
//!   anchor point and position (absolute or relative), row and column count; several at
//!   once.
//! - Text: the G0 (ASCII, with a note at 0x7F), G1 (Latin-1), G2 (typographic and box
//!   characters) and G3 (the [CC] sign) sets, 16-bit characters; backspace, carriage
//!   return (scrolling up), horizontal carriage return, form feed and pen location.
//! - Pens: foreground colour (transparent text hidden), italics, underline, and the edge
//!   colour as the outline (raised, depressed, uniform and drop shadows all draw as one).
//!
//! Left out: window fill and border colours, opacity and display effects (the user's look
//! draws the text, on a box if they choose one), pen sizes, fonts and text offsets,
//! justification other than by the anchor, print and scroll directions other than the
//! usual ones, and Delay / Delay Cancel (commands act at once).

use crate::{Body, Cue, Span, TextCue, Track};

/// The most columns a window row keeps.
const MAX_COLS: usize = 42;
/// The most rows a window has.
const MAX_ROWS: usize = 15;

/// How long the last captions show when nothing clears them.
const TAIL_US: i64 = 2_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pen {
    fg: [u8; 3],
    /// 0 solid, 1 flashing, 2 translucent, 3 transparent.
    fg_opacity: u8,
    edge_type: u8,
    edge: [u8; 3],
    italic: bool,
    underline: bool,
}

impl Default for Pen {
    fn default() -> Pen {
        Pen {
            fg: [255; 3],
            fg_opacity: 0,
            edge_type: 0,
            edge: [0; 3],
            italic: false,
            underline: false,
        }
    }
}

/// A 2-bit colour level as 8 bits.
fn level(v: u8) -> u8 {
    (v & 3) * 85
}

fn colour(c: u8) -> [u8; 3] {
    [level(c >> 4), level(c >> 2), level(c)]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    ch: char,
    pen: Pen,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Window {
    visible: bool,
    priority: u8,
    relative: bool,
    anchor_v: u8,
    anchor_h: u8,
    anchor_point: u8,
    rows: usize,
    cols: usize,
    grid: Vec<[Option<Cell>; MAX_COLS]>,
    row: usize,
    col: usize,
    pen: Pen,
}

impl Window {
    fn new() -> Window {
        Window {
            visible: false,
            priority: 0,
            relative: false,
            anchor_v: 0,
            anchor_h: 0,
            anchor_point: 0,
            rows: 1,
            cols: 32,
            grid: vec![[None; MAX_COLS]; 1],
            row: 0,
            col: 0,
            pen: Pen::default(),
        }
    }

    fn clear(&mut self) {
        for r in &mut self.grid {
            *r = [None; MAX_COLS];
        }
    }

    fn put(&mut self, ch: char) {
        if self.col >= MAX_COLS {
            return;
        }
        let (r, c) = (self.row.min(self.rows - 1), self.col);
        self.grid[r][c] = Some(Cell { ch, pen: self.pen });
        self.col += 1;
    }

    fn carriage_return(&mut self) {
        if self.row + 1 < self.rows {
            self.row += 1;
        } else {
            self.grid.rotate_left(1);
            if let Some(last) = self.grid.last_mut() {
                *last = [None; MAX_COLS];
            }
        }
        self.col = 0;
    }

    /// The window as a cue, when it shows text.
    fn cue(&self) -> Option<TextCue> {
        let mut lines = Vec::new();
        for row in &self.grid {
            let visible = |c: &Option<Cell>| c.is_some_and(|c| c.pen.fg_opacity != 3);
            let Some(first) = row.iter().position(visible) else {
                continue;
            };
            let last = row.iter().rposition(visible).unwrap_or(first);
            let mut spans: Vec<Span> = Vec::new();
            for cell in &row[first..=last] {
                let (ch, pen) = match cell {
                    Some(c) if c.pen.fg_opacity != 3 => (c.ch, c.pen),
                    _ => (' ', Pen::default()),
                };
                let fill = (pen.fg != [255; 3]).then(|| [pen.fg[0], pen.fg[1], pen.fg[2], 255]);
                let outline =
                    (pen.edge_type != 0).then(|| [pen.edge[0], pen.edge[1], pen.edge[2], 255]);
                match spans.last_mut() {
                    Some(s)
                        if s.colour == fill
                            && s.outline == outline
                            && s.italic == pen.italic
                            && s.underline == pen.underline =>
                    {
                        s.text.push(ch)
                    }
                    _ => {
                        let mut s = Span::plain(ch.to_string());
                        s.colour = fill;
                        s.outline = outline;
                        s.italic = pen.italic;
                        s.underline = pen.underline;
                        spans.push(s);
                    }
                }
            }
            if spans.iter().any(|s| !s.text.trim().is_empty()) {
                lines.push(spans);
            }
        }
        if lines.is_empty() {
            return None;
        }
        // Anchor: 0 top left .. 8 bottom right; a cue's alignment is numpad-like.
        let align = match self.anchor_point.min(8) {
            0 => 7,
            1 => 8,
            2 => 9,
            3 => 4,
            4 => 5,
            5 => 6,
            6 => 1,
            7 => 2,
            _ => 3,
        };
        let (x, y) = if self.relative {
            (
                f32::from(self.anchor_h) / 99.0,
                f32::from(self.anchor_v) / 99.0,
            )
        } else {
            // 16:9 grid: 210 by 75 positions.
            (
                f32::from(self.anchor_h) / 209.0,
                f32::from(self.anchor_v) / 74.0,
            )
        };
        Some(TextCue {
            lines,
            align,
            pos: Some((0.1 + 0.8 * x.clamp(0.0, 1.0), 0.1 + 0.8 * y.clamp(0.0, 1.0))),
            margin_v: None,
        })
    }
}

/// G2 characters (after EXT1) we show; others are a space or an underscore.
fn g2(b: u8) -> char {
    match b {
        0x20 | 0x21 => ' ',
        0x25 => '…',
        0x2A => 'Š',
        0x2C => 'Œ',
        0x30 => '█',
        0x31 => '‘',
        0x32 => '’',
        0x33 => '“',
        0x34 => '”',
        0x35 => '•',
        0x39 => '™',
        0x3A => 'š',
        0x3C => 'œ',
        0x3D => '℠',
        0x3F => 'Ÿ',
        0x76 => '⅛',
        0x77 => '⅜',
        0x78 => '⅝',
        0x79 => '⅞',
        0x7A => '│',
        0x7B => '┐',
        0x7C => '└',
        0x7D => '─',
        0x7E => '┘',
        0x7F => '┌',
        _ => '_',
    }
}

/// A decoder of one service: feed it `cc_data` triplets in presentation order.
pub struct Decoder {
    service: u8,
    windows: [Option<Window>; 8],
    current: Option<usize>,
    /// The caption channel packet being gathered.
    packet: Vec<u8>,
    dirty: bool,
    cues: Vec<Cue>,
    open: Option<(i64, Vec<TextCue>)>,
    last_us: i64,
}

impl Decoder {
    pub fn new(service: u8) -> Decoder {
        Decoder {
            service,
            windows: Default::default(),
            current: None,
            packet: Vec::new(),
            dirty: false,
            cues: Vec::new(),
            open: None,
            last_us: 0,
        }
    }

    /// One `cc_data` triplet: `cc_type` 3 starts a packet, 2 continues it.
    pub fn triplet(&mut self, cc_type: u8, d1: u8, d2: u8) {
        match cc_type {
            3 => {
                self.flush();
                self.packet.extend_from_slice(&[d1, d2]);
            }
            2 if !self.packet.is_empty() => self.packet.extend_from_slice(&[d1, d2]),
            _ => return,
        }
        if self.packet.len() >= packet_len(self.packet[0]) {
            self.flush();
        }
    }

    /// Interprets the gathered packet.
    fn flush(&mut self) {
        let p = std::mem::take(&mut self.packet);
        let Some(&header) = p.first() else { return };
        let end = packet_len(header).min(p.len());
        for (svc, block) in service_blocks(&p[1..end]) {
            if svc == self.service {
                self.block(block);
            }
        }
    }

    fn win(&mut self) -> Option<&mut Window> {
        let i = self.current?;
        self.windows[i].as_mut()
    }

    /// Marks the display changed when the current window shows.
    fn touch(&mut self) {
        if self.win().is_some_and(|w| w.visible) {
            self.dirty = true;
        }
    }

    fn block(&mut self, b: &[u8]) {
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            let at = i + 1;
            // The command's parameters.
            let arg = |k: usize| b.get(at + k).copied().unwrap_or(0);
            i = at;
            match c {
                0x00..=0x0F => match c {
                    0x08 => {
                        if let Some(w) = self.win()
                            && w.col > 0
                        {
                            w.col -= 1;
                            let (r, col) = (w.row.min(w.rows - 1), w.col);
                            w.grid[r][col] = None;
                        }
                        self.touch();
                    }
                    0x0C => {
                        if let Some(w) = self.win() {
                            w.clear();
                            w.row = 0;
                            w.col = 0;
                        }
                        self.touch();
                    }
                    0x0D => {
                        if let Some(w) = self.win() {
                            w.carriage_return();
                        }
                        self.touch();
                    }
                    0x0E => {
                        if let Some(w) = self.win() {
                            let r = w.row.min(w.rows - 1);
                            w.grid[r] = [None; MAX_COLS];
                            w.col = 0;
                        }
                        self.touch();
                    }
                    _ => {}
                },
                0x10 => {
                    let e = arg(0);
                    i += 1;
                    match e {
                        0x00..=0x07 => {}
                        0x08..=0x0F => i += 1,
                        0x10..=0x17 => i += 2,
                        0x18..=0x1F => i += 3,
                        0x20..=0x7F => self.put(g2(e)),
                        0x80..=0x87 => i += 4,
                        0x88..=0x8F => i += 5,
                        0x90..=0x9F => i += 1 + usize::from(b.get(i).copied().unwrap_or(0) & 0x3F),
                        0xA0 => {
                            for ch in "[CC]".chars() {
                                self.put(ch);
                            }
                        }
                        _ => self.put('_'),
                    }
                }
                0x11..=0x17 => i += 1,
                0x18 => {
                    let v = u32::from(u16::from_be_bytes([arg(0), arg(1)]));
                    i += 2;
                    self.put(char::from_u32(v).unwrap_or('_'));
                }
                0x19..=0x1F => i += 2,
                0x20..=0x7E => self.put(char::from(c)),
                0x7F => self.put('♪'),
                0x80..=0x87 => {
                    let n = usize::from(c - 0x80);
                    if self.windows[n].is_some() {
                        self.current = Some(n);
                    }
                }
                0x88..=0x8C => {
                    let mask = arg(0);
                    i += 1;
                    self.windows_cmd(c, mask);
                }
                0x8D => i += 1,
                0x8E => {}
                0x8F => {
                    if self.windows.iter().flatten().any(|w| w.visible) {
                        self.dirty = true;
                    }
                    self.windows = Default::default();
                    self.current = None;
                }
                0x90 => {
                    let a = arg(1);
                    i += 2;
                    if let Some(w) = self.win() {
                        w.pen.italic = a & 0x80 != 0;
                        w.pen.underline = a & 0x40 != 0;
                        w.pen.edge_type = (a >> 3) & 7;
                    }
                }
                0x91 => {
                    let (f, e) = (arg(0), arg(2));
                    i += 3;
                    if let Some(w) = self.win() {
                        w.pen.fg_opacity = f >> 6;
                        w.pen.fg = colour(f);
                        w.pen.edge = colour(e);
                    }
                }
                0x92 => {
                    let (r, col) = (usize::from(arg(0) & 0x0F), usize::from(arg(1) & 0x3F));
                    i += 2;
                    if let Some(w) = self.win() {
                        w.row = r.min(w.rows - 1);
                        w.col = col.min(MAX_COLS - 1);
                    }
                }
                0x93..=0x96 => {}
                0x97 => i += 4,
                0x98..=0x9F => {
                    let p: [u8; 6] = std::array::from_fn(arg);
                    i += 6;
                    self.define(usize::from(c - 0x98), p);
                }
                0xA0..=0xFF => self.put(char::from(c)),
            }
        }
    }

    fn put(&mut self, ch: char) {
        if let Some(w) = self.win() {
            w.put(ch);
        }
        self.touch();
    }

    fn windows_cmd(&mut self, c: u8, mask: u8) {
        for n in 0..8 {
            if mask & (1 << n) == 0 {
                continue;
            }
            let Some(w) = self.windows[n].as_mut() else {
                continue;
            };
            let was = w.visible;
            match c {
                0x88 => w.clear(),
                0x89 => w.visible = true,
                0x8A => w.visible = false,
                0x8B => w.visible = !w.visible,
                _ => {
                    self.windows[n] = None;
                    if self.current == Some(n) {
                        self.current = None;
                    }
                    self.dirty |= was;
                    continue;
                }
            }
            if was || w.visible {
                self.dirty = true;
            }
        }
    }

    fn define(&mut self, n: usize, p: [u8; 6]) {
        let fresh = self.windows[n].is_none();
        let w = self.windows[n].get_or_insert_with(Window::new);
        let was = w.visible;
        w.visible = p[0] & 0x20 != 0;
        w.priority = p[0] & 7;
        w.relative = p[1] & 0x80 != 0;
        w.anchor_v = p[1] & 0x7F;
        w.anchor_h = p[2];
        w.anchor_point = p[3] >> 4;
        w.rows = usize::from(p[3] & 0x0F) + 1;
        w.rows = w.rows.min(MAX_ROWS);
        w.cols = (usize::from(p[4] & 0x3F) + 1).min(MAX_COLS);
        if w.grid.len() > w.rows {
            // Fewer rows: the bottom ones stay.
            let cut = w.grid.len() - w.rows;
            w.grid.drain(..cut);
        }
        w.grid.resize(w.rows, [None; MAX_COLS]);
        w.row = w.row.min(w.rows - 1);
        if fresh || p[5] & 7 != 0 {
            w.pen = Pen::default();
        }
        self.dirty |= was || w.visible;
        self.current = Some(n);
    }

    /// The end of a frame's data at `t_us`.
    pub fn frame_end(&mut self, t_us: i64) {
        self.last_us = self.last_us.max(t_us);
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let mut shown: Vec<&Window> = self
            .windows
            .iter()
            .flatten()
            .filter(|w| w.visible)
            .collect();
        shown.sort_by_key(|w| std::cmp::Reverse(w.priority));
        let now: Vec<TextCue> = shown.iter().filter_map(|w| w.cue()).collect();
        if self.open.as_ref().is_some_and(|(_, c)| *c == now) {
            return;
        }
        self.close(t_us);
        if !now.is_empty() {
            self.open = Some((t_us, now));
        }
    }

    fn close(&mut self, t_us: i64) {
        if let Some((start, cues)) = self.open.take()
            && t_us > start
        {
            for c in cues {
                self.cues.push(Cue {
                    start_us: start,
                    end_us: t_us,
                    body: Body::Text(c),
                });
            }
        }
    }

    pub fn finish(mut self, end_us: Option<i64>) -> Track {
        self.flush();
        let end = end_us
            .filter(|&e| e > self.last_us)
            .unwrap_or(self.last_us + TAIL_US);
        self.close(end);
        Track::new(self.cues)
    }
}

/// A caption channel packet's length (header included) from its header byte.
fn packet_len(header: u8) -> usize {
    match header & 0x3F {
        0 => 128,
        n => usize::from(n) * 2,
    }
}

/// The service blocks of a packet's data: (service number, block).
pub fn service_blocks(mut d: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    while let Some(&h) = d.first() {
        let (mut svc, size) = (h >> 5, usize::from(h & 0x1F));
        d = &d[1..];
        if svc == 0 && size == 0 {
            break;
        }
        if svc == 7 && size != 0 {
            let Some(&x) = d.first() else { break };
            svc = x & 0x3F;
            d = &d[1..];
        }
        let Some(block) = d.get(..size) else { break };
        out.push((svc, block));
        d = &d[size..];
    }
    out
}

/// Decodes service `service` (1..=63) from packets of `cc_data` triplets (`cc_type`, byte 1,
/// byte 2), the frame's time first, in presentation order.
pub fn decode<'a>(service: u8, packets: impl IntoIterator<Item = (i64, &'a [u8])>) -> Track {
    let mut d = Decoder::new(service);
    for (t, data) in packets {
        for c in data.chunks_exact(3) {
            d.triplet(c[0], c[1], c[2]);
        }
        d.frame_end(t);
    }
    d.finish(None)
}

/// Builds caption channel data for tests.
#[doc(hidden)]
pub mod fixture {
    /// Service blocks for `service` wrapped in caption channel packets, as `cc_data`
    /// triplets (one packet per block of up to 31 bytes).
    pub fn triplets(service: u8, data: &[u8], seq: &mut u8) -> Vec<[u8; 3]> {
        let mut out = Vec::new();
        for chunk in data.chunks(31) {
            let mut p = vec![(service << 5) | chunk.len() as u8];
            p.extend_from_slice(chunk);
            // Header plus data, padded to an even length.
            let total = (p.len() + 1).div_ceil(2) * 2;
            p.resize(total - 1, 0);
            let mut bytes = vec![(*seq << 6) | (total / 2) as u8];
            *seq = (*seq + 1) & 3;
            bytes.extend(p);
            for (k, pair) in bytes.chunks(2).enumerate() {
                out.push([if k == 0 { 3 } else { 2 }, pair[0], pair[1]]);
            }
        }
        out
    }

    /// DefineWindow `n`: visible, anchor point, relative position (percent), rows, columns.
    pub fn define(n: u8, visible: bool, anchor: u8, v: u8, h: u8, rows: u8, cols: u8) -> Vec<u8> {
        vec![
            0x98 + n,
            if visible { 0x20 } else { 0 },
            0x80 | v,
            h,
            (anchor << 4) | (rows - 1),
            cols - 1,
            0x01,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::*;
    use super::*;

    const FRAME: i64 = 33_367;

    /// Each byte string one frame apart.
    fn run(service: u8, frames: &[Vec<u8>]) -> Track {
        let mut seq = 0;
        let packets: Vec<(i64, Vec<u8>)> = frames
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let t = triplets(service, d, &mut seq);
                (i as i64 * FRAME, t.concat())
            })
            .collect();
        decode(service, packets.iter().map(|(t, d)| (*t, d.as_slice())))
    }

    fn texts(t: &Track) -> Vec<String> {
        t.cues
            .iter()
            .map(|c| c.text().unwrap().plain_text())
            .collect()
    }

    #[test]
    fn a_window_shows_its_text() {
        let mut d = define(0, true, 7, 90, 50, 2, 32);
        d.extend(b"Hello");
        d.push(0x0D);
        d.extend(b"world \x7f");
        let t = run(1, &[d, vec![0x88, 0x01]]);
        assert_eq!(texts(&t), ["Hello\nworld ♪"]);
        let c = &t.cues[0];
        assert_eq!((c.start_us, c.end_us), (0, FRAME));
        let cue = c.text().unwrap();
        assert_eq!(cue.align, 2, "anchored bottom centre");
        let (x, y) = cue.pos.unwrap();
        assert!((x - (0.1 + 0.8 * 50.0 / 99.0)).abs() < 1e-4);
        assert!((y - (0.1 + 0.8 * 90.0 / 99.0)).abs() < 1e-4);
        // Another service sees nothing.
        let mut seq = 0;
        let mut d = define(0, true, 0, 0, 0, 1, 32);
        d.extend(b"x");
        let tr = triplets(2, &d, &mut seq).concat();
        assert!(decode(1, [(0, tr.as_slice())]).cues.is_empty());
        assert_eq!(texts(&decode(2, [(0, tr.as_slice())])), ["x"]);
    }

    #[test]
    fn hidden_windows_show_when_told() {
        let mut d = define(1, false, 0, 10, 10, 1, 32);
        d.extend(b"pop");
        // DisplayWindows(1) one frame later, HideWindows two frames after.
        let t = run(1, &[d, vec![0x89, 0x02], vec![], vec![0x8A, 0x02]]);
        assert_eq!(texts(&t), ["pop"]);
        assert_eq!((t.cues[0].start_us, t.cues[0].end_us), (FRAME, 3 * FRAME));
        // Toggle, then delete.
        let mut d = define(2, false, 0, 10, 10, 1, 32);
        d.extend(b"t");
        let t = run(1, &[d, vec![0x8B, 0x04], vec![0x8C, 0x04]]);
        assert_eq!((t.cues[0].start_us, t.cues[0].end_us), (FRAME, 2 * FRAME));
    }

    #[test]
    fn pens_and_character_sets() {
        let mut d = define(0, true, 0, 0, 0, 1, 32);
        // SetPenAttributes: italic, uniform edge; SetPenColor: red text, blue edge.
        d.extend([0x90, 0x00, 0x80 | (3 << 3), 0x91, 0x30, 0x00, 0x03]);
        d.extend(b"r");
        // Back to white, plain.
        d.extend([0x90, 0x00, 0x00, 0x91, 0x3F, 0x00, 0x00]);
        // G1 é, EXT1 G2 ™, P16 א, G3 [CC].
        d.extend([0xE9, 0x10, 0x39, 0x18, 0x05, 0xD0, 0x10, 0xA0]);
        let t = run(1, &[d]);
        let c = t.cues[0].text().unwrap();
        assert_eq!(c.plain_text(), "ré™א[CC]");
        let r = &c.lines[0][0];
        assert_eq!(r.colour, Some([255, 0, 0, 255]));
        assert_eq!(r.outline, Some([0, 0, 255, 255]));
        assert!(r.italic);
        assert_eq!(c.lines[0][1].colour, None);
        assert_eq!(c.lines[0][1].outline, None);
    }

    #[test]
    fn rows_scroll_and_clear() {
        let mut d = define(0, true, 0, 0, 0, 2, 32);
        d.extend(b"a\rb\rc");
        let t = run(1, &[d.clone()]);
        assert_eq!(texts(&t).last().unwrap(), "b\nc");
        // Horizontal carriage return clears the row; form feed the window; backspace.
        let mut e = d;
        e.extend(b"x\x08y\x0e");
        e.extend(b"z");
        let t = run(1, &[e, vec![0x0C], b"q".to_vec()]);
        assert_eq!(texts(&t), ["b\nz", "q"]);
        // SetPenLocation.
        let mut d = define(0, true, 0, 0, 0, 3, 32);
        d.extend([0x92, 2, 4]);
        d.extend(b"low");
        let t = run(1, &[d]);
        assert_eq!(texts(&t), ["low"]);
    }

    #[test]
    fn reset_and_extended_services() {
        let mut d = define(0, true, 0, 0, 0, 1, 32);
        d.extend(b"x");
        let t = run(1, &[d, vec![0x8F]]);
        assert_eq!((t.cues.len(), t.cues[0].end_us), (1, FRAME));
        // Service 9 in an extended header.
        let blocks = service_blocks(&[0xE2, 9, b'h', b'i', 0x21, b'z', 0, 0]);
        assert_eq!(blocks, [(9, &b"hi"[..]), (1, &b"z"[..])]);
    }
}
