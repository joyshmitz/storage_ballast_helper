//! Linux kernel writeback (dirty-page) tuning primitives for the PAL.
//!
//! Reads live `/proc/sys/vm/dirty_*` knobs, resolves the backing block device
//! and its rotational class via `/sys/block`, and applies absolute byte limits
//! to the running kernel. Persistence across reboot (a `sysctl.d` snippet) is
//! handled by the CLI layer, not here.

#![allow(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::errors::{Result, SbhError};
use crate::platform::pal::{BlockDeviceInfo, MountPoint};
use crate::tuning::writeback::WritebackState;

use super::disk::find_mount;

const PROC_VM: &str = "/proc/sys/vm";

/// Snapshot the current kernel dirty-page configuration.
///
/// Individual knobs degrade to `None` when unreadable, so this never fails.
pub(super) fn read_state(total_ram_bytes: u64) -> WritebackState {
    WritebackState {
        dirty_ratio: read_u64_opt("dirty_ratio"),
        dirty_background_ratio: read_u64_opt("dirty_background_ratio"),
        dirty_bytes: read_u64_opt("dirty_bytes"),
        dirty_background_bytes: read_u64_opt("dirty_background_bytes"),
        dirty_expire_centisecs: read_u64_opt("dirty_expire_centisecs"),
        dirty_writeback_centisecs: read_u64_opt("dirty_writeback_centisecs"),
        total_ram_bytes,
    }
}

/// Apply writeback byte limits to the running kernel. Requires privilege; the
/// caller is responsible for verifying root and reporting a friendly error.
pub(super) fn apply_runtime(dirty_bytes: u64, dirty_background_bytes: u64) -> Result<()> {
    // Set the background threshold first, then the hard cap. Writing either
    // *_bytes knob auto-zeros the matching ratio knob in the kernel.
    write_vm("dirty_background_bytes", dirty_background_bytes)?;
    write_vm("dirty_bytes", dirty_bytes)?;
    Ok(())
}

/// Resolve the backing block device characteristics for a path.
pub(super) fn block_device_for(path: &Path, mounts: &[MountPoint]) -> Result<BlockDeviceInfo> {
    let mount = find_mount(path, mounts).ok_or_else(|| SbhError::FsStats {
        path: path.to_path_buf(),
        details: "could not map path to mount point".to_string(),
    })?;
    let device = resolve_device_name(&mount.device);
    let sys_block = Path::new("/sys/block").join(&device);
    let rotational = read_u64_at(&sys_block.join("queue/rotational")).map(|value| value != 0);
    let model = fs::read_to_string(sys_block.join("device/model"))
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|model| !model.is_empty());

    Ok(BlockDeviceInfo {
        device,
        source_device: mount.device.clone(),
        fs_type: mount.fs_type.clone(),
        rotational,
        model,
    })
}

fn read_u64_opt(name: &str) -> Option<u64> {
    read_u64_at(&PathBuf::from(PROC_VM).join(name))
}

fn read_u64_at(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse::<u64>().ok()
}

fn write_vm(name: &str, value: u64) -> Result<()> {
    let path = PathBuf::from(PROC_VM).join(name);
    fs::write(&path, value.to_string()).map_err(|source| SbhError::Io { path, source })
}

/// Resolve a mount's device string to a base block device name under `/sys/block`.
///
/// Handles symlinks (`/dev/mapper/*` -> `/dev/dm-N`), NVMe/mmc partition naming
/// (`nvme0n1p2` -> `nvme0n1`), and SCSI/virtio partitions (`sda1` -> `sda`).
/// Falls back to the raw basename when the disk cannot be resolved.
fn resolve_device_name(source_device: &str) -> String {
    let raw = fs::canonicalize(source_device)
        .ok()
        .and_then(|canon| {
            canon
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .or_else(|| {
            Path::new(source_device)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| source_device.to_string());
    base_block_device(&raw)
}

fn base_block_device(name: &str) -> String {
    if sys_block_exists(name) {
        return name.to_string();
    }

    // NVMe / mmcblk style: <disk><digit>p<partition>, e.g. nvme0n1p2, mmcblk0p1.
    if let Some(idx) = name.rfind('p') {
        let (head, tail) = name.split_at(idx);
        let partition = &tail[1..];
        if !partition.is_empty()
            && partition.bytes().all(|b| b.is_ascii_digit())
            && head.bytes().next_back().is_some_and(|b| b.is_ascii_digit())
            && sys_block_exists(head)
        {
            return head.to_string();
        }
    }

    // SCSI / virtio / IDE style: strip trailing partition digits, e.g. sda1 -> sda.
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if !trimmed.is_empty() && trimmed != name && sys_block_exists(trimmed) {
        return trimmed.to_string();
    }

    name.to_string()
}

fn sys_block_exists(name: &str) -> bool {
    !name.is_empty() && Path::new("/sys/block").join(name).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_block_device_falls_back_when_unresolvable() {
        // No /sys/block entry in the test environment; falls back to the raw name.
        assert_eq!(
            base_block_device("definitely-not-a-disk"),
            "definitely-not-a-disk"
        );
    }

    #[test]
    fn resolve_device_name_uses_basename_for_unknown_device() {
        assert_eq!(resolve_device_name("tmpfs"), "tmpfs");
    }
}
