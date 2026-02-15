//! Dual-write logging: SQLite (WAL) + JSONL append-only with graceful degradation.

pub mod dual;
pub mod jsonl;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(feature = "sqlite")]
pub mod stats;
