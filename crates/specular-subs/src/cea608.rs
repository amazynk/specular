//! Line 21 closed captions (CEA-608 / CTA-608-E) as movies carry them in their video
//! (ATSC A/53 `cc_data`, DVD user data, MP4 `c608` tracks).
//!
//! Each video field carries a byte pair a frame: field 1 holds data channels CC1 and CC2
//! (and text channels T1, T2), field 2 holds CC3, CC4 (T3, T4). Control codes pick the
//! channel and the mode; printable bytes go to the channel last named.
//!
//! - Caption modes: pop-on (loaded off screen, shown by End of Caption), roll-up (2, 3 or 4
//!   rows scrolling up from a base row) and paint-on (written straight to the screen).
//! - Text mode (Text Restart, Resume Text Display): a screen of text from the top row,
//!   scrolling up when full.
//! - Preamble address codes (row, indent, colour, italics, underline), mid-row codes, tab
//!   offsets, backspace, delete to end of row, carriage return, erase displayed and
//!   non-displayed memory.
//! - The basic character set (with its differences from ASCII), the special characters and
//!   the extended Western European ones (each replaces the stand-in character sent before
//!   it).
//! - Control codes come twice; the second copy is dropped. Bytes with bad parity are
//!   dropped (a bad second character shows as a block, as the standard says).
//!
//! Left out: flashing, background attributes (the user's look draws the text), and the
//! extended character sets' box-drawing glyphs beyond plain fallbacks.
//!
//! Each change of what shows makes a new set of cues: one per row, placed at the row's
//! position in the caption grid (15 rows of 32 columns inside the picture's title-safe area).

use crate::{Body, Cue, Span, TextCue, Track};

/// Rows and columns of the caption grid.
pub const ROWS: usize = 15;
pub const COLS: usize = 32;

/// How long the last captions show when nothing clears them (the stream's last data plus
/// this).
const TAIL_US: i64 = 2_000_000;

/// The seven caption colours (white, green, blue, cyan, red, yellow, magenta).
const COLOURS: [[u8; 4]; 7] = [
    [255, 255, 255, 255],
    [0, 255, 0, 255],
    [64, 96, 255, 255],
    [0, 255, 255, 255],
    [255, 0, 0, 255],
    [255, 255, 0, 255],
    [255, 0, 255, 255],
];

/// A character on the grid with its pen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    ch: char,
    pen: Pen,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Pen {
    /// An index into [`COLOURS`].
    colour: u8,
    italic: bool,
    underline: bool,
}

/// A screen of 15 rows; unwritten cells are transparent (`None`).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Screen([[Option<Cell>; COLS]; ROWS]);

impl Default for Screen {
    fn default() -> Screen {
        Screen([[None; COLS]; ROWS])
    }
}

impl Screen {
    fn clear(&mut self) {
        *self = Screen::default();
    }

    fn clear_row(&mut self, r: usize) {
        self.0[r] = [None; COLS];
    }

    /// The rows that show something, as cues placed on the grid.
    fn cues(&self) -> Vec<TextCue> {
        let mut out = Vec::new();
        for (r, row) in self.0.iter().enumerate() {
            let Some(first) = row.iter().position(Option::is_some) else {
                continue;
            };
            let last = row.iter().rposition(Option::is_some).unwrap_or(first);
            let mut spans: Vec<Span> = Vec::new();
            for cell in &row[first..=last] {
                let (ch, pen) = cell.map_or((' ', Pen::default()), |c| (c.ch, c.pen));
                let colour = (pen.colour != 0).then(|| COLOURS[usize::from(pen.colour) % 7]);
                match spans.last_mut() {
                    Some(s)
                        if s.colour == colour
                            && s.italic == pen.italic
                            && s.underline == pen.underline =>
                    {
                        s.text.push(ch)
                    }
                    _ => {
                        let mut s = Span::plain(ch.to_string());
                        s.colour = colour;
                        s.italic = pen.italic;
                        s.underline = pen.underline;
                        spans.push(s);
                    }
                }
            }
            if spans.iter().all(|s| s.text.trim().is_empty()) {
                continue;
            }
            out.push(TextCue {
                lines: vec![spans],
                align: 7,
                pos: Some(grid_pos(r, first)),
                margin_v: None,
            });
        }
        out
    }
}

