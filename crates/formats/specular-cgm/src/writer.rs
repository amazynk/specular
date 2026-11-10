//! A small binary CGM writer for test fixtures ([`crate::fixture`] and the tests) (default precisions: 16-bit integers, indexes
//! and integer VDC, 8-bit colours and colour indexes, 32-bit fixed reals).

/// Parameter bytes of one element.
#[derive(Default, Clone)]
pub(crate) struct P(pub Vec<u8>);

impl P {
    pub fn new() -> Self {
        P(Vec::new())
    }
    pub fn i(mut self, v: i64) -> Self {
        self.0.extend_from_slice(&(v as i16).to_be_bytes());
        self
    }
    pub fn e(self, v: i64) -> Self {
        self.i(v)
    }
    pub fn pt(self, x: i64, y: i64) -> Self {
        self.i(x).i(y)
    }
    pub fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    pub fn rgb(self, r: u8, g: u8, b: u8) -> Self {
        self.u8(r).u8(g).u8(b)
    }
    /// A 32-bit fixed-point real.
    pub fn r(mut self, v: f64) -> Self {
        let whole = v.floor();
        let frac = ((v - whole) * 65536.0).round() as u32;
        self.0.extend_from_slice(&(whole as i16).to_be_bytes());
        self.0.extend_from_slice(&(frac as u16).to_be_bytes());
        self
    }
    pub fn s(mut self, s: &str) -> Self {
        assert!(s.len() < 255);
        self.0.push(s.len() as u8);
        self.0.extend_from_slice(s.as_bytes());
        self
    }
    pub fn raw(mut self, b: &[u8]) -> Self {
        self.0.extend_from_slice(b);
        self
    }
    /// Pads to a 16-bit boundary (cell rows).
    pub fn align(mut self) -> Self {
        if self.0.len() % 2 == 1 {
            self.0.push(0);
        }
        self
    }
}

/// A metafile being written.
#[derive(Default)]
pub(crate) struct W(pub Vec<u8>);

impl W {
    pub fn new() -> Self {
        W(Vec::new())
    }

    pub fn el(&mut self, class: u16, id: u16, p: P) -> &mut Self {
        let d = p.0;
        let head = (class << 12) | (id << 5);
        if d.len() < 31 {
            self.0
                .extend_from_slice(&(head | d.len() as u16).to_be_bytes());
            self.0.extend_from_slice(&d);
            if d.len() % 2 == 1 {
                self.0.push(0);
            }
        } else {
            self.0.extend_from_slice(&(head | 31).to_be_bytes());
            // Partitions of at most 1000 bytes, to exercise joining.
            let mut chunks = d.chunks(1000).peekable();
            while let Some(c) = chunks.next() {
                let more = if chunks.peek().is_some() { 0x8000 } else { 0 };
                self.0
                    .extend_from_slice(&(more | c.len() as u16).to_be_bytes());
                self.0.extend_from_slice(c);
                if c.len() % 2 == 1 {
                    self.0.push(0);
                }
            }
        }
        self
    }

    /// BEGIN METAFILE, version 1, an element list.
    pub fn begin(name: &str) -> W {
        let mut w = W::new();
        w.el(0, 1, P::new().s(name));
        w.el(1, 1, P::new().i(1));
        w
    }

    /// BEGIN PICTURE with a VDC extent, then BEGIN PICTURE BODY.
    pub fn picture(&mut self, name: &str, ext: [i64; 4]) -> &mut Self {
        self.el(0, 3, P::new().s(name));
        self.el(2, 6, P::new().pt(ext[0], ext[1]).pt(ext[2], ext[3]));
        self.el(0, 4, P::new())
    }

    pub fn end_picture(&mut self) -> &mut Self {
        self.el(0, 5, P::new())
    }

    pub fn finish(&mut self) -> Vec<u8> {
        self.el(0, 2, P::new());
        std::mem::take(&mut self.0)
    }
}
