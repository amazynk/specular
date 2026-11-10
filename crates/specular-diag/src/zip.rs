//! A minimal zip writer: stored (uncompressed) entries, which every unzip tool and mail
//! filter reads. The bundle is text of a few hundred KB, so compression would buy little.

/// Builds a zip archive in memory.
#[derive(Debug, Default)]
pub struct ZipWriter {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
    time: u16,
    date: u16,
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Days since 1970-01-01 → (year, month, day) in the proleptic Gregorian calendar.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// MS-DOS (time, date) for seconds since the Unix epoch (UTC), clamped to 1980..=2107.
pub fn dos_time(unix: u64) -> (u16, u16) {
    let secs = unix.min(i64::MAX as u64) as i64;
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    if y < 1980 {
        return (0, (1 << 5) | 1);
    }
    let y = y.min(2107);
    let time = ((s / 3600) << 11) | (((s % 3600) / 60) << 5) | ((s % 60) / 2);
    let date = ((y - 1980) << 9) | (i64::from(m) << 5) | i64::from(d);
    (time as u16, date as u16)
}

impl ZipWriter {
    /// Entries are stamped with `unix` (seconds, UTC).
    pub fn new(unix: u64) -> ZipWriter {
        let (time, date) = dos_time(unix);
        ZipWriter {
            time,
            date,
            ..ZipWriter::default()
        }
    }

    /// Adds a file. Names use `/`; the bundle's are short ASCII.
    ///
    /// # Panics
    /// With more than 65 535 entries or an entry or archive over 4 GiB (no zip64).
    pub fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32fast::hash(data);
        let len = u32::try_from(data.len()).expect("zip entry over 4 GiB");
        let offset = u32::try_from(self.out.len()).expect("zip over 4 GiB");
        let name_len = u16::try_from(name.len()).expect("zip entry name too long");
        // UTF-8 names (bit 11).
        let flags = 1 << 11;
        let o = &mut self.out;
        u32le(o, 0x0403_4b50);
        u16le(o, 20);
        u16le(o, flags);
        u16le(o, 0); // stored
        u16le(o, self.time);
        u16le(o, self.date);
        u32le(o, crc);
        u32le(o, len);
        u32le(o, len);
        u16le(o, name_len);
        u16le(o, 0);
        o.extend_from_slice(name.as_bytes());
        o.extend_from_slice(data);

        let c = &mut self.central;
        u32le(c, 0x0201_4b50);
        u16le(c, (3 << 8) | 20); // made by Unix, version 2.0
        u16le(c, 20);
        u16le(c, flags);
        u16le(c, 0);
        u16le(c, self.time);
        u16le(c, self.date);
        u32le(c, crc);
        u32le(c, len);
        u32le(c, len);
        u16le(c, name_len);
        u16le(c, 0); // extra
        u16le(c, 0); // comment
        u16le(c, 0); // disk
        u16le(c, 0); // internal attributes
        u32le(c, 0o100_644 << 16); // a regular file, rw-r--r--
        u32le(c, offset);
        c.extend_from_slice(name.as_bytes());
        self.count = self.count.checked_add(1).expect("too many zip entries");
    }

    /// The finished archive.
    pub fn finish(mut self) -> Vec<u8> {
        let start = u32::try_from(self.out.len()).expect("zip over 4 GiB");
        let size = u32::try_from(self.central.len()).expect("zip over 4 GiB");
        self.out.append(&mut self.central);
        let o = &mut self.out;
        u32le(o, 0x0605_4b50);
        u16le(o, 0);
        u16le(o, 0);
        u16le(o, self.count);
        u16le(o, self.count);
        u32le(o, size);
        u32le(o, start);
        u16le(o, 0);
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn dos_dates() {
        // 2026-10-12 10:20:30 UTC.
        let (t, d) = dos_time(1_791_800_430);
        assert_eq!((d >> 9) + 1980, 2026);
        assert_eq!((d >> 5) & 15, 10);
        assert_eq!(d & 31, 12);
        assert_eq!((t >> 11, (t >> 5) & 63, (t & 31) * 2), (10, 20, 30));
        assert_eq!(dos_time(0), (0, 33));
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
        assert_eq!(civil(11_016), (2000, 2, 29));
    }

    /// The `zip` crate (the oracle) reads back every entry, byte for byte, with its date.
    #[test]
    fn the_zip_crate_reads_it() {
        let files: [(&str, &[u8]); 4] = [
            ("summary.txt", b"Specular 1.0\n"),
            ("crash/crash-1.txt", "panicked at \u{e9}\n".as_bytes()),
            ("empty.txt", b""),
            ("log.txt", &[b'x'; 70_000]),
        ];
        let mut z = ZipWriter::new(1_791_800_430);
        for (n, d) in files {
            z.add(n, d);
        }
        let bytes = z.finish();
        let mut a = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(a.len(), files.len());
        for (i, (n, d)) in files.iter().enumerate() {
            let mut f = a.by_index(i).unwrap();
            assert_eq!(f.name().unwrap(), *n);
            let date = f.last_modified().unwrap();
            assert_eq!((date.year(), date.month(), date.day()), (2026, 10, 12));
            assert_eq!((date.hour(), date.minute(), date.second()), (10, 20, 30));
            let mut got = Vec::new();
            f.read_to_end(&mut got).unwrap();
            assert_eq!(&got, d, "{n}");
        }
    }
}
