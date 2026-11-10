//! Tag trees (B.10.2) and the packet-header bit reader (B.10.1).

/// Bit reader for packet headers: MSB first, a 0xFF byte is followed by a byte with only 7
/// bits (bit stuffing). Reading past the end yields 0 bits and sets `overrun`.
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    pub pos: usize,
    buf: u32,
    ct: u32,
    last_ff: bool,
    pub overrun: bool,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], pos: usize) -> BitReader<'a> {
        BitReader {
            data,
            pos,
            buf: 0,
            ct: 0,
            last_ff: false,
            overrun: false,
        }
    }

    #[inline]
    pub fn bit(&mut self) -> u32 {
        if self.ct == 0 {
            let b = match self.data.get(self.pos) {
                Some(&b) => b,
                None => {
                    self.overrun = true;
                    0
                }
            };
            self.pos += 1;
            self.ct = if self.last_ff { 7 } else { 8 };
            self.last_ff = b == 0xFF;
            self.buf = b as u32;
        }
        self.ct -= 1;
        (self.buf >> self.ct) & 1
    }

    pub fn bits(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit();
        }
        v
    }

    /// End of a packet header: skip the remaining bits and a stuffing byte after 0xFF.
    pub fn align(&mut self) {
        self.ct = 0;
        if self.last_ff {
            self.pos += 1;
            self.last_ff = false;
        }
    }
}

#[derive(Clone, Copy)]
struct Node {
    value: u32,
    low: u32,
    parent: u32,
}

pub(crate) struct TagTree {
    nodes: Vec<Node>,
}

const UNKNOWN: u32 = u32::MAX;

impl TagTree {
    pub fn new(w: u32, h: u32) -> TagTree {
        let mut dims = Vec::new();
        let (mut cw, mut ch) = (w.max(1), h.max(1));
        loop {
            dims.push((cw, ch));
            if cw == 1 && ch == 1 {
                break;
            }
            cw = cw.div_ceil(2);
            ch = ch.div_ceil(2);
        }
        let total: usize = dims.iter().map(|&(a, b)| a as usize * b as usize).sum();
        let mut nodes = Vec::with_capacity(total);
        let mut level_start = 0usize;
        for (l, &(lw, lh)) in dims.iter().enumerate() {
            let next_start = level_start + lw as usize * lh as usize;
            for y in 0..lh {
                for x in 0..lw {
                    let parent = if l + 1 < dims.len() {
                        let pw = dims[l + 1].0;
                        (next_start + (y / 2 * pw + x / 2) as usize) as u32
                    } else {
                        u32::MAX
                    };
                    nodes.push(Node {
                        value: UNKNOWN,
                        low: 0,
                        parent,
                    });
                }
            }
            level_start = next_start;
        }
        TagTree { nodes }
    }

    /// Decode whether leaf `leaf`'s value is below `threshold`; returns the value if it became
    /// known to be `< threshold`.
    pub fn decode(&mut self, br: &mut BitReader, leaf: usize, threshold: u32) -> Option<u32> {
        let mut stack = [0u32; 40];
        let mut n = 0;
        let mut node = leaf as u32;
        while node != u32::MAX && n < stack.len() {
            stack[n] = node;
            n += 1;
            node = self.nodes[node as usize].parent;
        }
        let mut low = 0u32;
        while n > 0 {
            n -= 1;
            let nd = &mut self.nodes[stack[n] as usize];
            if low > nd.low {
                nd.low = low;
            } else {
                low = nd.low;
            }
            while low < threshold && low < nd.value {
                if br.bit() != 0 {
                    nd.value = low;
                } else {
                    low += 1;
                }
                if br.overrun {
                    return None;
                }
            }
            nd.low = low;
        }
        let v = self.nodes[leaf].value;
        (v < threshold).then_some(v)
    }

    /// Decode the full value of a leaf (used for the zero bit-plane tree); capped at `max`.
    pub fn decode_value(&mut self, br: &mut BitReader, leaf: usize, max: u32) -> Option<u32> {
        let mut t = 1;
        loop {
            if let Some(v) = self.decode(br, leaf, t) {
                return Some(v);
            }
            if br.overrun || t > max {
                return None;
            }
            t += 1;
        }
    }
}
