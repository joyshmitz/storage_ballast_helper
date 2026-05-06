//! macOS PAL skeleton.

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

use std::path::{Path, PathBuf};

use crate::core::errors::Result;
use crate::core::paths::resolve_absolute_path;
use crate::platform::macos::libproc::{
    ProcFdInfo, ProcFdType, ProcTaskAllInfo, VnodeFdInfoWithPath, proc_listpids_safe,
    proc_pid_list_fds, proc_pidfdinfo_vnode_path, proc_pidinfo_task_all, proc_pidpath_safe,
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

    fn process_io(&self, _pid: i32) -> Result<ProcessIo> {
        macos_placeholder("bd-ly4w.3", "process_io")
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
}
