//! A constant field (no packed bits) needs no data, so a message of a few bytes can declare
//! 2^26 values: they are reserved from the memory budget before they are allocated.

use specular_core::budget::{MemoryBudget, with_budget};
use specular_grib::{Error, Packing, Scale};
use std::sync::Arc;

#[test]
fn constant_field_reserves_before_allocating() {
    let file = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/grids.grib2"
    ))
    .unwrap();
    let mut f = specular_grib::scan(&file).unwrap().remove(0);
    f.packing = Packing::Simple(Scale {
        reference: 1.0,
        binary: 0,
        decimal: 0,
        bits: 0,
    });
    f.values = 1 << 26;
    f.bitmap = None;
    let budget = Arc::new(MemoryBudget::fixed(64 << 20));
    let r = with_budget(budget.clone(), || f.decode(&file));
    assert!(
        matches!(r, Err(Error::NeedsMemory(_))),
        "{:?}",
        r.map(|v| v.len())
    );
    assert_eq!(budget.reserved(), 0);
}
