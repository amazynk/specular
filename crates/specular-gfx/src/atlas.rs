//! An R8 glyph atlas with a shelf packer. The CPU copy is authoritative; the dirty region is
//! uploaded once per frame.

#[derive(Debug)]
struct Shelf {
    y: u32,
    h: u32,
    x: u32,
}

#[derive(Debug)]
pub struct Atlas {
    pub size: u32,
    data: Vec<u8>,
    shelves: Vec<Shelf>,
    next_y: u32,
    dirty: Option<[u32; 4]>,
    /// Bumped on every reset; cached glyph positions from older generations are invalid.
    pub generation: u64,
}

const PAD: u32 = 1;

impl Atlas {
    pub fn new(size: u32) -> Atlas {
        Atlas {
            size,
            data: vec![0; (size * size) as usize],
            shelves: Vec::new(),
            next_y: 0,
            dirty: None,
            generation: 0,
        }
    }

    pub fn reset(&mut self) {
        self.data.fill(0);
        self.shelves.clear();
        self.next_y = 0;
        self.dirty = Some([0, 0, self.size, self.size]);
        self.generation += 1;
    }

    /// Reserves a `w`×`h` area; `None` when full.
    pub fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let (w, h) = (w + PAD, h + PAD);
        if w > self.size || h > self.size {
            return None;
        }
        // Best-fitting shelf: the lowest one tall enough (and not more than 1.5× too tall).
        let mut best: Option<usize> = None;
        for (i, s) in self.shelves.iter().enumerate() {
            if s.h >= h
                && s.h <= h + h / 2 + 2
                && s.x + w <= self.size
                && best.is_none_or(|b| self.shelves[b].h > s.h)
            {
                best = Some(i);
            }
        }
        if let Some(i) = best {
            let s = &mut self.shelves[i];
            let pos = (s.x, s.y);
            s.x += w;
            return Some(pos);
        }
        if self.next_y + h > self.size {
            return None;
        }
        let y = self.next_y;
        self.next_y += h;
        self.shelves.push(Shelf { y, h, x: w });
        Some((0, y))
    }

    /// Copies a tightly packed `w`×`h` coverage image to (`x`, `y`).
    pub fn write(&mut self, x: u32, y: u32, w: u32, h: u32, src: &[u8]) {
        for row in 0..h {
            let d = ((y + row) * self.size + x) as usize;
            let s = (row * w) as usize;
            self.data[d..d + w as usize].copy_from_slice(&src[s..s + w as usize]);
        }
        let r = [x, y, x + w, y + h];
        self.dirty = Some(match self.dirty {
            None => r,
            Some(d) => [
                d[0].min(r[0]),
                d[1].min(r[1]),
                d[2].max(r[2]),
                d[3].max(r[3]),
            ],
        });
    }

    /// Records a copy of the changed rows into `texture`, staged through
    /// `belt` so the frame never takes the allocator lock (see `UiRenderer`).
    pub fn upload(
        &mut self,
        belt: &mut wgpu::util::StagingBelt,
        enc: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) {
        let Some(d) = self.dirty.take() else { return };
        // Upload whole rows: simpler, and the stride stays the atlas width
        // (a multiple of the 256-byte row alignment for the 2048 atlas).
        let (y0, y1) = (d[1], d[3]);
        if y1 <= y0 {
            return;
        }
        let off = (y0 * self.size) as usize;
        let len = ((y1 - y0) * self.size) as usize;
        let Some(size) = wgpu::BufferSize::new(len.next_multiple_of(4) as u64) else {
            return;
        };
        let align = wgpu::BufferSize::new(256).expect("non-zero");
        let slice = belt.allocate(size, align);
        // A lost device (a driver reset) maps nothing: no upload, nothing more to draw on it.
        let Ok(mut range) = slice.get_mapped_range_mut() else {
            return;
        };
        range
            .slice(..len)
            .copy_from_slice(&self.data[off..off + len]);
        drop(range);
        enc.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: slice.buffer(),
                layout: wgpu::TexelCopyBufferLayout {
                    offset: slice.offset(),
                    bytes_per_row: Some(self.size),
                    rows_per_image: None,
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: y0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.size,
                height: y1 - y0,
                depth_or_array_layers: 1,
            },
        );
    }
}
