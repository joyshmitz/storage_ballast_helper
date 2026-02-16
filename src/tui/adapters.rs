//! Typed adapter boundaries for dashboard runtime inputs.

#![allow(missing_docs)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::core::errors::Result;
use crate::daemon::self_monitor::DAEMON_STATE_STALE_THRESHOLD_SECS;
use crate::daemon::self_monitor::DaemonState;
use crate::monitor::fs_stats::FsStatsCollector;
use crate::platform::pal::{Platform, detect_platform};

/// Health summary for runtime data sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterHealth {
    pub state_file_available: bool,
    pub telemetry_available: bool,
}

impl Default for AdapterHealth {
    fn default() -> Self {
        Self {
            state_file_available: true,
            telemetry_available: true,
        }
    }
}

/// Shared state-source contract. Implementations are added in `bd-xzt.2.3`.
pub trait StateAdapter {
    /// Returns `None` when data is unavailable or malformed.
    fn read_state(&self, state_file: &Path) -> Option<DaemonState>;

    /// Provides a coarse health signal for diagnostics.
    fn health(&self) -> AdapterHealth;
}

/// Bootstrap adapter used for scaffold wiring.
///
/// This intentionally returns `None` until the dedicated adapter bead
/// (`bd-xzt.2.3`) lands full parsing + staleness semantics.
#[derive(Debug, Default)]
pub struct NullStateAdapter;

impl StateAdapter for NullStateAdapter {
    fn read_state(&self, _state_file: &Path) -> Option<DaemonState> {
        None
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth {
            state_file_available: false,
            telemetry_available: false,
        }
    }
}

/// Freshness classification for state-file ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateFreshness {
    Fresh,
    Stale { age: Duration },
    Missing,
    Malformed,
    ReadError(String),
}

/// Source used for the mount telemetry shown in the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSource {
    DaemonState,
    FilesystemFallback,
    Unavailable,
}

/// Typed mount row consumed by the dashboard view layer.
#[derive(Debug, Clone, PartialEq)]
pub struct MountSnapshot {
    pub path: String,
    pub free_pct: f64,
    pub level: String,
    pub rate_bps: Option<f64>,
    pub source: SnapshotSource,
}

/// Typed data payload consumed by model/update code.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardSnapshot {
    pub daemon_state: Option<DaemonState>,
    pub mounts: Vec<MountSnapshot>,
    pub freshness: StateFreshness,
    pub source: SnapshotSource,
}

/// Typed state-file + fallback adapter for the new dashboard runtime.
pub struct DashboardStateAdapter {
    collector: FsStatsCollector,
    stale_threshold: Duration,
}

impl DashboardStateAdapter {
    /// Build an adapter from a specific platform implementation.
    #[must_use]
    pub fn new(
        platform: Arc<dyn Platform>,
        stale_threshold: Duration,
        cache_ttl: Duration,
    ) -> Self {
        Self {
            collector: FsStatsCollector::new(platform, cache_ttl),
            stale_threshold,
        }
    }

    /// Build an adapter using the detected host platform.
    ///
    /// # Errors
    /// Returns an error when the active platform cannot be detected.
    pub fn from_detected_platform() -> Result<Self> {
        let platform = detect_platform()?;
        Ok(Self::new(
            platform,
            Duration::from_secs(DAEMON_STATE_STALE_THRESHOLD_SECS),
            Duration::from_secs(1),
        ))
    }

