//! CABAC entropy decoding.

pub mod ctx;
#[cfg(test)]
pub mod encoder;
mod engine;
mod tables;

pub use ctx::{NUM_CTX, off};
#[cfg(test)]
pub use engine::initial_contexts;
pub use engine::{Cabac, initial_contexts_for};

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
