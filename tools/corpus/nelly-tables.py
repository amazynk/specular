"""Prints Rust consts for the four Nellymoser tables of FFmpeg's nellymoser.c (data only)."""

import re
import sys

src = open(sys.argv[1]).read()


def table(name):
    m = re.search(name + r"\[[A-Z_0-9]*\]\s*=\s*\{(.*?)\};", src, re.S)
    return [v for v in re.split(r"[\s,]+", m.group(1).strip()) if v]


def emit(rust, ty, vals, per):
    print(f"pub(crate) const {rust}: [{ty}; {len(vals)}] = [")
    for i in range(0, len(vals), per):
        print("    " + ", ".join(vals[i : i + per]) + ",")
    print("];")
    print()


print("""//! The Nellymoser Asao tables, as data.
//!
//! PROVENANCE: the values of FFmpeg n8.0 `libavcodec/nellymoser.c` (`ff_nelly_dequantization_table`,
//! `ff_nelly_band_sizes_table`, `ff_nelly_init_table`, `ff_nelly_delta_table`), MIT licence:
//! Copyright (c) 2007 a840bda5870ba11f19698ff6eb9581dfb0f95fa5,
//! 539459aeb7d425140b62a3ec7dbf6dc8e408a306 and 520e17cd55896441042b14df2566a6eb610ed444
//! (the nelly2pcm authors); Copyright (c) 2007 Loic Minier, Benjamin Larsson. Copied as data;
//! the decoding in `nelly_sound.rs` is our own.
#![allow(clippy::excessive_precision)]
""")
emit("DEQUANT", "f32", table("ff_nelly_dequantization_table"), 6)
emit("BAND_SIZES", "u8", table("ff_nelly_band_sizes_table"), 23)
emit("INIT", "u16", table("ff_nelly_init_table"), 12)
emit("DELTA", "i16", table("ff_nelly_delta_table"), 12)