    /// Read state-file data with stale detection and filesystem fallback.
    #[must_use]
    pub fn load_snapshot(&self, state_file: &Path, monitor_paths: &[PathBuf]) -> DashboardSnapshot {
        match self.read_state_outcome(state_file) {
            StateReadOutcome::Fresh(state) => DashboardSnapshot {
                mounts: mounts_from_state(&state),
                daemon_state: Some(state),
                freshness: StateFreshness::Fresh,
                source: SnapshotSource::DaemonState,
            },
            StateReadOutcome::Stale { state, age } => {
                let fallback_mounts = self.collect_fallback_mounts(monitor_paths);
                let source = fallback_source(&fallback_mounts);
                DashboardSnapshot {
                    daemon_state: Some(state),
                    mounts: fallback_mounts,
                    freshness: StateFreshness::Stale { age },
                    source,
                }
            }
            StateReadOutcome::Missing => {
                let fallback_mounts = self.collect_fallback_mounts(monitor_paths);
                let source = fallback_source(&fallback_mounts);
                DashboardSnapshot {
                    daemon_state: None,
                    mounts: fallback_mounts,
                    freshness: StateFreshness::Missing,
                    source,
                }
            }
            StateReadOutcome::Malformed => {
                let fallback_mounts = self.collect_fallback_mounts(monitor_paths);
                let source = fallback_source(&fallback_mounts);
                DashboardSnapshot {
                    daemon_state: None,
                    mounts: fallback_mounts,
                    freshness: StateFreshness::Malformed,
                    source,
                }
            }
            StateReadOutcome::ReadError(details) => {
                let fallback_mounts = self.collect_fallback_mounts(monitor_paths);
                let source = fallback_source(&fallback_mounts);
                DashboardSnapshot {
                    daemon_state: None,
                    mounts: fallback_mounts,
                    freshness: StateFreshness::ReadError(details),
                    source,
                }
            }
        }
    }

    fn read_state_outcome(&self, state_file: &Path) -> StateReadOutcome {
        let metadata = match std::fs::metadata(state_file) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return StateReadOutcome::Missing;
            }
            Err(error) => return StateReadOutcome::ReadError(error.to_string()),
        };

        let raw = match std::fs::read_to_string(state_file) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return StateReadOutcome::Missing;
            }
            Err(error) => return StateReadOutcome::ReadError(error.to_string()),
        };

        let Ok(state) = serde_json::from_str::<DaemonState>(&raw) else {
            return StateReadOutcome::Malformed;
        };

        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(error) => return StateReadOutcome::ReadError(error.to_string()),
        };
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();

        if age > self.stale_threshold {
            StateReadOutcome::Stale { state, age }
        } else {
            StateReadOutcome::Fresh(state)
        }
    }

    fn collect_fallback_mounts(&self, monitor_paths: &[PathBuf]) -> Vec<MountSnapshot> {
        let Ok(stats_by_path) = self.collector.collect_many(monitor_paths) else {
            return Vec::new();
        };

        let mut deduped = BTreeMap::<String, MountSnapshot>::new();
        for stats in stats_by_path.values() {
            let mount_path = stats.mount_point.display().to_string();
            deduped.entry(mount_path.clone()).or_insert_with(|| {
                let free_pct = stats.free_pct();
                MountSnapshot {
                    path: mount_path,
                    free_pct,
                    level: fallback_pressure_level(free_pct).to_string(),
                    rate_bps: None,
                    source: SnapshotSource::FilesystemFallback,
                }
            });
        }

        deduped.into_values().collect()
    }
}

impl StateAdapter for DashboardStateAdapter {
    fn read_state(&self, state_file: &Path) -> Option<DaemonState> {
        match self.read_state_outcome(state_file) {
            StateReadOutcome::Fresh(state) => Some(state),
            StateReadOutcome::Stale { .. }
            | StateReadOutcome::Missing
            | StateReadOutcome::Malformed
            | StateReadOutcome::ReadError(_) => None,
        }
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::default()
    }
}

#[derive(Debug)]
enum StateReadOutcome {
    Fresh(DaemonState),
    Stale { state: DaemonState, age: Duration },
    Missing,
    Malformed,
    ReadError(String),
}

fn mounts_from_state(state: &DaemonState) -> Vec<MountSnapshot> {
    let mut mounts: Vec<_> = state
        .pressure
        .mounts
        .iter()
        .map(|mount| MountSnapshot {
            path: mount.path.clone(),
            free_pct: mount.free_pct,
            level: mount.level.clone(),
            rate_bps: mount.rate_bps,
            source: SnapshotSource::DaemonState,
        })
        .collect();
    mounts.sort_by(|left, right| left.path.cmp(&right.path));
    mounts
}

