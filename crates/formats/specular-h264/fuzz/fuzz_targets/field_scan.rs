//! Fuzz the MP4 side of H.264: the `avcC` record, length-prefixed samples, [`FieldScan`]'s
//! slice-header peeks and field pairing, and [`Decoder::decode_sample`] on the same samples.
//! The input is `[avcC length: u16 BE][avcC][sample length: u16 BE][sample]...`. Run from
//! this directory with `cargo +nightly fuzz run field_scan corpus/field_scan`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use specular_h264::{Decoder, FieldScan, Options, OutputOrder};

fn take<'a>(data: &mut &'a [u8]) -> Option<&'a [u8]> {
    let n = u16::from_be_bytes([*data.first()?, *data.get(1)?]) as usize;
    let out = data.get(2..2 + n)?;
    *data = &data[2 + n..];
    Some(out)
}

fuzz_target!(|input: &[u8]| {
    let mut data = input;
    let Some(avcc) = take(&mut data) else { return };
    let mut samples = Vec::new();
    while let Some(s) = take(&mut data) {
        samples.push(s);
    }
    if let Ok(mut scan) = FieldScan::with_avcc(avcc) {
        // Whether the previous sample left a first field open (unknown after an error).
        let mut open = Some(false);
        for s in &samples {
            match scan.sample(s) {
                Ok(f) => {
                    // A sample can only complete a field left open before it.
                    assert!(
                        !f.completes_open || open != Some(false),
                        "completes a field never opened"
                    );
                    open = Some(f.leaves_open);
                }
                Err(_) => open = None,
            }
        }
    }
    let opts = Options {
        threads: if input.len() % 2 == 1 { 2 } else { 1 },
        output: if input.len() % 3 == 0 {
            OutputOrder::Decode
        } else {
            OutputOrder::Display
        },
    };
    let Ok(mut d) = Decoder::with_avcc(avcc, opts) else {
        return;
    };
    for (i, s) in samples.iter().enumerate() {
        let _ = d.decode_sample(s, i as i64);
        while d.next_picture(false).is_some() {}
    }
    d.flush();
    while d.next_picture(true).is_some() {}
});
