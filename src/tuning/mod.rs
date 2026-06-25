//! System-level tuning that complements sbh's own controls.
//!
//! sbh already runs its daemon at low CPU/IO priority, but the worst
//! interactive-latency problems on busy build hosts come from *kernel* writeback
//! behavior that no per-process nice/ionice can fix. This module models the
//! cross-platform tuning sbh can recommend and (when invoked with privilege)
//! apply on the operator's behalf.
//!
//! - [`writeback`] — kernel dirty-page (writeback) limit sizing and assessment.
//! - [`bandwidth`] — device write-bandwidth estimation used to size the limits.
//!
//! The platform-specific reads/writes (`/proc/sys/vm`, `/sys/block`,
//! `/etc/sysctl.d`) live behind the PAL; everything here is platform-agnostic
//! and unit-tested on every target.

pub mod bandwidth;
pub mod writeback;