/// The top-left corner of grid cell (`row`, `col`) as fractions of the picture: the grid
/// fills the middle 80 % (the title-safe area).
pub fn grid_pos(row: usize, col: usize) -> (f32, f32) {
    (
        0.1 + 0.8 * col as f32 / COLS as f32,
        0.1 + 0.8 * row as f32 / ROWS as f32,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    PopOn,
    /// Roll-up with this many rows.
    RollUp(usize),
    PaintOn,
}

/// One data channel's caption or text service.
struct Service {
    style: Style,
    /// What shows.
    shown: Screen,
    /// Pop-on captions being loaded.
    back: Screen,
    row: usize,
    col: usize,
    pen: Pen,
    /// The text mode's screen and cursor (text channels only).
    text_mode: bool,
    dirty: bool,
}

impl Service {
    fn new(text_mode: bool) -> Service {
        Service {
            style: Style::PopOn,
            shown: Screen::default(),
            back: Screen::default(),
            row: ROWS - 1,
            col: 0,
            pen: Pen::default(),
            text_mode,
            dirty: false,
        }
    }

    /// The screen characters go to.
    fn target(&mut self) -> &mut Screen {
        if !self.text_mode && self.style == Style::PopOn {
            &mut self.back
        } else {
            self.dirty = true;
            &mut self.shown
        }
    }

    fn put(&mut self, ch: char) {
        let (r, c, pen) = (self.row, self.col, self.pen);
        self.target().0[r][c] = Some(Cell { ch, pen });
        self.col = (c + 1).min(COLS - 1);
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            let (r, c) = (self.row, self.col);
            self.target().0[r][c] = None;
        }
    }

    /// A character of the extended sets: it replaces the stand-in sent before it.
    fn put_extended(&mut self, ch: char) {
        if self.col > 0 {
            self.col -= 1;
        }
        self.put(ch);
    }

    fn roll_up(&mut self, n: usize) {
        if self.text_mode {
            return;
        }
        if !matches!(self.style, Style::RollUp(_)) {
            self.shown.clear();
            self.back.clear();
            self.dirty = true;
            self.row = ROWS - 1;
        }
        self.style = Style::RollUp(n);
        // Rows above the window go.
        let top = (self.row + 1).saturating_sub(n);
        for r in 0..top {
            if self.shown.0[r].iter().any(Option::is_some) {
                self.shown.clear_row(r);
                self.dirty = true;
            }
        }
        self.col = 0;
    }

    fn carriage_return(&mut self) {
        if self.text_mode {
            if self.row + 1 < ROWS {
                self.row += 1;
            } else {
                self.shown.0.rotate_left(1);
                self.shown.clear_row(ROWS - 1);
            }
            self.col = 0;
            self.dirty = true;
            return;
        }
        let Style::RollUp(n) = self.style else {
            return;
        };
        let base = self.row;
        let top = (base + 1).saturating_sub(n);
        for r in top..base {
            self.shown.0[r] = self.shown.0[r + 1];
        }
        if top > 0 {
            self.shown.clear_row(top - 1);
        }
        self.shown.clear_row(base);
        self.col = 0;
        self.dirty = true;
    }

    /// A preamble address code: row (0-based), indent column, pen.
    fn preamble(&mut self, row: usize, col: usize, pen: Pen) {
        if self.text_mode {
            // Text mode keeps its own row; the indent and pen apply.
            self.col = col;
            self.pen = pen;
            return;
        }
        if let Style::RollUp(n) = self.style
            && row != self.row
        {
            // The window moves to the new base row, its rows with it.
            let old = self.shown.clone();
            self.shown.clear();
            for k in 0..n.min(row + 1).min(self.row + 1) {
                self.shown.0[row - k] = old.0[self.row - k];
            }
            self.dirty = true;
        }
        self.row = row;
        self.col = col;
        self.pen = pen;
    }

    fn command(&mut self, lo: u8) {
        match lo {
            // Resume caption loading: pop-on.
            0x20 => self.style = Style::PopOn,
            0x21 => self.backspace(),
            0x24 => {
                let (r, c) = (self.row, self.col);
                let s = self.target();
                for cell in &mut s.0[r][c..] {
                    *cell = None;
                }
            }
            0x25..=0x27 => self.roll_up(usize::from(lo - 0x23)),
            0x29 => self.style = Style::PaintOn,
            0x2C if !self.text_mode => {
                self.shown.clear();
                self.dirty = true;
            }
            0x2D => self.carriage_return(),
            0x2E if !self.text_mode => {
                self.back.clear();
            }
            0x2F if !self.text_mode => {
                std::mem::swap(&mut self.shown, &mut self.back);
                self.style = Style::PopOn;
                self.dirty = true;
            }
            _ => {}
        }
    }
}

