//! macOS-specific platform support data and helpers.

pub mod cleanup_catalog;
#[cfg(target_os = "macos")]
pub mod libproc;
pub mod pal;
pub mod sacred_catalog;

pub use pal::MacOsPal;
