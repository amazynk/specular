//! A bounds-checked FlatBuffers table reader (FlatGeobuf headers and features, Arrow IPC
//! metadata). Every accessor returns `None` (or the schema default) for a missing field or
//! an offset outside the buffer, so a corrupt file cannot make it panic.

/// A table in a FlatBuffers buffer.
#[derive(Clone, Copy, Debug)]
pub struct Table<'a> {
    buf: &'a [u8],
    pos: usize,
    vt: usize,
    vt_len: usize,
}

fn rd<const N: usize>(b: &[u8], at: usize) -> Option<[u8; N]> {
    b.get(at..at.checked_add(N)?)?.try_into().ok()
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    rd::<4>(b, at).map(u32::from_le_bytes)
}

/// A vector in the buffer: where its elements start and how many there are.
#[derive(Clone, Copy, Debug)]
pub struct Vector<'a> {
    buf: &'a [u8],
    start: usize,
    pub len: usize,
}

impl<'a> Vector<'a> {
    /// The raw element bytes, when `len × size` fit the buffer.
    pub fn bytes(&self, size: usize) -> Option<&'a [u8]> {
        let n = self.len.checked_mul(size)?;
        self.buf.get(self.start..self.start.checked_add(n)?)
    }

    pub fn f64s(&self) -> Option<impl ExactSizeIterator<Item = f64> + 'a + use<'a>> {
        Some(
            self.bytes(8)?
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap_or([0; 8]))),
        )
    }

    pub fn u32s(&self) -> Option<impl ExactSizeIterator<Item = u32> + 'a + use<'a>> {
        Some(
            self.bytes(4)?
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap_or([0; 4]))),
        )
    }

    /// Element `i` of a vector of tables.
    pub fn table(&self, i: usize) -> Option<Table<'a>> {
        if i >= self.len {
            return None;
        }
        let at = self.start.checked_add(i.checked_mul(4)?)?;
        let off = u32_at(self.buf, at)? as usize;
        Table::at(self.buf, at.checked_add(off)?)
    }

    pub fn tables(&self) -> impl Iterator<Item = Table<'a>> + '_ {
        (0..self.len).filter_map(|i| self.table(i))
    }

    /// Element `i` of a vector of inline structs of `size` bytes.
    pub fn struct_bytes(&self, i: usize, size: usize) -> Option<&'a [u8]> {
        if i >= self.len {
            return None;
        }
        let at = self.start.checked_add(i.checked_mul(size)?)?;
        self.buf.get(at..at.checked_add(size)?)
    }
}

impl<'a> Table<'a> {
    /// The root table of a buffer (its first four bytes are the root offset).
    pub fn root(buf: &'a [u8]) -> Option<Table<'a>> {
        let off = u32_at(buf, 0)? as usize;
        Table::at(buf, off)
    }

    /// The table at `pos`.
    pub fn at(buf: &'a [u8], pos: usize) -> Option<Table<'a>> {
        let soff = i32::from_le_bytes(rd::<4>(buf, pos)?);
        let vt = usize::try_from(i64::try_from(pos).ok()? - i64::from(soff)).ok()?;
        let vt_len = usize::from(u16::from_le_bytes(rd::<2>(buf, vt)?));
        if vt_len < 4 || vt.checked_add(vt_len)? > buf.len() {
            return None;
        }
        Some(Table {
            buf,
            pos,
            vt,
            vt_len,
        })
    }

    /// The absolute position of field `i`, when present.
    fn field(&self, i: usize) -> Option<usize> {
        let slot = 4 + 2 * i;
        if slot + 2 > self.vt_len {
            return None;
        }
        let off = u16::from_le_bytes(rd::<2>(self.buf, self.vt + slot)?);
        (off != 0).then_some(self.pos + usize::from(off))
    }

    pub fn present(&self, i: usize) -> bool {
        self.field(i).is_some()
    }

    pub fn u8(&self, i: usize, default: u8) -> u8 {
        self.field(i)
            .and_then(|p| self.buf.get(p).copied())
            .unwrap_or(default)
    }

    pub fn bool(&self, i: usize, default: bool) -> bool {
        self.u8(i, u8::from(default)) != 0
    }

    pub fn i16(&self, i: usize, default: i16) -> i16 {
        self.field(i)
            .and_then(|p| rd::<2>(self.buf, p))
            .map_or(default, i16::from_le_bytes)
    }

    pub fn u16(&self, i: usize, default: u16) -> u16 {
        self.field(i)
            .and_then(|p| rd::<2>(self.buf, p))
            .map_or(default, u16::from_le_bytes)
    }

    pub fn i32(&self, i: usize, default: i32) -> i32 {
        self.field(i)
            .and_then(|p| rd::<4>(self.buf, p))
            .map_or(default, i32::from_le_bytes)
    }

    pub fn u64(&self, i: usize, default: u64) -> u64 {
        self.field(i)
            .and_then(|p| rd::<8>(self.buf, p))
            .map_or(default, u64::from_le_bytes)
    }

    pub fn i64(&self, i: usize, default: i64) -> i64 {
        self.field(i)
            .and_then(|p| rd::<8>(self.buf, p))
            .map_or(default, i64::from_le_bytes)
    }

    /// The target of an offset field.
    fn indirect(&self, i: usize) -> Option<usize> {
        let p = self.field(i)?;
        p.checked_add(u32_at(self.buf, p)? as usize)
    }

    pub fn table(&self, i: usize) -> Option<Table<'a>> {
        Table::at(self.buf, self.indirect(i)?)
    }

    pub fn vector(&self, i: usize) -> Option<Vector<'a>> {
        let at = self.indirect(i)?;
        let len = u32_at(self.buf, at)? as usize;
        Some(Vector {
            buf: self.buf,
            start: at + 4,
            len,
        })
    }