/// The basic character set: ASCII with its differences.
fn basic(b: u8) -> char {
    match b {
        0x27 => '’',
        0x2A => 'á',
        0x5C => 'é',
        0x5E => 'í',
        0x5F => 'ó',
        0x60 => 'ú',
        0x7B => 'ç',
        0x7C => '÷',
        0x7D => 'Ñ',
        0x7E => 'ñ',
        0x7F => '█',
        b => char::from(b),
    }
}

/// Special characters (`11 30`..`11 3F`).
const SPECIAL: [char; 16] = [
    '®', '°', '½', '¿', '™', '¢', '£', '♪', 'à', ' ', 'è', 'â', 'ê', 'î', 'ô', 'û',
];

/// Extended Western European characters: Spanish, miscellaneous and French (`12 20`..`12 3F`).
const EXTENDED_12: [char; 32] = [
    'Á', 'É', 'Ó', 'Ú', 'Ü', 'ü', '‘', '¡', '*', '’', '—', '©', '℠', '•', '“', '”', 'À', 'Â', 'Ç',
    'È', 'Ê', 'Ë', 'ë', 'Î', 'Ï', 'ï', 'Ô', 'Ù', 'ù', 'Û', '«', '»',
];

/// Portuguese, German and Danish (`13 20`..`13 3F`).
const EXTENDED_13: [char; 32] = [
    'Ã', 'ã', 'Í', 'Ì', 'ì', 'Ò', 'ò', 'Õ', 'õ', '{', '}', '\\', '^', '_', '|', '~', 'Ä', 'ä', 'Ö',
    'ö', 'ß', '¥', '¤', '│', 'Å', 'å', 'Ø', 'ø', '┌', '┐', '└', '┘',
];

/// The row (0-based) of a preamble address code.
fn pac_row(hi: u8, lo: u8) -> usize {
    const ROW: [usize; 8] = [11, 1, 3, 12, 14, 5, 7, 9];
    let r = ROW[usize::from(hi & 7)];
    let r = if hi & 7 != 0 && lo & 0x20 != 0 {
        r + 1
    } else {
        r
    };
    r - 1
}

/// Whether a byte has odd parity, as every line 21 byte must.
fn odd_parity(b: u8) -> bool {
    !b.count_ones().is_multiple_of(2)
}

/// Which service of a field a decoder follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    /// 1..=4: CC1..CC4.
    pub number: u8,
    /// The text service (T1..T4) rather than captions.
    pub text: bool,
}

impl Channel {
    /// Channel 1..=4 as captions, 5..=8 as text services T1..T4 (what
    /// `specular_video::captions` stores as the stream's codec data).
    pub fn from_code(code: u8) -> Option<Channel> {
        match code {
            1..=4 => Some(Channel {
                number: code,
                text: false,
            }),
            5..=8 => Some(Channel {
                number: code - 4,
                text: true,
            }),
            _ => None,
        }
    }

    /// The `cc_type` of the field it is in: 0 (field 1) or 1 (field 2).
    pub fn field(self) -> u8 {
        u8::from(self.number >= 3)
    }

    /// Data channel 0 or 1 within the field.
    fn data_channel(self) -> u8 {
        u8::from(self.number.is_multiple_of(2))
    }
}

