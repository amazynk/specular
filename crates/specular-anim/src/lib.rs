//! Animation playback engine (DESIGN.md §12.2 "Animation").
//!
//! A [`Player`] plays any [`specular_decode::Frames`] decoder (GIF, APNG, WebP, AVIF sequences,
//! JPEG XL, ANI, FLI/FLC):
//!
//! * **State machine**: play/pause, step forward/back, seek, speed 0.25×–4×, the file's loop
//!   count or a "loop forever" override, optional ping-pong.
//! * **Timing**: browser-compatible delay clamping ([`DelayPolicy`]) and a drift-free schedule:
//!   frame `s` is due at `start + Σ delays before it` on an absolute clock. A late tick skips
//!   to the frame that is due (and reports the skipped ones as dropped) instead of slowing
//!   playback down. [`Player::tick`] takes the current time and says what to show and when to
//!   tick next, so the UI sleeps between frames and redraws only when the frame changes.
//! * **Decode-ahead**: a worker thread fills a bounded frame ring within a byte budget
//!   (`src/worker.rs` explains the window and eviction order). Animations whose frames all fit are cached
//!   completely after the first loop and the decoder is dropped; larger ones stream. Frame
//!   counts unknown up front (GIF) are discovered during the first pass.
//! * **Scrub strip** (`src/strip.rs`): small copies of frames spread over a movie, decoded
//!   in the background, shown at once while the seek bar is dragged.
//! * **Hand-off**: frames are [`FrameRef`]s (`Arc`), so the UI thread can keep showing or
//!   uploading a frame while the worker evicts it. The shared lock is held only for map
//!   lookups and inserts, never during a decode.
//!
//! Time is always passed in (`now: Instant`), so tests drive the player with a simulated clock.

mod fake;
mod player;
mod policy;
mod strip;
mod timeline;
mod worker;

pub use fake::{FakeFrames, FakeProbe, FakeSpec};
pub use player::{MediaClock, Player, Position, Stats, TickResult};
pub use policy::{DelayPolicy, LoopMode, MAX_SPEED, MIN_SPEED, PlayerConfig};
pub use strip::{
    Layout, STRIP_MAX_HEIGHT, StripConfig, StripKind, StripSource, StripStats,
    layout as strip_layout, members as strip_members, pick as strip_pick, shrink as strip_shrink,
};
pub use worker::Waker;

use specular_core::Raster;
use std::sync::Arc;
use std::time::Duration;

/// A decoded frame as handed to the UI.
#[derive(Debug)]
pub struct AnimFrame {
    pub index: u32,
    /// The composited canvas, ready to show.
    pub raster: Raster,
    /// Delay as stored in the file (before [`DelayPolicy`]).
    pub delay: Duration,
    /// A small copy from the scrub strip ([`Player::start_strip`]), shown while dragging
    /// until the full frame is decoded. It covers the same picture: show it scaled up.
    pub strip: bool,
}

/// Shared, immutable frame. Cheap to clone; keeps the pixels alive while the UI uses them.
pub type FrameRef = Arc<AnimFrame>;
