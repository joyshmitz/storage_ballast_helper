//! Safe macOS libproc adapter.

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

use std::io;
use std::path::PathBuf;

use proc_pidinfo::{Fd, Pid, VnodeInfoPath, proc_pidfdinfo, proc_pidinfo, proc_pidinfo_list};

pub type ProcTaskInfo = proc_pidinfo::ProcTaskInfo;
pub type ProcTaskAllInfo = proc_pidinfo::ProcTaskAllInfo;
pub type ProcFdInfo = proc_pidinfo::ProcFDInfo;
pub type ProcFdType = proc_pidinfo::ProcFDType;
pub type ProcFileInfo = proc_pidinfo::ProcFileInfo;
pub type VnodeFdInfoWithPath = proc_pidinfo::VnodeFdInfoWithPath;
pub type RUsageInfoV4 = libproc::libproc::pid_rusage::RUsageInfoV4;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcRegionInfo {
    pub pri_protection: u32,
    pub pri_max_protection: u32,
    pub pri_inheritance: u32,
    pub pri_flags: u32,
    pub pri_offset: u64,
    pub pri_behavior: u32,
    pub pri_user_wired_count: u32,
    pub pri_user_tag: u32,
    pub pri_pages_resident: u32,
    pub pri_pages_shared_now_private: u32,
    pub pri_pages_swapped_out: u32,
    pub pri_pages_dirtied: u32,
    pub pri_ref_count: u32,
    pub pri_shadow_depth: u32,
    pub pri_share_mode: u32,
    pub pri_private_pages_resident: u32,
    pub pri_shared_pages_resident: u32,
    pub pri_obj_id: u32,
    pub pri_depth: u32,
    pub pri_address: u64,
    pub pri_size: u64,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ProcRegionWithPathInfo {
    pub prp_prinfo: ProcRegionInfo,
    pub prp_vip: VnodeInfoPath,
}

impl libproc::libproc::proc_pid::PIDInfo for ProcRegionWithPathInfo {
    fn flavor() -> libproc::libproc::proc_pid::PidInfoFlavor {
        libproc::libproc::proc_pid::PidInfoFlavor::RegionPathInfo
    }
}

pub const PROC_ALL_PIDS: u32 = 1;
pub const PROC_PIDLISTFDS: i32 = 1;
pub const PROC_PIDTASKALLINFO: i32 = 2;
pub const PROC_PIDTASKINFO: i32 = 4;
pub const PROC_PIDPATHINFO: i32 = 11;
pub const PROC_PIDFDVNODEPATHINFO: i32 = 2;
pub const RUSAGE_INFO_V4: i32 = 4;

pub fn proc_listpids_safe() -> io::Result<Vec<i32>> {
    libproc::processes::pids_by_type(libproc::processes::ProcFilter::All)?
        .into_iter()
        .map(|pid| {
            i32::try_from(pid)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "pid exceeds i32 range"))
        })
        .collect()
}

pub fn proc_pidinfo_task(pid: i32) -> io::Result<ProcTaskInfo> {
    proc_pidinfo::<ProcTaskInfo>(pid_arg(pid)?)?.ok_or_else(|| process_not_found(pid))
}

pub fn proc_pidinfo_task_all(pid: i32) -> io::Result<ProcTaskAllInfo> {
    proc_pidinfo::<ProcTaskAllInfo>(pid_arg(pid)?)?.ok_or_else(|| process_not_found(pid))
}

pub fn proc_pidpath_safe(pid: i32) -> io::Result<PathBuf> {
    libproc::libproc::proc_pid::pidpath(pid)
        .map(PathBuf::from)
        .map_err(io::Error::other)
}

pub fn proc_pid_rusage_v4_safe(pid: i32) -> io::Result<RUsageInfoV4> {
    libproc::libproc::pid_rusage::pidrusage::<RUsageInfoV4>(pid).map_err(io::Error::other)
}

pub fn proc_pid_list_fds(pid: i32) -> io::Result<Vec<ProcFdInfo>> {
    proc_pidinfo_list::<ProcFdInfo>(pid_arg(pid)?)
}

pub fn proc_pidfdinfo_vnode_path(pid: i32, fd: i32) -> io::Result<Option<VnodeFdInfoWithPath>> {
    proc_pidfdinfo::<VnodeFdInfoWithPath>(pid_arg(pid)?, fd_arg(fd)?)
}

pub fn proc_pid_region_path(pid: i32, address: u64) -> io::Result<ProcRegionWithPathInfo> {
    if pid < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pid must be non-negative",
        ));
    }
    libproc::libproc::proc_pid::pidinfo::<ProcRegionWithPathInfo>(pid, address)
        .map_err(io::Error::other)
}