/// A decoder of one channel: feed it the field's byte pairs in presentation order.
pub struct Decoder {
    want: Channel,
    svc: Service,
    /// The channel and mode the field's last control code named.
    cur_chan: u8,
    text_mode: [bool; 2],
    last_ctrl: Option<(u8, u8)>,
    cues: Vec<Cue>,
    /// The cues showing now and when they started.
    open: Option<(i64, Vec<TextCue>)>,
    last_us: i64,
}

impl Decoder {
    pub fn new(want: Channel) -> Decoder {
        Decoder {
            want,
            svc: Service::new(want.text),
            cur_chan: 0,
            text_mode: [false; 2],
            last_ctrl: None,
            cues: Vec::new(),
            open: None,
            last_us: 0,
        }
    }

    /// Whether the pair is for the service this decoder follows.
    fn mine(&self) -> bool {
        self.cur_chan == self.want.data_channel()
            && self.text_mode[usize::from(self.cur_chan)] == self.want.text
    }

    /// One byte pair of the field (parity bits included).
    pub fn pair(&mut self, b1: u8, b2: u8) {
        if !odd_parity(b1) {
            return;
        }
        let hi = b1 & 0x7F;
        let lo = if odd_parity(b2) { b2 & 0x7F } else { 0x7F };
        if hi == 0 && lo == 0 {
            return;
        }
        if (0x10..=0x1F).contains(&hi) {
            // Control codes come twice: the copy is dropped.
            if self.last_ctrl == Some((hi, lo)) {
                self.last_ctrl = None;
                return;
            }
            self.last_ctrl = Some((hi, lo));
            self.control(hi, lo);
            return;
        }
        self.last_ctrl = None;
        if hi < 0x20 {
            return;
        }
        if self.mine() {
            self.svc.put(basic(hi));
            if lo >= 0x20 {
                self.svc.put(basic(lo));
            }
        }
    }

    fn control(&mut self, hi: u8, lo: u8) {
        let chan = u8::from(hi & 0x08 != 0);
        let base = hi & !0x08;
        // Miscellaneous commands name the channel and the mode.
        if matches!(base, 0x14 | 0x15) && (0x20..=0x2F).contains(&lo) {
            self.cur_chan = chan;
            match lo {
                0x2A | 0x2B => {
                    self.text_mode[usize::from(chan)] = true;
                    if self.mine() && lo == 0x2A {
                        // Text restart: a clear screen from the top.
                        self.svc.shown.clear();
                        self.svc.row = 0;
                        self.svc.col = 0;
                        self.svc.dirty = true;
                    }
                    return;
                }
                0x20 | 0x25..=0x27 | 0x29 => self.text_mode[usize::from(chan)] = false,
                _ => {}
            }
            // Erase and End of Caption act on captions whatever the mode.
            let caption_cmd = matches!(lo, 0x2C | 0x2E | 0x2F);
            let for_us = if caption_cmd {
                chan == self.want.data_channel() && !self.want.text
            } else {
                self.mine()
            };
            if for_us {
                self.svc.command(lo);
            }
            return;
        }
        self.cur_chan = chan;
        if !self.mine() {
            return;
        }
        match (base, lo) {
            // Tab offsets.
            (0x17, 0x21..=0x23) => {
                self.svc.col = (self.svc.col + usize::from(lo - 0x20)).min(COLS - 1);
            }
            // Mid-row codes: a space that changes the pen.
            (0x11, 0x20..=0x2F) => {
                let mut pen = self.svc.pen;
                if lo & 0x0E == 0x0E {
                    pen.italic = true;
                } else {
                    pen.colour = (lo - 0x20) >> 1;
                    pen.italic = false;
                }
                pen.underline = lo & 1 != 0;
                self.svc.pen = pen;
                self.svc.put(' ');
            }
            (0x11, 0x30..=0x3F) => self.svc.put(SPECIAL[usize::from(lo - 0x30)]),
            (0x12, 0x20..=0x3F) => self.svc.put_extended(EXTENDED_12[usize::from(lo - 0x20)]),
            (0x13, 0x20..=0x3F) => self.svc.put_extended(EXTENDED_13[usize::from(lo - 0x20)]),
            (0x10..=0x17, 0x40..=0x7F) => {
                let row = pac_row(base, lo);
                let attr = lo & 0x1F;
                let mut pen = Pen {
                    underline: attr & 1 != 0,
                    ..Pen::default()
                };
                let col = if attr & 0x10 != 0 {
                    usize::from((attr & 0x0E) >> 1) * 4
                } else {
                    match attr >> 1 {
                        7 => pen.italic = true,
                        c => pen.colour = c,
                    }
                    0
                };
                self.svc.preamble(row, col, pen);
            }
            _ => {}
        }
    }

