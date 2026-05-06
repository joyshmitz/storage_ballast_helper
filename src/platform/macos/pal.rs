//! macOS PAL skeleton.

#![allow(missing_docs)]

use std::path::{Path, PathBuf};

use crate::core::errors::Result;
use crate::platform::pal::{
    FsStats, MemoryInfo, MountPoint, Platform, PlatformPaths, ServiceManager,
};
use crate::platform::types::{
    Capacity, MappedRegion, MemoryPressure, MemoryPressureCallback, MountInfo, OpenFile,
    ProcessInfo, ProcessIo, SacredPath, SelfStats, ServiceKind, SubscriptionHandle,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MacOsPal;

impl MacOsPal {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Platform for MacOsPal {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn fs_stats(&self, _path: &Path) -> Result<FsStats> {
        macos_placeholder("bd-zlxb.7", "fs_stats")
    }

    fn mount_points(&self) -> Result<Vec<MountPoint>> {
        macos_placeholder("bd-zlxb.7", "mount_points")
    }

    fn is_ram_backed(&self, _path: &Path) -> Result<bool> {
        macos_placeholder("bd-zlxb.7", "is_ram_backed")
    }

    fn default_paths(&self) -> PlatformPaths {
        macos_placeholder("bd-1y7j.4", "default_paths")
    }

    fn memory_info(&self) -> Result<MemoryInfo> {
        macos_placeholder("bd-hqu2.4", "memory_info")
    }

    fn service_manager(&self) -> Box<dyn ServiceManager> {
        macos_placeholder("bd-1y7j.1", "service_manager")
    }

    fn capacity(&self, _mount: &Path) -> Result<Capacity> {
        macos_placeholder("bd-zlxb.7", "capacity")
    }

    fn mounts(&self) -> Result<Vec<MountInfo>> {
        macos_placeholder("bd-zlxb.7", "mounts")
    }

    fn memory_pressure(&self) -> Result<MemoryPressure> {
        macos_placeholder("bd-hqu2.4", "memory_pressure")
    }

    fn subscribe_memory_pressure(
        &self,
        _callback: MemoryPressureCallback,
    ) -> Result<SubscriptionHandle> {
        macos_placeholder("bd-68ik.1", "subscribe_memory_pressure")
    }

    fn process_list(&self) -> Result<Vec<ProcessInfo>> {
        macos_placeholder("bd-ly4w.2", "process_list")
    }

    fn process_io(&self, _pid: i32) -> Result<ProcessIo> {
        macos_placeholder("bd-ly4w.3", "process_io")
    }

    fn open_files_under(&self, _path: &Path) -> Result<Vec<OpenFile>> {
        macos_placeholder("bd-ezkk.1", "open_files_under")
    }

    fn executables_under(&self, _path: &Path) -> Result<Vec<ProcessInfo>> {
        macos_placeholder("bd-ezkk.2", "executables_under")
    }

    fn mmap_regions_under(&self, _path: &Path) -> Result<Vec<MappedRegion>> {
        macos_placeholder("bd-ezkk.3", "mmap_regions_under")
    }

    fn self_stats(&self) -> Result<SelfStats> {
        macos_placeholder("bd-wiqg.2", "self_stats")
    }

    fn preallocate_file(&self, _path: &Path, _size: u64) -> Result<()> {
        macos_placeholder("bd-hnxg.1", "preallocate_file")
    }

    fn file_block_count(&self, _path: &Path) -> Result<u64> {
        macos_placeholder("bd-hnxg.1", "file_block_count")
    }

    fn user_home(&self) -> PathBuf {
        macos_placeholder("bd-1y7j.4", "user_home")
    }

    fn temp_dirs(&self) -> Vec<PathBuf> {
        macos_placeholder("bd-1y7j.4", "temp_dirs")
    }

    fn cache_roots(&self) -> Vec<PathBuf> {
        macos_placeholder("bd-1y7j.4", "cache_roots")
    }

    fn sacred_paths(&self) -> Vec<SacredPath> {
        macos_placeholder("bd-h13a.5", "sacred_paths")
    }

    fn service_kind(&self) -> ServiceKind {
        macos_placeholder("bd-1y7j.1", "service_kind")
    }
}

fn macos_placeholder<T>(bead: &'static str, method: &'static str) -> T {
    unimplemented!("{bead}: MacOsPal::{method}")
}

#[cfg(test)]
mod tests {
    use crate::platform::pal::Platform;

    use super::MacOsPal;

    fn assert_platform<T: Platform>(_platform: &T) {}

    #[test]
    fn macos_pal_skeleton_implements_platform() {
        let platform = MacOsPal::new();
        assert_platform(&platform);
        assert_eq!(platform.name(), "macos");
    }
}
