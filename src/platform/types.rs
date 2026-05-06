//! Cross-platform PAL data contracts.

#![allow(missing_docs)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq, Eq)]
pub enum PalError {
    #[error("PAL method {method_name} is not implemented on {os_name}")]
    NotImplemented {
        os_name: String,
        method_name: String,
    },
    #[error("PAL method {method_name} failed on {os_name}: {details}")]
    MethodFailed {
        os_name: String,
        method_name: String,
        details: String,
    },
}

impl PalError {
    #[must_use]
    pub fn not_implemented(os_name: impl Into<String>, method_name: impl Into<String>) -> Self {
        Self::NotImplemented {
            os_name: os_name.into(),
            method_name: method_name.into(),
        }
    }

    #[must_use]
    pub fn method_failed(
        os_name: impl Into<String>,
        method_name: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::MethodFailed {
            os_name: os_name.into(),
            method_name: method_name.into(),
            details: details.into(),
        }
    }

    #[must_use]
    pub fn os_name(&self) -> &str {
        match self {
            Self::NotImplemented { os_name, .. } | Self::MethodFailed { os_name, .. } => os_name,
        }
    }

    #[must_use]
    pub fn method_name(&self) -> &str {
        match self {
            Self::NotImplemented { method_name, .. } | Self::MethodFailed { method_name, .. } => {
                method_name
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capacity {
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub is_readonly: bool,
    pub container_id: Option<String>,
    pub container_total_bytes: Option<u64>,
    pub container_available_bytes: Option<u64>,
    pub volume_total_bytes: Option<u64>,
    pub volume_available_bytes: Option<u64>,
    pub purgeable_bytes: Option<u64>,
    pub local_snapshot_bytes: Option<u64>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MountInfo {
    pub device: String,
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub container_id: Option<String>,
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub purgeable_bytes: Option<u64>,
    pub local_snapshot_bytes: Option<u64>,
    pub is_readonly: bool,
    pub is_ram_backed: bool,
    pub is_apfs_data_volume: bool,
    pub is_apfs_system_snapshot: bool,
    pub is_apfs_vm_volume: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryPressureLevel {
    Normal,
    Warn,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPressure {
    pub level: MemoryPressureLevel,
    pub free_pages: Option<u64>,
    pub used_pages: Option<u64>,
    pub page_size_bytes: Option<u64>,
    pub compressor_used_bytes: Option<u64>,
    pub swap_total_bytes: Option<u64>,
    pub swap_used_bytes: Option<u64>,
    pub linux_psi_avg10: Option<u64>,
}

pub type MemoryPressureCallback = Box<dyn Fn(MemoryPressure) + Send + Sync + 'static>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubscriptionHandle {
    pub source: String,
    pub active: bool,
}

impl SubscriptionHandle {
    #[must_use]
    pub fn inactive(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            active: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: i32,
    pub parent_pid: Option<i32>,
    pub name: String,
    pub command_line: Vec<String>,
    pub executable: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub start_time_unix_ms: Option<i64>,
    pub virtual_memory_bytes: Option<u64>,
    pub resident_memory_bytes: Option<u64>,
    pub cpu_user_micros: Option<u64>,
    pub cpu_system_micros: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessIo {
    pub pid: i32,
    pub bytes_read_total: u64,
    pub bytes_written_total: u64,
    pub bytes_read_recent_15m: Option<u64>,
    pub bytes_written_recent_15m: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OpenFileKind {
    Regular,
    Directory,
    Socket,
    Pipe,
    Device,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OpenFileMode {
    Read,
    Write,
    ReadWrite,
    Execute,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenFile {
    pub pid: i32,
    pub path: PathBuf,
    pub fd: Option<i32>,
    pub kind: OpenFileKind,
    pub mode: OpenFileMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MappedRegion {
    pub pid: i32,
    pub path: PathBuf,
    pub start_address: Option<u64>,
    pub end_address: Option<u64>,
    pub protection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelfStats {
    pub rss_bytes: u64,
    pub virtual_memory_bytes: u64,
    pub cpu_user_micros: u64,
    pub cpu_system_micros: u64,
    pub idle_wakeups: Option<u64>,
    pub bytes_read: Option<u64>,
    pub bytes_written: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SacredPathKind {
    ExactMatch,
    GlobMatch,
    ContainsAny,
    StowawayMarker,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SacredPathSource {
    Builtin,
    UserConfig,
    Marker,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SacredPath {
    pub pattern: String,
    pub kind: SacredPathKind,
    pub reason: String,
    pub source: SacredPathSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServiceKind {
    Systemd,
    Launchd,
    None,
}