fn fallback_source(mounts: &[MountSnapshot]) -> SnapshotSource {
    if mounts.is_empty() {
        SnapshotSource::Unavailable
    } else {
        SnapshotSource::FilesystemFallback
    }
}

fn fallback_pressure_level(free_pct: f64) -> &'static str {
    if free_pct < 5.0 {
        "red"
    } else if free_pct < 20.0 {
        "orange"
    } else if free_pct < 35.0 {
        "yellow"
    } else {
        "green"
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use filetime::{FileTime, set_file_mtime};
    use tempfile::TempDir;

    use crate::daemon::self_monitor::{
        BallastState, Counters, LastScanState, MountPressure, PressureState,
    };
    use crate::platform::pal::{FsStats, MemoryInfo, MockPlatform, MountPoint, PlatformPaths};

    use super::DashboardStateAdapter;
    use super::*;

    #[test]
    fn null_adapter_reports_unavailable() {
        let adapter = NullStateAdapter;
        assert!(
            adapter
                .read_state(PathBuf::from("/tmp/state.json").as_path())
                .is_none()
        );
        assert_eq!(
            adapter.health(),
            AdapterHealth {
                state_file_available: false,
                telemetry_available: false,
            }
        );
    }

    #[test]
    fn fresh_state_prefers_daemon_data() {
        let tmp = TempDir::new().expect("tempdir");
        let state_path = tmp.path().join("state.json");
        write_state_file(&state_path, sample_daemon_state()).expect("write state");

        let adapter = DashboardStateAdapter::new(
            mock_platform(),
            Duration::from_secs(90),
            Duration::from_secs(1),
        );
        let snapshot = adapter.load_snapshot(&state_path, &[PathBuf::from("/tmp/work")]);

        assert_eq!(snapshot.freshness, StateFreshness::Fresh);
        assert_eq!(snapshot.source, SnapshotSource::DaemonState);
        assert_eq!(snapshot.mounts.len(), 1);
        assert_eq!(snapshot.mounts[0].path, "/");
        assert_eq!(snapshot.mounts[0].source, SnapshotSource::DaemonState);
    }

    #[test]
    fn stale_state_falls_back_to_filesystem_stats() {
        let tmp = TempDir::new().expect("tempdir");
        let state_path = tmp.path().join("state.json");
        write_state_file(&state_path, sample_daemon_state()).expect("write state");

        let stale_mtime = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
        set_file_mtime(&state_path, stale_mtime).expect("set stale mtime");

        let adapter = DashboardStateAdapter::new(
            mock_platform(),
            Duration::from_secs(90),
            Duration::from_secs(1),
        );
        let snapshot = adapter.load_snapshot(&state_path, &[PathBuf::from("/tmp/work")]);

        match snapshot.freshness {
            StateFreshness::Stale { age } => assert!(age.as_secs() >= 3000),
            other => panic!("expected stale freshness, got {other:?}"),
        }
        assert_eq!(snapshot.source, SnapshotSource::FilesystemFallback);
        assert_eq!(snapshot.mounts.len(), 1);
        assert_eq!(snapshot.mounts[0].path, "/tmp");
        assert_eq!(
            snapshot.mounts[0].source,
            SnapshotSource::FilesystemFallback
        );
        assert!(snapshot.daemon_state.is_some());
    }

    #[test]
    fn malformed_state_falls_back() {
        let tmp = TempDir::new().expect("tempdir");
        let state_path = tmp.path().join("state.json");
        std::fs::write(&state_path, "not-json").expect("write malformed state");

        let adapter = DashboardStateAdapter::new(
            mock_platform(),
            Duration::from_secs(90),
            Duration::from_secs(1),
        );
        let snapshot = adapter.load_snapshot(&state_path, &[PathBuf::from("/tmp/work")]);

        assert_eq!(snapshot.freshness, StateFreshness::Malformed);
        assert_eq!(snapshot.source, SnapshotSource::FilesystemFallback);
        assert_eq!(snapshot.mounts.len(), 1);
    }

    #[test]
    fn missing_state_falls_back() {
        let tmp = TempDir::new().expect("tempdir");
        let state_path = tmp.path().join("missing-state.json");
        let adapter = DashboardStateAdapter::new(
            mock_platform(),
            Duration::from_secs(90),
            Duration::from_secs(1),
        );
        let snapshot = adapter.load_snapshot(&state_path, &[PathBuf::from("/tmp/work")]);

        assert_eq!(snapshot.freshness, StateFreshness::Missing);
        assert_eq!(snapshot.source, SnapshotSource::FilesystemFallback);
        assert_eq!(snapshot.mounts.len(), 1);
    }

    #[test]
    fn read_state_only_accepts_fresh_state() {
        let tmp = TempDir::new().expect("tempdir");
        let state_path = tmp.path().join("state.json");
        write_state_file(&state_path, sample_daemon_state()).expect("write state");

        let fresh_adapter = DashboardStateAdapter::new(
            mock_platform(),
            Duration::from_secs(90),
            Duration::from_secs(1),
        );
        assert!(fresh_adapter.read_state(&state_path).is_some());

        let stale_adapter = DashboardStateAdapter::new(
            mock_platform(),
            Duration::from_secs(1),
            Duration::from_secs(1),
        );
        let stale_mtime = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(300));
        set_file_mtime(&state_path, stale_mtime).expect("set stale mtime");
        assert!(stale_adapter.read_state(&state_path).is_none());
    }

    #[test]
    fn fallback_mounts_are_deduplicated_by_mount_path() {
        let tmp = TempDir::new().expect("tempdir");
        let state_path = tmp.path().join("missing-state.json");
        let adapter = DashboardStateAdapter::new(
            mock_platform(),
            Duration::from_secs(90),
            Duration::from_secs(1),
        );

        let snapshot = adapter.load_snapshot(
            &state_path,
            &[PathBuf::from("/tmp/work-a"), PathBuf::from("/tmp/work-b")],
        );

        assert_eq!(snapshot.freshness, StateFreshness::Missing);
        assert_eq!(snapshot.mounts.len(), 1);
        assert_eq!(snapshot.mounts[0].path, "/tmp");
    }

    fn write_state_file(path: &Path, state: DaemonState) -> std::io::Result<()> {
        let encoded = serde_json::to_string(&state).expect("state json");
        std::fs::write(path, encoded)
    }

    fn sample_daemon_state() -> DaemonState {
        DaemonState {
            version: "0.1.0".to_string(),
            pid: 42,
            started_at: "2026-02-16T00:00:00Z".to_string(),
            uptime_seconds: 60,
            last_updated: "2026-02-16T00:01:00Z".to_string(),
            pressure: PressureState {
                overall: "yellow".to_string(),
                mounts: vec![MountPressure {
                    path: "/".to_string(),
                    free_pct: 42.0,
                    level: "yellow".to_string(),
                    rate_bps: Some(1024.0),
                }],
            },
            ballast: BallastState {
                available: 9,
                total: 10,
                released: 1,
            },
            last_scan: LastScanState {
                at: Some("2026-02-16T00:00:30Z".to_string()),
                candidates: 7,
                deleted: 3,
            },
            counters: Counters {
                scans: 13,
                deletions: 5,
                bytes_freed: 4096,
                errors: 0,
                dropped_log_events: 0,
            },
            memory_rss_bytes: 1024 * 1024,
        }
    }

    fn mock_platform() -> Arc<dyn crate::platform::pal::Platform> {
        let mount = MountPoint {
            path: PathBuf::from("/tmp"),
            device: "tmpfs".to_string(),
            fs_type: "tmpfs".to_string(),
            is_ram_backed: true,
        };
        let stats = FsStats {
            total_bytes: 100,
            free_bytes: 70,
            available_bytes: 70,
            fs_type: "tmpfs".to_string(),
            mount_point: PathBuf::from("/tmp"),
            is_readonly: false,
        };
        Arc::new(MockPlatform::new(
            vec![mount.clone()],
            HashMap::from([(mount.path.clone(), stats)]),
            MemoryInfo {
                total_bytes: 1,
                available_bytes: 1,
                swap_total_bytes: 0,
                swap_free_bytes: 0,
            },
            PlatformPaths::default(),
        ))
    }
}
