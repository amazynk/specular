//! Test fixtures: a stored ZIP writer, a version 3 compound file writer and an OLE property
//! set writer, for this crate's and the registry's tests. Not for files anyone keeps.

/// CRC-32 (IEEE), bitwise.
fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// A ZIP of stored members, in order.
pub fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let off = out.len() as u32;
        let crc = crc32(data);
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes());
            h.extend_from_slice(&[0; 4]); // flags, method
            h.extend_from_slice(&[0; 4]); // time, date
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            if central {
                h.extend_from_slice(&[0; 6]); // comment length, disk, internal attributes
                h.extend_from_slice(&0u32.to_le_bytes());
                h.extend_from_slice(&off.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend_from_slice(&header(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend_from_slice(&header(0x0201_4b50, true));
    }
    let cd = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

const END: u32 = 0xFFFF_FFFE;
const FREE: u32 = 0xFFFF_FFFF;
const FATSECT: u32 = 0xFFFF_FFFD;

/// A version 3 compound file with `streams` in its root storage: streams under 4096 bytes in
/// the mini stream, the rest in sectors of their own.
pub fn cfb(streams: &[(&str, &[u8])]) -> Vec<u8> {
    const SEC: usize = 512;
    let mut sectors: Vec<[u8; SEC]> = Vec::new();
    let mut fat: Vec<u32> = Vec::new();
    let put = |data: &[u8], sectors: &mut Vec<[u8; SEC]>, fat: &mut Vec<u32>| -> u32 {
        if data.is_empty() {
            return END;
        }
        let first = sectors.len() as u32;
        for chunk in data.chunks(SEC) {
            let mut s = [0u8; SEC];
            s[..chunk.len()].copy_from_slice(chunk);
            sectors.push(s);
            fat.push(sectors.len() as u32);
        }
        *fat.last_mut().unwrap() = END;
        first
    };
    // The mini stream and its FAT.
    let mut mini = Vec::new();
    let mut minifat: Vec<u32> = Vec::new();
    let mut starts = Vec::new();
    for (_, d) in streams {
        if d.len() < 4096 {
            if d.is_empty() {
                starts.push(END);
                continue;
            }
            let first = minifat.len() as u32;
            for chunk in d.chunks(64) {
                let mut c = [0u8; 64];
                c[..chunk.len()].copy_from_slice(chunk);
                mini.extend_from_slice(&c);
                minifat.push(minifat.len() as u32 + 1);
            }
            *minifat.last_mut().unwrap() = END;
            starts.push(first);
        } else {
            starts.push(u32::MAX);
        }
    }
    let mini_start = put(&mini, &mut sectors, &mut fat);
    let minifat_bytes: Vec<u8> = minifat.iter().flat_map(|v| v.to_le_bytes()).collect();
    let minifat_start = put(&minifat_bytes, &mut sectors, &mut fat);
    let minifat_sectors = minifat_bytes.len().div_ceil(SEC) as u32;
    for (i, (_, d)) in streams.iter().enumerate() {
        if starts[i] == u32::MAX {
            starts[i] = put(d, &mut sectors, &mut fat);
        }
    }
    // Directory: the root, then the streams as a right-leaning chain in CFB order.
    let mut order: Vec<usize> = (0..streams.len()).collect();
    let key = |n: &str| (n.encode_utf16().count(), n.to_uppercase());
    order.sort_by_key(|&i| key(streams[i].0));
    let entry = |name: &str, kind: u8, child: u32, right: u32, start: u32, size: u64| {
        let mut e = [0u8; 128];
        let units: Vec<u16> = name.encode_utf16().collect();
        for (k, u) in units.iter().take(31).enumerate() {
            e[k * 2..k * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        e[64..66].copy_from_slice(&(((units.len().min(31)) + 1) as u16 * 2).to_le_bytes());
        e[66] = kind;
        e[67] = 1;
        e[68..72].copy_from_slice(&FREE.to_le_bytes());
        e[72..76].copy_from_slice(&right.to_le_bytes());
        e[76..80].copy_from_slice(&child.to_le_bytes());
        e[116..120].copy_from_slice(&start.to_le_bytes());
        e[120..128].copy_from_slice(&size.to_le_bytes());
        e
    };
    let mut dir = Vec::new();
    let first_child = if order.is_empty() { FREE } else { 1 };
    dir.extend_from_slice(&entry(
        "Root Entry",
        5,
        first_child,
        FREE,
        mini_start,
        mini.len() as u64,
    ));
    for (k, &i) in order.iter().enumerate() {
        let right = if k + 1 < order.len() {
            k as u32 + 2
        } else {
            FREE
        };
        let (name, d) = streams[i];
        dir.extend_from_slice(&entry(name, 2, FREE, right, starts[i], d.len() as u64));
    }
    while dir.len() % SEC != 0 {
        dir.extend_from_slice(&entry("", 0, FREE, FREE, FREE, 0));
    }
    let dir_start = put(&dir, &mut sectors, &mut fat);
    // FAT sectors at the end, covering themselves.
    let mut n_fat = 1usize;
    while (sectors.len() + n_fat) > n_fat * (SEC / 4) {
        n_fat += 1;
    }
    let fat_first = sectors.len() as u32;
    fat.extend(std::iter::repeat_n(FATSECT, n_fat));
    fat.resize(n_fat * (SEC / 4), FREE);
    let fat_bytes: Vec<u8> = fat.iter().flat_map(|v| v.to_le_bytes()).collect();
    for chunk in fat_bytes.chunks(SEC) {
        let mut s = [0u8; SEC];
        s.copy_from_slice(chunk);
        sectors.push(s);
    }
    assert!(n_fat <= 109, "fixture too large");
    let mut h = vec![0u8; SEC];
    h[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
    h[24..26].copy_from_slice(&0x3Eu16.to_le_bytes());
    h[26..28].copy_from_slice(&3u16.to_le_bytes());
    h[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes());
    h[30..32].copy_from_slice(&9u16.to_le_bytes());
    h[32..34].copy_from_slice(&6u16.to_le_bytes());
    h[44..48].copy_from_slice(&(n_fat as u32).to_le_bytes());
    h[48..52].copy_from_slice(&dir_start.to_le_bytes());
    h[56..60].copy_from_slice(&4096u32.to_le_bytes());
    h[60..64].copy_from_slice(&minifat_start.to_le_bytes());
    h[64..68].copy_from_slice(&minifat_sectors.to_le_bytes());
    h[68..72].copy_from_slice(&END.to_le_bytes());
    for k in 0..109 {
        let v = if k < n_fat {
            fat_first + k as u32
        } else {
            FREE
        };
        h[76 + k * 4..80 + k * 4].copy_from_slice(&v.to_le_bytes());
    }
    for s in sectors {
        h.extend_from_slice(&s);
    }
    h
}

/// A property value to write.
pub enum Prop<'a> {
    /// VT_LPSTR in code page 1252 (ASCII and Latin-1 only).
    Str(&'a str),
    I4(i32),
    /// VT_FILETIME.
    Time(u64),
    /// VT_CF: format tag, clipboard format, data.
    Clip(i32, u32, &'a [u8]),
}

/// A property set stream with one section (`fmtid`) holding `props` (code page 1252).
pub fn property_set(fmtid: [u8; 16], props: &[(u32, Prop)]) -> Vec<u8> {
    let mut values: Vec<(u32, Vec<u8>)> = vec![(1, {
        let mut v = 2u32.to_le_bytes().to_vec();
        v.extend_from_slice(&1252u16.to_le_bytes());
        v.extend_from_slice(&[0; 2]);
        v
    })];
    for (id, p) in props {
        let mut v = Vec::new();
        match p {
            Prop::Str(s) => {
                v.extend_from_slice(&30u32.to_le_bytes());
                let mut b: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
                b.push(0);
                v.extend_from_slice(&(b.len() as u32).to_le_bytes());
                v.extend_from_slice(&b);
            }
            Prop::I4(i) => {
                v.extend_from_slice(&3u32.to_le_bytes());
                v.extend_from_slice(&i.to_le_bytes());
            }
            Prop::Time(t) => {
                v.extend_from_slice(&64u32.to_le_bytes());
                v.extend_from_slice(&t.to_le_bytes());
            }
            Prop::Clip(tag, cf, data) => {
                v.extend_from_slice(&71u32.to_le_bytes());
                v.extend_from_slice(&((8 + data.len()) as u32).to_le_bytes());
                v.extend_from_slice(&tag.to_le_bytes());
                v.extend_from_slice(&cf.to_le_bytes());
                v.extend_from_slice(data);
            }
        }
        while v.len() % 4 != 0 {
            v.push(0);
        }
        values.push((*id, v));
    }
    let head = 8 + values.len() * 8;
    let mut sec = Vec::new();
    let mut body = Vec::new();
    let mut index = Vec::new();
    for (id, v) in &values {
        index.extend_from_slice(&id.to_le_bytes());
        index.extend_from_slice(&((head + body.len()) as u32).to_le_bytes());
        body.extend_from_slice(v);
    }
    sec.extend_from_slice(&((head + body.len()) as u32).to_le_bytes());
    sec.extend_from_slice(&(values.len() as u32).to_le_bytes());
    sec.extend_from_slice(&index);
    sec.extend_from_slice(&body);
    let mut out = Vec::new();
    out.extend_from_slice(&0xFFFEu16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0x0002_0006u32.to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&fmtid);
    out.extend_from_slice(&48u32.to_le_bytes());
    out.extend_from_slice(&sec);
    out
}

/// The summary information section's FMTID, F29F85E0-4FF9-1068-AB91-08002B27B3D9.
pub const FMTID_SUMMARY: [u8; 16] = [
    0xE0, 0x85, 0x9F, 0xF2, 0xF9, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27, 0xB3, 0xD9,
];
/// The document summary information's, D5CDD502-2E9C-101B-9397-08002B2CF9AE.
pub const FMTID_DOC_SUMMARY: [u8; 16] = [
    0x02, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C, 0xF9, 0xAE,
];

/// A tiny 2×2 24-bit packed DIB (red, green / blue, white; bottom-up rows).
pub fn dib_2x2() -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(&40u32.to_le_bytes());
    d.extend_from_slice(&2i32.to_le_bytes());
    d.extend_from_slice(&2i32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&24u16.to_le_bytes());
    d.extend_from_slice(&[0; 24]);
    // Bottom row: blue, white; top row: red, green (BGR, rows padded to 4 bytes).
    d.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);
    d.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
    d
}

/// A small WMF (no placeable header): a 100×100 window with a red square in it.
pub fn wmf_square() -> Vec<u8> {
    // (function, parameters): window origin and extent, a solid red brush, selected, a
    // rectangle (bottom, right, top, left), end of file.
    let records: [(u16, &[u16]); 6] = [
        (0x020B, &[0, 0]),
        (0x020C, &[100, 100]),
        (0x02FC, &[0, 0x00FF, 0, 0]),
        (0x012D, &[0]),
        (0x041B, &[90, 90, 10, 10]),
        (0, &[]),
    ];
    let mut body = Vec::new();
    let mut max = 0u32;
    for (f, params) in records {
        let words = 3 + params.len() as u32;
        max = max.max(words);
        body.extend_from_slice(&words.to_le_bytes());
        body.extend_from_slice(&f.to_le_bytes());
        for p in params {
            body.extend_from_slice(&p.to_le_bytes());
        }
    }
    let total = 9 + body.len() as u32 / 2;
    let mut w = Vec::new();
    for v in [1u16, 9, 0x300] {
        w.extend_from_slice(&v.to_le_bytes());
    }
    w.extend_from_slice(&total.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&max.to_le_bytes());
    w.extend_from_slice(&0u16.to_le_bytes());
    w.extend_from_slice(&body);
    w
}