    /// The end of a frame's data at `t_us`: a change of what shows starts new cues.
    pub fn frame_end(&mut self, t_us: i64) {
        self.last_us = self.last_us.max(t_us);
        if !std::mem::take(&mut self.svc.dirty) {
            return;
        }
        let now = self.svc.shown.cues();
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

    /// The cues; what still shows ends `end_us` (else a little after the last data).
    pub fn finish(mut self, end_us: Option<i64>) -> Track {
        let end = end_us
            .filter(|&e| e > self.last_us)
            .unwrap_or(self.last_us + TAIL_US);
        self.close(end);
        Track::new(self.cues)
    }

    /// Whether anything showed so far.
    pub fn has_cues(&self) -> bool {
        !self.cues.is_empty() || self.open.is_some()
    }
}

/// Decodes channel `code` (1..=4 CC1..CC4, 5..=8 T1..T4) from packets of `cc_data`
/// triplets (`cc_type`, byte 1, byte 2), the frame's time first, in presentation order.
pub fn decode<'a>(code: u8, packets: impl IntoIterator<Item = (i64, &'a [u8])>) -> Track {
    let Some(ch) = Channel::from_code(code) else {
        return Track::default();
    };
    let mut d = Decoder::new(ch);
    for (t, data) in packets {
        for c in data.chunks_exact(3) {
            if c[0] == ch.field() {
                d.pair(c[1], c[2]);
            }
        }
        d.frame_end(t);
    }
    d.finish(None)
}

/// Builds byte pairs for tests: odd parity added.
#[doc(hidden)]
pub mod fixture {
    /// The byte with its parity bit set for odd parity.
    pub fn parity(b: u8) -> u8 {
        let b = b & 0x7F;
        if b.count_ones().is_multiple_of(2) {
            b | 0x80
        } else {
            b
        }
    }

    /// A control code pair, sent twice as encoders do.
    pub fn ctrl(hi: u8, lo: u8) -> Vec<[u8; 2]> {
        vec![[parity(hi), parity(lo)]; 2]
    }

    /// Text as byte pairs (a trailing odd character padded with a null).
    pub fn text(s: &str) -> Vec<[u8; 2]> {
        s.as_bytes()
            .chunks(2)
            .map(|c| [parity(c[0]), parity(c.get(1).copied().unwrap_or(0))])
            .collect()
    }

    /// Preamble address code for `row` (1..=15) at indent `indent` (a multiple of 4), white,
    /// on data channel 1 (`ch2`: channel 2).
    pub fn pac(row: u8, indent: u8, ch2: bool) -> Vec<[u8; 2]> {
        const HI: [u8; 16] = [
            0, 0x11, 0x11, 0x12, 0x12, 0x15, 0x15, 0x16, 0x16, 0x17, 0x17, 0x10, 0x13, 0x13, 0x14,
            0x14,
        ];
        let hi = HI[usize::from(row)] | if ch2 { 0x08 } else { 0 };
        let upper = matches!(row, 2 | 4 | 6 | 8 | 10 | 13 | 15);
        let lo = 0x40 | if upper { 0x20 } else { 0 } | 0x10 | ((indent / 4) << 1);
        ctrl(hi, lo)
    }

