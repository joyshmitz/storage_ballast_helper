//! macOS PAL skeleton.

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

use std::path::{Path, PathBuf};

use crate::core::errors::Result;
use crate::core::paths::resolve_absolute_path;
use crate::platform::macos::libproc::{
    ProcFdInfo, ProcFdType, ProcRegionWithPathInfo, ProcTaskAllInfo, VnodeFdInfoWithPath,
    proc_listpids_safe, proc_pid_list_fds, proc_pid_region_path, proc_pid_rusage_v4_safe,
    proc_pidfdinfo_vnode_path, proc_pidinfo_task_all, proc_pidpath_safe,
};
use crate::platform::pal::{
    FsStats, MemoryInfo, MountPoint, Platform, PlatformPaths, ServiceManager,
};
use crate::platform::types::{
    Capacity, MappedRegion, MemoryPressure, MemoryPressureCallback, MountInfo, OpenFile,
    OpenFileKind, OpenFileMode, PalError, ProcessInfo, ProcessIo, SacredPath, SelfStats,
    ServiceKind, SubscriptionHandle,
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
        macos_placeholder("bd-wiqg.2", "self_stats")
    }

    fn preallocate_file(&self, _path: &Path, _size: u64) -> Result<()> {
        macos_placeholder("bd-hnxg.1", "preallocate_file")
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

fn macos_method_error(
    method: &'static str,
    error: &impl ToString,
) -> crate::core::errors::SbhError {
    PalError::method_failed("macos", method, error.to_string()).into()
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
