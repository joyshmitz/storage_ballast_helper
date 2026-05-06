//! Safe macOS filesystem syscall adapters.

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use nix::mount::MntFlags;
use nix::sys::statfs::{Statfs, statfs as nix_statfs};

const STATFS_STRUCT_SIZE_BYTES: usize = core::mem::size_of::<libc::statfs>();
const STATFS_MOUNT_NAME_BYTES: usize = core::mem::size_of::<[libc::c_char; 1024]>();
const STATFS_TYPE_NAME_BYTES: usize = core::mem::size_of::<[libc::c_char; 16]>();

const _: [(); 2168] = [(); STATFS_STRUCT_SIZE_BYTES];
const _: [(); 1024] = [(); STATFS_MOUNT_NAME_BYTES];
const _: [(); 16] = [(); STATFS_TYPE_NAME_BYTES];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatfsSnapshot {
    pub mount_point: PathBuf,
    pub device: String,
    pub fs_type: String,
    pub block_size: u64,
    pub blocks: u64,
    pub blocks_free: u64,
    pub blocks_available: u64,
    pub is_readonly: bool,
}

impl StatfsSnapshot {
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.blocks.saturating_mul(self.block_size)
    }

    #[must_use]
    pub fn free_bytes(&self) -> u64 {
        self.blocks_free.saturating_mul(self.block_size)
    }

    #[must_use]
    pub fn available_bytes(&self) -> u64 {
        self.blocks_available.saturating_mul(self.block_size)
    }

    #[must_use]
    pub fn is_ram_backed(&self) -> bool {
        matches!(
            self.fs_type.to_ascii_lowercase().as_str(),
            "devfs" | "mfs" | "ramfs" | "tmpfs"
        )
    }
}

pub fn statfs(path: &Path) -> io::Result<StatfsSnapshot> {
    let raw = nix_statfs(path).map_err(nix_error)?;
    let location = whichdisk::resolve(path).ok();
    let mount_point = location.as_ref().map_or_else(
        || path.to_path_buf(),
        |info| info.mount_point().to_path_buf(),
    );
    let device = location
        .as_ref()
        .map_or_else(String::new, |info| os_str_to_string(info.device()));

    Ok(snapshot_from_statfs(&raw, mount_point, device))
}

pub fn mounted_filesystems() -> io::Result<Vec<StatfsSnapshot>> {
    let mut filesystems = Vec::new();
    for mount in whichdisk::list().map_err(io::Error::other)? {
        match statfs_for_mount(mount.mount_point(), mount.device()) {
            Ok(snapshot) => filesystems.push(snapshot),
            Err(error) => eprintln!(
                "[sbh] warning: skipping macOS mount {}: {error}",
                mount.mount_point().display()
            ),
        }
    }
    filesystems.sort_by(|left, right| {
        right
            .mount_point
            .as_os_str()
            .len()
            .cmp(&left.mount_point.as_os_str().len())
            .then_with(|| left.mount_point.cmp(&right.mount_point))
    });
    Ok(filesystems)
}

fn statfs_for_mount(mount_point: &Path, device: &OsStr) -> io::Result<StatfsSnapshot> {
    let raw = nix_statfs(mount_point).map_err(nix_error)?;
    Ok(snapshot_from_statfs(
        &raw,
        mount_point.to_path_buf(),
        os_str_to_string(device),
    ))
}

fn snapshot_from_statfs(raw: &Statfs, mount_point: PathBuf, device: String) -> StatfsSnapshot {
    StatfsSnapshot {
        mount_point,
        device,
        fs_type: raw.filesystem_type_name().to_string(),
        block_size: u64::from(raw.block_size()),
        blocks: raw.blocks(),
        blocks_free: raw.blocks_free(),
        blocks_available: raw.blocks_available(),
        is_readonly: raw.flags().contains(MntFlags::MNT_RDONLY),
    }
}

fn os_str_to_string(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

fn nix_error(error: nix::errno::Errno) -> io::Error {
    io::Error::from_raw_os_error(error as i32)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{mounted_filesystems, statfs};

    #[test]
    fn statfs_tmp_reports_plausible_values() {
        let stats = statfs(Path::new("/tmp")).expect("/tmp statfs should work on macOS");
        assert_eq!(stats.block_size, 4096);
        assert!(stats.blocks > 0);
        assert!(stats.blocks_available > 0);
        assert!(stats.total_bytes() > stats.available_bytes());
        assert!(!stats.fs_type.is_empty());
        assert!(stats.mount_point.is_absolute());
    }

    #[test]
    fn mounted_filesystems_include_root() {
        let mounts = mounted_filesystems().expect("mounted filesystems should be discoverable");
        assert!(!mounts.is_empty());
        assert!(
            mounts
                .iter()
                .any(|mount| mount.mount_point == Path::new("/"))
        );
    }
}