    /// One pair a frame as (time, `cc_data` triplets of field `field`), `frame_us` apart from
    /// `start_us`.
    pub fn frames(
        field: u8,
        pairs: &[[u8; 2]],
        start_us: i64,
        frame_us: i64,
    ) -> Vec<(i64, Vec<u8>)> {
        pairs
            .iter()
            .enumerate()
            .map(|(i, p)| (start_us + i as i64 * frame_us, vec![field, p[0], p[1]]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::*;
    use super::*;

    const FRAME: i64 = 33_367;

    fn run(code: u8, pairs: &[[u8; 2]]) -> Track {
        let field = Channel::from_code(code).unwrap().field();
        let f = frames(field, pairs, 0, FRAME);
        decode(code, f.iter().map(|(t, d)| (*t, d.as_slice())))
    }

    fn texts(t: &Track) -> Vec<String> {
        t.cues
            .iter()
            .map(|c| c.text().unwrap().plain_text())
            .collect()
    }

    fn seq(parts: &[Vec<[u8; 2]>]) -> Vec<[u8; 2]> {
        parts.concat()
    }

    #[test]
    fn pop_on_shows_at_end_of_caption() {
        let p = seq(&[
            ctrl(0x14, 0x20),
            pac(14, 4, false),
            text("HELLO"),
            pac(15, 0, false),
            text("WORLD!"),
            ctrl(0x14, 0x2F),
            text("  "),
            ctrl(0x14, 0x2C),
        ]);
        let t = run(1, &p);
        assert_eq!(texts(&t), ["HELLO", "WORLD!"]);
        // Shown from End of Caption (its first copy) until Erase Displayed Memory.
        let n = p.len() as i64;
        assert_eq!(t.cues[0].start_us, (n - 5) * FRAME);
        assert_eq!(t.cues[0].end_us, (n - 2) * FRAME);
        let a = t.cues[0].text().unwrap();
        let b = t.cues[1].text().unwrap();
        assert_eq!(a.pos, Some(grid_pos(13, 4)));
        assert_eq!(b.pos, Some(grid_pos(14, 0)));
        assert_eq!(a.align, 7);
    }

    #[test]
    fn roll_up_scrolls() {
        let p = seq(&[
            ctrl(0x14, 0x25),
            pac(15, 0, false),
            text("ONE"),
            ctrl(0x14, 0x2D),
            text("TWO"),
            ctrl(0x14, 0x2D),
            text("THREE"),
        ]);
        let t = run(1, &p);
        // The last state: TWO on row 14, THREE on row 15 (two rows: ONE scrolled away).
        let last_start = t.cues.last().unwrap().start_us;
        let last: Vec<String> = t
            .cues
            .iter()
            .filter(|c| c.start_us == last_start)
            .map(|c| c.text().unwrap().plain_text())
            .collect();
        assert_eq!(last, ["TWO", "THREE"]);
        // Characters show as they come: "O", then "ONE" grows.
        assert!(texts(&t).iter().any(|s| s == "ON"));
        let rows: Vec<f32> = t
            .cues
            .iter()
            .filter(|c| c.start_us == last_start)
            .map(|c| c.text().unwrap().pos.unwrap().1)
            .collect();
        assert_eq!(rows, [grid_pos(13, 0).1, grid_pos(14, 0).1]);
    }

    #[test]
    fn paint_on_and_channels() {
        // CC2 paints "B"; CC1 has nothing; field 2's CC3 is separate.
        let p = seq(&[ctrl(0x1C, 0x29), pac(1, 8, true), text("B")]);
        assert!(run(1, &p).cues.is_empty());
        let t = run(2, &p);
        assert_eq!(texts(&t), ["B"]);
        assert_eq!(t.cues[0].text().unwrap().pos, Some(grid_pos(0, 8)));
        assert!(run(3, &p).cues.is_empty());
        // Field 2's own codes reach CC3.
        let p3 = seq(&[ctrl(0x15, 0x29), pac(2, 0, false), text("C3")]);
        assert_eq!(texts(&run(3, &p3)), ["C3"]);
    }

    #[test]
    fn text_mode_is_its_own_channel() {
        let p = seq(&[
            ctrl(0x14, 0x2A),
            text("TEXT ONE"),
            ctrl(0x14, 0x2D),
            text("TWO"),
            ctrl(0x14, 0x29),
            pac(15, 0, false),
            text("CAP"),
        ]);
        let t1 = run(5, &p);
        let last_start = t1.cues.last().unwrap().start_us;
        let last: Vec<String> = t1
            .cues
            .iter()
            .filter(|c| c.start_us == last_start)
            .map(|c| c.text().unwrap().plain_text())
            .collect();
        assert_eq!(last, ["TEXT ONE", "TWO"]);
        assert!(texts(&t1).iter().all(|s| !s.contains("CAP")));
        let cc1 = run(1, &p);
        assert_eq!(texts(&cc1).last().map(String::as_str), Some("CAP"));
        assert!(texts(&cc1).iter().all(|s| !s.contains("TEXT")));
    }

    #[test]
    fn character_sets() {
        let p = seq(&[
            ctrl(0x14, 0x29),
            pac(15, 0, false),
            // Basic set differences: ’ á é Ñ ñ.
            vec![[parity(0x27), parity(0x2A)], [parity(0x5C), parity(0x7D)]],
            vec![[parity(0x7E), 0x80]],
            // Special: ♪ ½.
            ctrl(0x11, 0x37),
            ctrl(0x11, 0x32),
            // Extended: "E" then É replaces it; "a" then ä.
            text("E"),
            ctrl(0x12, 0x21),
            text("a"),
            ctrl(0x13, 0x31),
        ]);
        let t = run(1, &p);
        assert_eq!(texts(&t).last().unwrap(), "’áéÑñ♪½Éä");
    }

    #[test]
    fn styles_backspace_and_tabs() {
        let p = seq(&[
            ctrl(0x14, 0x29),
            // Row 15, italic (attribute 0x0E).
            ctrl(0x14, 0x60 | 0x0E),
            text("IT"),
            // Mid-row: red, underlined.
            ctrl(0x11, 0x29),
            text("RX"),
            ctrl(0x14, 0x21),
            ctrl(0x17, 0x22),
            text("Z"),
        ]);
        let t = run(1, &p);
        let c = t.cues.last().unwrap().text().unwrap();
        assert_eq!(c.plain_text(), "IT R  Z");
        let it = &c.lines[0][0];
        assert!(it.italic && it.colour.is_none());
        let red = &c.lines[0][1];
        assert_eq!(red.colour, Some(COLOURS[4]));
        assert!(red.underline && !red.italic);
    }

    #[test]
    fn doubled_codes_act_once_and_parity_errors_drop() {
        // Two EOCs sent as two doubled pairs flip twice; one doubled pair flips once.
        let p = seq(&[
            ctrl(0x14, 0x20),
            pac(15, 0, false),
            text("X"),
            ctrl(0x14, 0x2F),
        ]);
        assert_eq!(texts(&run(1, &p)), ["X"]);
        // A pair with bad parity in the first byte is dropped.
        let bad = seq(&[ctrl(0x14, 0x29), pac(15, 0, false), vec![[0x41, 0x42]]]);
        assert!(run(1, &bad).cues.is_empty());
        // A bad second byte shows as a block.
        let block = seq(&[
            ctrl(0x14, 0x29),
            pac(15, 0, false),
            vec![[parity(b'A'), 0x42]],
        ]);
        assert_eq!(texts(&run(1, &block)), ["A█"]);
    }

    #[test]
    fn roll_up_window_moves_with_its_base_row() {
        let p = seq(&[
            ctrl(0x14, 0x26),
            pac(15, 0, false),
            text("A"),
            ctrl(0x14, 0x2D),
            text("B"),
            pac(5, 0, false),
            text("C"),
        ]);
        let t = run(1, &p);
        let last_start = t.cues.last().unwrap().start_us;
        let last: Vec<(String, f32)> = t
            .cues
            .iter()
            .filter(|c| c.start_us == last_start)
            .map(|c| {
                let x = c.text().unwrap();
                (x.plain_text(), x.pos.unwrap().1)
            })
            .collect();
        assert_eq!(
            last,
            [
                ("A".into(), grid_pos(3, 0).1),
                ("C".into(), grid_pos(4, 0).1)
            ]
        );
    }
}
