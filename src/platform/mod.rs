//! Platform Abstraction Layer (PAL) — trait-based cross-platform support.

#[cfg(target_os = "linux")]
pub mod linux;
pub mod macos;
pub mod pal;
pub mod types;

use pal::Platform;

#[cfg(target_os = "linux")]
/// Return the compile-time-selected platform implementation.
#[must_use]
pub fn current() -> impl Platform {
    linux::LinuxPal::new()
}

#[cfg(target_os = "macos")]
/// Return the compile-time-selected platform implementation.
#[must_use]
pub fn current() -> impl Platform {
    macos::MacOsPal::new()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("sbh requires Linux or macOS");

#[cfg(test)]
mod tests {
    use super::{Platform, current};

    #[test]
    fn current_platform_matches_compile_target() {
        let platform = current();

        #[cfg(target_os = "linux")]
        assert_eq!(platform.name(), "linux");
        #[cfg(target_os = "macos")]
        assert_eq!(platform.name(), "macos");
    }
}
