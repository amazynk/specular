//! Hostile parameter sets and slice headers: out-of-range syntax
//! elements must fail cleanly, never overflow later QP arithmetic.

use crate::Error;
use crate::bits::BitWriter;
use crate::synth::{build_slice, build_sps, build_vps, to_nal};

/// [`crate::synth::build_pps`] with the given QP fields.
fn pps(init_qp_minus26: i32, cb: i32, chroma_offsets: bool) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.ue(0);
    w.ue(0);
    w.put(0, 7); // dependent, output, extra bits, sign hiding, cabac init
    w.ue(0);
    w.ue(0);
    w.se(init_qp_minus26);
    w.put(0, 3); // constrained intra, transform skip, cu_qp_delta
    w.se(cb);
    w.se(0);
    w.put(u32::from(chroma_offsets), 1);
    w.put(0, 5); // weighted x2, transquant bypass, tiles, wpp
    w.put(0, 1); // across slices
    w.put(1, 1); // deblocking control present
    w.put(0, 1);
    w.put(1, 1); // deblocking disabled
    w.put(0, 2); // scaling list, lists modification
    w.ue(0);
    w.put(0, 2);
    w.rbsp_trailing();
    to_nal(34, &w.bytes)
}

/// An I slice header with the given QP fields; the slice data is junk
/// (the header is refused before it is read).
fn slice(qp_delta: i32, cb: Option<i32>) -> Vec<u8> {
    let mut w = BitWriter::default();
    w.put(1, 1);
    w.put(0, 1);
    w.ue(0);
    w.ue(2);
    w.se(qp_delta);
    if let Some(cb) = cb {
        w.se(cb);
        w.se(0);
    }
    w.put(1, 1);
    while w.len_bits() % 8 != 0 {
        w.put(0, 1);
    }
    w.bytes.extend([0u8; 16]);
    to_nal(19, &w.bytes)
}

fn decode(pps: &[u8], slice: &[u8]) -> crate::Result<crate::Frame> {
    let vps = to_nal(32, &build_vps());
    let sps = to_nal(33, &build_sps(64, 64, 1, 8));
    crate::decode_still(&[&vps, &sps, pps], &[slice])
}

/// The rejection names the field, so the test cannot pass on a
/// writer slip that breaks the syntax elsewhere.
fn refused(r: crate::Result<crate::Frame>, field: &str) -> bool {
    matches!(r, Err(Error::InvalidData(m)) if m.contains(field))
}

#[test]
fn hostile_writer_baseline() {
    assert!(decode(&pps(0, 0, false), &to_nal(19, &build_slice(1, 1))).is_ok());
}

#[test]
fn pps_qp_fields_out_of_range() {
    for init in [i32::MAX, -i32::MAX, 26, -75] {
        let r = decode(&pps(init, 0, false), &slice(0, None));
        assert!(refused(r, "init_qp_minus26"), "{init}");
    }
    for cb in [13, -i32::MAX] {
        assert!(refused(
            decode(&pps(0, cb, false), &slice(0, None)),
            "pps_cb_qp_offset"
        ));
    }
}

#[test]
fn slice_qp_fields_out_of_range() {
    let p = pps(0, 0, true);
    for delta in [i32::MAX, -i32::MAX] {
        assert!(refused(
            decode(&p, &slice(delta, Some(0))),
            "slice_qp_delta"
        ));
    }
    for delta in [26, -27] {
        assert!(
            refused(decode(&p, &slice(delta, Some(0))), "SliceQpY"),
            "{delta}"
        );
    }
    for cb in [13, i32::MAX] {
        assert!(refused(
            decode(&p, &slice(0, Some(cb))),
            "slice_cb_qp_offset"
        ));
    }
}
