//! intoPIX TicoRAW (the codestream of Nikon High Efficiency raw files): the header changes
//! and the packet order, on a small synthetic codestream built with the header of the Nikon
//! files (only the size changed).

/// Big-endian bit writer.
#[derive(Default)]
struct W {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl W {
    fn put(&mut self, v: u64, bits: u32) {
        for i in (0..bits).rev() {
            self.acc = self.acc << 1 | ((v >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    fn align(&mut self) {
        while self.n != 0 {
            self.put(0, 1);
        }
    }
}

/// Marker segments from SOC to the first slice header, as in a Nikon Z HE file, for a
/// `w`×`h` mosaic. The vendor bytes (the 13 extra picture-header bytes, the capability
/// string and the weights) are data copied from a CC0 raw.pixls.us Nikon Z sample.
fn header(w: u16, h: u16) -> Vec<u8> {
    header_with(w, h, true)
}

fn header_with(w: u16, h: u16, vendor: bool) -> Vec<u8> {
    let mut d = vec![0xFF, 0x10];
    let mut cap = b"CONTACT_INTOPIX_".to_vec();
    cap.extend_from_slice(&[0xEF, 0xC0]);
    cap.resize(32, 0);
    seg(&mut d, 0x50, &cap);
    let mut pih = vec![0, 0, 0, 0, 0, 0, 0, 0];
    pih.extend_from_slice(&w.to_be_bytes());
    pih.extend_from_slice(&h.to_be_bytes());
    pih.extend_from_slice(&[0, 0, 0, 1, 4, 4, 8, 18, 0x44, 0x10, 0x51, 0x14]);
    if vendor {
        pih.extend_from_slice(&[
            0x50, 0x88, 0x70, 0x83, 0xf0, 0x15, 0x23, 0xd1, 0x49, 0xcd, 0x3f, 0x7f, 0x07,
        ]);
    }
    seg(&mut d, 0x12, &pih);
    seg(
        &mut d,
        0x13,
        &[
            14, 0x11, 0x51, 14, 0x11, 0x51, 14, 0x11, 0x00, 14, 0x11, 0x51,
        ],
    );
    seg(&mut d, 0x14, &WGT);
    d
}

const WGT: [u8; 50] = [
    3, 2, 3, 19, 2, 9, 2, 22, 1, 10, 1, 21, 2, 5, 1, 3, 1, 17, 0, 1, 0, 7, 0, 13, 0, 11, 2, 6, 1,
    4, 1, 16, 0, 0, 0, 8, 0, 14, 1, 20, 0, 18, 0, 15, 0, 23, 0, 12, 0, 24,
];

fn seg(d: &mut Vec<u8>, m: u8, body: &[u8]) {
    d.extend_from_slice(&[0xFF, m]);
    d.extend_from_slice(&(body.len() as u16 + 2).to_be_bytes());
    d.extend_from_slice(body);
}

#[test]
fn header_changes() {
    let h = specular_jpegxs::header::parse(&codestream(64, 4)).unwrap();
    assert!(h.ticoraw());
    assert_eq!(h.component_grid(), (32, 2));
    assert_eq!(h.pih_extra.len(), 13);
    let levels: Vec<_> = h.components.iter().map(|c| c.levels).collect();
    assert_eq!(
        levels,
        [Some((5, 1)), Some((5, 1)), Some((0, 0)), Some((5, 1))]
    );
    assert_eq!((h.nlx, h.nly, h.bw, h.fq, h.cpih), (5, 1, 18, 4, 0));
    // Without the 13 bytes it is not TicoRAW (the component table's depths stay read).
    let mut d = header_with(64, 4, false);
    d.extend_from_slice(&[0xFF, 0x20, 0, 4, 0, 0]);
    let h = specular_jpegxs::header::parse(&d).unwrap();
    assert!(!h.ticoraw());
    assert_eq!(h.component_grid(), (64, 4));
}

/// One precinct (2 component rows): every packet raw-coded (4-bit counts), all counts zero
/// except the luma's low band, whose one coefficient is +4.
fn codestream(w: u16, h: u16) -> Vec<u8> {
    let mut d = header(w, h);
    d.extend_from_slice(&[0xFF, 0x20, 0, 4, 0, 0]);
    // Code groups per band line for 32-sample component rows: LL, H5, H4, H3, H2, HL on
    // the low row; LH, HH on the high one; the untransformed component's line.
    let low = [1usize, 1, 1, 1, 2, 4];
    let high = [4usize, 4];
    let flat = [8usize];
    let mut body = W::default();
    let packet = |b: &mut W, groups: &[usize], dc: bool| {
        let n: usize = groups.iter().sum();
        let gcli_bytes = (4 * n).div_ceil(8) as u64;
        b.put(1, 1); // raw counts
        b.put(if dc { 2 } else { 0 }, 20); // magnitude bytes
        b.put(gcli_bytes, 20);
        b.put(u64::from(dc), 15); // sign bytes
        b.align();
        for g in 0..n {
            b.put(if dc && g == 0 { 3 } else { 0 }, 4);
        }
        b.align();
        if dc {
            // Three bit planes of four coefficients: 4, 0, 0, 0; then its sign (+).
            b.put(0b1000_0000_0000, 12);
            b.align();
            b.put(0, 8);
        }
    };
    // Row 0: components 0, 1, 2 (untransformed), 3; row 1: the same.
    packet(&mut body, &low, true);
    packet(&mut body, &low, false);
    packet(&mut body, &flat, false);
    packet(&mut body, &low, false);
    packet(&mut body, &high, false);
    packet(&mut body, &high, false);
    packet(&mut body, &flat, false);
    packet(&mut body, &high, false);
    let body = body.out;
    let mut ph = W::default();
    ph.put(body.len() as u64, 24);
    ph.put(0, 8); // Q
    ph.put(0, 8); // R
    ph.put(0, 50); // coding modes: no prediction, no significance flags
    ph.align();
    d.extend_from_slice(&ph.out);
    d.extend_from_slice(&body);
    d.extend_from_slice(&[0xFF, 0x11]);
    d
}

#[test]
fn packets_in_ticoraw_order() {
    let c = specular_jpegxs::decode_coefficients(&codestream(64, 4)).unwrap();
    assert!(!c.truncated);
    assert_eq!(c.planes.len(), 4);
    for (w, h, _) in &c.planes {
        assert_eq!((*w, *h), (32, 2));
    }
    // The low band coefficient (4, with 4 fractional bits) spreads over the luma only.
    let sum = |i: usize| c.planes[i].2.iter().map(|&v| i64::from(v)).sum::<i64>();
    assert!(sum(0) > 0, "{:?}", c.planes[0].2);
    for i in 1..4 {
        assert!(c.planes[i].2.iter().all(|&v| v == 0), "component {i}");
    }
}

#[test]
fn damaged_ticoraw_is_flagged() {
    let mut d = codestream(64, 4);
    let n = d.len();
    d.truncate(n - 12);
    let c = specular_jpegxs::decode_coefficients(&d).unwrap();
    assert!(c.truncated);
}
