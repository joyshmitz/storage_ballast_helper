//! Linux PAL implementation.

#![cfg(target_os = "linux")]
#![allow(missing_docs)]

use std::path::Path;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use crate::core::errors::{Result, SbhError};
use crate::platform::pal::{
    FsStats, MemoryInfo, MountPoint, Platform, PlatformPaths, ServiceManager,
};
use crate::platform::types::ServiceKind;

pub mod disk;
pub mod memory;
pub mod process;
pub mod service;

/// Linux platform implementation using `/proc` + `statvfs`.
#[derive(Debug)]
pub struct LinuxPal {
    mounts_cache: RwLock<Option<(Vec<MountPoint>, Instant)>>,
    cache_ttl: Duration,
}

impl Default for LinuxPal {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxPal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            mounts_cache: RwLock::new(None),
            cache_ttl: Duration::from_secs(5),
        }
    }

    fn get_cached_mounts(&self) -> Result<Vec<MountPoint>> {
        {
            let cache = self.mounts_cache.read();
            if let Some((mounts, collected_at)) = &*cache
                && collected_at.elapsed() < self.cache_ttl
            {
                return Ok(mounts.clone());
            }
        }

        let mounts = disk::read_mount_points()?;
        *self.mounts_cache.write() = Some((mounts.clone(), Instant::now()));
        Ok(mounts)
    }
}

impl Platform for LinuxPal {
    fn name(&self) -> &'static str {
        "linux"
    }

    fn fs_stats(&self, path: &Path) -> Result<FsStats> {
        let mounts = self.mount_points()?;
        let mount = disk::find_mount(path, &mounts).ok_or_else(|| SbhError::FsStats {
            path: path.to_path_buf(),
            details: "could not map path to mount point".to_string(),
        })?;
        let stat = nix::sys::statvfs::statvfs(path).map_err(|error| SbhError::FsStats {
            path: path.to_path_buf(),
            details: error.to_string(),
        })?;
        let fragment = stat.fragment_size() as u64;
        Ok(FsStats {
            total_bytes: stat.blocks().saturating_mul(fragment),
            free_bytes: stat.blocks_free().saturating_mul(fragment),
            available_bytes: stat.blocks_available().saturating_mul(fragment),
            fs_type: mount.fs_type.clone(),
            mount_point: mount.path.clone(),
            is_readonly: stat.flags().contains(nix::sys::statvfs::FsFlags::ST_RDONLY),
        })
    }

    fn mount_points(&self) -> Result<Vec<MountPoint>> {
        self.get_cached_mounts()
    }

    fn is_ram_backed(&self, path: &Path) -> Result<bool> {
        let mounts = self.mount_points()?;
        let Some(mount) = disk::find_mount(path, &mounts) else {
            return Ok(false);
        };
        Ok(mount.is_ram_backed)
    }

    fn default_paths(&self) -> PlatformPaths {
        PlatformPaths::default()
    }

    fn memory_info(&self) -> Result<MemoryInfo> {
        memory::read_memory_info()
    }

    fn service_manager(&self) -> Box<dyn ServiceManager> {
        service::service_manager()
    }

    fn service_kind(&self) -> ServiceKind {
        ServiceKind::Systemd
    }
}
