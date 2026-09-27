//! Classical handwriting-ink extraction.
//!
//! No external crates: everything here is plain `std`, so the same code runs in
//! a CLI, a Tauri backend, a WebAssembly bundle or on mobile without change.

mod alpha;
mod background;
mod binarize;
mod cleanup;
mod color;
mod extract;
mod filters;
mod grid;
mod mask;
mod metrics;

pub use alpha::ink_opacity;
pub use background::{default_radius, estimate_background, normalize_illumination};
pub use binarize::sauvola;
pub use cleanup::despeckle;
pub use color::{estimate_ink_color, luma};
pub use extract::{
    Extraction, Options, alpha_from_gray_on_white, extract, looks_inverted,
};
pub use filters::{box_blur, grey_close, max_filter, min_filter, smooth};
pub use mask::Mask;
pub use metrics::{Confusion, Scores, drd, f_measure, pseudo_f_measure, psnr, score, skeleton};
pub use grid::Grid;
