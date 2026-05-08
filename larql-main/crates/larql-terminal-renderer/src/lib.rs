pub mod decode;
pub mod dither;
pub mod image_buffer;
pub mod oklab;
pub mod quantize;
pub mod render;
pub mod resize;
pub mod terminal;
pub mod video;

pub use dither::dither_floyd_steinberg;
pub use image_buffer::{Image, Rgb};
pub use quantize::{quantize, Palette};
pub use render::{
    AnsiRenderer, BackendType, ITerm2Renderer, KittyRenderer, RenderBackend, Renderer,
    SixelRenderer,
};
pub use resize::{fit_to_pixels, resize_bilinear, resize_nearest};
pub use terminal::{get_terminal_size, get_terminal_size_with_pixel_query, TerminalSize};
pub use video::{FfmpegDecoder, VideoFrame, VideoProbe};
#[cfg(feature = "ratatui")]
pub mod ratatui_impl;

#[cfg(feature = "ratatui")]
pub use ratatui_impl::{draw_frame, AtomicGraphicsBackend, GraphicsLayer, ImageWidget};
