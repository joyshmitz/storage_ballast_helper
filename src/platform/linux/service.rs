//! Linux service integration for the PAL.

#![cfg(target_os = "linux")]
#![allow(missing_docs)]

use crate::platform::pal::{NoopServiceManager, ServiceManager};

pub(super) fn service_manager() -> Box<dyn ServiceManager> {
    match crate::daemon::service::SystemdServiceManager::from_env(false) {
        Ok(mgr) => Box::new(mgr),
        Err(_) => Box::<NoopServiceManager>::default(),
    }
}
