//! macOS-specific platform support data and helpers.

pub mod cleanup_catalog;
#[cfg(target_os = "macos")]
pub mod libproc;
pub mod sacred_catalog;
