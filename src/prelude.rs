//! Convenience re-exports for library consumers.
//!
//! ```rust,no_run
//! use storage_ballast_helper::prelude::*;
//! ```

// Core
pub use crate::core::config::Config;
pub use crate::core::errors::{Result, SbhError};

// Platform
pub use crate::platform::pal::{FsStats, MountPoint, Platform, detect_platform};

// Monitor
pub use crate::monitor::ewma::{DiskRateEstimator, RateEstimate, Trend};
pub use crate::monitor::fs_stats::FsStatsCollector;
pub use crate::monitor::pid::{
    PidPressureController, PressureLevel, PressureReading, PressureResponse,
};

// Scanner
pub use crate::scanner::deletion::{DeletionConfig, DeletionExecutor};
pub use crate::scanner::patterns::ArtifactPatternRegistry;
pub use crate::scanner::protection::ProtectionRegistry;
pub use crate::scanner::scoring::{CandidacyScore, CandidateInput, ScoringEngine};
pub use crate::scanner::walker::{DirectoryWalker, WalkerConfig};

// Ballast
pub use crate::ballast::manager::BallastManager;
pub use crate::ballast::release::BallastReleaseController;
