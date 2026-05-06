//! PAL trait and platform-specific implementations (Linux, macOS, Windows).

#![allow(missing_docs)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::config::PathsConfig;
use crate::core::errors::{Result, SbhError};
use crate::platform::types::{
    Capacity, MappedRegion, MemoryPressure, MemoryPressureCallback, MountInfo, OpenFile, PalError,
    ProcessInfo, ProcessIo, SacredPath, SelfStats, ServiceKind, SubscriptionHandle,
};

/// Filesystem statistics for a path/mount.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsStats {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub fs_type: String,
    pub mount_point: PathBuf,
    pub is_readonly: bool,
}

impl FsStats {
    #[must_use]
    pub fn free_pct(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            (self.available_bytes as f64 * 100.0) / self.total_bytes as f64
        }
    }
}

/// Mount-point metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MountPoint {
    pub path: PathBuf,
    pub device: String,
    pub fs_type: String,
    pub is_ram_backed: bool,
}

/// Current system memory info.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
}

/// Platform-specific data/service directories.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformPaths {
    pub ballast_dir: PathBuf,
    pub state_file: PathBuf,
    pub sqlite_db: PathBuf,
    pub jsonl_log: PathBuf,
}

impl From<FsStats> for Capacity {
    fn from(value: FsStats) -> Self {
        Self {
            mount_point: value.mount_point,
            fs_type: value.fs_type,
            total_bytes: value.total_bytes,
            free_bytes: value.free_bytes,
            available_bytes: value.available_bytes,
            is_readonly: value.is_readonly,
            container_id: None,
            container_total_bytes: None,
            container_available_bytes: None,
            volume_total_bytes: Some(value.total_bytes),
            volume_available_bytes: Some(value.available_bytes),
            purgeable_bytes: None,
            local_snapshot_bytes: None,
        }
    }
}

impl From<MountPoint> for MountInfo {
    fn from(value: MountPoint) -> Self {
        Self {
            device: value.device,
            mount_point: value.path,
            fs_type: value.fs_type,
            container_id: None,
            total_bytes: None,
            available_bytes: None,
            purgeable_bytes: None,
            local_snapshot_bytes: None,
            is_readonly: false,
            is_ram_backed: value.is_ram_backed,
            is_apfs_data_volume: false,
            is_apfs_system_snapshot: false,
            is_apfs_vm_volume: false,
        }
    }
}

impl Default for PlatformPaths {
    fn default() -> Self {
        let defaults = PathsConfig::default();
        Self {
            ballast_dir: defaults.ballast_dir,
            state_file: defaults.state_file,
            sqlite_db: defaults.sqlite_db,
            jsonl_log: defaults.jsonl_log,
        }
    }
}

fn pal_not_implemented<T>(os_name: &'static str, method_name: &'static str) -> Result<T> {
    Err(PalError::not_implemented(os_name, method_name).into())
}

/// Service control surface (systemd, launchd, etc.).
pub trait ServiceManager: Send + Sync {
    fn install(&self) -> Result<()>;
    fn uninstall(&self) -> Result<()>;
    fn status(&self) -> Result<String>;
}

