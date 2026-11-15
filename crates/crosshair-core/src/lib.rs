//! `crosshair-core` — the crosshair model, geometry, rasteriser and preset
//! library shared by the Windows overlay and the editor.
//!
//! This crate is deliberately free of Win32 or GUI dependencies so the geometry
//! and the preset format can be unit tested on any host.

pub mod color;
pub mod geom;
pub mod image;
pub mod model;
pub mod preset;
pub mod render;
pub mod store;

pub use color::Rgba8;
pub use image::save_rgba_png;
pub use model::{Crosshair, Direction, Dot, DotShape, Layer, Line, LineShape, Outline, Point};
pub use render::{render, render_sized, RenderOptions, Rendered};
pub use store::{Hotkey, MoveShortcut, Settings, Store};

/// Crate version, shown in the app's About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