    pub fn bytes(&self, i: usize) -> Option<&'a [u8]> {
        self.vector(i)?.bytes(1)
    }

    /// A string field (lossy UTF-8).
    pub fn str(&self, i: usize) -> Option<String> {
        self.bytes(i)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }

    /// The bytes of an inline struct field of `size` bytes.
    pub fn struct_bytes(&self, i: usize, size: usize) -> Option<&'a [u8]> {
        let p = self.field(i)?;
        self.buf.get(p..p.checked_add(size)?)
    }

    /// A union's value table (the union type is the field before it).
    pub fn union(&self, i: usize) -> Option<Table<'a>> {
        self.table(i)
    }
}

#[cfg(test)]
pub(crate) mod build {
    //! A minimal FlatBuffers writer for test fixtures: tables are written back to front
    //! the simple way (each table, then its vtable right after, all offsets forward).

    /// One field value of a table under construction.
    pub enum F {
        U8(u8),
        U16(u16),
        I32(i32),
        U64(u64),
        /// An offset to an object written by [`Builder`].
        Off(usize),
    }

    /// Writes objects into a growing buffer; positions are absolute.
    #[derive(Default)]
    pub struct Builder {
        pub buf: Vec<u8>,
    }

    impl Builder {
        fn align(&mut self, a: usize) {
            while !self.buf.len().is_multiple_of(a) {
                self.buf.push(0);
            }
        }

        pub fn bytes(&mut self, b: &[u8]) -> usize {
            self.align(4);
            let at = self.buf.len();
            self.buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
            self.buf.extend_from_slice(b);
            self.buf.push(0);
            at
        }

        pub fn f64s(&mut self, v: &[f64]) -> usize {
            // Element data 8-aligned: the length sits just before.
            while !(self.buf.len() + 4).is_multiple_of(8) {
                self.buf.push(0);
            }
            let at = self.buf.len();
            self.buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for x in v {
                self.buf.extend_from_slice(&x.to_le_bytes());
            }
            at
        }

        /// A vector of `n` offsets to tables, to be filled with [`Builder::patch`] at
        /// `at + 4 + 4 * k` once the tables are written.
        pub fn offsets_n(&mut self, n: usize) -> usize {
            self.align(4);
            let at = self.buf.len();
            self.buf.extend_from_slice(&(n as u32).to_le_bytes());
            self.buf.resize(self.buf.len() + 4 * n, 0);
            at
        }

        /// A table whose fields come in slot order (`None` absent). Offset fields must
        /// point forward, so objects referenced are written after: use [`Builder::patch`].
        pub fn table(&mut self, fields: &[Option<F>]) -> (usize, Vec<(usize, usize)>) {
            self.align(8);
            let pos = self.buf.len();
            self.buf.extend_from_slice(&[0; 4]); // soffset, patched below
            let mut offs = Vec::new();
            let mut slots = Vec::new();
            for f in fields {
                let Some(f) = f else {
                    slots.push(0u16);
                    continue;
                };
                let size = match f {
                    F::U8(_) => 1,
                    F::U16(_) => 2,
                    F::I32(_) | F::Off(_) => 4,
                    F::U64(_) => 8,
                };
                while !(self.buf.len() - pos).is_multiple_of(size) {
                    self.buf.push(0);
                }
                slots.push((self.buf.len() - pos) as u16);
                match f {
                    F::U8(v) => self.buf.push(*v),
                    F::U16(v) => self.buf.extend_from_slice(&v.to_le_bytes()),
                    F::I32(v) => self.buf.extend_from_slice(&v.to_le_bytes()),
                    F::U64(v) => self.buf.extend_from_slice(&v.to_le_bytes()),
                    F::Off(t) => {
                        offs.push((self.buf.len(), *t));
                        self.buf.extend_from_slice(&[0; 4]);
                    }
                }
            }
            let tsize = self.buf.len() - pos;
            self.align(2);
            let vt = self.buf.len();
            self.buf
                .extend_from_slice(&((4 + 2 * slots.len()) as u16).to_le_bytes());
            self.buf.extend_from_slice(&(tsize as u16).to_le_bytes());
            for s in slots {
                self.buf.extend_from_slice(&s.to_le_bytes());
            }
            let soff = pos as i64 - vt as i64;
            self.buf[pos..pos + 4].copy_from_slice(&(soff as i32).to_le_bytes());
            (pos, offs)
        }

        /// Point the offset slot at `slot` to `target` (which must lie after it).
        pub fn patch(&mut self, slot: usize, target: usize) {
            let v = (target - slot) as u32;
            self.buf[slot..slot + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
}