/// OS abstraction used by monitoring and daemon orchestration.
pub trait Platform: Send + Sync {
    fn name(&self) -> &'static str {
        "unknown"
    }

    fn fs_stats(&self, path: &Path) -> Result<FsStats>;
    fn mount_points(&self) -> Result<Vec<MountPoint>>;
    fn is_ram_backed(&self, path: &Path) -> Result<bool>;
    fn default_paths(&self) -> PlatformPaths;
    fn memory_info(&self) -> Result<MemoryInfo>;
    fn service_manager(&self) -> Box<dyn ServiceManager>;

    fn capacity(&self, mount: &Path) -> Result<Capacity> {
        self.fs_stats(mount).map(Into::into)
    }

    fn mounts(&self) -> Result<Vec<MountInfo>> {
        self.mount_points()
            .map(|mounts| mounts.into_iter().map(Into::into).collect())
    }

    fn memory_pressure(&self) -> Result<MemoryPressure> {
        pal_not_implemented(self.name(), "memory_pressure")
    }

    fn subscribe_memory_pressure(
        &self,
        _callback: MemoryPressureCallback,
    ) -> Result<SubscriptionHandle> {
        pal_not_implemented(self.name(), "subscribe_memory_pressure")
    }

    fn process_list(&self) -> Result<Vec<ProcessInfo>> {
        pal_not_implemented(self.name(), "process_list")
    }

    fn process_io(&self, _pid: i32) -> Result<ProcessIo> {
        pal_not_implemented(self.name(), "process_io")
    }

    fn open_files_under(&self, _path: &Path) -> Result<Vec<OpenFile>> {
        pal_not_implemented(self.name(), "open_files_under")
    }

    fn executables_under(&self, _path: &Path) -> Result<Vec<ProcessInfo>> {
        pal_not_implemented(self.name(), "executables_under")
    }

    fn mmap_regions_under(&self, _path: &Path) -> Result<Vec<MappedRegion>> {
        pal_not_implemented(self.name(), "mmap_regions_under")
    }

    fn self_stats(&self) -> Result<SelfStats> {
        pal_not_implemented(self.name(), "self_stats")
    }

    fn preallocate_file(&self, _path: &Path, _size: u64) -> Result<()> {
        pal_not_implemented(self.name(), "preallocate_file")
    }

    fn file_block_count(&self, path: &Path) -> Result<u64> {
        let meta = fs::metadata(path).map_err(|source| SbhError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(meta.blocks())
        }
        #[cfg(not(unix))]
        {
            Ok(meta.len().saturating_add(511) / 512)
        }
    }

    fn user_home(&self) -> PathBuf {
        std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
    }

    fn temp_dirs(&self) -> Vec<PathBuf> {
        vec![std::env::temp_dir()]
    }

    fn cache_roots(&self) -> Vec<PathBuf> {
        std::env::var_os("HOME")
            .map_or_else(Vec::new, |home| vec![PathBuf::from(home).join(".cache")])
    }

    fn sacred_paths(&self) -> Vec<SacredPath> {
        Vec::new()
    }

    fn service_kind(&self) -> ServiceKind {
        ServiceKind::None
    }
}

/// No-op service manager for early development and tests.
#[derive(Debug, Default)]
pub struct NoopServiceManager;

impl ServiceManager for NoopServiceManager {
    fn install(&self) -> Result<()> {
        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        Ok(())
    }

    fn status(&self) -> Result<String> {
        Ok("unknown".to_string())
    }
}

/// In-memory mock implementation for deterministic tests.
#[derive(Debug, Clone)]
pub struct MockPlatform {
    mounts: Vec<MountPoint>,
    stats_by_mount: HashMap<PathBuf, FsStats>,
    memory: MemoryInfo,
    paths: PlatformPaths,
}

impl MockPlatform {
    #[must_use]
    pub fn new(
        mounts: Vec<MountPoint>,
        stats_by_mount: HashMap<PathBuf, FsStats>,
        memory: MemoryInfo,
        paths: PlatformPaths,
    ) -> Self {
        Self {
            mounts,
            stats_by_mount,
            memory,
            paths,
        }
    }
}

impl Platform for MockPlatform {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn fs_stats(&self, path: &Path) -> Result<FsStats> {
        let mount = find_mount(path, &self.mounts).ok_or_else(|| SbhError::FsStats {
            path: path.to_path_buf(),
            details: "mock mount not found".to_string(),
        })?;
        self.stats_by_mount
            .get(&mount.path)
            .cloned()
            .ok_or_else(|| SbhError::FsStats {
                path: mount.path.clone(),
                details: "mock stats not found".to_string(),
            })
    }

    fn mount_points(&self) -> Result<Vec<MountPoint>> {
        Ok(self.mounts.clone())
    }

    fn is_ram_backed(&self, path: &Path) -> Result<bool> {
        Ok(find_mount(path, &self.mounts).is_some_and(|mount| mount.is_ram_backed))
    }

    fn default_paths(&self) -> PlatformPaths {
        self.paths.clone()
    }

    fn memory_info(&self) -> Result<MemoryInfo> {
        Ok(self.memory.clone())
    }

    fn service_manager(&self) -> Box<dyn ServiceManager> {
        Box::<NoopServiceManager>::default()
    }
}

/// Detect active platform implementation.
pub fn detect_platform() -> Result<Arc<dyn Platform>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Arc::new(crate::platform::linux::LinuxPal::new()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(SbhError::UnsupportedPlatform {
            details: "only Linux is currently implemented".to_string(),
        })
    }
}

fn find_mount<'a>(path: &Path, mounts: &'a [MountPoint]) -> Option<&'a MountPoint> {
    mounts
        .iter()
        .filter(|mount| path.starts_with(&mount.path))
        .max_by_key(|mount| mount.path.as_os_str().len())
}
