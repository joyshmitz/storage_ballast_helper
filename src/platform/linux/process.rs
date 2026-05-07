//! Linux process readers for future PAL process methods.

#![cfg(target_os = "linux")]
#![allow(missing_docs)]

use std::fs;
use std::path::PathBuf;

use crate::core::errors::{Result, SbhError};
use crate::platform::types::{PalError, ProcessInfo, ProcessIo, SelfStats};

const PROC_SELF_STATUS: &str = "/proc/self/status";
const PROC_SELF_STAT: &str = "/proc/self/stat";
const PROC_SELF_IO: &str = "/proc/self/io";
const PROC_ROOT: &str = "/proc";
const PROC_STAT: &str = "/proc/stat";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatusMemory {
    rss_bytes: u64,
    virtual_memory_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CpuTimes {
    user_micros: u64,
    system_micros: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IoCounters {
    bytes_read: u64,
    bytes_written: u64,
}

pub(super) fn read_process_list() -> Result<Vec<ProcessInfo>> {
    let proc_dir = fs::read_dir(PROC_ROOT).map_err(|source| SbhError::Io {
        path: PathBuf::from(PROC_ROOT),
        source,
    })?;
    let current_pid = i32::try_from(std::process::id()).unwrap_or(i32::MAX);
    let boot_time_unix_ms = read_proc_file(PROC_STAT)
        .ok()
        .and_then(|raw| parse_proc_boot_time_unix_ms(&raw).ok());
    let mut processes = Vec::new();

    for entry in proc_dir {
        let Ok(entry) = entry else {
            continue;
        };
        let Some(pid) = pid_from_proc_entry_name(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        if pid <= 0 || pid == current_pid {
            continue;
        }
        if let Some(process) = process_info_for_pid(pid, boot_time_unix_ms) {
            processes.push(process);
        }
    }

    processes.sort_by(|left, right| {
        left.pid
            .cmp(&right.pid)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(processes)
}

pub(super) fn read_process_io(pid: i32) -> Result<ProcessIo> {
    let path = pid_proc_path(pid).join("io");
    let raw = fs::read_to_string(&path).map_err(|source| SbhError::Io {
        path: path.clone(),
        source,
    })?;
    let counters = parse_proc_io(&raw, "process_io")?;
    Ok(ProcessIo {
        pid,
        bytes_read_total: counters.bytes_read,
        bytes_written_total: counters.bytes_written,
        bytes_read_recent_15m: None,
        bytes_written_recent_15m: None,
    })
}

pub(super) fn read_self_stats() -> Result<SelfStats> {
    let status = read_proc_file(PROC_SELF_STATUS)?;
    let stat = read_proc_file(PROC_SELF_STAT)?;
    let memory = parse_proc_self_status(&status)?;
    let cpu = parse_proc_self_stat(&stat, clock_ticks_per_second())?;
    let io = read_proc_file(PROC_SELF_IO)
        .and_then(|raw| parse_proc_io(&raw, "self_stats"))
        .ok();

    Ok(SelfStats {
        rss_bytes: memory.rss_bytes,
        virtual_memory_bytes: memory.virtual_memory_bytes,
        cpu_user_micros: cpu.user_micros,
        cpu_system_micros: cpu.system_micros,
        idle_wakeups: None,
        bytes_read: io.map(|counters| counters.bytes_read),
        bytes_written: io.map(|counters| counters.bytes_written),
    })
}

fn read_proc_file(path: &str) -> Result<String> {
    fs::read_to_string(path).map_err(|source| SbhError::Io {
        path: PathBuf::from(path),
        source,
    })
}

fn pid_from_proc_entry_name(name: &str) -> Option<i32> {
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    name.parse::<i32>().ok()
}

fn pid_proc_path(pid: i32) -> PathBuf {
    PathBuf::from(PROC_ROOT).join(pid.to_string())
}

fn process_info_for_pid(pid: i32, boot_time_unix_ms: Option<i64>) -> Option<ProcessInfo> {
    let proc_path = pid_proc_path(pid);
    let name = fs::read_to_string(proc_path.join("comm"))
        .ok()?
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }

    let status = fs::read_to_string(proc_path.join("status")).ok();
    let stat = fs::read_to_string(proc_path.join("stat")).ok();
    let ticks_per_second = clock_ticks_per_second();
    let cpu = stat
        .as_deref()
        .and_then(|raw| parse_proc_self_stat(raw, ticks_per_second).ok());

    Some(ProcessInfo {
        pid,
        parent_pid: status
            .as_deref()
            .and_then(|raw| parse_status_i32_field(raw, "PPid")),
        name,
        command_line: read_command_line(&proc_path),
        executable: fs::read_link(proc_path.join("exe")).ok(),
        cwd: fs::read_link(proc_path.join("cwd")).ok(),
        start_time_unix_ms: stat
            .as_deref()
            .zip(boot_time_unix_ms)
            .and_then(|(raw, boot_time)| {
                parse_proc_start_time_unix_ms(raw, boot_time, ticks_per_second).ok()
            }),
        virtual_memory_bytes: status
            .as_deref()
            .and_then(|raw| parse_status_kib_field_from_raw(raw, "VmSize")),
        resident_memory_bytes: status
            .as_deref()
            .and_then(|raw| parse_status_kib_field_from_raw(raw, "VmRSS")),
        cpu_user_micros: cpu.map(|times| times.user_micros),
        cpu_system_micros: cpu.map(|times| times.system_micros),
    })
}

fn read_command_line(proc_path: &std::path::Path) -> Vec<String> {
    let Ok(raw) = fs::read(proc_path.join("cmdline")) else {
        return Vec::new();
    };
    raw.split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

fn parse_status_i32_field(raw: &str, key: &'static str) -> Option<i32> {
    raw.lines().find_map(|line| {
        let rest = line
            .strip_prefix(key)
            .and_then(|rest| rest.strip_prefix(':'))?;
        rest.split_whitespace().next()?.parse::<i32>().ok()
    })
}

fn parse_status_kib_field_from_raw(raw: &str, key: &'static str) -> Option<u64> {
    raw.lines()
        .find_map(|line| parse_status_kib_field(line, key).ok().flatten())
}

fn parse_proc_boot_time_unix_ms(raw: &str) -> Result<i64> {
    let boot_seconds = raw
        .lines()
        .find_map(|line| line.strip_prefix("btime "))
        .ok_or_else(|| process_parse_error("process_list", "missing btime in /proc/stat"))?
        .trim()
        .parse::<i64>()
        .map_err(|error| {
            process_parse_error("process_list", format!("invalid btime value: {error}"))
        })?;
    Ok(boot_seconds.saturating_mul(1_000))
}

fn parse_proc_self_status(raw: &str) -> Result<StatusMemory> {
    let mut rss_bytes = None;
    let mut virtual_memory_bytes = None;

    for line in raw.lines() {
        if let Some(value) = parse_status_kib_field(line, "VmRSS")? {
            rss_bytes = Some(value);
        } else if let Some(value) = parse_status_kib_field(line, "VmSize")? {
            virtual_memory_bytes = Some(value);
        }
    }

    Ok(StatusMemory {
        rss_bytes: required_counter(rss_bytes, "VmRSS")?,
        virtual_memory_bytes: required_counter(virtual_memory_bytes, "VmSize")?,
    })
}

fn parse_status_kib_field(line: &str, key: &'static str) -> Result<Option<u64>> {
    let Some(rest) = line
        .strip_prefix(key)
        .and_then(|rest| rest.strip_prefix(':'))
    else {
        return Ok(None);
    };
    let mut parts = rest.split_whitespace();
    let raw_value = parts
        .next()
        .ok_or_else(|| self_stats_parse_error(format!("missing value for {key}")))?;
    let value = raw_value.parse::<u64>().map_err(|error| {
        self_stats_parse_error(format!("invalid numeric value for {key}: {error}"))
    })?;
    let bytes = match parts.next() {
        Some("kB") => value.saturating_mul(1024),
        None => value,
        Some(unit) => {
            return Err(self_stats_parse_error(format!(
                "unsupported unit for {key}: {unit}"
            )));
        }
    };
    Ok(Some(bytes))
}

fn parse_proc_self_stat(raw: &str, ticks_per_second: u64) -> Result<CpuTimes> {
    let fields = proc_stat_fields_after_comm(raw)?;
    let user_ticks = parse_stat_tick_field(&fields, 11, "utime")?;
    let system_ticks = parse_stat_tick_field(&fields, 12, "stime")?;

    Ok(CpuTimes {
        user_micros: ticks_to_micros(user_ticks, ticks_per_second),
        system_micros: ticks_to_micros(system_ticks, ticks_per_second),
    })
}

fn parse_proc_start_time_unix_ms(
    raw: &str,
    boot_time_unix_ms: i64,
    ticks_per_second: u64,
) -> Result<i64> {
    let fields = proc_stat_fields_after_comm(raw)?;
    let start_ticks = parse_stat_tick_field(&fields, 19, "starttime")?;
    Ok(boot_time_unix_ms.saturating_add(ticks_to_millis(start_ticks, ticks_per_second)))
}

fn proc_stat_fields_after_comm(raw: &str) -> Result<Vec<&str>> {
    let Some((_, rest)) = raw.rsplit_once(") ") else {
        return Err(self_stats_parse_error(
            "invalid /proc/self/stat format: missing command terminator",
        ));
    };
    let fields: Vec<&str> = rest.split_whitespace().collect();
    if fields.len() <= 12 {
        return Err(self_stats_parse_error(format!(
            "invalid /proc/self/stat format: expected at least 13 fields after command, got {}",
            fields.len()
        )));
    }
    Ok(fields)
}

fn parse_stat_tick_field(fields: &[&str], index: usize, name: &'static str) -> Result<u64> {
    fields
        .get(index)
        .ok_or_else(|| self_stats_parse_error(format!("missing {name} field")))?
        .parse::<u64>()
        .map_err(|error| self_stats_parse_error(format!("invalid {name} value: {error}")))
}

fn parse_proc_io(raw: &str, method: &'static str) -> Result<IoCounters> {
    let mut bytes_read = None;
    let mut bytes_written = None;

    for line in raw.lines() {
        if let Some(value) = parse_io_field(line, "read_bytes", method)? {
            bytes_read = Some(value);
        } else if let Some(value) = parse_io_field(line, "write_bytes", method)? {
            bytes_written = Some(value);
        }
    }

    Ok(IoCounters {
        bytes_read: required_method_counter(bytes_read, "read_bytes", method)?,
        bytes_written: required_method_counter(bytes_written, "write_bytes", method)?,
    })
}

fn parse_io_field(line: &str, key: &'static str, method: &'static str) -> Result<Option<u64>> {
    let Some(rest) = line
        .strip_prefix(key)
        .and_then(|rest| rest.strip_prefix(':'))
    else {
        return Ok(None);
    };
    rest.trim()
        .parse::<u64>()
        .map(Some)
        .map_err(|error| process_parse_error(method, format!("invalid {key} value: {error}")))
}

fn required_counter(value: Option<u64>, field: &'static str) -> Result<u64> {
    value.ok_or_else(|| self_stats_parse_error(format!("missing required field: {field}")))
}

fn required_method_counter(
    value: Option<u64>,
    field: &'static str,
    method: &'static str,
) -> Result<u64> {
    value.ok_or_else(|| process_parse_error(method, format!("missing required field: {field}")))
}

fn clock_ticks_per_second() -> u64 {
    nix::unistd::sysconf(nix::unistd::SysconfVar::CLK_TCK)
        .ok()
        .flatten()
        .and_then(|ticks| u64::try_from(ticks).ok())
        .filter(|ticks| *ticks > 0)
        .unwrap_or(100)
}

fn ticks_to_micros(ticks: u64, ticks_per_second: u64) -> u64 {
    ticks
        .saturating_mul(1_000_000)
        .checked_div(ticks_per_second)
        .unwrap_or(0)
}

fn ticks_to_millis(ticks: u64, ticks_per_second: u64) -> i64 {
    let millis = ticks
        .saturating_mul(1_000)
        .checked_div(ticks_per_second)
        .unwrap_or(0);
    i64::try_from(millis).unwrap_or(i64::MAX)
}

fn self_stats_parse_error(details: impl Into<String>) -> SbhError {
    process_parse_error("self_stats", details)
}

fn process_parse_error(method: &'static str, details: impl Into<String>) -> SbhError {
    PalError::method_failed("linux", method, details.into()).into()
}

#[cfg(test)]
mod tests {
    use super::{
        parse_proc_boot_time_unix_ms, parse_proc_io, parse_proc_self_stat, parse_proc_self_status,
        parse_proc_start_time_unix_ms, read_process_io, read_process_list, read_self_stats,
        ticks_to_micros,
    };

    #[test]
    fn parses_proc_status_memory_fields() {
        let memory = parse_proc_self_status(
            "Name:\tsbh\n\
             VmPeak:\t  999999 kB\n\
             VmSize:\t  123456 kB\n\
             VmRSS:\t    7890 kB\n",
        )
        .expect("status should parse");

        assert_eq!(memory.virtual_memory_bytes, 123_456 * 1024);
        assert_eq!(memory.rss_bytes, 7_890 * 1024);
    }

    #[test]
    fn parses_proc_stat_cpu_times_with_spaces_in_command() {
        let cpu = parse_proc_self_stat(
            "12345 (sbh worker thread) S 1 2 3 4 5 6 7 8 9 10 345 67 0 0 20 0 1 0 0",
            100,
        )
        .expect("stat should parse");

        assert_eq!(cpu.user_micros, 3_450_000);
        assert_eq!(cpu.system_micros, 670_000);
    }

    #[test]
    fn parses_proc_stat_process_start_time() {
        let start_time = parse_proc_start_time_unix_ms(
            "12345 (sbh worker thread) S 1 2 3 4 5 6 7 8 9 10 345 67 0 0 20 0 1 0 12345",
            1_700_000_000_000,
            100,
        )
        .expect("stat should parse");

        assert_eq!(start_time, 1_700_000_123_450);
    }

    #[test]
    fn parses_proc_boot_time() {
        let boot_time =
            parse_proc_boot_time_unix_ms("cpu  1 2 3\nbtime 1700000000\n").expect("btime parses");

        assert_eq!(boot_time, 1_700_000_000_000);
    }

    #[test]
    fn parses_proc_io_lifetime_byte_counters() {
        let counters = parse_proc_io(
            "rchar: 1\n\
             wchar: 2\n\
             syscr: 3\n\
             syscw: 4\n\
             read_bytes: 4096\n\
             write_bytes: 8192\n",
            "test",
        )
        .expect("io counters should parse");

        assert_eq!(counters.bytes_read, 4096);
        assert_eq!(counters.bytes_written, 8192);
    }

    #[test]
    fn converts_ticks_to_microseconds_without_overflowing() {
        assert_eq!(ticks_to_micros(345, 100), 3_450_000);
        assert_eq!(ticks_to_micros(u64::MAX, 100), u64::MAX / 100);
    }

    #[test]
    fn linux_self_stats_reports_current_process() {
        let stats = read_self_stats().expect("self stats should be readable from /proc");

        assert!(stats.rss_bytes > 0);
        assert!(stats.virtual_memory_bytes >= stats.rss_bytes);
        assert_eq!(stats.idle_wakeups, None);
        assert!(stats.bytes_read.is_some());
        assert!(stats.bytes_written.is_some());
    }

    #[test]
    fn linux_process_list_reports_visible_processes() {
        let current_pid = i32::try_from(std::process::id()).expect("pid should fit i32");
        let processes = read_process_list().expect("process list should be readable from /proc");

        assert!(!processes.iter().any(|process| process.pid == current_pid));
        assert!(processes.iter().all(|process| process.pid > 0));
        assert!(processes.iter().all(|process| !process.name.is_empty()));
        assert!(
            processes
                .iter()
                .any(|process| process.start_time_unix_ms.is_some())
        );
    }

    #[test]
    fn linux_process_io_reports_current_process_counters() {
        let current_pid = i32::try_from(std::process::id()).expect("pid should fit i32");
        let io = read_process_io(current_pid).expect("current process io should be readable");

        assert_eq!(io.pid, current_pid);
        assert_eq!(io.bytes_read_recent_15m, None);
        assert_eq!(io.bytes_written_recent_15m, None);
    }
}
