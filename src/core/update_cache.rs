//! Deterministic on-disk cache for update metadata.

#![allow(missing_docs)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Cached metadata used by update checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedUpdateMetadata {
    pub target_tag: String,
    pub artifact_url: String,
    pub fetched_at_unix_secs: u64,
}

/// File-backed cache for update metadata with TTL semantics.
#[derive(Debug, Clone)]
pub struct UpdateMetadataCache {
    path: PathBuf,
    ttl: Duration,
}

impl UpdateMetadataCache {
    #[must_use]
    pub fn new(path: PathBuf, ttl: Duration) -> Self {
        Self { path, ttl }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Load cache entry if present and not stale for the provided `now`.
    pub fn load_fresh(&self, now: SystemTime) -> io::Result<Option<CachedUpdateMetadata>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&self.path)?;
        let entry: CachedUpdateMetadata = serde_json::from_str(&raw).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse update metadata cache: {error}"),
            )
        })?;

        if is_fresh(&entry, now, self.ttl) {
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    /// Store cache entry using atomic rename for crash safety.
    pub fn store(&self, entry: &CachedUpdateMetadata) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = self.path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(entry).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to serialize update metadata cache: {error}"),
            )
        })?;

        fs::write(&tmp_path, data)?;
        fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    /// Remove cache file if present.
    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn is_fresh(entry: &CachedUpdateMetadata, now: SystemTime, ttl: Duration) -> bool {
    let now_secs = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let age_secs = now_secs.saturating_sub(entry.fetched_at_unix_secs);
    age_secs <= ttl.as_secs()
}

#[cfg(test)]
mod tests {
    use super::{CachedUpdateMetadata, UpdateMetadataCache};
    use std::fs;
    use std::io;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn ts(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn sample_entry(at: u64) -> CachedUpdateMetadata {
        CachedUpdateMetadata {
            target_tag: "v0.2.0".to_string(),
            artifact_url:
                "https://github.com/Dicklesworthstone/storage_ballast_helper/releases/download/v0.2.0/sbh-x86_64-unknown-linux-gnu.tar.xz".to_string(),
            fetched_at_unix_secs: at,
        }
    }

    #[test]
    fn load_missing_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = UpdateMetadataCache::new(
            dir.path().join("update-metadata.json"),
            Duration::from_secs(60),
        );
        let got = cache.load_fresh(ts(100)).expect("load should succeed");
        assert!(got.is_none());
    }

    #[test]
    fn store_and_load_roundtrip_when_fresh() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = UpdateMetadataCache::new(
            dir.path().join("update-metadata.json"),
            Duration::from_secs(60),
        );
        let entry = sample_entry(1_000);
        cache.store(&entry).expect("store should succeed");
        let loaded = cache
            .load_fresh(ts(1_030))
            .expect("load should succeed")
            .expect("cache should be fresh");
        assert_eq!(loaded, entry);
    }

    #[test]
    fn stale_entry_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = UpdateMetadataCache::new(
            dir.path().join("update-metadata.json"),
            Duration::from_secs(5),
        );
        cache
            .store(&sample_entry(10))
            .expect("store should succeed");
        let loaded = cache.load_fresh(ts(30)).expect("load should succeed");
        assert!(loaded.is_none());
    }

    #[test]
    fn ttl_boundary_is_fresh() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = UpdateMetadataCache::new(
            dir.path().join("update-metadata.json"),
            Duration::from_secs(20),
        );
        let entry = sample_entry(1_000);
        cache.store(&entry).expect("store should succeed");
        let loaded = cache
            .load_fresh(ts(1_020))
            .expect("load should succeed")
            .expect("cache should still be fresh at ttl boundary");
        assert_eq!(loaded, entry);
    }

    #[test]
    fn store_creates_missing_parent_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_path = dir
            .path()
            .join("nested")
            .join("cache")
            .join("update-metadata.json");
        let cache = UpdateMetadataCache::new(cache_path.clone(), Duration::from_secs(60));
        cache
            .store(&sample_entry(123))
            .expect("store should create parent directories");
        assert!(cache_path.exists());
    }

    #[test]
    fn clear_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("update-metadata.json");
        let cache = UpdateMetadataCache::new(path.clone(), Duration::from_secs(60));
        cache
            .store(&sample_entry(42))
            .expect("store should succeed");
        assert!(path.exists());
        cache.clear().expect("clear should remove cache");
        assert!(!path.exists());
        cache.clear().expect("second clear should be no-op");
    }

    #[test]
    fn corrupt_json_returns_invalid_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("update-metadata.json");
        fs::write(&path, "{not-json").expect("write corrupt cache");
        let cache = UpdateMetadataCache::new(path, Duration::from_secs(60));
        let err = cache.load_fresh(ts(200)).expect_err("load should fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
