//! Software renderer: palette-indexed 320×200 framebuffer, column/span drawing,
//! BSP traversal, sprite compositing.
//!
//! Hot path — no Verus proofs in this crate.
//! `Framebuffer` and `PaletteLut` are the primary types consumed by `doom-tui`.

pub mod clip;
pub mod column;
pub mod framebuffer;
pub mod palette;
pub mod render;
pub mod seg;
pub mod span;
pub mod sprite;
pub mod texture;
pub mod visplane;

pub use column::{DrawColumnParams, IDENTITY_COLORMAP, draw_column, draw_column_solid};
pub use framebuffer::Framebuffer;
pub use palette::{PaletteLut, Rgb};
pub use render::render_level;
pub use span::{DrawSpanParams, FLAT_DIM, FLAT_MASK, draw_span, draw_span_solid};
