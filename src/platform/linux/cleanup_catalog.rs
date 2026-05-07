//! Static Linux cleanup heuristic catalog.

#![allow(missing_docs)]

use crate::platform::cleanup_catalog;
pub use crate::platform::cleanup_catalog::{
    AgeThreshold, CheckRequirement, CleanupConfidence, CleanupRule, ReclaimCommand,
};

const fn cleanup_rule(
    name: &'static str,
    path_glob: &'static str,
    age_threshold: AgeThreshold,
    fd_check: CheckRequirement,
    reclaim_command: ReclaimCommand,
    confidence: CleanupConfidence,
) -> CleanupRule {
    CleanupRule {
        name,
        path_glob,
        age_threshold,
        fd_check,
        parent_check: CheckRequirement::Required,
        sacred_overlaps_check: CheckRequirement::Required,
        reclaim_command,
        confidence,
    }
}

pub const TMP_DASH_TARGET: CleanupRule = cleanup_rule(
    "linux-tmp-dash-target",
    "/tmp/*-target",
    AgeThreshold::from_hours(24),
    CheckRequirement::Required,
    ReclaimCommand::RemoveTree,
    CleanupConfidence::Likely,
);

pub const TMP_UNDERSCORE_TARGET: CleanupRule = cleanup_rule(
    "linux-tmp-underscore-target",
    "/tmp/*_target",
    AgeThreshold::from_hours(24),
    CheckRequirement::Required,
    ReclaimCommand::RemoveTree,
    CleanupConfidence::Likely,
);

pub const DATA_TMP_DASH_TARGET: CleanupRule = cleanup_rule(
    "linux-data-tmp-dash-target",
    "/data/tmp/*-target",
    AgeThreshold::from_hours(24),
    CheckRequirement::Required,
    ReclaimCommand::RemoveTree,
    CleanupConfidence::Likely,
);

pub const DATA_TMP_UNDERSCORE_TARGET: CleanupRule = cleanup_rule(
    "linux-data-tmp-underscore-target",
    "/data/tmp/*_target",
    AgeThreshold::from_hours(24),
    CheckRequirement::Required,
    ReclaimCommand::RemoveTree,
    CleanupConfidence::Likely,
);

pub const VAR_TMP_DASH_TARGET: CleanupRule = cleanup_rule(
    "linux-var-tmp-dash-target",
    "/var/tmp/*-target",
    AgeThreshold::from_hours(24),
    CheckRequirement::Required,
    ReclaimCommand::RemoveTree,
    CleanupConfidence::Likely,
);

pub const VAR_TMP_UNDERSCORE_TARGET: CleanupRule = cleanup_rule(
    "linux-var-tmp-underscore-target",
    "/var/tmp/*_target",
    AgeThreshold::from_hours(24),
    CheckRequirement::Required,
    ReclaimCommand::RemoveTree,
    CleanupConfidence::Likely,
);

pub const LINUX_CLEANUP_RULES: &[CleanupRule] = &[
    TMP_DASH_TARGET,
    TMP_UNDERSCORE_TARGET,
    DATA_TMP_DASH_TARGET,
    DATA_TMP_UNDERSCORE_TARGET,
    VAR_TMP_DASH_TARGET,
    VAR_TMP_UNDERSCORE_TARGET,
];

#[must_use]
pub fn cleanup_rules() -> &'static [CleanupRule] {
    LINUX_CLEANUP_RULES
}

#[must_use]
pub fn find_rule(name: &str) -> Option<&'static CleanupRule> {
    cleanup_catalog::find_rule(LINUX_CLEANUP_RULES, name)
}

#[must_use]
pub fn match_rule(path: &std::path::Path) -> Option<&'static CleanupRule> {
    cleanup_catalog::match_rule(path, LINUX_CLEANUP_RULES)
}

#[must_use]
pub fn match_path_scanner_rule(path: &std::path::Path) -> Option<&'static CleanupRule> {
    cleanup_catalog::match_path_scanner_rule(path, LINUX_CLEANUP_RULES)
}

#[cfg(test)]
mod tests {
    use super::{DATA_TMP_UNDERSCORE_TARGET, cleanup_rules, match_path_scanner_rule};
    use crate::platform::cleanup_catalog::{AgeThreshold, CheckRequirement, ReclaimCommand};
    use std::path::Path;

    #[test]
    fn linux_catalog_exports_temp_target_rules() {
        assert_eq!(
            cleanup_rules()
                .iter()
                .filter(|rule| rule.fd_check == CheckRequirement::Required)
                .count(),
            6
        );
        assert_eq!(
            DATA_TMP_UNDERSCORE_TARGET.age_threshold,
            AgeThreshold::from_hours(24)
        );
        assert_eq!(
            DATA_TMP_UNDERSCORE_TARGET.reclaim_command,
            ReclaimCommand::RemoveTree
        );
    }

    #[test]
    fn linux_catalog_uses_shared_match_engine() {
        let rule = match_path_scanner_rule(Path::new("/data/tmp/cass_append_baseline_target"))
            .expect("linux data tmp target should match");

        assert_eq!(rule.name, "linux-data-tmp-underscore-target");
    }

    #[test]
    fn project_source_target_name_is_not_a_linux_catalog_rule() {
        assert!(
            match_path_scanner_rule(Path::new(
                "/data/projects/asupersync_ansi_c/tools/rust_fuzz_target"
            ))
            .is_none()
        );
    }
}
