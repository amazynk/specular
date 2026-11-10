//! Specular's own GPU toolkit on wgpu: device and surface setup, an instanced uber-shader for
//! rounded rectangles, shadows, glass, stroke icons and text, and the final composite with
//! sRGB encoding and blue-noise dither. No immediate-mode GUI library underneath.

pub mod atlas;
pub mod compose;
pub mod crossfade;
pub mod draw;
pub mod gpu;
pub mod icons;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod mark;
pub mod surface;
pub mod text;
pub mod ui;
pub mod unified;

pub use compose::{Compositor, FULLSCREEN_WGSL, HDR, Target, fullscreen_pipeline, uniform_buffer};
pub use crossfade::Crossfade;
pub use draw::{Backdrop, DrawList, Prim, Rect, TexId};
pub use gpu::{BufferInit, Gpu, PollError, poll_device, shader, video_memory};
pub use icons::{Icon, draw_icon};
pub use mark::Mark;
pub use surface::{Frame, HdrDisplay, WindowSurface};
pub use text::{Align, ColourGlyph, FontFace, PathEl, Text, TextBlock, TextStyle};
pub use ui::UiRenderer;
pub use {wgpu, winit};
