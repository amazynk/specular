//! Copied ISO/IEC 14496-3 data tables; where each came from, and under which licence, is
//! recorded in `PROVENANCE.md` beside them.
#![allow(clippy::excessive_precision)] // the digits as published, rounded to f32 by the compiler

pub(crate) mod lc;
pub(crate) mod ps;
pub(crate) mod sbr;
