//! macOS PAL skeleton.

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::core::errors::Result;
use crate::core::paths::resolve_absolute_path;
use crate::platform::macos::libproc::{
    ProcFdInfo, ProcFdType, ProcRegionWithPathInfo, ProcTaskAllInfo, VnodeFdInfoWithPath,
    proc_listpids_safe, proc_pid_list_fds, proc_pid_region_path, proc_pid_rusage_v4_safe,
    proc_pidfdinfo_vnode_path, proc_pidinfo_task_all, proc_pidpath_safe,
};
use crate::platform::macos::sacred_catalog::platform_macos_sacred_paths;
use crate::platform::macos::sys::{
    self, ApfsInventory, ApfsVolume, ApfsVolumeRole, StatfsSnapshot,
};
use crate::platform::pal::{
    FsStats, MemoryInfo, MountPoint, Platform, PlatformPaths, ServiceManager,
};
use crate::platform::types::{
    Capacity, FullDiskAccessState, FullDiskAccessStatus, MappedRegion, MemoryPressure,
    MemoryPressureCallback, MountInfo, OpenFile, OpenFileKind, OpenFileMode, PalError, ProcessInfo,
    ProcessIo, SacredPath, SelfStats, ServiceKind, SubscriptionHandle,
};
use parking_lot::RwLock;