fn pid_arg(pid: i32) -> io::Result<Pid> {
    u32::try_from(pid)
        .map(Pid)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "pid must be non-negative"))
}

fn fd_arg(fd: i32) -> io::Result<Fd> {
    if fd < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file descriptor must be non-negative",
        ));
    }
    Ok(Fd(fd))
}

fn process_not_found(pid: i32) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("libproc returned no data for pid {pid}"),
    )
}

#[cfg(test)]
mod tests {
    use std::mem;

    use super::{
        PROC_ALL_PIDS, PROC_PIDFDVNODEPATHINFO, PROC_PIDLISTFDS, PROC_PIDPATHINFO,
        PROC_PIDTASKALLINFO, PROC_PIDTASKINFO, ProcFdInfo, ProcFdType, ProcFileInfo,
        ProcRegionInfo, ProcRegionWithPathInfo, ProcTaskAllInfo, ProcTaskInfo, RUSAGE_INFO_V4,
        RUsageInfoV4, VnodeFdInfoWithPath, proc_listpids_safe, proc_pid_list_fds,
        proc_pid_region_path, proc_pid_rusage_v4_safe, proc_pidfdinfo_vnode_path,
        proc_pidinfo_task, proc_pidinfo_task_all, proc_pidpath_safe,
    };

    fn current_pid() -> i32 {
        i32::try_from(std::process::id()).expect("current process id should fit in i32")
    }

    #[test]
    fn constants_and_struct_layouts_match_macos_sdk() {
        assert_eq!(PROC_ALL_PIDS, 1);
        assert_eq!(PROC_PIDLISTFDS, 1);
        assert_eq!(PROC_PIDTASKALLINFO, 2);
        assert_eq!(PROC_PIDTASKINFO, 4);
        assert_eq!(PROC_PIDPATHINFO, 11);
        assert_eq!(PROC_PIDFDVNODEPATHINFO, 2);
        assert_eq!(RUSAGE_INFO_V4, 4);

        assert_eq!(mem::size_of::<ProcTaskInfo>(), 96);
        assert_eq!(mem::size_of::<ProcTaskAllInfo>(), 232);
        assert_eq!(mem::size_of::<RUsageInfoV4>(), 296);
        assert_eq!(mem::size_of::<ProcFdInfo>(), 8);
        assert_eq!(mem::size_of::<ProcFileInfo>(), 24);
        assert_eq!(mem::size_of::<VnodeFdInfoWithPath>(), 1200);
        assert_eq!(mem::size_of::<ProcRegionInfo>(), 96);
        assert_eq!(mem::size_of::<ProcRegionWithPathInfo>(), 1272);
    }

    #[test]
    fn proc_listpids_includes_current_process() {
        let current = current_pid();
        let pids = proc_listpids_safe().expect("proc_listpids should succeed");
        assert!(pids.contains(&current));
    }

    #[test]
    fn current_process_task_path_and_rusage_are_readable() {
        let current = current_pid();
        let task = proc_pidinfo_task(current).expect("task info should be readable");
        assert!(task.pti_virtual_size > 0);
        assert!(task.pti_resident_size > 0);

        let all = proc_pidinfo_task_all(current).expect("task-all info should be readable");
        assert_eq!(all.pbsd.pbi_pid.0, std::process::id());

        let path = proc_pidpath_safe(current).expect("pid path should be readable");
        assert!(!path.as_os_str().is_empty());

        let rusage = proc_pid_rusage_v4_safe(current).expect("rusage should be readable");
        assert!(rusage.ri_resident_size > 0);
    }

    #[test]
    fn current_process_region_path_is_readable() {
        let current = current_pid();
        let region = proc_pid_region_path(current, 0).expect("region path should be readable");
        assert!(region.prp_prinfo.pri_size > 0);
    }

    #[test]
    fn vnode_fdinfo_reports_open_temp_file_path() {
        let file = tempfile::NamedTempFile::new().expect("temp file should be created");
        let expected =
            std::fs::canonicalize(file.path()).expect("temp file path should canonicalize");
        let current = current_pid();

        let fds = proc_pid_list_fds(current).expect("fd list should be readable");
        let mut found = false;
        for fd in fds {
            if fd.fd_type() != Ok(ProcFdType::VNODE) {
                continue;
            }
            let Some(info) = proc_pidfdinfo_vnode_path(current, fd.proc_fd.0)
                .expect("vnode fd info should be readable")
            else {
                continue;
            };
            let Ok(path) = info.path() else {
                continue;
            };
            let Ok(actual) = std::fs::canonicalize(path) else {
                continue;
            };
            if actual == expected {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "open tempfile should appear in current process vnode fd list"
        );
    }
}
