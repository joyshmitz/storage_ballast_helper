//! Write-bandwidth estimation used to size kernel writeback (dirty-page) limits.
//!
//! Two strategies are provided:
//! - [`measure_bytes_per_sec`] runs a short, bounded, non-destructive micro-benchmark
//!   (write a temp file with random data, fsync, time it) against the target volume.
//! - [`heuristic_bytes_per_sec`] derives a conservative estimate from device class
//!   (NVMe / SSD / HDD) when benchmarking is disabled or fails.
//!
//! Random data is used for the probe so copy-on-write/compressing filesystems
//! (btrfs, zfs) cannot deduplicate or compress the write away and inflate the
//! measured throughput.

#![allow(missing_docs)]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use rand::Rng;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// Provenance of a bandwidth estimate, surfaced in tuning rationale for explainability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandwidthSource {
    /// Measured by the on-volume micro-benchmark.
    Measured,
    /// Heuristic for NVMe-class devices.
    HeuristicNvme,
    /// Heuristic for non-rotational (SATA/SAS SSD) devices.
    HeuristicSsd,
    /// Heuristic for rotational (HDD) devices.
    HeuristicHdd,
    /// Heuristic fallback when device class is unknown.
    HeuristicUnknown,
}

impl std::fmt::Display for BandwidthSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Measured => "measured",
            Self::HeuristicNvme => "heuristic: nvme",
            Self::HeuristicSsd => "heuristic: ssd",
            Self::HeuristicHdd => "heuristic: hdd",
            Self::HeuristicUnknown => "heuristic: unknown device",
        };
        f.write_str(label)
    }
}

/// Conservative per-class write throughput estimate in bytes/sec.
///
/// The SSD value (512 MiB/s) is chosen so the default plan (1s background drain,
/// 4:1 hard ratio) lands on the field-proven 512 MiB / 2 GiB limits.
#[must_use]
pub fn heuristic_bytes_per_sec(
    rotational: Option<bool>,
    device_name: &str,
) -> (u64, BandwidthSource) {
    if device_name.to_ascii_lowercase().contains("nvme") {
        return (GIB + GIB / 2, BandwidthSource::HeuristicNvme); // ~1.5 GiB/s
    }
    match rotational {
        Some(false) => (512 * MIB, BandwidthSource::HeuristicSsd),
        Some(true) => (150 * MIB, BandwidthSource::HeuristicHdd),
        None => (400 * MIB, BandwidthSource::HeuristicUnknown),
    }
}

/// Measure write throughput against the volume backing `dir`.
///
/// Writes `bytes` of random data to a fresh temp file, fsyncs, times the whole
/// operation, and always removes the temp file before returning an estimate in
/// bytes/sec. `bytes` is the caller's budget (typically tens of MiB); larger
/// values give a steadier estimate at the cost of a bigger transient write.
pub fn measure_bytes_per_sec(dir: &Path, bytes: u64) -> std::io::Result<u64> {
    let probe = dir.join(format!(".sbh-writeback-probe-{}.tmp", std::process::id()));

    // 4 MiB random buffer reused across chunks; random content defeats
    // compression/dedup on btrfs/zfs that would otherwise inflate the estimate.
    let chunk: usize = 4 * 1024 * 1024;
    let mut buffer = vec![0u8; chunk];
    rand::rng().fill_bytes(&mut buffer);

    let target = bytes.max(MIB);
    let start = Instant::now();
    let write_result = write_and_sync(&probe, &buffer, target);
    let elapsed = start.elapsed().as_secs_f64();

    // Best-effort cleanup regardless of outcome.
    let _ = std::fs::remove_file(&probe);
    write_result?;

    if !elapsed.is_finite() || elapsed <= 0.0 {
        // Sub-microsecond timing: treat as a one-second write to avoid div-by-zero.
        return Ok(target);
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let bps = (target as f64 / elapsed) as u64;
    Ok(bps.max(1))
}

/// Write `target` bytes (drawn from `buffer`, repeated) to `path`, flush, and fsync.
fn write_and_sync(path: &Path, buffer: &[u8], target: u64) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    let mut written: u64 = 0;
    while written < target {
        let remaining = usize::try_from(target - written)
            .unwrap_or(buffer.len())
            .min(buffer.len());
        file.write_all(&buffer[..remaining])?;
        written += remaining as u64;
    }
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvme_name_wins_over_rotational_flag() {
        let (bps, src) = heuristic_bytes_per_sec(Some(true), "/dev/nvme0n1");
        assert_eq!(src, BandwidthSource::HeuristicNvme);
        assert!(bps > GIB);
    }

    #[test]
    fn ssd_heuristic_lands_on_512_mib() {
        let (bps, src) = heuristic_bytes_per_sec(Some(false), "sda");
        assert_eq!(src, BandwidthSource::HeuristicSsd);
        assert_eq!(bps, 512 * MIB);
    }

    #[test]
    fn rotational_and_unknown_differ() {
        let (hdd, _) = heuristic_bytes_per_sec(Some(true), "sdb");
        let (unknown, src) = heuristic_bytes_per_sec(None, "dm-0");
        assert_eq!(src, BandwidthSource::HeuristicUnknown);
        assert!(unknown > hdd);
    }

    #[test]
    fn measure_returns_positive_estimate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bps = measure_bytes_per_sec(dir.path(), 8 * MIB).expect("measure");
        assert!(bps >= 1);
        // Probe file must not linger.
        let leftover = std::fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(std::result::Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".sbh-writeback-probe")
            });
        assert!(!leftover, "probe temp file should be cleaned up");
    }
}
