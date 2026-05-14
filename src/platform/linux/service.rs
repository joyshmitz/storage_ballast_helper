//! Linux service integration for the PAL.

#![allow(missing_docs)]

use crate::platform::pal::ServiceManager;
use crate::platform::types::ServiceKind;

pub(super) fn service_manager() -> Box<dyn ServiceManager> {
    crate::daemon::service::service_manager_for_kind(ServiceKind::Systemd, false)
}
