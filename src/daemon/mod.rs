//! Daemon subsystem: main monitoring loop, service integration, signal handling,
//! self-monitoring, and multi-channel notifications.

#[cfg(feature = "daemon")]
pub mod loop_main;
pub mod notifications;
pub mod policy;
pub mod self_monitor;
pub mod service;
#[cfg(feature = "daemon")]
pub mod signals;
