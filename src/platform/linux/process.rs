//! Linux process readers for future PAL process methods.

#![cfg(target_os = "linux")]
#![allow(missing_docs)]

use std::fs;
use std::path::PathBuf;

use crate::core::errors::{Result, SbhError};
use crate::platform::types::{PalError, SelfStats};

const PROC_SELF_STATUS: &str = "/proc/self/status";
const PROC_SELF_STAT: &str = "/proc/self/stat";
const PROC_SELF_IO: &str = "/proc/self/io";

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

pub(super) fn read_self_stats() -> Result<SelfStats> {
    let status = read_proc_file(PROC_SELF_STATUS)?;
    let stat = read_proc_file(PROC_SELF_STAT)?;
    let memory = parse_proc_self_status(&status)?;
    let cpu = parse_proc_self_stat(&stat, clock_ticks_per_second())?;
    let io = read_proc_file(PROC_SELF_IO)
        .and_then(|raw| parse_proc_self_io(&raw))
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

fn parse_proc_self_io(raw: &str) -> Result<IoCounters> {
    let mut bytes_read = None;
    let mut bytes_written = None;

    for line in raw.lines() {
        if let Some(value) = parse_io_field(line, "read_bytes")? {
            bytes_read = Some(value);
        } else if let Some(value) = parse_io_field(line, "write_bytes")? {
            bytes_written = Some(value);
        }
    }

    Ok(IoCounters {
        bytes_read: required_counter(bytes_read, "read_bytes")?,
        bytes_written: required_counter(bytes_written, "write_bytes")?,
    })
}

fn parse_io_field(line: &str, key: &'static str) -> Result<Option<u64>> {
    let Some(rest) = line
        .strip_prefix(key)
        .and_then(|rest| rest.strip_prefix(':'))
    else {
        return Ok(None);
    };
    rest.trim()
        .parse::<u64>()
        .map(Some)
        .map_err(|error| self_stats_parse_error(format!("invalid {key} value: {error}")))
}

fn required_counter(value: Option<u64>, field: &'static str) -> Result<u64> {
    value.ok_or_else(|| self_stats_parse_error(format!("missing required field: {field}")))
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

fn self_stats_parse_error(details: impl Into<String>) -> SbhError {
    PalError::method_failed("linux", "self_stats", details.into()).into()
}

#[cfg(test)]
mod tests {
    use super::{
        parse_proc_self_io, parse_proc_self_stat, parse_proc_self_status, read_self_stats,
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
    fn parses_proc_io_lifetime_byte_counters() {
        let counters = parse_proc_self_io(
            "rchar: 1\n\
             wchar: 2\n\
             syscr: 3\n\
             syscw: 4\n\
             read_bytes: 4096\n\
             write_bytes: 8192\n",
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
}
