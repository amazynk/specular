//! No-op stand-in for the `tracing` crate used by the forked jxl-oxide modules.
//!
//! Upstream logs and opens spans through `tracing`; the fork keeps the call sites (so diffs
//! against upstream stay small) but compiles them to nothing.

#![allow(unused_macros, unused_imports, dead_code)]

pub(crate) struct Span;

impl Span {
    #[inline(always)]
    pub(crate) fn in_scope<R>(&self, f: impl FnOnce() -> R) -> R {
        f()
    }
    #[inline(always)]
    pub(crate) fn entered(self) -> Self {
        self
    }
    #[inline(always)]
    pub(crate) fn enter(&self) {}
}

pub(crate) struct Level;

impl Level {
    pub(crate) const TRACE: Level = Level;
}

macro_rules! noop_log {
    ($($t:tt)*) => {
        ()
    };
}

macro_rules! noop_span {
    ($($t:tt)*) => {
        $crate::tracing::Span
    };
}

pub(crate) use noop_log as debug;
pub(crate) use noop_log as error;
pub(crate) use noop_log as info;
pub(crate) use noop_log as trace;
pub(crate) use noop_log as warn;
pub(crate) use noop_span as span;
pub(crate) use noop_span as trace_span;