const FDA_CACHE_TTL_SECS: u64 = 60;
const FDA_CACHE_TTL: Duration = Duration::from_secs(FDA_CACHE_TTL_SECS);
static FDA_STATUS_CACHE: OnceLock<RwLock<Option<(Instant, FullDiskAccessStatus)>>> =
    OnceLock::new();

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

    fn fs_stats(&self, path: &Path) -> Result<FsStats> {
        let stats = sys::statfs(path).map_err(|error| macos_method_error("fs_stats", &error))?;
        let inventory = sys::apfs_inventory().ok();
        Ok(capacity_to_fs_stats(statfs_to_capacity(
            stats,
            inventory.as_ref(),
        )))
    }

    fn mount_points(&self) -> Result<Vec<MountPoint>> {
        sys::mounted_filesystems()
            .map(|mounts| mounts.into_iter().map(statfs_to_mount_point).collect())
            .map_err(|error| macos_method_error("mount_points", &error))
    }

    fn is_ram_backed(&self, path: &Path) -> Result<bool> {
        sys::statfs(path)
            .map(|stats| stats.is_ram_backed())
            .map_err(|error| macos_method_error("is_ram_backed", &error))
    }

    fn default_paths(&self) -> PlatformPaths {
        PlatformPaths::default()
    }

    fn memory_info(&self) -> Result<MemoryInfo> {
        macos_not_implemented("bd-hqu2.4", "memory_info")
    }

    fn service_manager(&self) -> Box<dyn ServiceManager> {
        crate::daemon::service::service_manager_for_kind(ServiceKind::Launchd, false)
    }

    fn capacity(&self, mount: &Path) -> Result<Capacity> {
        let stats = sys::statfs(mount).map_err(|error| macos_method_error("capacity", &error))?;
        let inventory = sys::apfs_inventory().ok();
        let local_snapshot_bytes = local_snapshot_bytes_for_capacity(&stats, inventory.as_ref());
        let mut capacity = statfs_to_capacity(stats, inventory.as_ref());
        capacity.purgeable_bytes = purgeable_bytes_for_volume(
            &capacity.mount_point,
            capacity.available_bytes,
            inventory.as_ref(),
            capacity.container_id.as_deref(),
        );
        capacity.local_snapshot_bytes = local_snapshot_bytes;
        Ok(capacity)
    }

    fn mounts(&self) -> Result<Vec<MountInfo>> {
        let inventory = sys::apfs_inventory().ok();
        sys::mounted_filesystems()
            .map(|mounts| {
                mounts
                    .into_iter()
                    .map(|stats| {
                        let local_snapshot_bytes =
                            local_snapshot_bytes_for_capacity(&stats, inventory.as_ref());
                        statfs_to_mount_info(stats, inventory.as_ref(), local_snapshot_bytes)
                    })
                    .collect()
            })
            .map_err(|error| macos_method_error("mounts", &error))
    }

    fn memory_pressure(&self) -> Result<MemoryPressure> {
        macos_not_implemented("bd-hqu2.4", "memory_pressure")
    }

    fn full_disk_access_status(&self) -> Result<FullDiskAccessStatus> {
        Ok(cached_full_disk_access_status(&self.user_home()))
    }

    fn subscribe_memory_pressure(
        &self,
        _callback: MemoryPressureCallback,
    ) -> Result<SubscriptionHandle> {
        macos_not_implemented("bd-68ik.1", "subscribe_memory_pressure")
    }

    fn process_list(&self) -> Result<Vec<ProcessInfo>> {
        let current_pid = current_process_pid();
        let processes = proc_listpids_safe()
            .map_err(|error| macos_method_error("process_list", &error))?
            .into_iter()
            .filter(|pid| *pid > 0 && *pid != current_pid)
            .filter_map(process_info_for_pid)
            .collect();
        Ok(processes)
    }

    fn process_io(&self, pid: i32) -> Result<ProcessIo> {
        let usage = proc_pid_rusage_v4_safe(pid)
            .map_err(|error| macos_method_error("process_io", &error))?;
        Ok(ProcessIo {
            pid,
            bytes_read_total: usage.ri_diskio_bytesread,
            bytes_written_total: usage.ri_diskio_byteswritten,
            bytes_read_recent_15m: None,
            bytes_written_recent_15m: None,
        })
    }

    fn open_files_under(&self, path: &Path) -> Result<Vec<OpenFile>> {
        let root = resolve_absolute_path(path);
        let mut open_files: Vec<OpenFile> = proc_listpids_safe()
            .map_err(|error| macos_method_error("open_files_under", &error))?
            .into_iter()
            .filter(|pid| *pid > 0)
            .flat_map(|pid| open_files_for_pid_under(pid, &root))
            .collect();
        open_files.sort_by(|left, right| {
            left.pid
                .cmp(&right.pid)
                .then_with(|| left.fd.cmp(&right.fd))
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(open_files)
    }

    fn executables_under(&self, path: &Path) -> Result<Vec<ProcessInfo>> {
        let root = resolve_absolute_path(path);
        let mut processes: Vec<ProcessInfo> = proc_listpids_safe()
            .map_err(|error| macos_method_error("executables_under", &error))?
            .into_iter()
            .filter(|pid| *pid > 0)
            .filter_map(process_info_for_pid)
            .filter(|process| executable_is_under(process, &root))
            .collect();
        processes.sort_by(|left, right| {
            left.pid
                .cmp(&right.pid)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(processes)
    }

    fn mmap_regions_under(&self, path: &Path) -> Result<Vec<MappedRegion>> {
        let root = resolve_absolute_path(path);
        let mut regions: Vec<MappedRegion> = proc_listpids_safe()
            .map_err(|error| macos_method_error("mmap_regions_under", &error))?
            .into_iter()
            .filter(|pid| *pid > 0)
            .flat_map(|pid| mapped_regions_for_pid_under(pid, &root))
            .collect();
        regions.sort_by(|left, right| {
            left.pid
                .cmp(&right.pid)
                .then_with(|| left.start_address.cmp(&right.start_address))
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(regions)
    }

    fn self_stats(&self) -> Result<SelfStats> {
        macos_not_implemented("bd-wiqg.2", "self_stats")
    }

    fn preallocate_file(&self, _path: &Path, _size: u64) -> Result<()> {
        macos_not_implemented("bd-hnxg.1", "preallocate_file")
    }

    fn user_home(&self) -> PathBuf {
        std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
    }

    fn temp_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![std::env::temp_dir(), PathBuf::from("/private/tmp")];
        dirs.sort();
        dirs.dedup();
        dirs
    }

    fn cache_roots(&self) -> Vec<PathBuf> {
        std::env::var_os("HOME").map_or_else(Vec::new, |home| {
            vec![PathBuf::from(home).join("Library/Caches")]
        })
    }

    fn sacred_paths(&self) -> Vec<SacredPath> {
        platform_macos_sacred_paths()
    }

    fn service_kind(&self) -> ServiceKind {
        ServiceKind::Launchd
    }
}

fn macos_not_implemented<T>(bead: &'static str, method: &'static str) -> Result<T> {
    Err(PalError::not_implemented_with_bead("macos", method, Some(bead)).into())
}

fn macos_method_error(
    method: &'static str,
    error: &impl ToString,
) -> crate::core::errors::SbhError {
    PalError::method_failed("macos", method, error.to_string()).into()
}

fn cached_full_disk_access_status(home: &Path) -> FullDiskAccessStatus {
    let cache = FDA_STATUS_CACHE.get_or_init(|| RwLock::new(None));
    {
        let cached = cache.read();
        if let Some((checked_at, status)) = &*cached
            && checked_at.elapsed() < FDA_CACHE_TTL
        {
            let mut status = status.clone();
            status.cached = true;
            return status;
        }
    }

    let status = probe_full_disk_access_status(home);
    *cache.write() = Some((Instant::now(), status.clone()));
    status
}

fn probe_full_disk_access_status(home: &Path) -> FullDiskAccessStatus {
    let mail_root = home.join("Library/Mail");
    let entries = match fs::read_dir(&mail_root) {
        Ok(entries) => entries,
        Err(error) if is_fda_permission_denied(&error) => {
            return fda_status(
                FullDiskAccessState::Missing,
                Some(mail_root),
                "permission denied while listing Mail data; grant Full Disk Access to sbh",
            );
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return fda_status(
                FullDiskAccessState::NotConfigured,
                Some(mail_root),
                "Mail data directory is not present for this user",
            );
        }
        Err(error) => {
            return fda_status(
                FullDiskAccessState::Unknown,
                Some(mail_root),
                format!("could not inspect Mail data directory: {error}"),
            );
        }
    };

    let mut version_dirs = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if is_fda_permission_denied(&error) => {
                return fda_status(
                    FullDiskAccessState::Missing,
                    Some(mail_root),
                    "permission denied while reading Mail data; grant Full Disk Access to sbh",
                );
            }
            Err(error) => {
                return fda_status(
                    FullDiskAccessState::Unknown,
                    Some(mail_root),
                    format!("could not read Mail data entry: {error}"),
                );
            }
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('V') {
            version_dirs.push(entry.path());
        }
    }
    version_dirs.sort();

    let mut last_probe = None;
    for version_dir in version_dirs {
        let probe = version_dir.join("MailData").join("Envelope Index");
        last_probe = Some(probe.clone());
        match File::open(&probe) {
            Ok(_) => {
                return fda_status(
                    FullDiskAccessState::Granted,
                    Some(probe),
                    "Mail Envelope Index was readable",
                );
            }
            Err(error) if is_fda_permission_denied(&error) => {
                return fda_status(
                    FullDiskAccessState::Missing,
                    Some(probe),
                    "permission denied while reading Mail Envelope Index; grant Full Disk Access to sbh",
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return fda_status(
                    FullDiskAccessState::Unknown,
                    Some(probe),
                    format!("could not read Mail Envelope Index: {error}"),
                );
            }
        }
    }

    fda_status(
        FullDiskAccessState::NotConfigured,
        Some(last_probe.unwrap_or(mail_root)),
        "no MailData/Envelope Index probe file was found under ~/Library/Mail/V*",
    )
}

fn fda_status(
    state: FullDiskAccessState,
    probe_path: Option<PathBuf>,
    detail: impl Into<String>,
) -> FullDiskAccessStatus {
    FullDiskAccessStatus {
        state,
        probe_path,
        detail: detail.into(),
        cache_ttl_seconds: FDA_CACHE_TTL_SECS,
        cached: false,
    }
}

fn is_fda_permission_denied(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::EPERM) || error.kind() == io::ErrorKind::PermissionDenied
}

fn capacity_to_fs_stats(capacity: Capacity) -> FsStats {
    FsStats {
        total_bytes: capacity.total_bytes,
        free_bytes: capacity.free_bytes,
        available_bytes: capacity.available_bytes,
        fs_type: capacity.fs_type,
        mount_point: capacity.mount_point,
        is_readonly: capacity.is_readonly,
    }
}

fn statfs_to_mount_point(stats: StatfsSnapshot) -> MountPoint {
    let is_ram_backed = stats.is_ram_backed();
    MountPoint {
        path: stats.mount_point,
        device: stats.device,
        fs_type: stats.fs_type,
        is_ram_backed,
    }
}

fn statfs_to_capacity(stats: StatfsSnapshot, inventory: Option<&ApfsInventory>) -> Capacity {
    let volume_total_bytes = stats.total_bytes();
    let volume_free_bytes = stats.free_bytes();
    let volume_available_bytes = stats.available_bytes();
    let volume = inventory.and_then(|inventory| inventory.volume_for_device(&stats.device));
    let container_total_bytes = volume.and_then(|volume| volume.container_total_bytes);
    let container_available_bytes = volume.and_then(|volume| volume.container_available_bytes);
    let effective_total_bytes = container_total_bytes.unwrap_or(volume_total_bytes);
    let effective_available_bytes = container_available_bytes.unwrap_or(volume_available_bytes);
    let is_primary = volume.is_some_and(|volume| volume.has_role(&ApfsVolumeRole::Data))
        || (stats.fs_type.eq_ignore_ascii_case("apfs")
            && stats.mount_point == Path::new("/System/Volumes/Data"));
    Capacity {
        mount_point: stats.mount_point,
        fs_type: stats.fs_type,
        total_bytes: effective_total_bytes,
        free_bytes: container_available_bytes.unwrap_or(volume_free_bytes),
        available_bytes: effective_available_bytes,
        is_readonly: stats.is_readonly,
        container_id: volume.map(|volume| volume.container_id.clone()),
        container_total_bytes,
        container_available_bytes,
        volume_total_bytes: Some(volume_total_bytes),
        volume_available_bytes: Some(volume_available_bytes),
        volume_role: volume.and_then(ApfsVolume::role_label),
        shared_volumes: inventory
            .zip(volume)
            .map_or_else(Vec::new, |(inventory, volume)| {
                inventory.sibling_volume_names(volume)
            }),
        is_primary,
        purgeable_bytes: None,
        local_snapshot_bytes: None,
    }
}

fn local_snapshot_bytes_for_capacity(
    stats: &StatfsSnapshot,
    inventory: Option<&ApfsInventory>,
) -> Option<u64> {
    let volume = inventory.and_then(|inventory| inventory.volume_for_device(&stats.device));
    let snapshots =
        sys::local_time_machine_snapshots(&stats.mount_point, inventory, volume).ok()?;
    let total = snapshots
        .iter()
        .filter_map(|snapshot| snapshot.retained_bytes_estimate)
        .fold(0_u64, u64::saturating_add);
    (total > 0).then_some(total)
}

fn statfs_to_mount_info(
    stats: StatfsSnapshot,
    inventory: Option<&ApfsInventory>,
    local_snapshot_bytes: Option<u64>,
) -> MountInfo {
    let is_apfs = stats.fs_type.eq_ignore_ascii_case("apfs");
    let total_bytes = stats.total_bytes();
    let available_bytes = stats.available_bytes();
    let is_ram_backed = stats.is_ram_backed();
    let volume = inventory.and_then(|inventory| inventory.volume_for_device(&stats.device));
    let is_apfs_data_volume = volume.is_some_and(|volume| volume.has_role(&ApfsVolumeRole::Data))
        || (is_apfs && stats.mount_point == Path::new("/System/Volumes/Data"));
    let is_apfs_system_snapshot = volume
        .is_some_and(|volume| volume.has_role(&ApfsVolumeRole::System))
        || (is_apfs && stats.mount_point == Path::new("/") && stats.is_readonly);
    let is_apfs_vm_volume = volume.is_some_and(|volume| volume.has_role(&ApfsVolumeRole::Vm))
        || (is_apfs && stats.mount_point == Path::new("/System/Volumes/VM"));
    let effective_available_bytes = volume
        .and_then(|volume| volume.container_available_bytes)
        .unwrap_or(available_bytes);
    let purgeable_bytes = purgeable_bytes_for_volume(
        &stats.mount_point,
        effective_available_bytes,
        inventory,
        volume.map(|volume| volume.container_id.as_str()),
    );
    let mount_point = stats.mount_point;
    MountInfo {
        device: stats.device,
        mount_point,
        fs_type: stats.fs_type,
        container_id: volume.map(|volume| volume.container_id.clone()),
        total_bytes: volume
            .and_then(|volume| volume.container_total_bytes)
            .or(Some(total_bytes)),
        available_bytes: Some(effective_available_bytes),
        purgeable_bytes,
        local_snapshot_bytes,
        is_readonly: stats.is_readonly,
        is_ram_backed,
        is_apfs_data_volume,
        is_apfs_system_snapshot,
        is_apfs_vm_volume,
    }
}

fn purgeable_bytes_for_volume(
    mount_point: &Path,
    counted_available_bytes: u64,
    inventory: Option<&ApfsInventory>,
    container_id: Option<&str>,
) -> Option<u64> {
    let foundation_estimate = sys::important_usage_available_bytes(mount_point)
        .ok()
        .flatten()
        .and_then(|important_available| {
            purgeable_bytes_from_important_available(important_available, counted_available_bytes)
        });

    foundation_estimate.or_else(|| purgeable_bytes_from_apfs_inventory(inventory, container_id))
}

fn purgeable_bytes_from_important_available(
    important_available_bytes: u64,
    counted_available_bytes: u64,
) -> Option<u64> {
    important_available_bytes
        .checked_sub(counted_available_bytes)
        .filter(|bytes| *bytes > 0)
}

fn purgeable_bytes_from_apfs_inventory(
    inventory: Option<&ApfsInventory>,
    container_id: Option<&str>,
) -> Option<u64> {
    let container_id = container_id?;
    inventory?
        .unattributed_container_used_bytes(container_id)
        .filter(|bytes| *bytes > 0)
}

fn current_process_pid() -> i32 {
    i32::try_from(std::process::id()).expect("current process id should fit in i32")
}

fn process_info_for_pid(pid: i32) -> Option<ProcessInfo> {
    let raw = proc_pidinfo_task_all(pid).ok()?;
    Some(process_info_from_task_all(pid, raw))
}

fn process_info_from_task_all(pid: i32, raw: ProcTaskAllInfo) -> ProcessInfo {
    let name = process_name(&raw);
    let executable = proc_pidpath_safe(pid).ok();
    ProcessInfo {
        pid,
        parent_pid: positive_pid(raw.pbsd.pbi_ppid.0),
        name,
        command_line: Vec::new(),
        executable,
        cwd: None,
        start_time_unix_ms: start_time_unix_ms(raw.pbsd.pbi_start_tvsec, raw.pbsd.pbi_start_tvusec),
        virtual_memory_bytes: Some(raw.ptinfo.pti_virtual_size),
        resident_memory_bytes: Some(raw.ptinfo.pti_resident_size),
        cpu_user_micros: None,
        cpu_system_micros: None,
    }
}

fn process_name(raw: &ProcTaskAllInfo) -> String {
    let name = c_char_array_to_string(&raw.pbsd.pbi_name);
    if name.is_empty() {
        c_char_array_to_string(&raw.pbsd.pbi_comm)
    } else {
        name
    }
}

fn c_char_array_to_string(raw: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = raw
        .iter()
        .copied()
        .take_while(|c| *c != 0)
        .map(|c| c.to_ne_bytes()[0])
        .collect();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

fn positive_pid(pid: u32) -> Option<i32> {
    if pid == 0 {
        return None;
    }
    i32::try_from(pid).ok()
}

fn start_time_unix_ms(seconds: u64, micros: u64) -> Option<i64> {
    let millis = seconds
        .checked_mul(1000)?
        .checked_add(micros.checked_div(1000)?)?;
    i64::try_from(millis).ok()
}

fn executable_is_under(process: &ProcessInfo, root: &Path) -> bool {
    process
        .executable
        .as_deref()
        .is_some_and(|path| resolve_absolute_path(path).starts_with(root))
}

fn open_files_for_pid_under(pid: i32, root: &Path) -> Vec<OpenFile> {
    let Ok(fds) = proc_pid_list_fds(pid) else {
        return Vec::new();
    };
    fds.into_iter()
        .filter_map(|fd| open_file_for_fd_under(pid, fd, root))
        .collect()
}

fn open_file_for_fd_under(pid: i32, fd: ProcFdInfo, root: &Path) -> Option<OpenFile> {
    if fd.fd_type().ok()? != ProcFdType::VNODE {
        return None;
    }
    let info = proc_pidfdinfo_vnode_path(pid, fd.proc_fd.0)
        .ok()
        .flatten()?;
    let path = resolve_absolute_path(info.path().ok()?);
    if !path.starts_with(root) {
        return None;
    }
    Some(OpenFile {
        pid,
        path,
        fd: Some(fd.proc_fd.0),
        kind: open_file_kind(&info),
        mode: open_file_mode(info.pfi.fi_openflags),
    })
}

fn open_file_kind(info: &VnodeFdInfoWithPath) -> OpenFileKind {
    match info.pvip.vip_vi.vi_stat.vst_mode & libc::S_IFMT {
        libc::S_IFREG => OpenFileKind::Regular,
        libc::S_IFDIR => OpenFileKind::Directory,
        libc::S_IFSOCK => OpenFileKind::Socket,
        libc::S_IFIFO => OpenFileKind::Pipe,
        libc::S_IFCHR | libc::S_IFBLK => OpenFileKind::Device,
        _ => OpenFileKind::Unknown,
    }
}

fn open_file_mode(open_flags: u32) -> OpenFileMode {
    const FREAD: u32 = 0x1;
    const FWRITE: u32 = 0x2;
    match (open_flags & FREAD != 0, open_flags & FWRITE != 0) {
        (true, true) => OpenFileMode::ReadWrite,
        (true, false) => OpenFileMode::Read,
        (false, true) => OpenFileMode::Write,
        (false, false) => OpenFileMode::Unknown,
    }
}

fn mapped_regions_for_pid_under(pid: i32, root: &Path) -> Vec<MappedRegion> {
    const MAX_REGIONS_PER_PID: usize = 16_384;

    let mut address = 0;
    let mut regions = Vec::new();
    for _ in 0..MAX_REGIONS_PER_PID {
        let Ok(info) = proc_pid_region_path(pid, address) else {
            break;
        };
        let start = info.prp_prinfo.pri_address;
        let size = info.prp_prinfo.pri_size;
        if size == 0 {
            break;
        }
        if let Some(region) = mapped_region_under(pid, &info, root) {
            regions.push(region);
        }
        let Some(next_address) = start.checked_add(size) else {
            break;
        };
        if next_address <= address {
            break;
        }
        address = next_address;
    }
    regions
}

fn mapped_region_under(
    pid: i32,
    info: &ProcRegionWithPathInfo,
    root: &Path,
) -> Option<MappedRegion> {
    let path = resolve_absolute_path(info.prp_vip.path().ok()?);
    if !path.starts_with(root) {
        return None;
    }
    let start = info.prp_prinfo.pri_address;
    let end = start.checked_add(info.prp_prinfo.pri_size);
    Some(MappedRegion {
        pid,
        path,
        start_address: Some(start),
        end_address: end,
        protection: Some(region_protection(info.prp_prinfo.pri_protection)),
    })
}

fn region_protection(bits: u32) -> String {
    const VM_PROT_READ: u32 = 0x01;
    const VM_PROT_WRITE: u32 = 0x02;
    const VM_PROT_EXECUTE: u32 = 0x04;
    let read = if bits & VM_PROT_READ != 0 { 'r' } else { '-' };
    let write = if bits & VM_PROT_WRITE != 0 { 'w' } else { '-' };
    let execute = if bits & VM_PROT_EXECUTE != 0 {
        'x'
    } else {
        '-'
    };
    format!("{read}{write}{execute}")
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;

    use crate::platform::macos::sys::{
        ApfsContainer, ApfsInventory, ApfsVolume, ApfsVolumeRole, StatfsSnapshot,
    };
    use crate::platform::pal::Platform;

    use super::MacOsPal;

    fn assert_platform<T: Platform>(_platform: &T) {}

    #[test]
    fn macos_pal_skeleton_implements_platform() {
        let platform = MacOsPal::new();
        assert_platform(&platform);
        assert_eq!(platform.name(), "macos");
    }

    #[test]
    fn capacity_uses_apfs_container_totals_for_data_volume() {
        let stats = StatfsSnapshot {
            mount_point: PathBuf::from("/System/Volumes/Data"),
            device: "/dev/disk3s5".to_string(),
            fs_type: "apfs".to_string(),
            block_size: 1,
            blocks: 400,
            blocks_free: 120,
            blocks_available: 100,
            is_readonly: false,
        };
        let inventory = ApfsInventory {
            containers: vec![ApfsContainer {
                container_id: "/dev/disk3".to_string(),
                uuid: Some("container-uuid".to_string()),
                capacity_total_bytes: Some(1_000),
                capacity_available_bytes: Some(250),
                physical_stores: vec!["/dev/disk0s2".to_string()],
            }],
            volumes: vec![
                ApfsVolume {
                    device_id: "/dev/disk3s1".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("Macintosh HD".to_string()),
                    roles: vec![ApfsVolumeRole::System],
                    capacity_in_use_bytes: Some(200),
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
                ApfsVolume {
                    device_id: "/dev/disk3s5".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("Data".to_string()),
                    roles: vec![ApfsVolumeRole::Data],
                    capacity_in_use_bytes: Some(650),
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
                ApfsVolume {
                    device_id: "/dev/disk3s6".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("VM".to_string()),
                    roles: vec![ApfsVolumeRole::Vm],
                    capacity_in_use_bytes: None,
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
            ],
        };

        let capacity = super::statfs_to_capacity(stats, Some(&inventory));

        assert_eq!(capacity.total_bytes, 1_000);
        assert_eq!(capacity.free_bytes, 250);
        assert_eq!(capacity.available_bytes, 250);
        assert_eq!(capacity.container_id.as_deref(), Some("/dev/disk3"));
        assert_eq!(capacity.container_total_bytes, Some(1_000));
        assert_eq!(capacity.container_available_bytes, Some(250));
        assert_eq!(capacity.volume_total_bytes, Some(400));
        assert_eq!(capacity.volume_available_bytes, Some(100));
        assert_eq!(capacity.volume_role.as_deref(), Some("Data"));
        assert_eq!(capacity.shared_volumes, vec!["Macintosh HD", "VM"]);
        assert!(capacity.is_primary);
    }

    #[test]
    fn fs_stats_projection_uses_effective_apfs_container_capacity() {
        let capacity = crate::platform::types::Capacity {
            mount_point: PathBuf::from("/System/Volumes/Data"),
            fs_type: "apfs".to_string(),
            total_bytes: 1_000,
            free_bytes: 250,
            available_bytes: 250,
            is_readonly: false,
            container_id: Some("/dev/disk3".to_string()),
            container_total_bytes: Some(1_000),
            container_available_bytes: Some(250),
            volume_total_bytes: Some(400),
            volume_available_bytes: Some(100),
            volume_role: Some("Data".to_string()),
            shared_volumes: vec!["Macintosh HD".to_string()],
            is_primary: true,
            purgeable_bytes: Some(50),
            local_snapshot_bytes: Some(64),
        };

        let stats = super::capacity_to_fs_stats(capacity);

        assert_eq!(stats.total_bytes, 1_000);
        assert_eq!(stats.free_bytes, 250);
        assert_eq!(stats.available_bytes, 250);
        assert!((stats.free_pct() - 25.0).abs() < f64::EPSILON);
        assert_eq!(stats.mount_point, PathBuf::from("/System/Volumes/Data"));
    }

    #[test]
    fn mount_info_uses_apfs_container_metadata_and_snapshot_estimate() {
        let stats = StatfsSnapshot {
            mount_point: PathBuf::from("/Volumes/TestData"),
            device: "/dev/disk3s5".to_string(),
            fs_type: "apfs".to_string(),
            block_size: 1,
            blocks: 400,
            blocks_free: 120,
            blocks_available: 100,
            is_readonly: false,
        };
        let inventory = ApfsInventory {
            containers: vec![ApfsContainer {
                container_id: "/dev/disk3".to_string(),
                uuid: Some("container-uuid".to_string()),
                capacity_total_bytes: Some(1_000),
                capacity_available_bytes: Some(250),
                physical_stores: vec!["/dev/disk0s2".to_string()],
            }],
            volumes: vec![
                ApfsVolume {
                    device_id: "/dev/disk3s1".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("Macintosh HD".to_string()),
                    roles: vec![ApfsVolumeRole::System],
                    capacity_in_use_bytes: Some(100),
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
                ApfsVolume {
                    device_id: "/dev/disk3s5".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("Data".to_string()),
                    roles: vec![ApfsVolumeRole::Data],
                    capacity_in_use_bytes: Some(600),
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
            ],
        };

        let mount_info = super::statfs_to_mount_info(stats, Some(&inventory), Some(64));

        assert_eq!(mount_info.container_id.as_deref(), Some("/dev/disk3"));
        assert_eq!(mount_info.total_bytes, Some(1_000));
        assert_eq!(mount_info.available_bytes, Some(250));
        assert_eq!(mount_info.purgeable_bytes, Some(50));
        assert_eq!(mount_info.local_snapshot_bytes, Some(64));
        assert!(mount_info.is_apfs_data_volume);
        assert!(!mount_info.is_apfs_system_snapshot);
        assert!(!mount_info.is_apfs_vm_volume);
    }

    #[test]
    fn mount_info_marks_apfs_system_and_vm_roles() {
        let inventory = ApfsInventory {
            containers: vec![ApfsContainer {
                container_id: "/dev/disk3".to_string(),
                uuid: Some("container-uuid".to_string()),
                capacity_total_bytes: Some(1_000),
                capacity_available_bytes: Some(250),
                physical_stores: vec!["/dev/disk0s2".to_string()],
            }],
            volumes: vec![
                ApfsVolume {
                    device_id: "/dev/disk3s1".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("Macintosh HD".to_string()),
                    roles: vec![ApfsVolumeRole::System],
                    capacity_in_use_bytes: Some(100),
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
                ApfsVolume {
                    device_id: "/dev/disk3s6".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("VM".to_string()),
                    roles: vec![ApfsVolumeRole::Vm],
                    capacity_in_use_bytes: Some(50),
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
            ],
        };

        let system = super::statfs_to_mount_info(
            StatfsSnapshot {
                mount_point: PathBuf::from("/Volumes/TestSystem"),
                device: "/dev/disk3s1".to_string(),
                fs_type: "apfs".to_string(),
                block_size: 1,
                blocks: 400,
                blocks_free: 120,
                blocks_available: 100,
                is_readonly: true,
            },
            Some(&inventory),
            None,
        );
        let vm = super::statfs_to_mount_info(
            StatfsSnapshot {
                mount_point: PathBuf::from("/Volumes/TestVM"),
                device: "/dev/disk3s6".to_string(),
                fs_type: "apfs".to_string(),
                block_size: 1,
                blocks: 400,
                blocks_free: 120,
                blocks_available: 100,
                is_readonly: false,
            },
            Some(&inventory),
            None,
        );

        assert!(system.is_apfs_system_snapshot);
        assert!(!system.is_apfs_data_volume);
        assert!(!system.is_apfs_vm_volume);
        assert!(vm.is_apfs_vm_volume);
        assert!(!vm.is_apfs_system_snapshot);
        assert!(!vm.is_apfs_data_volume);
    }

    #[test]
    fn macos_pal_mounts_reports_live_apfs_annotations_when_available() {
        let Ok(inventory) = crate::platform::macos::sys::apfs_inventory() else {
            return;
        };
        if inventory.volumes.is_empty() {
            return;
        }

        let platform = MacOsPal::new();
        let mounts = platform
            .mounts()
            .expect("macOS mount inventory should be readable");
        let apfs_mounts: Vec<_> = mounts
            .iter()
            .filter(|mount| mount.fs_type.eq_ignore_ascii_case("apfs"))
            .collect();
        if apfs_mounts.is_empty() {
            return;
        }

        assert!(apfs_mounts.iter().any(|mount| mount.container_id.is_some()));
        assert!(apfs_mounts.iter().any(|mount| {
            mount.is_apfs_data_volume || mount.is_apfs_system_snapshot || mount.is_apfs_vm_volume
        }));
    }

    #[test]
    fn purgeable_estimate_subtracts_counted_free_space() {
        assert_eq!(
            super::purgeable_bytes_from_important_available(900, 250),
            Some(650)
        );
        assert_eq!(
            super::purgeable_bytes_from_important_available(250, 250),
            None
        );
        assert_eq!(
            super::purgeable_bytes_from_important_available(200, 250),
            None
        );
    }

    #[test]
    fn purgeable_inventory_fallback_uses_unattributed_container_bytes() {
        let inventory = ApfsInventory {
            containers: vec![ApfsContainer {
                container_id: "/dev/disk3".to_string(),
                uuid: Some("container-uuid".to_string()),
                capacity_total_bytes: Some(1_000),
                capacity_available_bytes: Some(250),
                physical_stores: vec!["/dev/disk0s2".to_string()],
            }],
            volumes: vec![
                ApfsVolume {
                    device_id: "/dev/disk3s1".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("Macintosh HD".to_string()),
                    roles: vec![ApfsVolumeRole::System],
                    capacity_in_use_bytes: Some(200),
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
                ApfsVolume {
                    device_id: "/dev/disk3s5".to_string(),
                    container_id: "/dev/disk3".to_string(),
                    name: Some("Data".to_string()),
                    roles: vec![ApfsVolumeRole::Data],
                    capacity_in_use_bytes: Some(500),
                    container_total_bytes: Some(1_000),
                    container_available_bytes: Some(250),
                },
            ],
        };

        assert_eq!(
            super::purgeable_bytes_from_apfs_inventory(Some(&inventory), Some("/dev/disk3")),
            Some(50)
        );
    }

    #[test]
    fn process_list_returns_visible_processes_except_self_and_pid_zero() {
        let platform = MacOsPal::new();
        let processes = platform
            .process_list()
            .expect("macOS process list should be readable");
        assert!(!processes.is_empty());
        assert!(!processes.iter().any(|process| process.pid == 0));
        assert!(
            !processes
                .iter()
                .any(|process| process.pid == super::current_process_pid())
        );
        assert!(processes.iter().all(|process| !process.name.is_empty()));
        assert!(
            processes
                .iter()
                .any(|process| process.resident_memory_bytes.is_some())
        );
    }

    #[test]
    fn process_io_returns_lifetime_rusage_counters_for_current_process() {
        let current = super::current_process_pid();
        let platform = MacOsPal::new();
        let io = platform
            .process_io(current)
            .expect("current process rusage should be readable");
        let raw = crate::platform::macos::libproc::proc_pid_rusage_v4_safe(current)
            .expect("current process raw rusage should be readable");

        assert_eq!(io.pid, current);
        assert!(io.bytes_read_total <= raw.ri_diskio_bytesread);
        assert!(io.bytes_written_total <= raw.ri_diskio_byteswritten);
        assert_eq!(io.bytes_read_recent_15m, None);
        assert_eq!(io.bytes_written_recent_15m, None);
    }

    #[test]
    fn open_files_under_returns_current_process_tempfile_fd() {
        let dir = tempfile::TempDir::new().expect("temp dir should be created");
        let file_path = dir.path().join("open-file.txt");
        let _file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .expect("temp file should stay open for fd scan");
        let expected = std::fs::canonicalize(&file_path).expect("temp file should canonicalize");

        let platform = MacOsPal::new();
        let open_files = platform
            .open_files_under(dir.path())
            .expect("macOS open fd scan should be readable");

        let actual = open_files
            .iter()
            .find(|open_file| {
                open_file.pid == super::current_process_pid() && open_file.path == expected
            })
            .expect("current process open tempfile should be reported");
        assert_eq!(actual.kind, crate::platform::types::OpenFileKind::Regular);
        assert_eq!(actual.mode, crate::platform::types::OpenFileMode::ReadWrite);
        assert!(actual.fd.is_some());
    }

    #[test]
    fn executables_under_returns_current_process_executable() {
        let current_exe = std::env::current_exe().expect("current executable should be known");
        let root = current_exe
            .parent()
            .expect("current executable should have parent");
        let expected =
            std::fs::canonicalize(&current_exe).expect("current executable should canonicalize");

        let platform = MacOsPal::new();
        let processes = platform
            .executables_under(root)
            .expect("macOS executable scan should be readable");

        assert!(processes.iter().any(|process| {
            process.pid == super::current_process_pid()
                && process.executable.as_deref() == Some(expected.as_path())
        }));
    }

    #[test]
    fn file_block_count_reports_allocated_blocks_for_macos_file() {
        let dir = tempfile::TempDir::new().expect("temp dir should be created");
        let path = dir.path().join("block-count.bin");
        let payload = vec![0x5a; 16 * 1024];
        let mut file = std::fs::File::create(&path).expect("temp file should be created");
        file.write_all(&payload)
            .expect("temp file payload should be written");
        file.sync_all().expect("temp file should sync");
        drop(file);

        let platform = MacOsPal::new();
        let blocks = platform
            .file_block_count(&path)
            .expect("file block count should be readable");

        let metadata_blocks = std::fs::metadata(&path)
            .expect("temp file metadata should be readable")
            .blocks();
        assert_eq!(blocks, metadata_blocks);
        assert!(blocks > 0);
    }

    #[test]
    fn mmap_regions_under_reports_current_process_executable_mapping() {
        let current_exe = std::env::current_exe().expect("current executable should be known");
        let root = current_exe
            .parent()
            .expect("current executable should have parent");
        let expected =
            std::fs::canonicalize(&current_exe).expect("current executable should canonicalize");

        let platform = MacOsPal::new();
        let regions = platform
            .mmap_regions_under(root)
            .expect("macOS mapped region scan should be readable");

        assert!(regions.iter().any(|region| {
            region.pid == super::current_process_pid()
                && region.path == expected
                && region.start_address.is_some()
                && region.end_address.is_some()
                && region.protection.is_some()
        }));
    }
}
