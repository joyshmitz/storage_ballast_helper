//! Platform Abstraction Layer (PAL) — trait-based cross-platform support.

#[cfg(target_os = "linux")]
pub mod linux;
pub mod macos;
pub mod pal;
pub mod types;
